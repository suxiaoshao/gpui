use std::{sync::Arc, time::Instant};

use async_channel::{Receiver, Sender};
use reqwest::{Client, redirect::Policy};

use super::{
    prepared::PreparedRequest,
    response::{CompletedBody, ResponseHead, ResponseProgress},
    runtime::RequestProblem,
};

mod body;
mod redirect;
mod worker;

const WORKER_EVENT_CAPACITY: usize = 8;

pub(crate) enum WorkerEvent {
    HeadReceived {
        head: ResponseHead,
        head_after: std::time::Duration,
        progress: ResponseProgress,
    },
    BodyProgress(ResponseProgress),
    Finished {
        result: Result<CompletedBody, RequestProblem>,
        finished_after: std::time::Duration,
    },
}

#[derive(Clone)]
pub(crate) struct HttpTransport {
    client: Result<Client, Arc<RequestProblem>>,
}

impl HttpTransport {
    pub(crate) fn new() -> Self {
        let client = Client::builder()
            .redirect(Policy::none())
            .referer(false)
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_zstd()
            .build()
            .map_err(|error| Arc::new(RequestProblem::transport(error.without_url())));
        Self { client }
    }

    pub(crate) fn channel() -> (Sender<WorkerEvent>, Receiver<WorkerEvent>) {
        async_channel::bounded(WORKER_EVENT_CAPACITY)
    }

    /// Runs exactly one frozen request and emits exactly one terminal event
    /// unless its receiver has been dropped as part of cancellation.
    pub(crate) async fn run(self, prepared: PreparedRequest, sender: Sender<WorkerEvent>) {
        let started_at = Instant::now();
        let timeout = prepared.timeout;
        let result = match self.client {
            Ok(client) => {
                let attempt = worker::execute(prepared, client, &sender, started_at);
                match timeout {
                    Some(timeout) => match tokio::time::timeout(timeout, attempt).await {
                        Ok(result) => result,
                        Err(_) => Err(RequestProblem::timeout()),
                    },
                    None => attempt.await,
                }
            }
            Err(problem) => Err((*problem).clone()),
        };

        let _ = sender
            .send(WorkerEvent::Finished {
                result,
                finished_after: started_at.elapsed(),
            })
            .await;
    }
}

impl Default for HttpTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::{io::Write as _, time::Duration};

    use http::{HeaderMap, Method, header};
    use tokio::{
        io::{AsyncReadExt as _, AsyncWriteExt as _},
        net::TcpListener,
        task::JoinHandle,
    };
    use url::Url;

    use super::*;
    use crate::features::request::{
        prepared::{
            BodyContentType, PreparedBody, PreparedMultipartPart, PreparedRedirect, PreparedRequest,
        },
        response::ActiveBodyStorage,
        runtime::RequestProblemKind,
    };

    struct FixtureResponse {
        bytes: &'static [u8],
        delay: Duration,
    }

    async fn fixture(responses: Vec<FixtureResponse>) -> (Url, JoinHandle<Vec<Vec<u8>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let mut requests = Vec::new();
            for response in responses {
                let (mut stream, _) = listener.accept().await.unwrap();
                requests.push(read_request(&mut stream).await);
                if !response.delay.is_zero() {
                    tokio::time::sleep(response.delay).await;
                }
                stream.write_all(response.bytes).await.unwrap();
                stream.shutdown().await.unwrap();
            }
            requests
        });
        (
            Url::parse(&format!("http://{address}/start")).unwrap(),
            task,
        )
    }

    async fn read_request(stream: &mut tokio::net::TcpStream) -> Vec<u8> {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        let header_end = loop {
            let read = stream.read(&mut buffer).await.unwrap();
            assert_ne!(read, 0, "request ended before its head");
            request.extend_from_slice(&buffer[..read]);
            if let Some(position) = request.windows(4).position(|part| part == b"\r\n\r\n") {
                break position + 4;
            }
        };
        let head = std::str::from_utf8(&request[..header_end]).unwrap();
        let content_length = head
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().unwrap())
            })
            .unwrap_or(0);
        while request.len() < header_end + content_length {
            let read = stream.read(&mut buffer).await.unwrap();
            assert_ne!(read, 0, "request ended before its body");
            request.extend_from_slice(&buffer[..read]);
        }
        request
    }

    fn prepared(
        url: Url,
        body: PreparedBody,
        body_content_type: BodyContentType,
    ) -> PreparedRequest {
        PreparedRequest {
            method: Method::POST,
            url,
            headers: HeaderMap::new(),
            body,
            body_content_type,
            redirect: PreparedRedirect {
                follow: true,
                max_hops: 10,
                preserve_method: false,
                forward_authorization_cross_host: false,
            },
            timeout: Some(Duration::from_secs(2)),
        }
    }

    async fn run_to_terminal(prepared: PreparedRequest) -> Result<CompletedBody, RequestProblem> {
        run_to_terminal_with_head(prepared).await.1
    }

    async fn run_to_terminal_with_head(
        prepared: PreparedRequest,
    ) -> (Option<ResponseHead>, Result<CompletedBody, RequestProblem>) {
        let (sender, receiver) = HttpTransport::channel();
        let worker = tokio::spawn(HttpTransport::new().run(prepared, sender));
        let mut head = None;
        let result = loop {
            match receiver.recv().await.unwrap() {
                WorkerEvent::HeadReceived { head: received, .. } => head = Some(received),
                WorkerEvent::BodyProgress(_) => assert!(head.is_some()),
                WorkerEvent::Finished { result, .. } => break result,
            }
        };
        worker.await.unwrap();
        (head, result)
    }

    #[tokio::test]
    async fn post_redirect_rewrites_to_get_and_explicit_headers_override_generated_ones() {
        let (url, server) = fixture(vec![
            FixtureResponse {
                bytes: b"HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                delay: Duration::ZERO,
            },
            FixtureResponse {
                bytes: b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                delay: Duration::ZERO,
            },
        ])
        .await;
        let mut request = prepared(
            url,
            PreparedBody::Text(b"payload".to_vec()),
            BodyContentType::Fixed(http::HeaderValue::from_static("application/json")),
        );
        request.headers.insert(
            header::CONTENT_TYPE,
            http::HeaderValue::from_static("application/custom"),
        );
        request
            .headers
            .append("x-repeat", http::HeaderValue::from_static("one"));
        request
            .headers
            .append("x-repeat", http::HeaderValue::from_static("two"));

        let completed = run_to_terminal(request).await.unwrap();
        assert_eq!(completed.sizes.stored_body_bytes, 2);
        let requests = server.await.unwrap();
        let first = String::from_utf8_lossy(&requests[0]).to_ascii_lowercase();
        assert!(first.starts_with("post /start http/1.1\r\n"));
        assert!(first.contains("content-type: application/custom\r\n"));
        assert!(!first.contains("content-type: application/json\r\n"));
        assert_eq!(first.matches("x-repeat:").count(), 2);
        assert!(first.ends_with("\r\n\r\npayload"));

        let second = String::from_utf8_lossy(&requests[1]).to_ascii_lowercase();
        assert!(second.starts_with("get /final http/1.1\r\n"));
        assert!(!second.contains("content-length:"));
        assert!(!second.contains("content-type:"));
        assert!(!second.contains("transfer-encoding:"));
        assert!(second.ends_with("\r\n\r\n"));
    }

    #[tokio::test]
    async fn temporary_redirect_rebuilds_multipart_file_stream_for_each_hop() {
        let (url, server) = fixture(vec![
            FixtureResponse {
                bytes: b"HTTP/1.1 307 Temporary Redirect\r\nLocation: /final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                delay: Duration::ZERO,
            },
            FixtureResponse {
                bytes: b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                delay: Duration::ZERO,
            },
        ])
        .await;
        let mut source = tempfile::NamedTempFile::new().unwrap();
        source.write_all(b"file-payload").unwrap();
        let body = PreparedBody::Multipart(vec![
            PreparedMultipartPart::Text {
                name: "text".into(),
                value: "text-payload".into(),
                content_type: Some("text/plain".parse().unwrap()),
            },
            PreparedMultipartPart::File {
                name: "file".into(),
                path: source.path().to_path_buf(),
                file_name: "source.txt".into(),
                content_type: "text/plain".parse().unwrap(),
            },
        ]);

        run_to_terminal(prepared(url, body, BodyContentType::MultipartBoundary))
            .await
            .unwrap();
        let requests = server.await.unwrap();
        for request in requests {
            assert!(request.windows(12).any(|part| part == b"file-payload"));
            assert!(request.windows(12).any(|part| part == b"text-payload"));
            let request = String::from_utf8_lossy(&request).to_ascii_lowercase();
            assert!(request.starts_with("post "));
            assert!(request.contains("content-type: multipart/form-data; boundary="));
        }
    }

    #[tokio::test]
    async fn url_encoded_and_binary_bodies_use_their_frozen_bytes() {
        let (url, server) = fixture(vec![
            FixtureResponse {
                bytes: b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                delay: Duration::ZERO,
            },
            FixtureResponse {
                bytes: b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                delay: Duration::ZERO,
            },
        ])
        .await;
        run_to_terminal(prepared(
            url.clone(),
            PreparedBody::UrlEncoded(b"first=one&second=two".to_vec()),
            BodyContentType::Fixed(http::HeaderValue::from_static(
                "application/x-www-form-urlencoded",
            )),
        ))
        .await
        .unwrap();

        let mut source = tempfile::NamedTempFile::new().unwrap();
        source.write_all(b"binary-payload").unwrap();
        run_to_terminal(prepared(
            url,
            PreparedBody::Binary(source.path().to_path_buf()),
            BodyContentType::None,
        ))
        .await
        .unwrap();

        let requests = server.await.unwrap();
        let encoded = String::from_utf8_lossy(&requests[0]).to_ascii_lowercase();
        assert!(encoded.contains("content-type: application/x-www-form-urlencoded\r\n"));
        assert!(encoded.ends_with("\r\n\r\nfirst=one&second=two"));
        assert!(requests[1].ends_with(b"\r\n\r\nbinary-payload"));
    }

    #[tokio::test]
    async fn frozen_total_timeout_covers_waiting_for_the_final_head() {
        let (url, server) = fixture(vec![FixtureResponse {
            bytes: b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            delay: Duration::from_millis(200),
        }])
        .await;
        let mut request = prepared(url, PreparedBody::None, BodyContentType::None);
        request.method = Method::GET;
        request.timeout = Some(Duration::from_millis(20));

        let problem = run_to_terminal(request).await.unwrap_err();
        assert_eq!(problem.kind(), RequestProblemKind::Timeout);
        server.abort();
        let _ = server.await;
    }

    #[tokio::test]
    async fn removed_binary_file_is_a_request_body_problem() {
        let source = tempfile::NamedTempFile::new().unwrap();
        let path = source.path().to_path_buf();
        drop(source);
        let request = prepared(
            Url::parse("http://127.0.0.1:9/request").unwrap(),
            PreparedBody::Binary(path),
            BodyContentType::None,
        );

        let problem = run_to_terminal(request).await.unwrap_err();
        assert_eq!(problem.kind(), RequestProblemKind::RequestBodyRead);
    }

    #[tokio::test]
    async fn disabled_redirect_and_http_error_statuses_are_normal_responses() {
        let (url, server) = fixture(vec![
            FixtureResponse {
                bytes: b"HTTP/1.1 302 Found\r\nLocation: /ignored\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                delay: Duration::ZERO,
            },
            FixtureResponse {
                bytes: b"HTTP/1.1 404 Not Found\r\nContent-Length: 3\r\nConnection: close\r\n\r\nno!",
                delay: Duration::ZERO,
            },
        ])
        .await;

        let mut redirect = prepared(url.clone(), PreparedBody::None, BodyContentType::None);
        redirect.method = Method::GET;
        redirect.redirect.follow = false;
        let (head, completed) = run_to_terminal_with_head(redirect).await;
        assert_eq!(head.unwrap().status, http::StatusCode::FOUND);
        assert_eq!(completed.unwrap().sizes.stored_body_bytes, 0);

        let mut not_found = prepared(url, PreparedBody::None, BodyContentType::None);
        not_found.method = Method::GET;
        let (head, completed) = run_to_terminal_with_head(not_found).await;
        assert_eq!(head.unwrap().status, http::StatusCode::NOT_FOUND);
        assert_eq!(completed.unwrap().sizes.stored_body_bytes, 3);

        assert_eq!(server.await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn head_and_no_content_responses_complete_without_a_body() {
        let (url, server) = fixture(vec![
            FixtureResponse {
                bytes: b"HTTP/1.1 200 OK\r\nContent-Length: 42\r\nConnection: close\r\n\r\n",
                delay: Duration::ZERO,
            },
            FixtureResponse {
                bytes: b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n",
                delay: Duration::ZERO,
            },
        ])
        .await;

        let mut head_request = prepared(url.clone(), PreparedBody::None, BodyContentType::None);
        head_request.method = Method::HEAD;
        let (head, completed) = run_to_terminal_with_head(head_request).await;
        assert_eq!(head.unwrap().status, http::StatusCode::OK);
        let completed = completed.unwrap();
        assert_eq!(completed.sizes.declared_encoded_bytes, Some(42));
        assert_eq!(completed.sizes.received_encoded_bytes, 0);
        assert_eq!(completed.sizes.stored_body_bytes, 0);

        let mut no_content = prepared(url, PreparedBody::None, BodyContentType::None);
        no_content.method = Method::GET;
        let (head, completed) = run_to_terminal_with_head(no_content).await;
        assert_eq!(head.unwrap().status, http::StatusCode::NO_CONTENT);
        assert_eq!(completed.unwrap().sizes.stored_body_bytes, 0);

        assert_eq!(server.await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn interrupted_body_keeps_the_head_but_never_completes_partial_bytes() {
        let (url, server) = fixture(vec![FixtureResponse {
            bytes: b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\nshort",
            delay: Duration::ZERO,
        }])
        .await;
        let mut request = prepared(url, PreparedBody::None, BodyContentType::None);
        request.method = Method::GET;

        let (head, result) = run_to_terminal_with_head(request).await;

        assert_eq!(head.unwrap().status, http::StatusCode::OK);
        assert_eq!(
            result.unwrap_err().kind(),
            RequestProblemKind::ResponseBodyRead
        );
        assert_eq!(server.await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn full_progress_mailbox_drops_extra_progress_but_not_the_terminal_event() {
        let (sender, receiver) = HttpTransport::channel();
        let progress = ResponseProgress {
            declared_encoded_bytes: None,
            received_encoded_bytes: 1,
            stored_body_bytes: 1,
            storage: ActiveBodyStorage::Memory,
        };
        for _ in 0..WORKER_EVENT_CAPACITY {
            assert!(sender.try_send(WorkerEvent::BodyProgress(progress)).is_ok());
        }
        assert!(matches!(
            sender.try_send(WorkerEvent::BodyProgress(progress)),
            Err(async_channel::TrySendError::Full(_))
        ));

        let terminal = tokio::spawn(async move {
            assert!(
                sender
                    .send(WorkerEvent::Finished {
                        result: Err(RequestProblem::internal()),
                        finished_after: Duration::from_millis(1),
                    })
                    .await
                    .is_ok()
            );
        });
        tokio::task::yield_now().await;
        assert!(!terminal.is_finished());

        assert!(matches!(
            receiver.recv().await.unwrap(),
            WorkerEvent::BodyProgress(_)
        ));
        terminal.await.unwrap();

        let mut terminal_seen = false;
        while !receiver.is_empty() {
            if matches!(receiver.recv().await.unwrap(), WorkerEvent::Finished { .. }) {
                terminal_seen = true;
            }
        }
        assert!(terminal_seen);
    }
}

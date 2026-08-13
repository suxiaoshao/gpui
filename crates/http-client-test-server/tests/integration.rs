use std::{
    error::Error,
    io::{self, BufRead as _},
    process::{Child, Command, ExitStatus, Stdio},
    time::Duration,
};

use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use bytes::Bytes;
use futures_util::{future::join_all, stream};
use http_client_test_server::{
    AbortSpec, ContentEncoding, HeaderSpec, RespondSpec, ResponseBodySpec, ResponseFraming,
    SpecUrlError, TestServer,
};
use reqwest::{Client, Method, StatusCode, header};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::TcpStream,
    time::{Instant, sleep, timeout},
};

const REQUEST_BODY_LIMIT: usize = 64 * 1024 * 1024;
const QUERY_SPEC_LIMIT: usize = 8 * 1024;
const POST_CONTROL_LIMIT: usize = 24 * 1024 * 1024;

fn test_client() -> Result<Client, reqwest::Error> {
    Client::builder().no_proxy().build()
}

struct ChildGuard {
    child: Child,
}

impl ChildGuard {
    fn new(child: Child) -> Self {
        Self { child }
    }

    fn take_stdout(&mut self) -> std::process::ChildStdout {
        self.child.stdout.take().expect("child stdout is piped")
    }

    async fn wait_for_exit(&mut self, deadline: Duration) -> Result<ExitStatus, Box<dyn Error>> {
        Ok(timeout(deadline, async {
            loop {
                if let Some(status) = self.child.try_wait()? {
                    return Ok::<_, io::Error>(status);
                }
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "child did not exit in time"))??)
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

#[tokio::test]
async fn spawn_health_and_explicit_shutdown_are_deterministic() -> Result<(), Box<dyn Error>> {
    let server = TestServer::spawn().await?;
    assert!(server.base_url().starts_with("http://127.0.0.1:"));
    let base_url = server.base_url().to_owned();
    let client = test_client()?;
    let response = client.get(format!("{base_url}/healthz")).send().await?;
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()?
        .to_owned();
    let body = response.bytes().await?;
    let shutdown_started = Instant::now();
    timeout(Duration::from_secs(3), server.shutdown()).await??;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type, "text/plain; charset=utf-8");
    assert_eq!(&body[..], b"ok\n");
    assert!(shutdown_started.elapsed() < Duration::from_secs(3));
    assert!(
        client
            .get(format!("{base_url}/healthz"))
            .send()
            .await
            .is_err()
    );
    Ok(())
}

#[tokio::test]
async fn cli_rejects_bad_arguments_and_prints_exact_readiness_line() -> Result<(), Box<dyn Error>> {
    let binary = env!("CARGO_BIN_EXE_http-client-test-server");
    let invalid = Command::new(binary).arg("--listen-everywhere").output()?;
    assert_eq!(invalid.status.code(), Some(2));
    assert!(invalid.stdout.is_empty());
    assert_eq!(
        String::from_utf8(invalid.stderr)?,
        "Usage: http-client-test-server [--port <u16>]\n"
    );

    let mut child = ChildGuard::new(
        Command::new(binary)
            .args(["--port", "0"])
            .stdout(Stdio::piped())
            .spawn()?,
    );
    let mut line = String::new();
    std::io::BufReader::new(child.take_stdout()).read_line(&mut line)?;
    let base_url = line
        .strip_prefix("HTTP_CLIENT_TEST_SERVER=")
        .and_then(|line| line.strip_suffix('\n'))
        .expect("CLI prints the frozen readiness assignment");
    assert!(base_url.starts_with("http://127.0.0.1:"));
    let health = test_client()?
        .get(format!("{base_url}/healthz"))
        .send()
        .await?
        .bytes()
        .await?;
    assert_eq!(&health[..], b"ok\n");

    #[cfg(unix)]
    {
        let signalled = Command::new("kill")
            .args(["-INT", &child.child.id().to_string()])
            .status()?;
        assert!(signalled.success());
        assert!(child.wait_for_exit(Duration::from_secs(3)).await?.success());
    }
    #[cfg(windows)]
    {
        child.child.kill()?;
        child.wait_for_exit(Duration::from_secs(3)).await?;
    }
    Ok(())
}

#[tokio::test]
async fn respond_preserves_status_body_headers_and_head_length() -> Result<(), Box<dyn Error>> {
    let server = TestServer::spawn().await?;
    let spec = RespondSpec {
        status: 404,
        headers: vec![
            HeaderSpec {
                name: "set-cookie".to_owned(),
                value: "a=1".to_owned(),
            },
            HeaderSpec {
                name: "set-cookie".to_owned(),
                value: "b=2".to_owned(),
            },
        ],
        body: ResponseBodySpec::Json {
            value: serde_json::json!({"value": 7}),
        },
        chunk_size_bytes: 3,
        ..RespondSpec::default()
    };
    let url = server.respond_url(&spec)?;
    let client = test_client()?;
    let response = client.get(&url).send().await?;
    let status = response.status();
    let content_type = response.headers().get(header::CONTENT_TYPE).cloned();
    let cookies = response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|value| value.to_str().map(str::to_owned))
        .collect::<Result<Vec<_>, _>>()?;
    let body = response.bytes().await?;

    let head = client.head(&url).send().await?;
    let head_length = head
        .headers()
        .get(header::CONTENT_LENGTH)
        .expect("HEAD keeps the GET representation length")
        .to_str()?
        .parse::<u64>()?;
    let head_body = head.bytes().await?;
    server.shutdown().await?;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(content_type.is_none());
    assert_eq!(cookies, ["a=1", "b=2"]);
    assert_eq!(&body[..], br#"{"value":7}"#);
    assert_eq!(head_length, body.len() as u64);
    assert!(head_body.is_empty());
    Ok(())
}

#[tokio::test]
async fn respond_supports_chunked_repeat_and_business_request_body() -> Result<(), Box<dyn Error>> {
    let server = TestServer::spawn().await?;
    let spec = RespondSpec {
        body: ResponseBodySpec::Repeat {
            byte: b'z',
            len: 100_000,
        },
        chunk_size_bytes: 1024,
        framing: ResponseFraming::Chunked,
        ..RespondSpec::default()
    };
    let response = test_client()?
        .request(Method::PATCH, server.respond_url(&spec)?)
        .body(vec![b'q'; 256 * 1024])
        .send()
        .await?;
    let transfer_encoding = response
        .headers()
        .get(header::TRANSFER_ENCODING)
        .map(|value| value.to_str().map(str::to_owned))
        .transpose()?;
    let body = response.bytes().await?;
    server.shutdown().await?;

    assert_eq!(body.len(), 100_000);
    assert!(body.iter().all(|byte| *byte == b'z'));
    assert_eq!(transfer_encoding.as_deref(), Some("chunked"));
    Ok(())
}

#[tokio::test]
async fn invalid_controls_return_only_stable_codes() -> Result<(), Box<dyn Error>> {
    let server = TestServer::spawn().await?;
    let client = test_client()?;
    let invalid_body_variant = client
        .post(format!("{}/v1/respond", server.base_url()))
        .body(r#"{"body":{"kind":"empty","secret":"must-not-reflect"}}"#)
        .send()
        .await?;
    let first_status = invalid_body_variant.status();
    let first_body = invalid_body_variant.text().await?;
    let invalid_status = client
        .post(format!("{}/v1/respond", server.base_url()))
        .body(r#"{"status":199}"#)
        .send()
        .await?;
    let second_status = invalid_status.status();
    let second_body = invalid_status.text().await?;
    let invalid_abort = client
        .post(format!("{}/v1/abort", server.base_url()))
        .body(r#"{"phase":"before-head","secret":"must-not-reflect"}"#)
        .send()
        .await?;
    let abort_status = invalid_abort.status();
    let abort_body = invalid_abort.text().await?;
    let invalid_header = client
        .post(format!("{}/v1/respond", server.base_url()))
        .body(r#"{"headers":[{"name":"bad header","value":"secret"}]}"#)
        .send()
        .await?;
    let invalid_header_status = invalid_header.status();
    let invalid_header_body = invalid_header.text().await?;
    let restricted_header = client
        .post(format!("{}/v1/respond", server.base_url()))
        .body(r#"{"headers":[{"name":"content-length","value":"1"}]}"#)
        .send()
        .await?;
    let restricted_header_status = restricted_header.status();
    let restricted_header_body = restricted_header.text().await?;
    server.shutdown().await?;

    assert_eq!(first_status, StatusCode::BAD_REQUEST);
    assert_eq!(first_body, r#"{"code":"invalid_request"}"#);
    assert!(!first_body.contains("must-not-reflect"));
    assert_eq!(second_status, StatusCode::BAD_REQUEST);
    assert_eq!(second_body, r#"{"code":"invalid_status"}"#);
    assert_eq!(abort_status, StatusCode::BAD_REQUEST);
    assert_eq!(abort_body, r#"{"code":"invalid_request"}"#);
    assert!(!abort_body.contains("must-not-reflect"));
    assert_eq!(invalid_header_status, StatusCode::BAD_REQUEST);
    assert_eq!(invalid_header_body, r#"{"code":"invalid_header"}"#);
    assert_eq!(restricted_header_status, StatusCode::BAD_REQUEST);
    assert_eq!(restricted_header_body, r#"{"code":"restricted_header"}"#);
    Ok(())
}

#[tokio::test]
async fn response_codings_are_generated_and_conflicts_are_rejected() -> Result<(), Box<dyn Error>> {
    let server = TestServer::spawn().await?;
    let client = Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    for (encoding, expected) in [
        (ContentEncoding::Gzip, "gzip"),
        (ContentEncoding::Br, "br"),
        (ContentEncoding::Deflate, "deflate"),
        (ContentEncoding::Zstd, "zstd"),
    ] {
        let spec = RespondSpec {
            body: ResponseBodySpec::Base64 {
                value: STANDARD.encode(b"compressible response response response"),
            },
            content_encoding: Some(encoding),
            ..RespondSpec::default()
        };
        let response = client.get(server.respond_url(&spec)?).send().await?;
        assert_eq!(
            response.headers().get(header::CONTENT_ENCODING).unwrap(),
            expected
        );
        assert!(!response.bytes().await?.is_empty());
    }

    let unknown = RespondSpec {
        headers: vec![HeaderSpec {
            name: "content-encoding".to_owned(),
            value: "future-coding".to_owned(),
        }],
        body: ResponseBodySpec::Base64 {
            value: STANDARD.encode(b"unencoded unknown coding body"),
        },
        ..RespondSpec::default()
    };
    let response = client.get(server.respond_url(&unknown)?).send().await?;
    assert_eq!(
        response.headers().get(header::CONTENT_ENCODING).unwrap(),
        "future-coding"
    );
    assert_eq!(
        &response.bytes().await?[..],
        b"unencoded unknown coding body"
    );

    let target = RespondSpec {
        status: 201,
        body: ResponseBodySpec::Base64 {
            value: STANDARD.encode(b"redirect target"),
        },
        ..RespondSpec::default()
    };
    let target_url = server.respond_url(&target)?;
    let redirect = RespondSpec {
        status: 302,
        headers: vec![HeaderSpec {
            name: "location".to_owned(),
            value: target_url,
        }],
        ..RespondSpec::default()
    };
    let response = client.get(server.respond_url(&redirect)?).send().await?;
    assert_eq!(response.status(), StatusCode::FOUND);
    let location = response
        .headers()
        .get(header::LOCATION)
        .expect("redirect location")
        .to_str()?
        .to_owned();
    let response = client.get(location).send().await?;
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(&response.bytes().await?[..], b"redirect target");

    let conflict = RespondSpec {
        headers: vec![HeaderSpec {
            name: "content-encoding".to_owned(),
            value: "custom".to_owned(),
        }],
        content_encoding: Some(ContentEncoding::Gzip),
        ..RespondSpec::default()
    };
    let response = client.get(server.respond_url(&conflict)?).send().await?;
    let status = response.status();
    let body = response.text().await?;
    server.shutdown().await?;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body, r#"{"code":"conflicting_content_encoding"}"#);
    Ok(())
}

#[tokio::test]
async fn header_and_chunk_delays_are_observable() -> Result<(), Box<dyn Error>> {
    let server = TestServer::spawn().await?;
    let spec = RespondSpec {
        body: ResponseBodySpec::Repeat { byte: b'd', len: 2 },
        delay_before_headers_ms: 40,
        chunk_size_bytes: 1,
        delay_between_chunks_ms: 40,
        ..RespondSpec::default()
    };
    let started = Instant::now();
    let response = test_client()?
        .get(server.respond_url(&spec)?)
        .send()
        .await?;
    let head_elapsed = started.elapsed();
    let body = response.bytes().await?;
    let total_elapsed = started.elapsed();
    server.shutdown().await?;

    assert_eq!(&body[..], b"dd");
    assert!(head_elapsed >= Duration::from_millis(25));
    assert!(total_elapsed >= Duration::from_millis(65));
    Ok(())
}

#[tokio::test]
async fn slow_body_reader_does_not_block_other_connections() -> Result<(), Box<dyn Error>> {
    let server = TestServer::spawn().await?;
    let spec = RespondSpec {
        body: ResponseBodySpec::Repeat {
            byte: b'b',
            len: REQUEST_BODY_LIMIT as u64,
        },
        chunk_size_bytes: 64 * 1024,
        framing: ResponseFraming::Chunked,
        ..RespondSpec::default()
    };
    let url = server.respond_url(&spec)?;
    let path = relative_path(&url, server.base_url());
    let address = server
        .base_url()
        .strip_prefix("http://")
        .expect("test URL uses HTTP");
    let mut slow_reader = TcpStream::connect(address).await?;
    slow_reader
        .write_all(
            format!("GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .await?;
    let mut initial = [0_u8; 1024];
    let initial_len = timeout(Duration::from_secs(1), slow_reader.read(&mut initial)).await??;
    assert!(initial_len > 0);

    sleep(Duration::from_millis(100)).await;
    let health = timeout(
        Duration::from_millis(500),
        test_client()?
            .get(format!("{}/healthz", server.base_url()))
            .send(),
    )
    .await??;
    assert_eq!(health.status(), StatusCode::OK);
    assert_eq!(&health.bytes().await?[..], b"ok\n");

    drop(slow_reader);
    timeout(Duration::from_secs(3), server.shutdown()).await??;
    Ok(())
}

#[tokio::test]
async fn active_connection_limit_closes_every_excess_connection_promptly()
-> Result<(), Box<dyn Error>> {
    let server = TestServer::spawn().await?;
    let spec = RespondSpec {
        delay_before_headers_ms: 5_000,
        ..RespondSpec::default()
    };
    let url = server.respond_url(&spec)?;
    let path = relative_path(&url, server.base_url());
    let address = server
        .base_url()
        .strip_prefix("http://")
        .expect("test URL uses HTTP");
    let request = format!("GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n");

    let mut held = Vec::new();
    for _ in 0..64 {
        let mut connection = TcpStream::connect(address).await?;
        connection.write_all(request.as_bytes()).await?;
        held.push(connection);
    }
    sleep(Duration::from_millis(500)).await;

    let mut excess = Vec::new();
    for _ in 0..16 {
        let mut connection = TcpStream::connect(address).await?;
        connection.write_all(request.as_bytes()).await?;
        excess.push(connection);
    }
    let results = join_all(excess.into_iter().map(|mut connection| async move {
        let mut one = [0_u8; 1];
        timeout(Duration::from_secs(1), connection.read(&mut one)).await
    }))
    .await;
    let promptly_closed = results
        .into_iter()
        .filter(|result| matches!(result, Ok(Ok(0)) | Ok(Err(_))))
        .count();
    assert!(
        promptly_closed == 16,
        "all connections above the 64-connection limit close promptly"
    );

    drop(held);
    timeout(Duration::from_secs(3), server.shutdown()).await??;
    Ok(())
}

#[tokio::test]
async fn abort_phases_have_distinct_wire_results() -> Result<(), Box<dyn Error>> {
    let server = TestServer::spawn().await?;
    let before_url = server.abort_url(&AbortSpec::BeforeHead)?;
    let before = raw_get(
        server.base_url(),
        relative_path(&before_url, server.base_url()),
    )
    .await?;
    let head_before = raw_method(
        server.base_url(),
        "HEAD",
        relative_path(&before_url, server.base_url()),
    )
    .await?;

    let mid_url = server.abort_url(&AbortSpec::MidBody {
        bytes_before_abort: 17,
        chunk_size_bytes: 5,
        delay_between_chunks_ms: 0,
    })?;
    let mid = raw_get(
        server.base_url(),
        relative_path(&mid_url, server.base_url()),
    )
    .await?;
    let head_mid = test_client()?.head(&mid_url).send().await?;
    let head_mid_status = head_mid.status();
    let head_mid_length = head_mid
        .headers()
        .get(header::CONTENT_LENGTH)
        .expect("HEAD invalid_request keeps the error representation length")
        .to_str()?
        .parse::<usize>()?;
    let head_mid_body = head_mid.bytes().await?;
    server.shutdown().await?;

    assert!(before.is_empty());
    assert!(head_before.is_empty());
    let separator = find_subslice(&mid, b"\r\n\r\n").expect("response head exists") + 4;
    assert!(String::from_utf8_lossy(&mid[..separator]).starts_with("HTTP/1.1 200"));
    assert!(String::from_utf8_lossy(&mid[..separator]).contains("content-length: 18"));
    assert_eq!(&mid[separator..], vec![b'x'; 17]);
    assert_eq!(head_mid_status, StatusCode::BAD_REQUEST);
    assert_eq!(head_mid_length, r#"{"code":"invalid_request"}"#.len());
    assert!(head_mid_body.is_empty());
    Ok(())
}

#[tokio::test]
async fn echo_is_exact_and_rejects_declared_overflow_without_partial_data()
-> Result<(), Box<dyn Error>> {
    let server = TestServer::spawn().await?;
    let data = vec![0, 1, 2, 0xff, 4, 5];
    let client = test_client()?;
    let mut request_headers = reqwest::header::HeaderMap::new();
    request_headers.append(header::CONTENT_TYPE, "application/x-test-a".parse()?);
    request_headers.append(header::CONTENT_TYPE, "application/x-test-b".parse()?);
    let response = client
        .post(format!("{}/v1/echo", server.base_url()))
        .headers(request_headers)
        .body(data.clone())
        .send()
        .await?;
    let content_types = response
        .headers()
        .get_all(header::CONTENT_TYPE)
        .iter()
        .map(|value| value.to_str().map(str::to_owned))
        .collect::<Result<Vec<_>, _>>()?;
    let echoed = response.bytes().await?;

    let no_type = client
        .post(format!("{}/v1/echo", server.base_url()))
        .body("no explicit content type")
        .send()
        .await?;
    let default_content_type = no_type
        .headers()
        .get(header::CONTENT_TYPE)
        .expect("echo adds the frozen default content type")
        .to_str()?
        .to_owned();
    let no_type_body = no_type.bytes().await?;

    let at_limit = Bytes::from(vec![0x6b; REQUEST_BODY_LIMIT]);
    let response = client
        .post(format!("{}/v1/echo", server.base_url()))
        .body(at_limit.clone())
        .send()
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let at_limit_echo = response.bytes().await?;
    assert_eq!(at_limit_echo, at_limit);
    drop(at_limit_echo);
    drop(at_limit);

    let overflow = raw_exchange(
        server.base_url(),
        concat!(
            "POST /v1/echo HTTP/1.1\r\n",
            "Host: 127.0.0.1\r\n",
            "Content-Length: 67108865\r\n",
            "Connection: close\r\n\r\n"
        )
        .as_bytes(),
    )
    .await?;

    let chunks = (0..=REQUEST_BODY_LIMIT / (1024 * 1024))
        .map(|_| Ok::<Bytes, io::Error>(Bytes::from(vec![0x73; 1024 * 1024])));
    let observed_overflow = client
        .post(format!("{}/v1/echo", server.base_url()))
        .body(reqwest::Body::wrap_stream(stream::iter(chunks)))
        .send()
        .await?;
    let observed_status = observed_overflow.status();
    let observed_body = observed_overflow.text().await?;
    server.shutdown().await?;

    assert_eq!(
        content_types,
        ["application/x-test-a", "application/x-test-b"]
    );
    assert_eq!(&echoed[..], &data);
    assert_eq!(default_content_type, "application/octet-stream");
    assert_eq!(&no_type_body[..], b"no explicit content type");
    assert!(String::from_utf8_lossy(&overflow).starts_with("HTTP/1.1 413"));
    assert!(String::from_utf8_lossy(&overflow).contains("request_body_too_large"));
    assert!(!overflow.windows(data.len()).any(|window| window == data));
    assert_eq!(observed_status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(observed_body, r#"{"code":"request_body_too_large"}"#);
    Ok(())
}

#[tokio::test]
async fn query_and_post_control_boundaries_are_exact() -> Result<(), Box<dyn Error>> {
    let server = TestServer::spawn().await?;
    let client = test_client()?;

    // 6 KiB of JSON encodes to the exact 8 KiB query-value boundary.
    let encoded_boundary_spec = respond_spec_with_serialized_length(6 * 1024);
    let encoded_boundary_json = serde_json::to_vec(&encoded_boundary_spec)?;
    let encoded_boundary = URL_SAFE_NO_PAD.encode(&encoded_boundary_json);
    assert_eq!(encoded_boundary_json.len(), 6 * 1024);
    assert_eq!(encoded_boundary.len(), QUERY_SPEC_LIMIT);
    let response = client
        .get(format!(
            "{}/v1/respond?spec={encoded_boundary}",
            server.base_url()
        ))
        .send()
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    drop(response);

    let encoded_over_spec = respond_spec_with_serialized_length(6 * 1024 + 1);
    let encoded_over = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&encoded_over_spec)?);
    assert!(encoded_over.len() > QUERY_SPEC_LIMIT);
    let response = client
        .get(format!(
            "{}/v1/respond?spec={encoded_over}",
            server.base_url()
        ))
        .send()
        .await?;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(response.text().await?, r#"{"code":"limit_exceeded"}"#);

    // An exact 8 KiB decoded spec necessarily exceeds the independently enforced encoded cap.
    let decoded_boundary_spec = respond_spec_with_serialized_length(QUERY_SPEC_LIMIT);
    assert_eq!(
        serde_json::to_vec(&decoded_boundary_spec)?.len(),
        QUERY_SPEC_LIMIT
    );
    assert_eq!(
        server.respond_url(&decoded_boundary_spec),
        Err(SpecUrlError::TooLarge)
    );

    let mut post_boundary = br#"{"status":200}"#.to_vec();
    post_boundary.resize(POST_CONTROL_LIMIT, b' ');
    let response = client
        .post(format!("{}/v1/respond", server.base_url()))
        .body(Bytes::from(post_boundary))
        .send()
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.bytes().await?.is_empty());

    server.shutdown().await?;
    Ok(())
}

fn respond_spec_with_serialized_length(target: usize) -> RespondSpec {
    let mut spec = RespondSpec {
        headers: vec![HeaderSpec {
            name: "x-boundary".to_owned(),
            value: String::new(),
        }],
        ..RespondSpec::default()
    };
    let base_length = serde_json::to_vec(&spec).unwrap().len();
    assert!(base_length <= target);
    spec.headers[0].value = "p".repeat(target - base_length);
    assert_eq!(serde_json::to_vec(&spec).unwrap().len(), target);
    spec
}

async fn raw_get(base_url: &str, path: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    raw_method(base_url, "GET", path).await
}

async fn raw_method(base_url: &str, method: &str, path: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    raw_exchange(
        base_url,
        format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
            .as_bytes(),
    )
    .await
}

async fn raw_exchange(base_url: &str, request: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let address = base_url
        .strip_prefix("http://")
        .expect("test URL uses HTTP");
    let mut stream = TcpStream::connect(address).await?;
    stream.write_all(request).await?;
    stream.shutdown().await?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await?;
    Ok(response)
}

fn relative_path<'a>(url: &'a str, base_url: &str) -> &'a str {
    url.strip_prefix(base_url)
        .expect("helper URL uses server base")
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

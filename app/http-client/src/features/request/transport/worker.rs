use std::time::{Duration, Instant};

use async_channel::{Sender, TrySendError};
use http::{HeaderMap, header};
use reqwest::Client;

use super::{
    WorkerEvent, body,
    redirect::{RedirectError, RedirectState},
};
use crate::features::request::{
    prepared::{PreparedBody, PreparedRequest},
    response::{CAPTURE_LIMIT_BYTES, ResponseHead, ResponseProgress, collect_response_body},
    runtime::{BodySizeDimension, RedirectProblemKind, RequestProblem},
};

const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

pub(super) async fn execute(
    prepared: PreparedRequest,
    client: Client,
    sender: &Sender<WorkerEvent>,
    started_at: Instant,
) -> Result<crate::features::request::response::CompletedBody, RequestProblem> {
    let PreparedRequest {
        mut method,
        mut url,
        mut headers,
        body,
        body_content_type,
        redirect,
        timeout: _,
    } = prepared;
    let mut body_available = !matches!(body, PreparedBody::None);
    let mut redirects = RedirectState::new(redirect, &url);

    loop {
        let mut builder = client.request(method.clone(), url.clone());
        if body_available {
            builder = body::apply(builder, &body, &body_content_type)
                .await
                .map_err(RequestProblem::request_body_read)?;
        }
        // Generated body headers are applied first. The frozen explicit map
        // then replaces every same-name generated value while preserving its
        // own duplicate values.
        builder = builder.headers(headers.clone());

        let response = builder.send().await.map_err(classify_send_error)?;
        let status = response.status();
        if let Some(next) = redirects
            .next(
                status,
                response.headers().get(header::LOCATION),
                &url,
                &method,
                &mut headers,
                body_available,
            )
            .map_err(map_redirect_error)?
        {
            if !next.keep_body {
                strip_body_headers(&mut headers);
            }
            url = next.url;
            method = next.method;
            body_available = next.keep_body;
            continue;
        }

        return receive_final(response, sender, started_at).await;
    }
}

async fn receive_final(
    response: reqwest::Response,
    sender: &Sender<WorkerEvent>,
    started_at: Instant,
) -> Result<crate::features::request::response::CompletedBody, RequestProblem> {
    let declared_encoded_bytes = declared_content_length(response.headers());
    let head = ResponseHead {
        status: response.status(),
        version: response.version(),
        final_url: response.url().clone(),
        headers: response.headers().clone(),
    };
    let initial = ResponseProgress::initial(declared_encoded_bytes);
    sender
        .send(WorkerEvent::HeadReceived {
            head,
            head_after: started_at.elapsed(),
            progress: initial,
        })
        .await
        .map_err(|_| RequestProblem::internal())?;

    if let Some(declared) = declared_encoded_bytes
        && declared > CAPTURE_LIMIT_BYTES
    {
        return Err(RequestProblem::too_large(
            BodySizeDimension::Encoded,
            CAPTURE_LIMIT_BYTES,
            declared,
        ));
    }

    let headers = response.headers().clone();
    let mut last_progress = None;
    let mut last_sent_at = None;
    let result = collect_response_body(&headers, response.bytes_stream(), |progress| {
        last_progress = Some(progress);
        let now = Instant::now();
        if last_sent_at.is_none_or(|previous| now.duration_since(previous) >= PROGRESS_INTERVAL) {
            match sender.try_send(WorkerEvent::BodyProgress(progress)) {
                Ok(()) => last_sent_at = Some(now),
                Err(TrySendError::Full(_)) => {}
                Err(TrySendError::Closed(_)) => {}
            }
        }
    })
    .await;

    if result.is_err()
        && let Some(progress) = last_progress
    {
        sender
            .send(WorkerEvent::BodyProgress(progress))
            .await
            .map_err(|_| RequestProblem::internal())?;
    }
    result
}

fn strip_body_headers(headers: &mut HeaderMap) {
    headers.remove(header::CONTENT_LENGTH);
    headers.remove(header::CONTENT_TYPE);
    headers.remove(header::TRANSFER_ENCODING);
}

fn declared_content_length(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse().ok())
}

fn classify_send_error(error: reqwest::Error) -> RequestProblem {
    let is_request_body = error.is_body();
    let error = error.without_url();
    if is_request_body {
        RequestProblem::request_body_read(error)
    } else {
        RequestProblem::transport(error)
    }
}

fn map_redirect_error(error: RedirectError) -> RequestProblem {
    RequestProblem::redirect(match error {
        RedirectError::InvalidLocation => RedirectProblemKind::InvalidLocation,
        RedirectError::Loop => RedirectProblemKind::Loop,
        RedirectError::HopLimit => RedirectProblemKind::HopLimit,
    })
}

#[cfg(test)]
mod tests {
    use http::Method;

    use super::*;

    #[test]
    fn declared_content_length_is_strictly_numeric() {
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_LENGTH, "123".parse().unwrap());
        assert_eq!(declared_content_length(&headers), Some(123));
        headers.insert(header::CONTENT_LENGTH, "invalid".parse().unwrap());
        assert_eq!(declared_content_length(&headers), None);
    }

    #[test]
    fn body_rewrite_removes_every_body_metadata_header() {
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_LENGTH, "1".parse().unwrap());
        headers.insert(header::CONTENT_TYPE, "text/plain".parse().unwrap());
        headers.insert(header::TRANSFER_ENCODING, "chunked".parse().unwrap());
        headers.insert("x-test", "kept".parse().unwrap());
        strip_body_headers(&mut headers);
        assert!(!headers.contains_key(header::CONTENT_LENGTH));
        assert!(!headers.contains_key(header::CONTENT_TYPE));
        assert!(!headers.contains_key(header::TRANSFER_ENCODING));
        assert_eq!(headers.get("x-test").unwrap(), "kept");
    }

    #[test]
    fn send_error_classification_does_not_expose_a_url() {
        // Reqwest builder errors can carry the request URL. The production
        // classifier always removes it before retaining the source.
        let client = Client::builder().build().unwrap();
        let error = client.get("not a URL").build().unwrap_err();
        let problem = classify_send_error(error);
        assert!(!format!("{problem:?}").contains("not a URL"));
        assert!(!problem.to_string().contains("not a URL"));
    }

    #[test]
    fn head_requests_keep_the_method_on_see_other() {
        let mut state = RedirectState::new(
            crate::features::request::prepared::PreparedRedirect {
                follow: true,
                max_hops: 10,
                preserve_method: false,
                forward_authorization_cross_host: false,
            },
            &url::Url::parse("http://example.test/start").unwrap(),
        );
        let next = state
            .next(
                http::StatusCode::SEE_OTHER,
                Some(&http::HeaderValue::from_static("/next")),
                &url::Url::parse("http://example.test/start").unwrap(),
                &Method::HEAD,
                &mut HeaderMap::new(),
                false,
            )
            .unwrap()
            .unwrap();
        assert_eq!(next.method, Method::HEAD);
        assert!(!next.keep_body);
    }
}

use std::time::Duration;

use bytes::Bytes;
use futures_util::stream;
use http_body_util::{BodyExt as _, StreamBody};
use hyper::{
    Method, Request, Response, StatusCode,
    body::{Frame, Incoming},
    header::{CONTENT_LENGTH, CONTENT_TYPE, HeaderValue},
};
use tokio_util::sync::CancellationToken;

use crate::{
    AbortSpec,
    contract::{
        ControlCode, ControlError, REQUEST_BODY_LIMIT, declared_content_length,
        discard_bounded_body, parse_post_spec, parse_query_spec, validate_abort,
    },
    server::{ServerBody, WireError, control_error_response, empty_response},
};

const QUERY_CONTROL_ALLOW: &str = "GET, HEAD, POST, PUT, DELETE, OPTIONS, PATCH, TRACE";

struct AbortState {
    byte: u8,
    remaining: u32,
    chunk_size: u32,
    delay: Duration,
    emitted: bool,
    terminal_error_emitted: bool,
    cancellation: CancellationToken,
}

pub(crate) async fn handle(
    request: Request<Incoming>,
    cancellation: CancellationToken,
) -> Result<Response<ServerBody>, WireError> {
    let method = request.method().clone();
    let query = request.uri().query().map(str::to_owned);
    if query.is_some() {
        if !supports_query_control(&method) {
            return Ok(empty_response(
                StatusCode::METHOD_NOT_ALLOWED,
                Some(QUERY_CONTROL_ALLOW),
            ));
        }
    } else if method != Method::POST {
        return Ok(empty_response(StatusCode::METHOD_NOT_ALLOWED, Some("POST")));
    }
    let declared_length = declared_content_length(request.headers());
    let body = request.into_body();
    let spec = if let Some(query) = query.as_deref() {
        let spec = match parse_query_spec::<AbortSpec>(Some(query)).await {
            Ok(Some(spec)) => spec,
            Ok(None) => unreachable!("query was present"),
            Err(error) => return Ok(control_error_response(error)),
        };
        if let Err(error) = discard_bounded_body(
            body,
            declared_length,
            REQUEST_BODY_LIMIT,
            ControlCode::RequestBodyTooLarge,
        )
        .await
        {
            return Ok(control_error_response(error));
        }
        spec
    } else {
        match parse_post_spec(body, declared_length).await {
            Ok(spec) => spec,
            Err(error) => return Ok(control_error_response(error)),
        }
    };

    if let Err(error) = validate_abort(&spec) {
        return Ok(control_error_response(error));
    }
    if let Err(error) = validate_method_phase(&method, &spec) {
        return Ok(control_error_response(error));
    }
    match spec {
        AbortSpec::BeforeHead => Err(WireError::BeforeHead),
        AbortSpec::MidBody {
            bytes_before_abort,
            chunk_size_bytes,
            delay_between_chunks_ms,
        } => Ok(mid_body_response(
            bytes_before_abort,
            chunk_size_bytes,
            delay_between_chunks_ms,
            cancellation,
        )),
    }
}

fn supports_query_control(method: &Method) -> bool {
    [
        Method::GET,
        Method::HEAD,
        Method::POST,
        Method::PUT,
        Method::DELETE,
        Method::OPTIONS,
        Method::PATCH,
        Method::TRACE,
    ]
    .contains(method)
}

fn validate_method_phase(method: &Method, spec: &AbortSpec) -> Result<(), ControlError> {
    if method == Method::HEAD && matches!(spec, AbortSpec::MidBody { .. }) {
        Err(ControlError::invalid(ControlCode::InvalidRequest))
    } else {
        Ok(())
    }
}

fn mid_body_response(
    bytes_before_abort: u32,
    chunk_size_bytes: u32,
    delay_between_chunks_ms: u32,
    cancellation: CancellationToken,
) -> Response<ServerBody> {
    let state = AbortState {
        byte: b'x',
        remaining: bytes_before_abort,
        chunk_size: chunk_size_bytes,
        delay: Duration::from_millis(u64::from(delay_between_chunks_ms)),
        emitted: false,
        terminal_error_emitted: false,
        cancellation,
    };
    let frames = stream::unfold(state, |mut state| async move {
        if state.terminal_error_emitted || state.cancellation.is_cancelled() {
            return None;
        }
        if state.remaining == 0 {
            // Force at least one Pending between the final prefix frame and the body error so
            // Hyper has an opportunity to flush the observable partial payload to the socket.
            tokio::time::sleep(Duration::from_millis(25)).await;
            state.terminal_error_emitted = true;
            return Some((Err::<Frame<Bytes>, _>(WireError::BodyInterrupted), state));
        }
        if state.emitted && !state.delay.is_zero() {
            tokio::select! {
                _ = tokio::time::sleep(state.delay) => {}
                _ = state.cancellation.cancelled() => return None,
            }
        }
        if state.cancellation.is_cancelled() {
            return None;
        }
        let length = state.remaining.min(state.chunk_size) as usize;
        state.remaining -= length as u32;
        state.emitted = true;
        Some((
            Ok(Frame::data(Bytes::from(vec![state.byte; length]))),
            state,
        ))
    });
    let body = StreamBody::new(frames).boxed_unsync();
    let mut response = Response::new(body);
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    response.headers_mut().insert(
        CONTENT_LENGTH,
        HeaderValue::from_str(&(u64::from(bytes_before_abort) + 1).to_string())
            .expect("decimal content length is valid"),
    );
    response
}

#[cfg(test)]
mod tests {
    use hyper::Method;

    use super::{supports_query_control, validate_method_phase};
    use crate::{
        AbortSpec,
        contract::{ControlCode, ControlError},
    };

    #[test]
    fn query_control_accepts_only_standard_non_connect_methods() {
        for method in [
            Method::GET,
            Method::HEAD,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
            Method::PATCH,
            Method::TRACE,
        ] {
            assert!(supports_query_control(&method));
        }
        assert!(!supports_query_control(&Method::CONNECT));
        assert!(!supports_query_control(
            &Method::from_bytes(b"CUSTOM").expect("custom method is syntactically valid")
        ));
    }

    #[test]
    fn head_abort_semantics_depend_on_phase() {
        assert_eq!(
            validate_method_phase(&Method::HEAD, &AbortSpec::BeforeHead),
            Ok(())
        );
        assert_eq!(
            validate_method_phase(
                &Method::HEAD,
                &AbortSpec::MidBody {
                    bytes_before_abort: 1,
                    chunk_size_bytes: 1,
                    delay_between_chunks_ms: 0,
                }
            ),
            Err(ControlError::invalid(ControlCode::InvalidRequest))
        );
        assert_eq!(
            validate_method_phase(
                &Method::GET,
                &AbortSpec::MidBody {
                    bytes_before_abort: 1,
                    chunk_size_bytes: 1,
                    delay_between_chunks_ms: 0,
                }
            ),
            Ok(())
        );
    }
}

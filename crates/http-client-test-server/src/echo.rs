use bytes::Bytes;
use hyper::{
    Request, Response, StatusCode,
    body::Incoming,
    header::{CONTENT_LENGTH, CONTENT_TYPE, HeaderValue},
};

use crate::{
    contract::{ControlCode, REQUEST_BODY_LIMIT, declared_content_length, read_bounded_body},
    server::{ServerBody, control_error_response, full_body},
};

pub(crate) async fn handle(request: Request<Incoming>) -> Response<ServerBody> {
    let content_types = request
        .headers()
        .get_all(CONTENT_TYPE)
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    let declared_length = declared_content_length(request.headers());
    let bytes = match read_bounded_body(
        request.into_body(),
        declared_length,
        REQUEST_BODY_LIMIT,
        ControlCode::RequestBodyTooLarge,
    )
    .await
    {
        Ok(bytes) => bytes,
        Err(error) => return control_error_response(error),
    };
    echo_response(bytes, content_types)
}

fn echo_response(bytes: Bytes, content_types: Vec<HeaderValue>) -> Response<ServerBody> {
    let length = HeaderValue::from_str(&bytes.len().to_string()).expect("decimal length is valid");
    let mut response = Response::new(full_body(bytes));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(CONTENT_LENGTH, length);
    if content_types.is_empty() {
        response.headers_mut().insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
    } else {
        for content_type in content_types {
            response.headers_mut().append(CONTENT_TYPE, content_type);
        }
    }
    response
}

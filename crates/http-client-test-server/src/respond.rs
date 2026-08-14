use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use async_compression::tokio::write::{BrotliEncoder, GzipEncoder, ZlibEncoder, ZstdEncoder};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use bytes::Bytes;
use futures_util::stream;
use http_body_util::{BodyExt as _, StreamBody};
use hyper::{
    Method, Request, Response, StatusCode,
    body::{Frame, Incoming},
    header::{CONTENT_ENCODING, CONTENT_LENGTH, HeaderValue},
};
use tokio::io::{AsyncWrite, AsyncWriteExt as _};
use tokio_util::sync::CancellationToken;

use crate::{
    ContentEncoding, RespondSpec, ResponseBodySpec, ResponseFraming,
    contract::{
        COMPRESSION_OUTPUT_LIMIT, ControlCode, ControlError, MAX_CHUNK_BYTES, MAX_CHUNKS,
        MAX_HEAD_DELAY_MS, REPEAT_LIMIT, REQUEST_BODY_LIMIT, RESPONSE_SOURCE_LIMIT,
        declared_content_length, discard_bounded_body, parse_post_spec, parse_query_spec,
        validate_delays, validate_headers,
    },
    server::{ServerBody, WireError, control_error_response, empty_body, empty_response},
};

enum ResponseSource {
    Bytes(Bytes),
    Repeat { byte: u8, len: u64 },
}

impl ResponseSource {
    fn len(&self) -> u64 {
        match self {
            Self::Bytes(bytes) => bytes.len() as u64,
            Self::Repeat { len, .. } => *len,
        }
    }

    fn into_stream_source(self) -> StreamSource {
        match self {
            Self::Bytes(bytes) => StreamSource::Bytes { bytes, offset: 0 },
            Self::Repeat { byte, len } => StreamSource::Repeat {
                byte,
                remaining: len,
            },
        }
    }
}

enum StreamSource {
    Bytes { bytes: Bytes, offset: usize },
    Repeat { byte: u8, remaining: u64 },
}

impl StreamSource {
    fn is_empty(&self) -> bool {
        match self {
            Self::Bytes { bytes, offset } => *offset >= bytes.len(),
            Self::Repeat { remaining, .. } => *remaining == 0,
        }
    }

    fn take_chunk(&mut self, chunk_size: usize) -> Bytes {
        match self {
            Self::Bytes { bytes, offset } => {
                let end = offset.saturating_add(chunk_size).min(bytes.len());
                let chunk = bytes.slice(*offset..end);
                *offset = end;
                chunk
            }
            Self::Repeat { byte, remaining } => {
                let length = (*remaining).min(chunk_size as u64) as usize;
                *remaining -= length as u64;
                Bytes::from(vec![*byte; length])
            }
        }
    }
}

struct StreamState {
    source: StreamSource,
    chunk_size: usize,
    delay: Duration,
    emitted: bool,
    cancellation: CancellationToken,
}

pub(crate) async fn handle(
    request: Request<Incoming>,
    cancellation: CancellationToken,
) -> Result<Response<ServerBody>, WireError> {
    let method = request.method().clone();
    let query = request.uri().query().map(str::to_owned);
    if query.is_none() && method != Method::POST {
        return Ok(empty_response(StatusCode::METHOD_NOT_ALLOWED, Some("POST")));
    }
    let declared_length = declared_content_length(request.headers());
    let body = request.into_body();

    let parsed = if let Some(query) = query.as_deref() {
        let spec = match parse_query_spec::<RespondSpec>(Some(query)).await {
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

    let prepared = match prepare(parsed).await {
        Ok(prepared) => prepared,
        Err(error) => return Ok(control_error_response(error)),
    };

    if prepared.delay_before_headers_ms != 0 {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(u64::from(prepared.delay_before_headers_ms))) => {}
            _ = cancellation.cancelled() => return Err(WireError::Cancelled),
        }
    } else if cancellation.is_cancelled() {
        return Err(WireError::Cancelled);
    }

    Ok(prepared.into_response(method == Method::HEAD, cancellation))
}

struct PreparedResponse {
    status: StatusCode,
    headers: hyper::HeaderMap,
    source: ResponseSource,
    chunk_size: usize,
    delay_between_chunks_ms: u32,
    delay_before_headers_ms: u32,
    content_encoding: Option<ContentEncoding>,
    framing: ResponseFraming,
}

impl PreparedResponse {
    fn into_response(
        mut self,
        is_head: bool,
        cancellation: CancellationToken,
    ) -> Response<ServerBody> {
        let content_length = self.source.len();
        let body =
            if is_head || (content_length == 0 && self.framing == ResponseFraming::ContentLength) {
                empty_body()
            } else {
                stream_body(
                    self.source,
                    self.chunk_size,
                    self.delay_between_chunks_ms,
                    cancellation,
                )
            };
        let mut response = Response::new(body);
        *response.status_mut() = self.status;
        *response.headers_mut() = std::mem::take(&mut self.headers);
        if let Some(encoding) = self.content_encoding {
            response.headers_mut().insert(
                CONTENT_ENCODING,
                HeaderValue::from_static(encoding.header_value()),
            );
        }
        if self.framing == ResponseFraming::ContentLength {
            response.headers_mut().insert(
                CONTENT_LENGTH,
                HeaderValue::from_str(&content_length.to_string())
                    .expect("decimal content length is valid"),
            );
        }
        response
    }
}

async fn prepare(spec: RespondSpec) -> Result<PreparedResponse, ControlError> {
    let status = StatusCode::from_u16(spec.status)
        .map_err(|_| ControlError::invalid(ControlCode::InvalidStatus))?;
    if !(200..=599).contains(&spec.status) {
        return Err(ControlError::invalid(ControlCode::InvalidStatus));
    }
    if spec.delay_before_headers_ms > MAX_HEAD_DELAY_MS
        || !(1..=MAX_CHUNK_BYTES).contains(&spec.chunk_size_bytes)
    {
        return Err(ControlError::limit(ControlCode::LimitExceeded));
    }
    let validated_headers = validate_headers(&spec.headers)?;
    if spec.content_encoding.is_some() && validated_headers.has_content_encoding {
        return Err(ControlError::invalid(
            ControlCode::ConflictingContentEncoding,
        ));
    }

    let requires_empty = matches!(spec.status, 204 | 205 | 304);
    if requires_empty
        && (!matches!(&spec.body, ResponseBodySpec::Empty)
            || spec.content_encoding.is_some()
            || validated_headers.has_content_encoding)
    {
        return Err(ControlError::invalid(ControlCode::InvalidRequest));
    }

    let mut source = source_from_spec(spec.body)?;
    if let Some(encoding) = spec.content_encoding {
        if source.len() > RESPONSE_SOURCE_LIMIT as u64 {
            return Err(ControlError::limit(ControlCode::LimitExceeded));
        }
        source = ResponseSource::Bytes(Bytes::from(compress(source, encoding).await?));
    }
    let chunks = source.len().div_ceil(u64::from(spec.chunk_size_bytes));
    if chunks > MAX_CHUNKS {
        return Err(ControlError::limit(ControlCode::LimitExceeded));
    }
    validate_delays(spec.delay_between_chunks_ms, chunks)?;

    Ok(PreparedResponse {
        status,
        headers: validated_headers.values,
        source,
        chunk_size: spec.chunk_size_bytes as usize,
        delay_between_chunks_ms: spec.delay_between_chunks_ms,
        delay_before_headers_ms: spec.delay_before_headers_ms,
        content_encoding: spec.content_encoding,
        framing: spec.framing,
    })
}

fn source_from_spec(spec: ResponseBodySpec) -> Result<ResponseSource, ControlError> {
    match spec {
        ResponseBodySpec::Empty => Ok(ResponseSource::Bytes(Bytes::new())),
        ResponseBodySpec::Json { value } => {
            let bytes = serde_json::to_vec(&value)
                .map_err(|_| ControlError::invalid(ControlCode::InvalidRequest))?;
            if bytes.len() > RESPONSE_SOURCE_LIMIT {
                return Err(ControlError::limit(ControlCode::LimitExceeded));
            }
            Ok(ResponseSource::Bytes(Bytes::from(bytes)))
        }
        ResponseBodySpec::Base64 { value } => {
            let maximum_encoded_length = RESPONSE_SOURCE_LIMIT.div_ceil(3) * 4 + 4;
            if value.len() > maximum_encoded_length {
                return Err(ControlError::limit(ControlCode::LimitExceeded));
            }
            let bytes = STANDARD
                .decode(value)
                .map_err(|_| ControlError::invalid(ControlCode::InvalidRequest))?;
            if bytes.len() > RESPONSE_SOURCE_LIMIT {
                return Err(ControlError::limit(ControlCode::LimitExceeded));
            }
            Ok(ResponseSource::Bytes(Bytes::from(bytes)))
        }
        ResponseBodySpec::Repeat { byte, len } => {
            if len > REPEAT_LIMIT {
                return Err(ControlError::limit(ControlCode::LimitExceeded));
            }
            Ok(ResponseSource::Repeat { byte, len })
        }
    }
}

fn stream_body(
    source: ResponseSource,
    chunk_size: usize,
    delay_between_chunks_ms: u32,
    cancellation: CancellationToken,
) -> ServerBody {
    let state = StreamState {
        source: source.into_stream_source(),
        chunk_size,
        delay: Duration::from_millis(u64::from(delay_between_chunks_ms)),
        emitted: false,
        cancellation,
    };
    let frames = stream::unfold(state, |mut state| async move {
        if state.source.is_empty() || state.cancellation.is_cancelled() {
            return None;
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
        let bytes = state.source.take_chunk(state.chunk_size);
        state.emitted = true;
        Some((Ok::<_, WireError>(Frame::data(bytes)), state))
    });
    StreamBody::new(frames).boxed_unsync()
}

async fn compress(
    source: ResponseSource,
    encoding: ContentEncoding,
) -> Result<Vec<u8>, ControlError> {
    match encoding {
        ContentEncoding::Gzip => encode_with(GzipEncoder::new(BoundedWriter::new()), source).await,
        ContentEncoding::Br => encode_with(BrotliEncoder::new(BoundedWriter::new()), source).await,
        ContentEncoding::Deflate => {
            encode_with(ZlibEncoder::new(BoundedWriter::new()), source).await
        }
        ContentEncoding::Zstd => encode_with(ZstdEncoder::new(BoundedWriter::new()), source).await,
    }
}

async fn encode_with<E>(mut encoder: E, source: ResponseSource) -> Result<Vec<u8>, ControlError>
where
    E: AsyncWrite + Unpin + IntoBoundedWriter,
{
    let mut source = source.into_stream_source();
    while !source.is_empty() {
        let chunk = source.take_chunk(MAX_CHUNK_BYTES as usize);
        encoder
            .write_all(&chunk)
            .await
            .map_err(|_| ControlError::limit(ControlCode::LimitExceeded))?;
    }
    encoder
        .shutdown()
        .await
        .map_err(|_| ControlError::limit(ControlCode::LimitExceeded))?;
    Ok(encoder.into_bounded_writer().bytes)
}

trait IntoBoundedWriter {
    fn into_bounded_writer(self) -> BoundedWriter;
}

macro_rules! impl_into_bounded_writer {
    ($($type:ident),+ $(,)?) => {
        $(
            impl IntoBoundedWriter for $type<BoundedWriter> {
                fn into_bounded_writer(self) -> BoundedWriter {
                    self.into_inner()
                }
            }
        )+
    };
}

impl_into_bounded_writer!(GzipEncoder, BrotliEncoder, ZlibEncoder, ZstdEncoder);

struct BoundedWriter {
    bytes: Vec<u8>,
}

impl BoundedWriter {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }
}

impl AsyncWrite for BoundedWriter {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<io::Result<usize>> {
        let Some(next) = self.bytes.len().checked_add(buffer.len()) else {
            return Poll::Ready(Err(io::Error::other("bounded output exceeded")));
        };
        if next > COMPRESSION_OUTPUT_LIMIT {
            return Poll::Ready(Err(io::Error::other("bounded output exceeded")));
        }
        self.bytes.extend_from_slice(buffer);
        Poll::Ready(Ok(buffer.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

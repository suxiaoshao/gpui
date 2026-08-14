use std::fmt;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use bytes::{Bytes, BytesMut};
use http_body_util::BodyExt as _;
use hyper::{
    HeaderMap,
    body::Incoming,
    header::{
        CONNECTION, CONTENT_ENCODING, CONTENT_LENGTH, HeaderName, HeaderValue, TRAILER,
        TRANSFER_ENCODING, UPGRADE,
    },
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub(crate) const QUERY_SPEC_LIMIT: usize = 8 * 1024;
pub(crate) const POST_CONTROL_LIMIT: usize = 24 * 1024 * 1024;
pub(crate) const RESPONSE_SOURCE_LIMIT: usize = 16 * 1024 * 1024;
pub(crate) const REPEAT_LIMIT: u64 = 64 * 1024 * 1024;
pub(crate) const COMPRESSION_OUTPUT_LIMIT: usize = 18 * 1024 * 1024;
pub(crate) const REQUEST_BODY_LIMIT: usize = 64 * 1024 * 1024;
pub(crate) const MAX_HEADERS: usize = 128;
pub(crate) const MAX_HEADER_BYTES: usize = 8 * 1024;
pub(crate) const MAX_CHUNK_BYTES: u32 = 64 * 1024;
pub(crate) const MAX_CHUNKS: u64 = 4096;
pub(crate) const MAX_HEAD_DELAY_MS: u32 = 30_000;
pub(crate) const MAX_CHUNK_DELAY_MS: u32 = 1_000;
pub(crate) const MAX_STREAM_DELAY_MS: u64 = 60_000;

fn status_200() -> u16 {
    200
}

fn chunk_16_kib() -> u32 {
    16 * 1024
}

fn sixteen() -> u32 {
    16
}

/// Describes one response header. Repeated names are appended in order.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeaderSpec {
    pub name: String,
    pub value: String,
}

impl fmt::Debug for HeaderSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HeaderSpec")
            .field("name_length", &self.name.len())
            .field("value_length", &self.value.len())
            .finish()
    }
}

/// Source bytes for a controlled response.
#[derive(Clone, Default, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResponseBodySpec {
    #[default]
    Empty,
    Json {
        value: serde_json::Value,
    },
    Base64 {
        value: String,
    },
    Repeat {
        byte: u8,
        len: u64,
    },
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum StrictResponseBodySpec {
    Empty {},
    Json { value: serde_json::Value },
    Base64 { value: String },
    Repeat { byte: u8, len: u64 },
}

impl<'de> Deserialize<'de> for ResponseBodySpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(match StrictResponseBodySpec::deserialize(deserializer)? {
            StrictResponseBodySpec::Empty {} => Self::Empty,
            StrictResponseBodySpec::Json { value } => Self::Json { value },
            StrictResponseBodySpec::Base64 { value } => Self::Base64 { value },
            StrictResponseBodySpec::Repeat { byte, len } => Self::Repeat { byte, len },
        })
    }
}

impl fmt::Debug for ResponseBodySpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("Empty"),
            Self::Json { value } => f
                .debug_struct("Json")
                .field(
                    "serialized_length",
                    &serde_json::to_vec(value).map_or(0, |v| v.len()),
                )
                .finish(),
            Self::Base64 { value } => f
                .debug_struct("Base64")
                .field("encoded_length", &value.len())
                .finish(),
            Self::Repeat { len, .. } => f.debug_struct("Repeat").field("length", len).finish(),
        }
    }
}

/// HTTP/1 response framing selected by a test.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ResponseFraming {
    #[default]
    ContentLength,
    Chunked,
}

/// Content coding applied by the server before the response is streamed.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ContentEncoding {
    Gzip,
    Br,
    Deflate,
    Zstd,
}

impl ContentEncoding {
    pub(crate) fn header_value(self) -> &'static str {
        match self {
            Self::Gzip => "gzip",
            Self::Br => "br",
            Self::Deflate => "deflate",
            Self::Zstd => "zstd",
        }
    }
}

/// Complete control specification for `/v1/respond`.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RespondSpec {
    #[serde(default = "status_200")]
    pub status: u16,
    #[serde(default)]
    pub headers: Vec<HeaderSpec>,
    #[serde(default)]
    pub body: ResponseBodySpec,
    #[serde(default)]
    pub delay_before_headers_ms: u32,
    #[serde(default = "chunk_16_kib")]
    pub chunk_size_bytes: u32,
    #[serde(default)]
    pub delay_between_chunks_ms: u32,
    #[serde(default)]
    pub content_encoding: Option<ContentEncoding>,
    #[serde(default)]
    pub framing: ResponseFraming,
}

impl Default for RespondSpec {
    fn default() -> Self {
        Self {
            status: status_200(),
            headers: Vec::new(),
            body: ResponseBodySpec::Empty,
            delay_before_headers_ms: 0,
            chunk_size_bytes: chunk_16_kib(),
            delay_between_chunks_ms: 0,
            content_encoding: None,
            framing: ResponseFraming::ContentLength,
        }
    }
}

impl fmt::Debug for RespondSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RespondSpec")
            .field("status", &self.status)
            .field("header_count", &self.headers.len())
            .field("body", &self.body)
            .field("delay_before_headers_ms", &self.delay_before_headers_ms)
            .field("chunk_size_bytes", &self.chunk_size_bytes)
            .field("delay_between_chunks_ms", &self.delay_between_chunks_ms)
            .field("content_encoding", &self.content_encoding)
            .field("framing", &self.framing)
            .finish()
    }
}

/// Specifies a connection interruption before or after the response head.
#[derive(Clone, Serialize)]
#[serde(tag = "phase", rename_all = "kebab-case")]
pub enum AbortSpec {
    BeforeHead,
    MidBody {
        #[serde(default = "sixteen")]
        bytes_before_abort: u32,
        #[serde(default = "chunk_16_kib")]
        chunk_size_bytes: u32,
        #[serde(default)]
        delay_between_chunks_ms: u32,
    },
}

#[derive(Deserialize)]
#[serde(tag = "phase", rename_all = "kebab-case", deny_unknown_fields)]
enum StrictAbortSpec {
    BeforeHead {},
    MidBody {
        #[serde(default = "sixteen")]
        bytes_before_abort: u32,
        #[serde(default = "chunk_16_kib")]
        chunk_size_bytes: u32,
        #[serde(default)]
        delay_between_chunks_ms: u32,
    },
}

impl<'de> Deserialize<'de> for AbortSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(match StrictAbortSpec::deserialize(deserializer)? {
            StrictAbortSpec::BeforeHead {} => Self::BeforeHead,
            StrictAbortSpec::MidBody {
                bytes_before_abort,
                chunk_size_bytes,
                delay_between_chunks_ms,
            } => Self::MidBody {
                bytes_before_abort,
                chunk_size_bytes,
                delay_between_chunks_ms,
            },
        })
    }
}

impl fmt::Debug for AbortSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BeforeHead => f.write_str("BeforeHead"),
            Self::MidBody {
                bytes_before_abort,
                chunk_size_bytes,
                delay_between_chunks_ms,
            } => f
                .debug_struct("MidBody")
                .field("bytes_before_abort", bytes_before_abort)
                .field("chunk_size_bytes", chunk_size_bytes)
                .field("delay_between_chunks_ms", delay_between_chunks_ms)
                .finish(),
        }
    }
}

/// Failure to encode a control specification into a bounded URL.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SpecUrlError {
    #[error("control specification is too large for a URL")]
    TooLarge,
}

pub(crate) fn encode_spec<T: Serialize>(spec: &T) -> Result<String, SpecUrlError> {
    let json = serde_json::to_vec(spec).map_err(|_| SpecUrlError::TooLarge)?;
    if json.len() > QUERY_SPEC_LIMIT {
        return Err(SpecUrlError::TooLarge);
    }
    let encoded = URL_SAFE_NO_PAD.encode(json);
    if encoded.len() > QUERY_SPEC_LIMIT {
        return Err(SpecUrlError::TooLarge);
    }
    Ok(encoded)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ControlCode {
    InvalidRequest,
    InvalidStatus,
    InvalidHeader,
    RestrictedHeader,
    ConflictingContentEncoding,
    LimitExceeded,
    RequestBodyTooLarge,
}

impl ControlCode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::InvalidStatus => "invalid_status",
            Self::InvalidHeader => "invalid_header",
            Self::RestrictedHeader => "restricted_header",
            Self::ConflictingContentEncoding => "conflicting_content_encoding",
            Self::LimitExceeded => "limit_exceeded",
            Self::RequestBodyTooLarge => "request_body_too_large",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ControlError {
    pub(crate) code: ControlCode,
    pub(crate) too_large: bool,
}

impl ControlError {
    pub(crate) const fn invalid(code: ControlCode) -> Self {
        Self {
            code,
            too_large: false,
        }
    }

    pub(crate) const fn limit(code: ControlCode) -> Self {
        Self {
            code,
            too_large: true,
        }
    }
}

pub(crate) async fn parse_query_spec<T: DeserializeOwned>(
    query: Option<&str>,
) -> Result<Option<T>, ControlError> {
    let Some(query) = query else {
        return Ok(None);
    };
    let mut pairs = query.split('&');
    let Some(pair) = pairs.next() else {
        return Err(ControlError::invalid(ControlCode::InvalidRequest));
    };
    if pairs.next().is_some() {
        return Err(ControlError::invalid(ControlCode::InvalidRequest));
    }
    let Some(encoded) = pair.strip_prefix("spec=") else {
        return Err(ControlError::invalid(ControlCode::InvalidRequest));
    };
    if encoded.is_empty() || encoded.len() > QUERY_SPEC_LIMIT {
        return Err(if encoded.len() > QUERY_SPEC_LIMIT {
            ControlError::limit(ControlCode::LimitExceeded)
        } else {
            ControlError::invalid(ControlCode::InvalidRequest)
        });
    }
    let decoded = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| ControlError::invalid(ControlCode::InvalidRequest))?;
    if decoded.len() > QUERY_SPEC_LIMIT {
        return Err(ControlError::limit(ControlCode::LimitExceeded));
    }
    serde_json::from_slice(&decoded)
        .map(Some)
        .map_err(|_| ControlError::invalid(ControlCode::InvalidRequest))
}

pub(crate) async fn read_bounded_body(
    mut body: Incoming,
    declared_length: Option<u64>,
    limit: usize,
    overflow_code: ControlCode,
) -> Result<Bytes, ControlError> {
    if declared_length.is_some_and(|length| length > limit as u64) {
        return Err(ControlError::limit(overflow_code));
    }
    let mut result = BytesMut::new();
    while let Some(frame) = body.frame().await {
        let frame = frame.map_err(|_| ControlError::invalid(ControlCode::InvalidRequest))?;
        let Ok(data) = frame.into_data() else {
            continue;
        };
        let next = result
            .len()
            .checked_add(data.len())
            .ok_or_else(|| ControlError::limit(overflow_code))?;
        if next > limit {
            return Err(ControlError::limit(overflow_code));
        }
        result.extend_from_slice(&data);
    }
    Ok(result.freeze())
}

pub(crate) async fn discard_bounded_body(
    mut body: Incoming,
    declared_length: Option<u64>,
    limit: usize,
    overflow_code: ControlCode,
) -> Result<(), ControlError> {
    if declared_length.is_some_and(|length| length > limit as u64) {
        return Err(ControlError::limit(overflow_code));
    }
    let mut observed = 0usize;
    while let Some(frame) = body.frame().await {
        let frame = frame.map_err(|_| ControlError::invalid(ControlCode::InvalidRequest))?;
        let Ok(data) = frame.into_data() else {
            continue;
        };
        observed = observed
            .checked_add(data.len())
            .ok_or_else(|| ControlError::limit(overflow_code))?;
        if observed > limit {
            return Err(ControlError::limit(overflow_code));
        }
    }
    Ok(())
}

pub(crate) async fn parse_post_spec<T: DeserializeOwned>(
    body: Incoming,
    declared_length: Option<u64>,
) -> Result<T, ControlError> {
    let bytes = read_bounded_body(
        body,
        declared_length,
        POST_CONTROL_LIMIT,
        ControlCode::LimitExceeded,
    )
    .await?;
    serde_json::from_slice(&bytes).map_err(|_| ControlError::invalid(ControlCode::InvalidRequest))
}

pub(crate) fn declared_content_length(headers: &HeaderMap) -> Option<u64> {
    let mut values = headers.get_all(CONTENT_LENGTH).iter();
    let first = values.next()?.to_str().ok()?.parse().ok()?;
    for value in values {
        if value.to_str().ok()?.parse::<u64>().ok()? != first {
            return None;
        }
    }
    Some(first)
}

pub(crate) struct ValidatedHeaders {
    pub(crate) values: HeaderMap,
    pub(crate) has_content_encoding: bool,
}

pub(crate) fn validate_headers(headers: &[HeaderSpec]) -> Result<ValidatedHeaders, ControlError> {
    if headers.len() > MAX_HEADERS {
        return Err(ControlError::limit(ControlCode::LimitExceeded));
    }
    let mut values = HeaderMap::new();
    let mut has_content_encoding = false;
    for header in headers {
        let combined = header
            .name
            .len()
            .checked_add(header.value.len())
            .ok_or_else(|| ControlError::limit(ControlCode::LimitExceeded))?;
        if combined > MAX_HEADER_BYTES {
            return Err(ControlError::limit(ControlCode::LimitExceeded));
        }
        let name = HeaderName::from_bytes(header.name.as_bytes())
            .map_err(|_| ControlError::invalid(ControlCode::InvalidHeader))?;
        if name == HeaderName::from_static("keep-alive")
            || [
                CONTENT_LENGTH,
                TRANSFER_ENCODING,
                CONNECTION,
                UPGRADE,
                TRAILER,
            ]
            .contains(&name)
        {
            return Err(ControlError::invalid(ControlCode::RestrictedHeader));
        }
        let value = HeaderValue::from_str(&header.value)
            .map_err(|_| ControlError::invalid(ControlCode::InvalidHeader))?;
        has_content_encoding |= name == CONTENT_ENCODING;
        values.append(name, value);
    }
    Ok(ValidatedHeaders {
        values,
        has_content_encoding,
    })
}

pub(crate) fn validate_abort(spec: &AbortSpec) -> Result<(), ControlError> {
    if let AbortSpec::MidBody {
        bytes_before_abort,
        chunk_size_bytes,
        delay_between_chunks_ms,
    } = spec
    {
        if !(1..=MAX_CHUNK_BYTES).contains(bytes_before_abort)
            || !(1..=MAX_CHUNK_BYTES).contains(chunk_size_bytes)
        {
            return Err(ControlError::limit(ControlCode::LimitExceeded));
        }
        let chunks = u64::from(*bytes_before_abort).div_ceil(u64::from(*chunk_size_bytes));
        validate_delays(*delay_between_chunks_ms, chunks)?;
    }
    Ok(())
}

pub(crate) fn validate_delays(delay_ms: u32, chunks: u64) -> Result<(), ControlError> {
    if delay_ms > MAX_CHUNK_DELAY_MS || chunks > MAX_CHUNKS {
        return Err(ControlError::limit(ControlCode::LimitExceeded));
    }
    let delay_count = chunks.saturating_sub(1);
    if delay_count.saturating_mul(u64::from(delay_ms)) > MAX_STREAM_DELAY_MS {
        return Err(ControlError::limit(ControlCode::LimitExceeded));
    }
    Ok(())
}

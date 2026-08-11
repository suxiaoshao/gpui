use std::{
    error::Error,
    fmt, io,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use async_compression::tokio::bufread::{BrotliDecoder, GzipDecoder, ZlibDecoder, ZstdDecoder};
use bytes::Bytes;
use encoding_rs::{DecoderResult, Encoding, UTF_8};
use futures_util::{Stream, StreamExt as _};
use http::{HeaderMap, HeaderValue, header};
use mime::Mime;
use tokio::io::{AsyncRead, AsyncReadExt as _, BufReader};
use tokio_util::io::StreamReader;

use super::super::runtime::{BodySizeDimension, RequestProblem};
use super::{
    collector::BodyCollector,
    data::{BodyDecoding, CAPTURE_LIMIT_BYTES, CompletedBody, ResponseProgress, ResponseSizes},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ContentEncoding {
    Identity,
    Gzip,
    Brotli,
    Deflate,
    Zstd,
}

pub(crate) struct ContentDecoder {
    encodings: Vec<ContentEncoding>,
    body_decoding: BodyDecoding,
}

impl ContentDecoder {
    pub(crate) fn from_headers(headers: &HeaderMap) -> Self {
        let Some(encodings) = parse_content_encodings(headers) else {
            return Self {
                encodings: Vec::new(),
                body_decoding: BodyDecoding::Unsupported,
            };
        };
        let body_decoding = if encodings
            .iter()
            .any(|encoding| *encoding != ContentEncoding::Identity)
        {
            BodyDecoding::Decoded
        } else {
            BodyDecoding::Identity
        };
        Self {
            encodings,
            body_decoding,
        }
    }

    pub(crate) const fn body_decoding(&self) -> BodyDecoding {
        self.body_decoding
    }

    fn wrap<R>(&self, source: R) -> Pin<Box<dyn AsyncRead + Send>>
    where
        R: AsyncRead + Send + 'static,
    {
        let mut reader: Pin<Box<dyn AsyncRead + Send>> = Box::pin(source);
        if self.body_decoding != BodyDecoding::Decoded {
            return reader;
        }

        for encoding in self.encodings.iter().rev() {
            reader = match encoding {
                ContentEncoding::Identity => reader,
                ContentEncoding::Gzip => {
                    let mut decoder = GzipDecoder::new(BufReader::new(reader));
                    decoder.multiple_members(true);
                    Box::pin(decoder)
                }
                ContentEncoding::Brotli => {
                    let mut decoder = BrotliDecoder::new(BufReader::new(reader));
                    decoder.multiple_members(true);
                    Box::pin(decoder)
                }
                ContentEncoding::Deflate => {
                    let mut decoder = ZlibDecoder::new(BufReader::new(reader));
                    decoder.multiple_members(true);
                    Box::pin(decoder)
                }
                ContentEncoding::Zstd => {
                    let mut decoder = ZstdDecoder::new(BufReader::new(reader));
                    decoder.multiple_members(true);
                    Box::pin(decoder)
                }
            };
        }
        reader
    }
}

impl fmt::Debug for ContentDecoder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ContentDecoder")
            .field("encodings", &self.encodings)
            .field("body_decoding", &self.body_decoding)
            .finish()
    }
}

pub(crate) fn declared_encoded_bytes(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse().ok())
}

pub(crate) async fn collect_response_body<S, E, F>(
    headers: &HeaderMap,
    stream: S,
    mut on_progress: F,
) -> Result<CompletedBody, RequestProblem>
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: Error + Send + Sync + 'static,
    F: FnMut(ResponseProgress) + Send,
{
    let declared_encoded_bytes = declared_encoded_bytes(headers);
    if let Some(observed) = declared_encoded_bytes.filter(|length| *length > CAPTURE_LIMIT_BYTES) {
        return Err(RequestProblem::too_large(
            BodySizeDimension::Encoded,
            CAPTURE_LIMIT_BYTES,
            observed,
        ));
    }

    let decoder = ContentDecoder::from_headers(headers);
    let received_encoded_bytes = Arc::new(AtomicU64::new(0));
    let source_counter = Arc::clone(&received_encoded_bytes);
    let source = stream.map(move |result| match result {
        Ok(bytes) => {
            let current = source_counter.load(Ordering::Relaxed);
            match checked_encoded_len(current, bytes.len()) {
                Ok(next) => {
                    source_counter.store(next, Ordering::Relaxed);
                    Ok(bytes)
                }
                Err(failure) => Err(io::Error::other(failure)),
            }
        }
        Err(error) => Err(io::Error::other(ResponseStreamFailure::read(error))),
    });
    let source = StreamReader::new(Box::pin(source));
    let mut reader = decoder.wrap(source);
    let mut collector = BodyCollector::new();
    let mut buffer = vec![0_u8; 64 * 1024];

    loop {
        let read = match reader.read(&mut buffer).await {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) => return Err(map_pipeline_read_problem(error, decoder.body_decoding())),
        };
        collector.write(&buffer[..read]).await?;
        on_progress(ResponseProgress {
            declared_encoded_bytes,
            received_encoded_bytes: received_encoded_bytes.load(Ordering::Relaxed),
            stored_body_bytes: collector.len(),
            storage: collector.storage(),
        });
    }

    let received_encoded_bytes = received_encoded_bytes.load(Ordering::Relaxed);
    let stored_body_bytes = collector.len();
    let storage = collector.storage();
    let body = collector.finish().await?;
    debug_assert_eq!(body.len(), stored_body_bytes);
    on_progress(ResponseProgress {
        declared_encoded_bytes,
        received_encoded_bytes,
        stored_body_bytes,
        storage,
    });
    Ok(CompletedBody {
        body,
        body_decoding: decoder.body_decoding(),
        sizes: ResponseSizes {
            declared_encoded_bytes,
            received_encoded_bytes,
            stored_body_bytes,
        },
    })
}

fn parse_content_encodings(headers: &HeaderMap) -> Option<Vec<ContentEncoding>> {
    let mut encodings = Vec::new();
    for value in headers.get_all(header::CONTENT_ENCODING) {
        let value = value.to_str().ok()?;
        for token in value.split(',') {
            let encoding = match token.trim().to_ascii_lowercase().as_str() {
                "identity" => ContentEncoding::Identity,
                "gzip" => ContentEncoding::Gzip,
                "br" => ContentEncoding::Brotli,
                "deflate" => ContentEncoding::Deflate,
                "zstd" => ContentEncoding::Zstd,
                _ => return None,
            };
            encodings.push(encoding);
        }
    }
    if encodings.is_empty() {
        encodings.push(ContentEncoding::Identity);
    }
    Some(encodings)
}

fn checked_encoded_len(current: u64, incoming: usize) -> Result<u64, ResponseStreamFailure> {
    let observed = current.saturating_add(incoming as u64);
    if observed > CAPTURE_LIMIT_BYTES {
        Err(ResponseStreamFailure::encoded_too_large(observed))
    } else {
        Ok(observed)
    }
}

fn map_pipeline_read_problem(error: io::Error, body_decoding: BodyDecoding) -> RequestProblem {
    match stream_failure_kind(&error) {
        Some(ResponseStreamFailureKind::Read) => RequestProblem::response_body_read(error),
        Some(ResponseStreamFailureKind::EncodedTooLarge { observed }) => {
            RequestProblem::too_large(BodySizeDimension::Encoded, CAPTURE_LIMIT_BYTES, observed)
        }
        None if body_decoding == BodyDecoding::Decoded => {
            RequestProblem::response_body_decode(error)
        }
        None => RequestProblem::response_body_read(error),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ContentKind {
    Text(SourceLanguage),
    Json,
    Xml,
    Image,
    Audio,
    Video,
    Pdf,
    Bytes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceLanguage {
    Plain,
    Json,
    Xml,
    Html,
    Css,
    JavaScript,
    Yaml,
    Markdown,
    Svg,
}

impl SourceLanguage {
    const fn tag(self) -> &'static str {
        match self {
            Self::Plain => "text",
            Self::Json => "json",
            Self::Xml => "xml",
            Self::Html => "html",
            Self::Css => "css",
            Self::JavaScript => "javascript",
            Self::Yaml => "yaml",
            Self::Markdown => "markdown",
            Self::Svg => "svg",
        }
    }
}

pub(crate) fn classify_content_type(headers: &HeaderMap) -> ContentKind {
    let Some(media_type) = response_media_type(headers) else {
        return ContentKind::Bytes;
    };
    let essence = media_type.essence_str();
    let suffix = media_type.suffix().map(|suffix| suffix.as_str());

    if essence == "application/json" || suffix == Some("json") {
        return ContentKind::Json;
    }
    if essence == "image/svg+xml" {
        return ContentKind::Text(SourceLanguage::Svg);
    }
    if matches!(essence, "application/xml" | "text/xml") || suffix == Some("xml") {
        return ContentKind::Xml;
    }
    if matches!(
        essence,
        "image/png" | "image/jpeg" | "image/gif" | "image/webp"
    ) {
        return ContentKind::Image;
    }
    if essence == "application/pdf" {
        return ContentKind::Pdf;
    }
    if media_type.type_() == mime::AUDIO {
        return ContentKind::Audio;
    }
    if media_type.type_() == mime::VIDEO {
        return ContentKind::Video;
    }

    match essence {
        "text/html" => ContentKind::Text(SourceLanguage::Html),
        "text/css" => ContentKind::Text(SourceLanguage::Css),
        "text/javascript"
        | "application/javascript"
        | "application/ecmascript"
        | "application/x-javascript" => ContentKind::Text(SourceLanguage::JavaScript),
        "text/yaml" | "application/yaml" | "application/x-yaml" => {
            ContentKind::Text(SourceLanguage::Yaml)
        }
        "text/markdown" => ContentKind::Text(SourceLanguage::Markdown),
        _ if media_type.type_() == mime::TEXT => ContentKind::Text(SourceLanguage::Plain),
        _ => ContentKind::Bytes,
    }
}

pub(crate) fn decode_text(
    bytes: &[u8],
    headers: &HeaderMap,
    complete: bool,
) -> Result<String, TextDecodingProblem> {
    let explicit_encoding = response_media_type(headers)
        .and_then(|media_type| {
            media_type
                .get_param(mime::CHARSET)
                .map(|value| value.as_str().as_bytes().to_vec())
        })
        .map(|label| Encoding::for_label(&label).ok_or(TextDecodingProblem::UnknownCharset))
        .transpose()?;
    let (encoding, bom_len) = match explicit_encoding {
        Some(encoding) => (encoding, 0),
        None => Encoding::for_bom(bytes).unwrap_or((UTF_8, 0)),
    };
    let bytes = &bytes[bom_len..];
    let mut decoder = encoding.new_decoder_without_bom_handling();
    let capacity = decoder
        .max_utf8_buffer_length_without_replacement(bytes.len())
        .ok_or(TextDecodingProblem::InvalidBytes)?;
    let mut output = String::with_capacity(capacity);
    let (result, read) = decoder.decode_to_string_without_replacement(bytes, &mut output, complete);
    match result {
        DecoderResult::InputEmpty if read == bytes.len() => Ok(output),
        DecoderResult::Malformed(_, _) | DecoderResult::InputEmpty => {
            Err(TextDecodingProblem::InvalidBytes)
        }
        DecoderResult::OutputFull => {
            debug_assert!(false, "encoding_rs capacity estimate was too small");
            Err(TextDecodingProblem::InvalidBytes)
        }
    }
}

fn response_media_type(headers: &HeaderMap) -> Option<Mime> {
    headers
        .get(header::CONTENT_TYPE)?
        .to_str()
        .ok()?
        .parse()
        .ok()
}

pub(crate) fn fenced_source(source: &str, language: SourceLanguage) -> String {
    let backticks = longest_run(source.as_bytes(), b'`');
    let tildes = longest_run(source.as_bytes(), b'~');
    let (marker, len) = if backticks <= tildes {
        ('`', backticks.saturating_add(1).max(3))
    } else {
        ('~', tildes.saturating_add(1).max(3))
    };
    let fence: String = std::iter::repeat_n(marker, len).collect();
    format!("{fence}{}\n{source}\n{fence}", language.tag())
}

fn longest_run(bytes: &[u8], needle: u8) -> usize {
    let mut longest = 0;
    let mut current = 0;
    for byte in bytes {
        if *byte == needle {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }
    longest
}

pub(crate) fn escape_header_value(value: &HeaderValue) -> String {
    let mut escaped = String::with_capacity(value.as_bytes().len());
    for byte in value.as_bytes() {
        match byte {
            b' '..=b'~' if *byte != b'\\' => escaped.push(*byte as char),
            b'\\' => escaped.push_str("\\\\"),
            b'\t' => escaped.push_str("\\t"),
            _ => {
                use std::fmt::Write as _;
                write!(&mut escaped, "\\x{byte:02X}").expect("writing to String cannot fail");
            }
        }
    }
    escaped
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum TextDecodingProblem {
    UnknownCharset,
    InvalidBytes,
}

impl fmt::Display for TextDecodingProblem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnknownCharset => "response charset is unsupported",
            Self::InvalidBytes => "response text cannot be decoded without replacement",
        })
    }
}

impl fmt::Debug for TextDecodingProblem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl Error for TextDecodingProblem {}

fn stream_failure_kind(error: &(dyn Error + 'static)) -> Option<ResponseStreamFailureKind> {
    let mut source = Some(error);
    while let Some(error) = source {
        if let Some(failure) = error.downcast_ref::<ResponseStreamFailure>() {
            return Some(failure.kind);
        }
        source = error.source();
    }
    None
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResponseStreamFailureKind {
    Read,
    EncodedTooLarge { observed: u64 },
}

struct ResponseStreamFailure {
    kind: ResponseStreamFailureKind,
    source: Option<Box<dyn Error + Send + Sync>>,
}

impl ResponseStreamFailure {
    fn read(source: impl Error + Send + Sync + 'static) -> Self {
        Self {
            kind: ResponseStreamFailureKind::Read,
            source: Some(Box::new(source)),
        }
    }

    const fn encoded_too_large(observed: u64) -> Self {
        Self {
            kind: ResponseStreamFailureKind::EncodedTooLarge { observed },
            source: None,
        }
    }
}

impl fmt::Display for ResponseStreamFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.kind {
            ResponseStreamFailureKind::Read => "response stream read failed",
            ResponseStreamFailureKind::EncodedTooLarge { .. } => {
                "encoded response body exceeded its capture limit"
            }
        })
    }
}

impl fmt::Debug for ResponseStreamFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResponseStreamFailure")
            .field("kind", &self.kind)
            .field("source", &self.source.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

impl Error for ResponseStreamFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

#[cfg(test)]
mod tests {
    use async_compression::tokio::write::{BrotliEncoder, GzipEncoder, ZlibEncoder, ZstdEncoder};
    use futures_util::stream;
    use http::HeaderValue;
    use tokio::io::AsyncWriteExt as _;

    use super::*;
    use crate::features::request::{response::StoredBody, runtime::RequestProblemKind};

    #[derive(Debug)]
    struct TestReadProblem;

    impl fmt::Display for TestReadProblem {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("secret source")
        }
    }

    impl Error for TestReadProblem {}

    async fn gzip(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = GzipEncoder::new(Vec::new());
        encoder.write_all(bytes).await.unwrap();
        encoder.shutdown().await.unwrap();
        encoder.into_inner()
    }

    async fn brotli(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = BrotliEncoder::new(Vec::new());
        encoder.write_all(bytes).await.unwrap();
        encoder.shutdown().await.unwrap();
        encoder.into_inner()
    }

    async fn zlib(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = ZlibEncoder::new(Vec::new());
        encoder.write_all(bytes).await.unwrap();
        encoder.shutdown().await.unwrap();
        encoder.into_inner()
    }

    async fn zstd(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = ZstdEncoder::new(Vec::new());
        encoder.write_all(bytes).await.unwrap();
        encoder.shutdown().await.unwrap();
        encoder.into_inner()
    }

    async fn collect(headers: HeaderMap, bytes: Vec<u8>) -> CompletedBody {
        collect_response_body(
            &headers,
            stream::iter([Ok::<_, TestReadProblem>(Bytes::from(bytes))]),
            |_| {},
        )
        .await
        .unwrap()
    }

    fn memory(body: &CompletedBody) -> &[u8] {
        match &body.body {
            StoredBody::Memory(bytes) => bytes,
            body => panic!("unexpected body storage: {body:?}"),
        }
    }

    #[tokio::test]
    async fn supported_content_codings_decode_to_the_original_bytes() {
        let source = b"decoded response body";
        for (name, encoded) in [
            ("gzip", gzip(source).await),
            ("br", brotli(source).await),
            ("deflate", zlib(source).await),
            ("zstd", zstd(source).await),
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_ENCODING, HeaderValue::from_static(name));
            let body = collect(headers, encoded).await;
            assert_eq!(memory(&body), source);
            assert_eq!(body.body_decoding, BodyDecoding::Decoded);
        }
    }

    #[tokio::test]
    async fn coding_chain_is_decoded_in_reverse_application_order() {
        let source = b"layered response body";
        let encoded = brotli(&gzip(source).await).await;
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_ENCODING,
            HeaderValue::from_static("gzip, br"),
        );

        let body = collect(headers, encoded).await;
        assert_eq!(memory(&body), source);
        assert_eq!(body.body_decoding, BodyDecoding::Decoded);
    }

    #[tokio::test]
    async fn unknown_empty_or_non_utf8_coding_preserves_all_encoded_bytes() {
        for value in [
            HeaderValue::from_static("gzip, unknown"),
            HeaderValue::from_static("gzip, "),
            HeaderValue::from_bytes(b"\xff").unwrap(),
        ] {
            let encoded = gzip(b"body").await;
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_ENCODING, value);
            let body = collect(headers, encoded.clone()).await;
            assert_eq!(memory(&body), encoded);
            assert_eq!(body.body_decoding, BodyDecoding::Unsupported);
        }
    }

    #[tokio::test]
    async fn decoder_and_stream_failures_have_distinct_safe_kinds() {
        let mut gzip_headers = HeaderMap::new();
        gzip_headers.insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
        let decode_problem = collect_response_body(
            &gzip_headers,
            stream::iter([Ok::<_, TestReadProblem>(Bytes::from_static(b"not gzip"))]),
            |_| {},
        )
        .await
        .unwrap_err();
        assert_eq!(
            decode_problem.kind(),
            RequestProblemKind::ResponseBodyDecode
        );

        let read_problem = collect_response_body(
            &HeaderMap::new(),
            stream::iter([Err::<Bytes, _>(TestReadProblem)]),
            |_| {},
        )
        .await
        .unwrap_err();
        assert_eq!(read_problem.kind(), RequestProblemKind::ResponseBodyRead);
        let diagnostic = format!("{read_problem:?} {read_problem}");
        assert!(!diagnostic.contains("secret source"));
    }

    #[tokio::test]
    async fn declared_encoded_limit_fails_before_polling_the_stream() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_LENGTH,
            HeaderValue::from_str(&(CAPTURE_LIMIT_BYTES + 1).to_string()).unwrap(),
        );
        let problem = collect_response_body(
            &headers,
            stream::pending::<Result<Bytes, TestReadProblem>>(),
            |_| {},
        )
        .await
        .unwrap_err();
        assert_eq!(
            problem.kind(),
            RequestProblemKind::BodyTooLarge {
                dimension: BodySizeDimension::Encoded,
                limit: CAPTURE_LIMIT_BYTES,
                observed: CAPTURE_LIMIT_BYTES + 1,
            }
        );
    }

    #[test]
    fn encoded_limit_is_checked_without_wrapping() {
        assert_eq!(
            checked_encoded_len(CAPTURE_LIMIT_BYTES - 1, 1).unwrap(),
            CAPTURE_LIMIT_BYTES
        );
        assert_eq!(
            checked_encoded_len(CAPTURE_LIMIT_BYTES, 1)
                .unwrap_err()
                .kind,
            ResponseStreamFailureKind::EncodedTooLarge {
                observed: CAPTURE_LIMIT_BYTES + 1,
            }
        );
    }

    #[test]
    fn content_type_classification_handles_suffixes_and_never_treats_svg_as_image() {
        for (media_type, expected) in [
            ("application/problem+json", ContentKind::Json),
            ("application/atom+xml", ContentKind::Xml),
            ("text/html", ContentKind::Text(SourceLanguage::Html)),
            ("image/png", ContentKind::Image),
            ("image/svg+xml", ContentKind::Text(SourceLanguage::Svg)),
            ("application/pdf", ContentKind::Pdf),
            ("audio/ogg", ContentKind::Audio),
            ("video/mp4; codecs=avc1", ContentKind::Video),
            ("application/octet-stream", ContentKind::Bytes),
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_str(media_type).unwrap(),
            );
            assert_eq!(classify_content_type(&headers), expected);
        }
    }

    #[test]
    fn text_decode_honors_charset_bom_and_truncated_multibyte_tail() {
        let mut windows_headers = HeaderMap::new();
        windows_headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=windows-1252"),
        );
        assert_eq!(decode_text(b"\x80", &windows_headers, true).unwrap(), "€");

        assert_eq!(
            decode_text(b"\xEF\xBB\xBFbody", &HeaderMap::new(), true).unwrap(),
            "body"
        );
        assert_eq!(
            decode_text(&[0xE2, 0x82], &HeaderMap::new(), false).unwrap(),
            ""
        );
        assert_eq!(
            decode_text(&[0xE2, 0x82], &HeaderMap::new(), true).unwrap_err(),
            TextDecodingProblem::InvalidBytes
        );
    }

    #[test]
    fn unknown_charset_and_malformed_interior_bytes_are_rejected() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=does-not-exist"),
        );
        assert_eq!(
            decode_text(b"body", &headers, true).unwrap_err(),
            TextDecodingProblem::UnknownCharset
        );
        assert_eq!(
            decode_text(b"a\xFFb", &HeaderMap::new(), false).unwrap_err(),
            TextDecodingProblem::InvalidBytes
        );
    }

    #[test]
    fn fenced_source_cannot_be_closed_by_response_markup() {
        let source = "before\n```\n<script>secret()</script>\n~~~~\nafter";
        let fenced = fenced_source(source, SourceLanguage::Html);
        let opening = fenced.lines().next().unwrap();
        let marker = opening.strip_suffix("html").unwrap();
        assert!(marker.len() > longest_run(source.as_bytes(), marker.as_bytes()[0]));
        assert_eq!(fenced.lines().last(), Some(marker));
        assert!(fenced.contains(source));
    }

    #[test]
    fn header_escape_preserves_visible_ascii_and_escapes_opaque_bytes() {
        let value = HeaderValue::from_bytes(b"visible\\value\t\xFF").unwrap();
        assert_eq!(escape_header_value(&value), "visible\\\\value\\t\\xFF");
    }
}

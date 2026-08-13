use std::{
    error::Error,
    fmt,
    io::{BufReader, Cursor},
    sync::Arc,
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use gpui::RenderImage;
use image::{AnimationDecoder as _, ImageDecoder as _, ImageFormat, Limits};

use super::{
    BodyDecoding, ContentKind, INLINE_PREVIEW_BYTES, ResponseData, SourceLanguage,
    TextDecodingProblem, classify_content_type, decode_text,
};

const MAX_IMAGE_DIMENSION: u32 = 4096;
const MAX_IMAGE_FRAMES: usize = 16;
const MAX_IMAGE_RGBA_BYTES: u64 = 64 * 1024 * 1024;
const MAX_EDITOR_LINES: usize = 50_000;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub(crate) enum ViewerMode {
    #[default]
    Auto,
    Text,
    Json,
    Xml,
    Hex,
    Base64,
    Image,
    Audio,
    Video,
    Pdf,
}

impl ViewerMode {
    pub(crate) const ALL: [Self; 10] = [
        Self::Auto,
        Self::Text,
        Self::Json,
        Self::Xml,
        Self::Hex,
        Self::Base64,
        Self::Image,
        Self::Audio,
        Self::Video,
        Self::Pdf,
    ];
}

pub(crate) fn resolved_viewer_mode(
    response: &ResponseData,
    requested: ViewerMode,
) -> Option<ViewerMode> {
    if response.body_decoding() == BodyDecoding::Unsupported {
        return match requested {
            ViewerMode::Auto | ViewerMode::Hex => Some(ViewerMode::Hex),
            ViewerMode::Base64 => Some(ViewerMode::Base64),
            _ => None,
        };
    }
    if requested != ViewerMode::Auto {
        return Some(requested);
    }
    Some(match classify_content_type(&response.head().headers) {
        ContentKind::Text(_) => ViewerMode::Text,
        ContentKind::Json => ViewerMode::Json,
        ContentKind::Xml => ViewerMode::Xml,
        ContentKind::Image => ViewerMode::Image,
        ContentKind::Audio => ViewerMode::Audio,
        ContentKind::Video => ViewerMode::Video,
        ContentKind::Pdf => ViewerMode::Pdf,
        ContentKind::Bytes => ViewerMode::Hex,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResponseViewWarning {
    Truncated,
    UnsupportedDecoding,
    ModeUnavailable,
    InvalidJson,
    InvalidImage,
    ImageTooLarge,
}

pub(crate) enum ResponseProjection {
    Empty,
    Text {
        source: String,
        language: SourceLanguage,
        warning: Option<ResponseViewWarning>,
    },
    Image {
        image: Arc<RenderImage>,
        warning: Option<ResponseViewWarning>,
    },
    Unavailable(ResponseViewWarning),
}

impl ResponseProjection {
    pub(crate) const fn warning(&self) -> Option<ResponseViewWarning> {
        match self {
            Self::Empty => None,
            Self::Text { warning, .. } | Self::Image { warning, .. } => *warning,
            Self::Unavailable(warning) => Some(*warning),
        }
    }
}

impl fmt::Debug for ResponseProjection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("ResponseProjection::Empty"),
            Self::Text {
                source,
                language,
                warning,
            } => formatter
                .debug_struct("ResponseProjection::Text")
                .field("len", &source.len())
                .field("language", language)
                .field("warning", warning)
                .finish(),
            Self::Image { warning, .. } => formatter
                .debug_struct("ResponseProjection::Image")
                .field("warning", warning)
                .finish_non_exhaustive(),
            Self::Unavailable(warning) => formatter
                .debug_tuple("ResponseProjection::Unavailable")
                .field(warning)
                .finish(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResponseViewProblem {
    Read,
}

impl fmt::Display for ResponseViewProblem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("response preview could not be read")
    }
}

impl Error for ResponseViewProblem {}

pub(crate) async fn project_response(
    response: Arc<ResponseData>,
    requested: ViewerMode,
) -> Result<ResponseProjection, ResponseViewProblem> {
    if response.sizes().stored_body_bytes == 0 {
        return Ok(ResponseProjection::Empty);
    }
    let prefix = response
        .read_lease()
        .read_prefix(INLINE_PREVIEW_BYTES as usize)
        .await
        .map_err(|_| ResponseViewProblem::Read)?;
    let headers = &response.head().headers;
    let kind = classify_content_type(headers);
    let mode = effective_mode(requested, response.body_decoding(), kind);

    match mode {
        EffectiveMode::Text(language) => Ok(text_projection(
            &prefix.bytes,
            headers,
            prefix.complete,
            language,
            None,
        )),
        EffectiveMode::Json => Ok(json_projection(&prefix.bytes, headers, prefix.complete)),
        EffectiveMode::Xml => Ok(text_projection(
            &prefix.bytes,
            headers,
            prefix.complete,
            SourceLanguage::Xml,
            None,
        )),
        EffectiveMode::Hex => Ok(hex_projection(
            &prefix.bytes,
            prefix.complete,
            response.body_decoding(),
        )),
        EffectiveMode::Base64 => Ok(base64_projection(
            &prefix.bytes,
            prefix.complete,
            response.body_decoding(),
        )),
        EffectiveMode::Image if !prefix.complete => Ok(ResponseProjection::Unavailable(
            ResponseViewWarning::ImageTooLarge,
        )),
        EffectiveMode::Image => {
            let bytes = prefix.bytes.to_vec();
            let decoded = tokio::task::spawn_blocking(move || decode_image(&bytes))
                .await
                .map_err(|_| ResponseViewProblem::Read)?;
            Ok(match decoded {
                Ok(image) => ResponseProjection::Image {
                    image,
                    warning: None,
                },
                Err(ImageProjectionProblem::TooLarge) => {
                    ResponseProjection::Unavailable(ResponseViewWarning::ImageTooLarge)
                }
                Err(ImageProjectionProblem::Invalid) => {
                    ResponseProjection::Unavailable(ResponseViewWarning::InvalidImage)
                }
            })
        }
        EffectiveMode::Unavailable => Ok(ResponseProjection::Unavailable(
            ResponseViewWarning::ModeUnavailable,
        )),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EffectiveMode {
    Text(SourceLanguage),
    Json,
    Xml,
    Hex,
    Base64,
    Image,
    Unavailable,
}

fn effective_mode(
    requested: ViewerMode,
    decoding: BodyDecoding,
    kind: ContentKind,
) -> EffectiveMode {
    if decoding == BodyDecoding::Unsupported {
        return match requested {
            ViewerMode::Auto | ViewerMode::Hex => EffectiveMode::Hex,
            ViewerMode::Base64 => EffectiveMode::Base64,
            _ => EffectiveMode::Unavailable,
        };
    }
    match requested {
        ViewerMode::Auto => match kind {
            ContentKind::Text(language) => EffectiveMode::Text(language),
            ContentKind::Json => EffectiveMode::Json,
            ContentKind::Xml => EffectiveMode::Xml,
            ContentKind::Image => EffectiveMode::Image,
            ContentKind::Audio | ContentKind::Video | ContentKind::Pdf => {
                EffectiveMode::Unavailable
            }
            ContentKind::Bytes => EffectiveMode::Hex,
        },
        ViewerMode::Text => EffectiveMode::Text(match kind {
            ContentKind::Text(language) => language,
            ContentKind::Json => SourceLanguage::Json,
            ContentKind::Xml => SourceLanguage::Xml,
            ContentKind::Image
            | ContentKind::Audio
            | ContentKind::Video
            | ContentKind::Pdf
            | ContentKind::Bytes => SourceLanguage::Plain,
        }),
        ViewerMode::Json => EffectiveMode::Json,
        ViewerMode::Xml => EffectiveMode::Xml,
        ViewerMode::Hex => EffectiveMode::Hex,
        ViewerMode::Base64 => EffectiveMode::Base64,
        ViewerMode::Image => EffectiveMode::Image,
        ViewerMode::Audio | ViewerMode::Video | ViewerMode::Pdf => EffectiveMode::Unavailable,
    }
}

fn text_projection(
    bytes: &[u8],
    headers: &http::HeaderMap,
    complete: bool,
    language: SourceLanguage,
    preferred_warning: Option<ResponseViewWarning>,
) -> ResponseProjection {
    match decode_text(bytes, headers, complete) {
        Ok(source) => {
            let (source, output_truncated) = bounded_source(source);
            ResponseProjection::Text {
                source,
                language,
                warning: preferred_warning
                    .or((!complete || output_truncated).then_some(ResponseViewWarning::Truncated)),
            }
        }
        Err(TextDecodingProblem::UnknownCharset | TextDecodingProblem::InvalidBytes) => {
            ResponseProjection::Unavailable(ResponseViewWarning::ModeUnavailable)
        }
    }
}

fn json_projection(bytes: &[u8], headers: &http::HeaderMap, complete: bool) -> ResponseProjection {
    if !complete {
        return text_projection(
            bytes,
            headers,
            false,
            SourceLanguage::Json,
            Some(ResponseViewWarning::Truncated),
        );
    }
    let Ok(source) = decode_text(bytes, headers, true) else {
        return ResponseProjection::Unavailable(ResponseViewWarning::ModeUnavailable);
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&source) else {
        return text_projection(
            bytes,
            headers,
            true,
            SourceLanguage::Json,
            Some(ResponseViewWarning::InvalidJson),
        );
    };
    let Ok(pretty) = serde_json::to_string_pretty(&value) else {
        return ResponseProjection::Unavailable(ResponseViewWarning::InvalidJson);
    };
    let (source, truncated) = bounded_source(pretty);
    if truncated {
        text_projection(
            bytes,
            headers,
            true,
            SourceLanguage::Json,
            Some(ResponseViewWarning::Truncated),
        )
    } else {
        ResponseProjection::Text {
            source,
            language: SourceLanguage::Json,
            warning: None,
        }
    }
}

fn bounded_source(mut source: String) -> (String, bool) {
    let limit = INLINE_PREVIEW_BYTES as usize;
    let original_len = source.len();
    if source.len() > limit {
        let boundary = floor_char_boundary(&source, limit);
        source.truncate(boundary);
    }
    if let Some((boundary, _)) = source.match_indices('\n').nth(MAX_EDITOR_LINES - 1) {
        source.truncate(boundary);
    }
    let truncated = source.len() != original_len;
    (source, truncated)
}

fn floor_char_boundary(source: &str, mut index: usize) -> usize {
    index = index.min(source.len());
    while index > 0 && !source.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn hex_projection(bytes: &[u8], complete: bool, decoding: BodyDecoding) -> ResponseProjection {
    let mut output = String::with_capacity((bytes.len() * 3).min(INLINE_PREVIEW_BYTES as usize));
    let mut consumed = 0;
    for byte in bytes {
        let separator = if consumed > 0 && consumed % 16 == 0 {
            '\n'
        } else if consumed > 0 {
            ' '
        } else {
            '\0'
        };
        let needed = 2 + usize::from(separator != '\0');
        if output.len() + needed > INLINE_PREVIEW_BYTES as usize {
            break;
        }
        if separator != '\0' {
            output.push(separator);
        }
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02X}").expect("writing to String cannot fail");
        consumed += 1;
    }
    let warning = if decoding == BodyDecoding::Unsupported {
        Some(ResponseViewWarning::UnsupportedDecoding)
    } else if !complete || consumed < bytes.len() {
        Some(ResponseViewWarning::Truncated)
    } else {
        None
    };
    let (source, _) = bounded_source(output);
    ResponseProjection::Text {
        source,
        language: SourceLanguage::Plain,
        warning,
    }
}

fn base64_projection(bytes: &[u8], complete: bool, decoding: BodyDecoding) -> ResponseProjection {
    let max_input = (INLINE_PREVIEW_BYTES as usize / 4) * 3;
    let selected = &bytes[..bytes.len().min(max_input)];
    let encoded = STANDARD.encode(selected);
    debug_assert!(encoded.len() <= INLINE_PREVIEW_BYTES as usize);
    let warning = if decoding == BodyDecoding::Unsupported {
        Some(ResponseViewWarning::UnsupportedDecoding)
    } else if !complete || selected.len() < bytes.len() {
        Some(ResponseViewWarning::Truncated)
    } else {
        None
    };
    let (source, _) = bounded_source(encoded);
    ResponseProjection::Text {
        source,
        language: SourceLanguage::Plain,
        warning,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ImageProjectionProblem {
    Invalid,
    TooLarge,
}

fn decode_image(bytes: &[u8]) -> Result<Arc<RenderImage>, ImageProjectionProblem> {
    let format = image::guess_format(bytes).map_err(|_| ImageProjectionProblem::Invalid)?;
    if !matches!(
        format,
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::Gif | ImageFormat::WebP
    ) {
        return Err(ImageProjectionProblem::Invalid);
    }
    let frames = match format {
        ImageFormat::Gif => {
            let mut decoder =
                image::codecs::gif::GifDecoder::new(BufReader::new(Cursor::new(bytes)))
                    .map_err(|_| ImageProjectionProblem::Invalid)?;
            decoder
                .set_limits(image_limits())
                .map_err(|_| ImageProjectionProblem::TooLarge)?;
            decode_animation(decoder.dimensions(), decoder.into_frames())?
        }
        ImageFormat::WebP => {
            let mut decoder =
                image::codecs::webp::WebPDecoder::new(BufReader::new(Cursor::new(bytes)))
                    .map_err(|_| ImageProjectionProblem::Invalid)?;
            decoder
                .set_limits(image_limits())
                .map_err(|_| ImageProjectionProblem::TooLarge)?;
            decode_animation(decoder.dimensions(), decoder.into_frames())?
        }
        ImageFormat::Png | ImageFormat::Jpeg => {
            let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
            reader.limits(image_limits());
            let mut buffer = reader
                .decode()
                .map_err(map_image_decode_problem)?
                .into_rgba8();
            reserve_frame_budget(buffer.width(), buffer.height(), 0)?;
            rgba_to_bgra(&mut buffer);
            vec![image::Frame::new(buffer)]
        }
        _ => unreachable!("unsupported image formats were rejected above"),
    };
    if frames.is_empty() {
        return Err(ImageProjectionProblem::Invalid);
    }
    Ok(Arc::new(RenderImage::new(frames)))
}

fn image_limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_IMAGE_RGBA_BYTES);
    limits
}

fn decode_animation<'a>(
    dimensions: (u32, u32),
    mut frames: image::Frames<'a>,
) -> Result<Vec<image::Frame>, ImageProjectionProblem> {
    let mut decoded = Vec::new();
    let mut used = 0_u64;
    loop {
        if decoded.len() == MAX_IMAGE_FRAMES {
            return if frames.next().is_some() {
                Err(ImageProjectionProblem::TooLarge)
            } else {
                Ok(decoded)
            };
        }
        let reserved = reserve_frame_budget(dimensions.0, dimensions.1, used)?;
        let Some(frame) = frames.next() else {
            break;
        };
        let frame = frame.map_err(map_image_decode_problem)?;
        used = reserved;
        let left = frame.left();
        let top = frame.top();
        let delay = frame.delay();
        let mut buffer = frame.into_buffer();
        rgba_to_bgra(&mut buffer);
        decoded.push(image::Frame::from_parts(buffer, left, top, delay));
    }
    Ok(decoded)
}

fn reserve_frame_budget(width: u32, height: u32, used: u64) -> Result<u64, ImageProjectionProblem> {
    if width > MAX_IMAGE_DIMENSION || height > MAX_IMAGE_DIMENSION {
        return Err(ImageProjectionProblem::TooLarge);
    }
    let bytes = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(ImageProjectionProblem::TooLarge)?;
    let reserved = used
        .checked_add(bytes)
        .ok_or(ImageProjectionProblem::TooLarge)?;
    if reserved > MAX_IMAGE_RGBA_BYTES {
        Err(ImageProjectionProblem::TooLarge)
    } else {
        Ok(reserved)
    }
}

fn rgba_to_bgra(buffer: &mut image::RgbaImage) {
    for pixel in buffer.pixels_mut() {
        pixel.0.swap(0, 2);
    }
}

fn map_image_decode_problem(error: image::ImageError) -> ImageProjectionProblem {
    match error {
        image::ImageError::Limits(_) => ImageProjectionProblem::TooLarge,
        _ => ImageProjectionProblem::Invalid,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bytes::Bytes;
    use http::{HeaderMap, HeaderValue, StatusCode, Version, header};
    use url::Url;

    use super::*;
    use crate::features::request::response::{
        CompletedBody, ResponseHead, ResponseSizes, ResponseTiming, StoredBody,
    };

    fn response(
        bytes: &'static [u8],
        content_type: &str,
        body_decoding: BodyDecoding,
    ) -> Arc<ResponseData> {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_str(content_type).unwrap(),
        );
        Arc::new(ResponseData::new(
            ResponseHead::new(
                StatusCode::OK,
                Version::HTTP_11,
                Url::parse("https://example.test/private").unwrap(),
                headers,
            ),
            ResponseTiming {
                head_after: Duration::ZERO,
                completed_after: Duration::ZERO,
            },
            CompletedBody {
                body: StoredBody::Memory(Bytes::from_static(bytes)),
                body_decoding,
                sizes: ResponseSizes {
                    declared_encoded_bytes: Some(bytes.len() as u64),
                    received_encoded_bytes: bytes.len() as u64,
                    stored_body_bytes: bytes.len() as u64,
                },
            },
        ))
    }

    #[tokio::test]
    async fn auto_json_is_pretty_and_response_markup_remains_plain_editor_source() {
        let source = br#"{"html":"```\\n<img src='https://tracker.test'>"}"#;
        let projection = project_response(
            response(source, "application/problem+json", BodyDecoding::Identity),
            ViewerMode::Auto,
        )
        .await
        .unwrap();
        let ResponseProjection::Text {
            source,
            language,
            warning,
        } = projection
        else {
            panic!("JSON did not produce text")
        };
        assert_eq!(warning, None);
        assert_eq!(language, SourceLanguage::Json);
        assert!(source.contains("tracker.test"));
        assert!(source.starts_with("{\n"));
        assert!(!source.starts_with("```"));
    }

    #[tokio::test]
    async fn svg_is_always_plain_editor_source_and_never_an_image() {
        let projection = project_response(
            response(
                b"<svg><image href='https://tracker.test'/></svg>",
                "image/svg+xml",
                BodyDecoding::Identity,
            ),
            ViewerMode::Auto,
        )
        .await
        .unwrap();
        assert!(matches!(
            projection,
            ResponseProjection::Text {
                source,
                language: SourceLanguage::Svg,
                ..
            } if source == "<svg><image href='https://tracker.test'/></svg>"
        ));
        assert!(matches!(
            project_response(
                response(
                    b"<svg><image href='https://tracker.test'/></svg>",
                    "image/svg+xml",
                    BodyDecoding::Identity,
                ),
                ViewerMode::Image,
            )
            .await
            .unwrap(),
            ResponseProjection::Unavailable(ResponseViewWarning::InvalidImage)
        ));
    }

    #[tokio::test]
    async fn bounded_png_is_decoded_to_a_render_image() {
        let mut encoded = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            2,
            1,
            image::Rgba([1, 2, 3, 255]),
        ))
        .write_to(&mut encoded, ImageFormat::Png)
        .unwrap();
        let bytes = Box::leak(encoded.into_inner().into_boxed_slice());
        let projection = project_response(
            response(bytes, "image/png", BodyDecoding::Identity),
            ViewerMode::Auto,
        )
        .await
        .unwrap();
        let ResponseProjection::Image { image, warning } = projection else {
            panic!("PNG did not produce an image projection")
        };
        assert_eq!(warning, None);
        assert_eq!(image.size(0).width.0, 2);
        assert_eq!(image.size(0).height.0, 1);
    }

    #[test]
    fn animated_image_accepts_sixteen_frames_and_rejects_the_seventeenth() {
        let gif = |frame_count| {
            let mut encoded = Vec::new();
            let mut encoder = image::codecs::gif::GifEncoder::new(&mut encoded);
            encoder
                .encode_frames((0..frame_count).map(|_| {
                    image::Frame::new(image::RgbaImage::from_pixel(
                        1,
                        1,
                        image::Rgba([1, 2, 3, 255]),
                    ))
                }))
                .unwrap();
            drop(encoder);
            encoded
        };

        let sixteen = decode_image(&gif(MAX_IMAGE_FRAMES)).unwrap();
        assert_eq!(sixteen.frame_count(), MAX_IMAGE_FRAMES);
        assert_eq!(
            decode_image(&gif(MAX_IMAGE_FRAMES + 1)).unwrap_err(),
            ImageProjectionProblem::TooLarge
        );
    }

    #[tokio::test]
    async fn unsupported_content_coding_only_allows_bounded_byte_views() {
        let response = response(b"encoded bytes", "text/plain", BodyDecoding::Unsupported);
        assert!(matches!(
            project_response(response.clone(), ViewerMode::Auto)
                .await
                .unwrap(),
            ResponseProjection::Text {
                warning: Some(ResponseViewWarning::UnsupportedDecoding),
                ..
            }
        ));
        assert!(matches!(
            project_response(response, ViewerMode::Text).await.unwrap(),
            ResponseProjection::Unavailable(ResponseViewWarning::ModeUnavailable)
        ));
    }

    #[test]
    fn auto_resolves_media_and_pdf_from_content_type_without_sniffing() {
        for (content_type, expected) in [
            ("audio/mpeg", ViewerMode::Audio),
            ("video/webm", ViewerMode::Video),
            ("application/pdf", ViewerMode::Pdf),
            ("application/octet-stream", ViewerMode::Hex),
        ] {
            let response = response(b"opaque", content_type, BodyDecoding::Identity);
            assert_eq!(
                resolved_viewer_mode(&response, ViewerMode::Auto),
                Some(expected)
            );
        }

        let unknown = response(
            b"opaque",
            "application/octet-stream",
            BodyDecoding::Identity,
        );
        assert_eq!(
            resolved_viewer_mode(&unknown, ViewerMode::Audio),
            Some(ViewerMode::Audio)
        );
        assert_eq!(
            resolved_viewer_mode(&unknown, ViewerMode::Pdf),
            Some(ViewerMode::Pdf)
        );
    }

    #[test]
    fn unsupported_content_coding_disables_parser_and_player_modes() {
        let response = response(b"encoded", "video/mp4", BodyDecoding::Unsupported);
        for mode in [
            ViewerMode::Text,
            ViewerMode::Json,
            ViewerMode::Xml,
            ViewerMode::Image,
            ViewerMode::Audio,
            ViewerMode::Video,
            ViewerMode::Pdf,
        ] {
            assert_eq!(resolved_viewer_mode(&response, mode), None);
        }
        assert_eq!(
            resolved_viewer_mode(&response, ViewerMode::Auto),
            Some(ViewerMode::Hex)
        );
        assert_eq!(
            resolved_viewer_mode(&response, ViewerMode::Base64),
            Some(ViewerMode::Base64)
        );
    }

    #[test]
    fn output_builders_and_image_budgets_never_exceed_limits() {
        let source = "é".repeat(INLINE_PREVIEW_BYTES as usize);
        let (bounded, truncated) = bounded_source(source);
        assert!(truncated);
        assert!(bounded.len() <= INLINE_PREVIEW_BYTES as usize);
        assert!(bounded.is_char_boundary(bounded.len()));

        let source = "line\n".repeat(MAX_EDITOR_LINES + 1);
        let (bounded, truncated) = bounded_source(source);
        assert!(truncated);
        assert_eq!(bounded.lines().count(), MAX_EDITOR_LINES);

        assert_eq!(
            reserve_frame_budget(MAX_IMAGE_DIMENSION, MAX_IMAGE_DIMENSION, 0).unwrap(),
            MAX_IMAGE_RGBA_BYTES
        );
        assert_eq!(
            reserve_frame_budget(MAX_IMAGE_DIMENSION, MAX_IMAGE_DIMENSION, 1),
            Err(ImageProjectionProblem::TooLarge)
        );
        assert_eq!(
            reserve_frame_budget(u32::MAX, u32::MAX, u64::MAX),
            Err(ImageProjectionProblem::TooLarge)
        );
    }
}

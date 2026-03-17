use crate::{
    capture::{ImageFrame, ScreenRect},
    error::OcrError,
};
#[cfg(target_os = "macos")]
use objc2::rc::Retained;
#[cfg(target_os = "macos")]
use objc2::runtime::{AnyObject, NSObject};
#[cfg(target_os = "macos")]
use objc2::{extern_class, extern_methods, msg_send, AnyThread, ClassType};
#[cfg(target_os = "macos")]
use objc2_core_foundation::{CFData, CGRect, CGFloat};
#[cfg(target_os = "macos")]
use objc2_core_graphics::{
    CGBitmapInfo, CGColorRenderingIntent, CGColorSpace, CGDataProvider, CGImage,
    CGImageAlphaInfo, CGImageByteOrderInfo,
};
#[cfg(target_os = "macos")]
use objc2_foundation::{NSArray, NSDictionary, NSError, NSRange, NSString, NSUInteger};

#[cfg(target_os = "macos")]
#[link(name = "Vision", kind = "framework")]
unsafe extern "C" {}

#[cfg(target_os = "macos")]
type VisionOptions = NSDictionary<NSString, AnyObject>;

#[cfg(target_os = "macos")]
extern_class!(
    #[unsafe(super(NSObject))]
    #[derive(PartialEq, Eq, Hash)]
    struct VNRequest;
);

#[cfg(target_os = "macos")]
extern_class!(
    #[unsafe(super(VNRequest))]
    #[derive(PartialEq, Eq, Hash)]
    struct VNImageBasedRequest;
);

#[cfg(target_os = "macos")]
extern_class!(
    #[unsafe(super(VNImageBasedRequest))]
    #[derive(PartialEq, Eq, Hash)]
    struct VNRecognizeTextRequest;
);

#[cfg(target_os = "macos")]
extern_class!(
    #[unsafe(super(NSObject))]
    #[derive(PartialEq, Eq, Hash)]
    struct VNImageRequestHandler;
);

#[cfg(target_os = "macos")]
extern_class!(
    #[unsafe(super(NSObject))]
    #[derive(PartialEq, Eq, Hash)]
    struct VNRectangleObservation;
);

#[cfg(target_os = "macos")]
extern_class!(
    #[unsafe(super(VNRectangleObservation))]
    #[derive(PartialEq, Eq, Hash)]
    struct VNRecognizedTextObservation;
);

#[cfg(target_os = "macos")]
extern_class!(
    #[unsafe(super(NSObject))]
    #[derive(PartialEq, Eq, Hash)]
    struct VNRecognizedText;
);

#[cfg(target_os = "macos")]
#[allow(non_snake_case)]
impl VNRecognizeTextRequest {
    extern_methods!(
        #[unsafe(method(setRecognitionLanguages:))]
        fn setRecognitionLanguages(&self, languages: &NSArray<NSString>);

        #[unsafe(method(setUsesLanguageCorrection:))]
        fn setUsesLanguageCorrection(&self, enabled: bool);

        #[unsafe(method(results))]
        fn results(&self) -> Option<Retained<NSArray<VNRecognizedTextObservation>>>;
    );
}

#[cfg(target_os = "macos")]
#[allow(non_snake_case)]
impl VNImageRequestHandler {
    extern_methods!(
        #[unsafe(method(initWithCGImage:options:))]
        #[unsafe(method_family = init)]
        unsafe fn initWithCGImage_options(
            this: objc2::rc::Allocated<Self>,
            image: &CGImage,
            options: &VisionOptions,
        ) -> Retained<Self>;
    );
}

#[cfg(target_os = "macos")]
#[allow(non_snake_case)]
impl VNRecognizedTextObservation {
    extern_methods!(
        #[unsafe(method(topCandidates:))]
        fn topCandidates(&self, max_candidate_count: NSUInteger) -> Retained<NSArray<VNRecognizedText>>;

        #[unsafe(method(boundingBox))]
        fn boundingBox(&self) -> CGRect;
    );
}

#[cfg(target_os = "macos")]
#[allow(non_snake_case)]
impl VNRecognizedText {
    extern_methods!(
        #[unsafe(method(string))]
        fn string(&self) -> Retained<NSString>;

        #[unsafe(method(confidence))]
        fn confidence(&self) -> f32;

        #[unsafe(method(boundingBoxForRange:error:))]
        fn boundingBoxForRange_error(
            &self,
            range: NSRange,
            error: *mut *mut NSError,
        ) -> Option<Retained<VNRectangleObservation>>;
    );
}

#[cfg(target_os = "macos")]
#[allow(non_snake_case)]
impl VNRectangleObservation {
    extern_methods!(
        #[unsafe(method(boundingBox))]
        fn boundingBox(&self) -> CGRect;
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OcrLanguage {
    English,
    SimplifiedChinese,
    TraditionalChinese,
    Japanese,
    Korean,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OcrRequest {
    pub image: ImageFrame,
    pub languages: Vec<OcrLanguage>,
    pub include_word_boxes: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OcrWord {
    pub text: String,
    pub bounds: ScreenRect,
    pub confidence: Option<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OcrLine {
    pub text: String,
    pub bounds: ScreenRect,
    pub words: Vec<OcrWord>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OcrResult {
    pub text: String,
    pub lines: Vec<OcrLine>,
}

impl OcrResult {
    #[must_use]
    pub fn from_lines(lines: Vec<OcrLine>) -> Self {
        let text = lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        Self { text, lines }
    }
}

pub fn recognize_text(request: OcrRequest) -> Result<OcrResult, OcrError> {
    validate_image_frame(&request.image)?;
    recognize_text_impl(request)
}

fn validate_image_frame(image: &ImageFrame) -> Result<(), OcrError> {
    if image.width == 0 || image.height == 0 {
        return Err(OcrError::InvalidInput(
            "image dimensions must be greater than zero",
        ));
    }

    let width = usize::try_from(image.width)
        .map_err(|_| OcrError::InvalidInput("image width is too large"))?;
    let height = usize::try_from(image.height)
        .map_err(|_| OcrError::InvalidInput("image height is too large"))?;
    let expected_len = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(OcrError::InvalidInput("image dimensions overflow"))?;

    if image.bytes_rgba8.len() != expected_len {
        return Err(OcrError::InvalidInput(
            "image must contain width * height * 4 RGBA bytes",
        ));
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn recognize_text_impl(request: OcrRequest) -> Result<OcrResult, OcrError> {
    recognize_text_macos(request)
}

#[cfg(target_os = "windows")]
fn recognize_text_impl(_: OcrRequest) -> Result<OcrResult, OcrError> {
    Err(OcrError::BackendUnavailable(
        "windows OCR backend is not implemented yet",
    ))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn recognize_text_impl(_: OcrRequest) -> Result<OcrResult, OcrError> {
    Err(OcrError::UnsupportedPlatform)
}

#[cfg(target_os = "macos")]
fn recognize_text_macos(request: OcrRequest) -> Result<OcrResult, OcrError> {
    let image = cg_image_from_frame(&request.image)?;
    let recognize_request = new_recognize_text_request()?;
    configure_recognize_text_request(&recognize_request, &request.languages);

    let options = VisionOptions::new();
    let handler = unsafe {
        VNImageRequestHandler::initWithCGImage_options(VNImageRequestHandler::alloc(), &image, &options)
    };
    let requests = NSArray::from_slice(&[&*recognize_request]);
    let mut raw_error: *mut NSError = std::ptr::null_mut();
    let performed: bool = unsafe { msg_send![&*handler, performRequests: &*requests, error: &mut raw_error] };
    if !performed {
        return Err(vision_error(unsafe { Retained::from_raw(raw_error) }));
    }

    let observations = recognize_request.results().unwrap_or_else(NSArray::new);
    let mut lines = observations
        .iter()
        .map(|observation| line_from_observation(&observation, &request.image, request.include_word_boxes))
        .collect::<Result<Vec<_>, _>>()?;
    lines.sort_by(compare_lines);
    Ok(OcrResult::from_lines(lines))
}

#[cfg(target_os = "macos")]
fn new_recognize_text_request() -> Result<Retained<VNRecognizeTextRequest>, OcrError> {
    let request: Retained<VNRecognizeTextRequest> = unsafe { msg_send![VNRecognizeTextRequest::class(), new] };
    Ok(request)
}

#[cfg(target_os = "macos")]
fn configure_recognize_text_request(
    request: &VNRecognizeTextRequest,
    languages: &[OcrLanguage],
) {
    request.setUsesLanguageCorrection(true);
    if languages.is_empty() {
        return;
    }

    let language_strings = languages
        .iter()
        .map(|language| NSString::from_str(language_identifier(*language)))
        .collect::<Vec<_>>();
    let language_array = NSArray::from_retained_slice(&language_strings);
    request.setRecognitionLanguages(&language_array);
}

#[cfg(target_os = "macos")]
fn line_from_observation(
    observation: &VNRecognizedTextObservation,
    image: &ImageFrame,
    include_word_boxes: bool,
) -> Result<OcrLine, OcrError> {
    let candidates = observation.topCandidates(1);
    if candidates.count() == 0 {
        return Ok(OcrLine {
            text: String::new(),
            bounds: normalized_rect_to_screen_rect(observation.boundingBox(), image),
            words: Vec::new(),
        });
    }

    let candidate = candidates.objectAtIndex(0);
    let text = candidate.string().to_string();
    let bounds = normalized_rect_to_screen_rect(observation.boundingBox(), image);
    let words = if include_word_boxes {
        word_boxes_from_candidate(&candidate, image)?
    } else {
        Vec::new()
    };

    Ok(OcrLine { text, bounds, words })
}

#[cfg(target_os = "macos")]
fn word_boxes_from_candidate(
    candidate: &VNRecognizedText,
    image: &ImageFrame,
) -> Result<Vec<OcrWord>, OcrError> {
    let string = candidate.string().to_string();
    let ranges = word_ranges(&string);
    if ranges.is_empty() {
        return Ok(Vec::new());
    }

    let mut words = Vec::with_capacity(ranges.len());
    for (text, range) in ranges {
        let mut raw_error: *mut NSError = std::ptr::null_mut();
        let observation = candidate.boundingBoxForRange_error(range, &mut raw_error);
        let error = unsafe { Retained::from_raw(raw_error) };
        if let Some(error) = error {
            return Err(vision_error(Some(error)));
        }
        let observation = observation
            .ok_or_else(|| OcrError::SystemFailure("failed to resolve word bounding box".into()))?;
        words.push(OcrWord {
            text,
            bounds: normalized_rect_to_screen_rect(observation.boundingBox(), image),
            confidence: Some(candidate.confidence()),
        });
    }
    Ok(words)
}

#[cfg(target_os = "macos")]
fn cg_image_from_frame(image: &ImageFrame) -> Result<objc2_core_foundation::CFRetained<CGImage>, OcrError> {
    let data = unsafe {
        CFData::new(
            None,
            image.bytes_rgba8.as_ptr(),
            image.bytes_rgba8.len() as isize,
        )
    }
    .ok_or_else(|| OcrError::SystemFailure("failed to allocate image data".into()))?;
    let provider = CGDataProvider::with_cf_data(Some(&data))
        .ok_or_else(|| OcrError::SystemFailure("failed to create image provider".into()))?;
    let color_space = CGColorSpace::new_device_rgb()
        .ok_or_else(|| OcrError::SystemFailure("failed to create rgb color space".into()))?;
    let bitmap_info = CGBitmapInfo(CGImageByteOrderInfo::Order32Big.0)
        | CGBitmapInfo(CGImageAlphaInfo::PremultipliedLast.0);
    unsafe {
        CGImage::new(
            image.width as usize,
            image.height as usize,
            8,
            32,
            image.width as usize * 4,
            Some(&color_space),
            bitmap_info,
            Some(&provider),
            std::ptr::null::<CGFloat>(),
            false,
            CGColorRenderingIntent::RenderingIntentDefault,
        )
    }
    .ok_or_else(|| OcrError::SystemFailure("failed to create cg image".into()))
}

#[cfg(target_os = "macos")]
fn normalized_rect_to_screen_rect(rect: CGRect, image: &ImageFrame) -> ScreenRect {
    let x = rect.origin.x * image.width as f64;
    let width = rect.size.width * image.width as f64;
    let height = rect.size.height * image.height as f64;
    let y = (1.0 - rect.origin.y - rect.size.height) * image.height as f64;
    ScreenRect::new(x, y, width, height)
}

#[cfg(target_os = "macos")]
fn language_identifier(language: OcrLanguage) -> &'static str {
    match language {
        OcrLanguage::English => "en-US",
        OcrLanguage::SimplifiedChinese => "zh-Hans",
        OcrLanguage::TraditionalChinese => "zh-Hant",
        OcrLanguage::Japanese => "ja-JP",
        OcrLanguage::Korean => "ko-KR",
    }
}

#[cfg(target_os = "macos")]
fn vision_error(error: Option<Retained<NSError>>) -> OcrError {
    match error {
        Some(error) => OcrError::SystemFailure(error.localizedDescription().to_string()),
        None => OcrError::SystemFailure("vision request failed without NSError".into()),
    }
}

fn word_ranges(text: &str) -> Vec<(String, NSRange)> {
    let mut ranges = Vec::new();
    let mut current_start_utf16 = None;
    let mut current_start_byte = 0usize;
    let mut utf16_offset = 0usize;

    for (byte_index, ch) in text.char_indices() {
        if ch.is_whitespace() {
            if let Some(start_utf16) = current_start_utf16.take() {
                let word = text[current_start_byte..byte_index].to_string();
                ranges.push((word, NSRange::from(start_utf16..utf16_offset)));
            }
        } else if current_start_utf16.is_none() {
            current_start_utf16 = Some(utf16_offset);
            current_start_byte = byte_index;
        }
        utf16_offset += ch.len_utf16();
    }

    if let Some(start_utf16) = current_start_utf16 {
        ranges.push((text[current_start_byte..].to_string(), NSRange::from(start_utf16..utf16_offset)));
    }

    ranges
}

fn compare_lines(left: &OcrLine, right: &OcrLine) -> std::cmp::Ordering {
    left.bounds
        .y
        .partial_cmp(&right.bounds.y)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| {
            left.bounds
                .x
                .partial_cmp(&right.bounds.x)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

#[cfg(test)]
mod tests {
    use super::{word_ranges, OcrLine, OcrResult, OcrWord};
    use crate::capture::ScreenRect;

    #[test]
    fn flattens_lines_into_text() {
        let result = OcrResult::from_lines(vec![
            OcrLine {
                text: "hello".into(),
                bounds: ScreenRect::default(),
                words: vec![OcrWord {
                    text: "hello".into(),
                    bounds: ScreenRect::default(),
                    confidence: Some(0.9),
                }],
            },
            OcrLine {
                text: "world".into(),
                bounds: ScreenRect::default(),
                words: vec![],
            },
        ]);

        assert_eq!(result.text, "hello\nworld");
    }

    #[test]
    fn word_ranges_use_utf16_offsets() {
        let ranges = word_ranges("A中 B");

        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0].0, "A中");
        assert_eq!(ranges[0].1.location, 0);
        assert_eq!(ranges[0].1.length, 2);
        assert_eq!(ranges[1].0, "B");
        assert_eq!(ranges[1].1.location, 3);
        assert_eq!(ranges[1].1.length, 1);
    }
}

use crate::{
    OcrError,
    capture::ImageFrame,
    ocr::{RecognizedLine, collapse_lines},
};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{AnyThread, ClassType, extern_class, extern_methods, msg_send};
use objc2_core_foundation::{CFData, CGFloat, CGRect};
use objc2_core_graphics::{
    CGBitmapInfo, CGColorRenderingIntent, CGColorSpace, CGDataProvider, CGImage, CGImageAlphaInfo,
    CGImageByteOrderInfo,
};
use objc2_foundation::{NSArray, NSDictionary, NSError, NSString, NSUInteger};

#[link(name = "Vision", kind = "framework")]
unsafe extern "C" {}

type VisionOptions = NSDictionary<NSString, AnyObject>;

extern_class!(
    #[unsafe(super(NSObject))]
    #[derive(PartialEq, Eq, Hash)]
    struct VNRequest;
);

extern_class!(
    #[unsafe(super(VNRequest))]
    #[derive(PartialEq, Eq, Hash)]
    struct VNImageBasedRequest;
);

extern_class!(
    #[unsafe(super(VNImageBasedRequest))]
    #[derive(PartialEq, Eq, Hash)]
    struct VNRecognizeTextRequest;
);

extern_class!(
    #[unsafe(super(NSObject))]
    #[derive(PartialEq, Eq, Hash)]
    struct VNImageRequestHandler;
);

extern_class!(
    #[unsafe(super(NSObject))]
    #[derive(PartialEq, Eq, Hash)]
    struct VNRecognizedTextObservation;
);

extern_class!(
    #[unsafe(super(NSObject))]
    #[derive(PartialEq, Eq, Hash)]
    struct VNRecognizedText;
);

#[allow(non_snake_case)]
impl VNRecognizeTextRequest {
    extern_methods!(
        #[unsafe(method(setUsesLanguageCorrection:))]
        fn setUsesLanguageCorrection(&self, enabled: bool);

        #[unsafe(method(results))]
        fn results(&self) -> Option<Retained<NSArray<VNRecognizedTextObservation>>>;
    );
}

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

#[allow(non_snake_case)]
impl VNRecognizedTextObservation {
    extern_methods!(
        #[unsafe(method(topCandidates:))]
        fn topCandidates(
            &self,
            max_candidate_count: NSUInteger,
        ) -> Retained<NSArray<VNRecognizedText>>;

        #[unsafe(method(boundingBox))]
        fn boundingBox(&self) -> CGRect;
    );
}

#[allow(non_snake_case)]
impl VNRecognizedText {
    extern_methods!(
        #[unsafe(method(string))]
        fn string(&self) -> Retained<NSString>;
    );
}

pub(super) fn recognize_text(image: &ImageFrame) -> Result<String, OcrError> {
    let image_width = image.width as f64;
    let image_height = image.height as f64;
    let image = cg_image_from_frame(image)?;
    let request = new_recognize_text_request()?;
    request.setUsesLanguageCorrection(true);

    let options = VisionOptions::new();
    let handler = unsafe {
        VNImageRequestHandler::initWithCGImage_options(
            VNImageRequestHandler::alloc(),
            &image,
            &options,
        )
    };
    let requests = NSArray::from_slice(&[&*request]);
    let mut raw_error: *mut NSError = std::ptr::null_mut();
    let performed: bool =
        unsafe { msg_send![&*handler, performRequests: &*requests, error: &mut raw_error] };
    if !performed {
        return Err(vision_error(unsafe { Retained::from_raw(raw_error) }));
    }

    let lines = request
        .results()
        .unwrap_or_else(NSArray::new)
        .iter()
        .map(|observation| {
            let candidates = observation.topCandidates(1);
            let text = if candidates.count() == 0 {
                String::new()
            } else {
                candidates.objectAtIndex(0).string().to_string()
            };
            let bounds = observation.boundingBox();
            RecognizedLine {
                text,
                x: bounds.origin.x * image_width,
                y: (1.0 - bounds.origin.y - bounds.size.height) * image_height,
            }
        })
        .collect::<Vec<_>>();

    Ok(collapse_lines(lines))
}

fn new_recognize_text_request() -> Result<Retained<VNRecognizeTextRequest>, OcrError> {
    let request: Retained<VNRecognizeTextRequest> =
        unsafe { msg_send![VNRecognizeTextRequest::class(), new] };
    Ok(request)
}

fn cg_image_from_frame(
    image: &ImageFrame,
) -> Result<objc2_core_foundation::CFRetained<CGImage>, OcrError> {
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

fn vision_error(error: Option<Retained<NSError>>) -> OcrError {
    match error {
        Some(error) => OcrError::SystemFailure(error.localizedDescription().to_string()),
        None => OcrError::SystemFailure("vision request failed without NSError".into()),
    }
}

use crate::error::OcrError;

mod shared;
pub use shared::{OcrLanguage, OcrLine, OcrRequest, OcrResult, OcrWord};
pub(crate) use shared::compare_lines;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
use shared::validate_image_frame;

pub fn recognize_text(request: OcrRequest) -> Result<OcrResult, OcrError> {
    validate_image_frame(&request.image)?;
    platform::recognize_text(request)
}

#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use unsupported as platform;
#[cfg(target_os = "windows")]
use windows as platform;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod unsupported {
    use crate::{
        OcrError,
        ocr::{OcrRequest, OcrResult},
    };

    pub(super) fn recognize_text(_: OcrRequest) -> Result<OcrResult, OcrError> {
        Err(OcrError::UnsupportedPlatform)
    }
}

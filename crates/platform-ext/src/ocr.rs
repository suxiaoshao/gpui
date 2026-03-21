use crate::error::OcrError;

mod shared;
use crate::capture::ImageFrame;
pub(crate) use shared::{RecognizedLine, collapse_lines};
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
use shared::validate_image_frame;

pub fn recognize_text(image: &ImageFrame) -> Result<String, OcrError> {
    validate_image_frame(image)?;
    platform::recognize_text(image)
}

#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use unsupported as platform;
#[cfg(target_os = "windows")]
use windows as platform;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod unsupported {
    use crate::{OcrError, capture::ImageFrame};

    pub(super) fn recognize_text(_: &ImageFrame) -> Result<String, OcrError> {
        Err(OcrError::UnsupportedPlatform)
    }
}

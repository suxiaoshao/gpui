use crate::error::OcrError;
use tracing::{Level, event};

mod image_frame;
mod shared;
pub use image_frame::ImageFrame;
pub(crate) use shared::{RecognizedLine, collapse_lines};
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
mod windows_support;
use shared::validate_image_frame;

pub fn recognize_text(image: &ImageFrame) -> Result<String, OcrError> {
    event!(
        Level::INFO,
        width = image.width,
        height = image.height,
        bytes_len = image.bytes_rgba8.len(),
        "Starting OCR"
    );
    validate_image_frame(image)?;
    let result = platform::recognize_text(image);
    match &result {
        Ok(text) => event!(
            Level::INFO,
            chars = text.chars().count(),
            is_empty = text.trim().is_empty(),
            "OCR completed"
        ),
        Err(err) => event!(Level::ERROR, error = ?err, "OCR failed"),
    }
    result
}

#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use unsupported as platform;
#[cfg(target_os = "windows")]
use windows as platform;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod unsupported {
    use crate::{OcrError, ocr::ImageFrame};

    pub(super) fn recognize_text(_: &ImageFrame) -> Result<String, OcrError> {
        Err(OcrError::UnsupportedPlatform)
    }
}

use crate::error::CaptureError;

mod shared;
pub use shared::{DisplayId, ImageFrame, ScreenRect};
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
pub(crate) mod windows;

pub fn capture_display(display_id: DisplayId) -> Result<ImageFrame, CaptureError> {
    platform::capture_display(display_id)
}

pub fn capture_rect(display_id: DisplayId, rect: ScreenRect) -> Result<ImageFrame, CaptureError> {
    let rect = rect.normalized();
    if rect.is_empty() {
        return Err(CaptureError::InvalidInput(
            "capture rectangle width and height must be positive",
        ));
    }

    platform::capture_rect(display_id, rect)
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
        CaptureError,
        capture::{DisplayId, ImageFrame, ScreenRect},
    };

    pub(super) fn capture_display(_: DisplayId) -> Result<ImageFrame, CaptureError> {
        Err(CaptureError::UnsupportedPlatform)
    }

    pub(super) fn capture_rect(_: DisplayId, _: ScreenRect) -> Result<ImageFrame, CaptureError> {
        Err(CaptureError::UnsupportedPlatform)
    }
}

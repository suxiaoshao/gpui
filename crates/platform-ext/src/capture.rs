use crate::error::CaptureError;
use futures_channel::oneshot;

mod shared;
pub use shared::ImageFrame;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
pub(crate) mod windows;

pub async fn capture_user_selected_area() -> Result<ImageFrame, CaptureError> {
    let (tx, rx) = oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(platform::capture_user_selected_area());
    });

    rx.await.unwrap_or_else(|_| {
        Err(CaptureError::SystemFailure(
            "capture worker terminated unexpectedly".into(),
        ))
    })
}

pub fn handle_capture_callback_url(url: &str) -> Result<bool, CaptureError> {
    platform::handle_capture_callback_url(url)
}

#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use unsupported as platform;
#[cfg(target_os = "windows")]
use windows as platform;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod unsupported {
    use crate::{CaptureError, capture::ImageFrame};

    pub(super) fn capture_user_selected_area() -> Result<ImageFrame, CaptureError> {
        Err(CaptureError::UnsupportedPlatform)
    }

    pub(super) fn handle_capture_callback_url(_: &str) -> Result<bool, CaptureError> {
        Ok(false)
    }
}

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PlatformExtError {
    #[error("main thread is unavailable")]
    MainThreadUnavailable,
    #[error("failed to load application icon")]
    FailedToLoadIcon,
}

#[derive(Debug, Error, PartialEq)]
pub enum CaptureError {
    #[error("capture is unsupported on this platform")]
    UnsupportedPlatform,
    #[error("capture was cancelled")]
    Cancelled,
    #[error("capture permission was denied")]
    PermissionDenied,
    #[error("capture backend is unavailable: {0}")]
    BackendUnavailable(&'static str),
    #[error("invalid capture input: {0}")]
    InvalidInput(&'static str),
    #[error("capture failed: {0}")]
    SystemFailure(String),
}

#[derive(Debug, Error, PartialEq)]
pub enum OcrError {
    #[error("ocr is unsupported on this platform")]
    UnsupportedPlatform,
    #[error("ocr backend is unavailable: {0}")]
    BackendUnavailable(&'static str),
    #[error("invalid ocr input: {0}")]
    InvalidInput(&'static str),
    #[error("ocr failed: {0}")]
    SystemFailure(String),
}

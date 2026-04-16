use thiserror::Error;

#[derive(Debug, Error)]
pub enum PlatformExtError {
    #[error("main thread is unavailable")]
    MainThreadUnavailable,
    #[error("failed to load application icon")]
    FailedToLoadIcon,
    #[error("main menu is unavailable")]
    MainMenuUnavailable,
    #[error("menu item is unavailable at index {0}")]
    MenuItemUnavailable(usize),
    #[error("menu item at index {0} has no submenu")]
    MenuItemHasNoSubmenu(usize),
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

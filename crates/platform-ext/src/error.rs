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

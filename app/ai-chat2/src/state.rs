pub(crate) mod config;
pub(crate) mod hotkey;
pub(crate) mod layout;
pub(crate) mod projects;
pub(crate) mod providers;
pub(crate) mod theme;
pub(crate) mod workspace;

pub(crate) use config::{AiChat2AppSettings, AiChat2Config};
pub(crate) use hotkey::GlobalHotkeyState;
pub(crate) use layout::{AiChat2LayoutState, LayoutStateStore};
pub(crate) use workspace::{AiChat2WorkspaceStore, HomeRoute};

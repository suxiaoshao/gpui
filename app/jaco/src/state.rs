pub(crate) mod config;
pub(crate) mod hotkey;
pub(crate) mod layout;
pub(crate) mod mcp;
pub(crate) mod projects;
pub(crate) mod prompts;
pub(crate) mod providers;
pub(crate) mod shortcuts;
pub(crate) mod theme;

pub(crate) use config::JacoConfig;
pub(crate) use hotkey::GlobalHotkeyState;
pub(crate) use layout::{JacoLayoutState, LayoutStateStore};

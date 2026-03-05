#[cfg(not(target_os = "windows"))]
pub mod ai_chat;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod settings;
#[cfg(target_os = "windows")]
pub mod windows;

use crate::cli::BundleAiChatArgs;
use crate::error::Result;

pub fn run(args: BundleAiChatArgs) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        windows::run(args)
    }

    #[cfg(not(target_os = "windows"))]
    {
        ai_chat::run(args)
    }
}

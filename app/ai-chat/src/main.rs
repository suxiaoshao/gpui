#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use crate::errors::AiChatResult;

mod app;
mod assets;
mod components;
mod database;
mod errors;
mod hotkey;
mod i18n;
mod llm;
mod platform;
mod state;
mod tray;
mod views;

fn main() -> AiChatResult<()> {
    app::run()
}

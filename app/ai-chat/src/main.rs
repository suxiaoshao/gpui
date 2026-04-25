#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use crate::errors::AiChatResult;

mod app;
mod components;
mod database;
mod errors;
mod export;
mod features;
mod foundation;
mod llm;
mod platform;
mod state;

fn main() -> AiChatResult<()> {
    app::run()
}

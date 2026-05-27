#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod app;
mod database;
mod errors;
mod foundation;
mod state;

fn main() {
    if let Err(err) = app::run() {
        eprintln!("ai-chat2 failed: {err}");
    }
}

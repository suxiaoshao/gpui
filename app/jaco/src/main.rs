#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use crate::errors::JacoResult;

mod app;
mod components;
mod database;
mod errors;
mod features;
mod foundation;
mod platform;
mod state;

fn main() -> JacoResult<()> {
    app::run()
}

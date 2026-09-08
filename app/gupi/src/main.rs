#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod app;
mod components;
mod features;
mod foundation;
mod pi;
mod state;

fn main() {
    app::run();
}

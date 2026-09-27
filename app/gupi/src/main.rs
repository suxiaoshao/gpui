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
    #[cfg(target_os = "macos")]
    if std::env::args_os().nth(1).as_deref()
        == Some(std::ffi::OsStr::new("--gupi-print-shell-path"))
    {
        if foundation::shell_path::print_path().is_err() {
            std::process::exit(1);
        }
        return;
    }
    app::run();
}

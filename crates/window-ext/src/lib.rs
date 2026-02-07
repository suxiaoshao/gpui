#![allow(deprecated)]
use gpui::Window;
pub use objc2::rc::Retained;
use objc2::{MainThreadMarker, rc::Id};
pub use objc2_app_kit::NSRunningApplication;
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSApplication, NSView, NSWindow};
use raw_window_handle::{AppKitWindowHandle, HandleError, HasRawWindowHandle, RawWindowHandle};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WindowExtError {
    #[error("Failed to get NSWindow, {}",.0)]
    FailedToGetHandle(HandleError),
    #[error("Failed to get NSView")]
    FailedToGetNSView,
    #[error("Failed to get NSWindow")]
    FailedToGetNSWindow,
    #[error("Failed to get NSApplication")]
    FailedToGetNSApplication,
}

pub trait WindowExt {
    fn hide(&self) -> Result<(), WindowExtError>;
    fn show(&self) -> Result<(), WindowExtError>;
    fn set_floating(&self) -> Result<(), WindowExtError>;
}

impl WindowExt for Window {
    fn hide(&self) -> Result<(), WindowExtError> {
        let raw_window = get_raw_window(self)?;
        match raw_window {
            RawWindowHandle::AppKit(handle) => {
                let ns_window = get_ns_window(handle)?;
                ns_window.orderOut(None);
            }
            _ => {}
        }
        Ok(())
    }

    fn show(&self) -> Result<(), WindowExtError> {
        let raw_window = get_raw_window(self)?;
        match raw_window {
            RawWindowHandle::AppKit(handle) => {
                let ns_window = get_ns_window(handle)?;
                ns_window.makeKeyAndOrderFront(None);
                let ns_app = NSApplication::sharedApplication(
                    MainThreadMarker::new().ok_or(WindowExtError::FailedToGetNSApplication)?,
                );
                ns_app.activate();
            }
            _ => {}
        }
        Ok(())
    }
    fn set_floating(&self) -> Result<(), WindowExtError> {
        let raw_window = get_raw_window(self)?;
        match raw_window {
            RawWindowHandle::AppKit(handle) => {
                let ns_window = get_ns_window(handle)?;
                ns_window.setLevel(5 as _);
                let ns_app = NSApplication::sharedApplication(
                    MainThreadMarker::new().ok_or(WindowExtError::FailedToGetNSApplication)?,
                );
                ns_app.activate();
            }
            _ => {}
        }
        Ok(())
    }
}

fn get_raw_window(window: &Window) -> Result<RawWindowHandle, WindowExtError> {
    let raw_window = window
        .raw_window_handle()
        .map_err(WindowExtError::FailedToGetHandle)?;
    Ok(raw_window)
}

fn get_ns_window(window: AppKitWindowHandle) -> Result<Retained<NSWindow>, WindowExtError> {
    let ns_view = window.ns_view.as_ptr();
    let ns_view: Id<NSView> =
        unsafe { Id::retain(ns_view.cast()) }.ok_or(WindowExtError::FailedToGetNSView)?;
    let ns_window = ns_view
        .window()
        .ok_or(WindowExtError::FailedToGetNSWindow)?;

    Ok(ns_window)
}

#[cfg(target_os = "macos")]
pub fn record_frontmost_app() -> Option<Retained<NSRunningApplication>> {
    // 获取 [NSWorkspace sharedWorkspace].frontmostApplication

    use objc2_app_kit::NSWorkspace;
    let workspace = NSWorkspace::sharedWorkspace();
    workspace.frontmostApplication()
}

#[cfg(target_os = "macos")]
pub fn restore_frontmost_app(prev_app: &Option<Retained<NSRunningApplication>>) {
    // 调用 [prevApp activateWithOptions:NSApplicationActivateIgnoringOtherApps]
    const NSAPPLICATION_ACTIVATE_IGNORING_OTHER_APPS: usize = 1 << 1;
    if let Some(app) = prev_app.as_ref() {
        use objc2_app_kit::NSApplicationActivationOptions;

        app.activateWithOptions(NSApplicationActivationOptions(
            NSAPPLICATION_ACTIVATE_IGNORING_OTHER_APPS,
        ));
    }
}

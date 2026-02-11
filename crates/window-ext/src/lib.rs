#![allow(deprecated)]
use gpui::Window;
#[cfg(target_os = "macos")]
pub use objc2::rc::Retained;
#[cfg(target_os = "macos")]
use objc2::{AnyThread, MainThreadMarker, rc::Id};
#[cfg(target_os = "macos")]
pub use objc2_app_kit::NSRunningApplication;
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSApplication, NSImage, NSView, NSWindow};
#[cfg(target_os = "macos")]
use objc2_foundation::NSData;
#[cfg(target_os = "macos")]
use raw_window_handle::AppKitWindowHandle;
use raw_window_handle::{HandleError, HasRawWindowHandle, RawWindowHandle};
use thiserror::Error;
#[cfg(target_os = "windows")]
use windows::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{
        HWND_TOPMOST, SW_HIDE, SW_SHOW, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos, ShowWindow,
    },
};

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
    #[error("Failed to load app icon")]
    FailedToLoadAppIcon,
    #[error("Failed to set topmost")]
    FailedSetTopMost,
}

pub trait WindowExt {
    fn hide(&self) -> Result<(), WindowExtError>;
    fn show(&self) -> Result<(), WindowExtError>;
    fn set_floating(&self) -> Result<(), WindowExtError>;
    fn is_visible(&self) -> Result<bool, WindowExtError>;
}

impl WindowExt for Window {
    fn hide(&self) -> Result<(), WindowExtError> {
        let raw_window = get_raw_window(self)?;
        match raw_window {
            #[allow(unused_variables)]
            RawWindowHandle::AppKit(handle) => {
                #[cfg(target_os = "macos")]
                {
                    let ns_window = get_ns_window(handle)?;
                    ns_window.orderOut(None);
                }
            }
            #[allow(unused_variables)]
            RawWindowHandle::Win32(handle) => {
                #[cfg(target_os = "windows")]
                {
                    let hwnd = HWND(handle.hwnd.get() as _);
                    unsafe {
                        let _ = ShowWindow(hwnd, SW_HIDE);
                    };
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn show(&self) -> Result<(), WindowExtError> {
        let raw_window = get_raw_window(self)?;
        match raw_window {
            #[allow(unused_variables)]
            RawWindowHandle::AppKit(handle) => {
                #[cfg(target_os = "macos")]
                {
                    let ns_window = get_ns_window(handle)?;
                    ns_window.makeKeyAndOrderFront(None);
                    let ns_app = NSApplication::sharedApplication(
                        MainThreadMarker::new().ok_or(WindowExtError::FailedToGetNSApplication)?,
                    );
                    ns_app.activate();
                }
            }
            #[allow(unused_variables)]
            RawWindowHandle::Win32(handle) => {
                #[cfg(target_os = "windows")]
                {
                    let hwnd = HWND(handle.hwnd.get() as _);
                    unsafe {
                        let _ = ShowWindow(hwnd, SW_SHOW);
                    };
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn set_floating(&self) -> Result<(), WindowExtError> {
        let raw_window = get_raw_window(self)?;
        match raw_window {
            #[allow(unused_variables)]
            RawWindowHandle::AppKit(handle) => {
                #[cfg(target_os = "macos")]
                {
                    let ns_window = get_ns_window(handle)?;
                    ns_window.setLevel(5 as _);
                    let ns_app = NSApplication::sharedApplication(
                        MainThreadMarker::new().ok_or(WindowExtError::FailedToGetNSApplication)?,
                    );
                    ns_app.activate();
                }
            }
            #[allow(unused_variables)]
            RawWindowHandle::Win32(handle) => {
                #[cfg(target_os = "windows")]
                {
                    let hwnd = HWND(handle.hwnd.get() as _);
                    unsafe {
                        SetWindowPos(
                            hwnd,
                            Some(HWND_TOPMOST),
                            0,
                            0,
                            0,
                            0,
                            SWP_NOSIZE | SWP_NOMOVE,
                        )
                        .map_err(|_| WindowExtError::FailedSetTopMost)?;
                    }
                }
            }
            _ => (),
        }
        Ok(())
    }

    fn is_visible(&self) -> Result<bool, WindowExtError> {
        let raw_window = get_raw_window(self)?;
        match raw_window {
            #[allow(unused_variables)]
            RawWindowHandle::AppKit(handle) => {
                #[cfg(target_os = "macos")]
                {
                    let ns_window = get_ns_window(handle)?;
                    return Ok(ns_window.isVisible());
                }
            }

            #[allow(unused_variables)]
            RawWindowHandle::Win32(handle) => {
                #[cfg(target_os = "windows")]
                {
                    let hwnd = HWND(handle.hwnd.get() as _);
                    unsafe {
                        use windows::Win32::UI::WindowsAndMessaging::IsWindowVisible;
                        return Ok(IsWindowVisible(hwnd).as_bool());
                    };
                }
            }
            _ => {}
        };
        Ok(true)
    }
}

fn get_raw_window(window: &Window) -> Result<RawWindowHandle, WindowExtError> {
    let raw_window = window
        .raw_window_handle()
        .map_err(WindowExtError::FailedToGetHandle)?;
    Ok(raw_window)
}

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
pub fn set_application_icon_from_bytes(icon_bytes: &[u8]) -> Result<(), WindowExtError> {
    let ns_app = NSApplication::sharedApplication(
        MainThreadMarker::new().ok_or(WindowExtError::FailedToGetNSApplication)?,
    );
    let data = NSData::with_bytes(icon_bytes);
    let image = NSImage::initWithData(NSImage::alloc(), &data)
        .ok_or(WindowExtError::FailedToLoadAppIcon)?;
    unsafe {
        ns_app.setApplicationIconImage(Some(&image));
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn set_application_icon_from_bytes(_icon_bytes: &[u8]) -> Result<(), WindowExtError> {
    Ok(())
}

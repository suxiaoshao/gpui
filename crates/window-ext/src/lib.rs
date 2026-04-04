#![allow(deprecated)]
use gpui::{Bounds, DisplayId, Pixels, Window};
#[cfg(target_os = "macos")]
use objc2::{MainThreadMarker, rc::Id};
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSApplication, NSScreen, NSView, NSWindow};
#[cfg(target_os = "macos")]
use raw_window_handle::AppKitWindowHandle;
use raw_window_handle::{HandleError, HasRawWindowHandle, RawWindowHandle};
use thiserror::Error;
#[cfg(target_os = "windows")]
use windows::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{
        HWND_TOPMOST, SW_HIDE, SW_SHOW, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos,
        ShowWindow,
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
    #[error("Failed to set topmost")]
    FailedSetTopMost,
    #[error("Failed to set window bounds")]
    FailedSetBounds,
}

pub trait WindowExt {
    fn hide(&self) -> Result<(), WindowExtError>;
    fn show(&self) -> Result<(), WindowExtError>;
    fn set_floating(&self) -> Result<(), WindowExtError>;
    fn is_visible(&self) -> Result<bool, WindowExtError>;
    fn move_and_resize(
        &self,
        bounds: Bounds<Pixels>,
        display_id: Option<DisplayId>,
    ) -> Result<(), WindowExtError>;
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

    fn move_and_resize(
        &self,
        bounds: Bounds<Pixels>,
        display_id: Option<DisplayId>,
    ) -> Result<(), WindowExtError> {
        let raw_window = get_raw_window(self)?;
        match raw_window {
            #[allow(unused_variables)]
            RawWindowHandle::AppKit(handle) => {
                #[cfg(target_os = "macos")]
                {
                    let ns_window = get_ns_window(handle)?;
                    let screen_frame = resolve_screen_frame(&ns_window, display_id)?;
                    let frame = objc2_foundation::NSRect::new(
                        objc2_foundation::NSPoint::new(
                            screen_frame.origin.x + f64::from(f32::from(bounds.origin.x)),
                            screen_frame.origin.y + screen_frame.size.height
                                - f64::from(f32::from(bounds.origin.y))
                                - f64::from(f32::from(bounds.size.height)),
                        ),
                        objc2_foundation::NSSize::new(
                            f64::from(f32::from(bounds.size.width)),
                            f64::from(f32::from(bounds.size.height)),
                        ),
                    );
                    ns_window.setFrame_display(frame, true);
                }
            }
            #[allow(unused_variables)]
            RawWindowHandle::Win32(handle) => {
                #[cfg(target_os = "windows")]
                {
                    let hwnd = HWND(handle.hwnd.get() as _);
                    let (x, y, width, height) =
                        logical_bounds_to_device_rect(bounds, self.scale_factor());
                    unsafe {
                        SetWindowPos(
                            hwnd,
                            None,
                            x,
                            y,
                            width,
                            height,
                            SWP_NOZORDER,
                        )
                        .map_err(|_| WindowExtError::FailedSetBounds)?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn logical_bounds_to_device_rect(bounds: Bounds<Pixels>, scale_factor: f32) -> (i32, i32, i32, i32) {
    let bounds = bounds.to_device_pixels(scale_factor);
    (
        bounds.origin.x.0,
        bounds.origin.y.0,
        bounds.size.width.0,
        bounds.size.height.0,
    )
}

#[cfg(test)]
mod tests {
    use super::logical_bounds_to_device_rect;
    use gpui::{bounds, point, px, size};

    #[test]
    fn logical_bounds_to_device_rect_scales_coordinates_and_size() {
        let result = logical_bounds_to_device_rect(
            bounds(point(px(10.0), px(20.0)), size(px(300.0), px(200.0))),
            1.5,
        );

        assert_eq!(result, (15, 30, 450, 300));
    }
}

fn get_raw_window(window: &Window) -> Result<RawWindowHandle, WindowExtError> {
    let raw_window = window
        .raw_window_handle()
        .map_err(WindowExtError::FailedToGetHandle)?;
    Ok(raw_window)
}

#[cfg(target_os = "macos")]
fn get_ns_window(
    window: AppKitWindowHandle,
) -> Result<objc2::rc::Retained<NSWindow>, WindowExtError> {
    let ns_view = window.ns_view.as_ptr();
    let ns_view: Id<NSView> =
        unsafe { Id::retain(ns_view.cast()) }.ok_or(WindowExtError::FailedToGetNSView)?;
    let ns_window = ns_view
        .window()
        .ok_or(WindowExtError::FailedToGetNSWindow)?;

    Ok(ns_window)
}

#[cfg(target_os = "macos")]
fn resolve_screen_frame(
    ns_window: &NSWindow,
    display_id: Option<DisplayId>,
) -> Result<objc2_foundation::NSRect, WindowExtError> {
    if let Some(display_id) = display_id {
        let mtm = MainThreadMarker::new().ok_or(WindowExtError::FailedToGetNSApplication)?;
        let screens = NSScreen::screens(mtm);
        for screen in &screens {
            if screen.CGDirectDisplayID() == u32::from(display_id) {
                return Ok(screen.frame());
            }
        }
    }

    let screen = ns_window
        .screen()
        .ok_or(WindowExtError::FailedToGetNSWindow)?;
    Ok(screen.frame())
}

use crate::error::PlatformExtError;

#[cfg(target_os = "macos")]
pub use objc2::rc::Retained;
#[cfg(target_os = "macos")]
use objc2::{AnyThread, MainThreadMarker};
#[cfg(target_os = "macos")]
pub use objc2_app_kit::NSRunningApplication;
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSApplication, NSEvent, NSImage, NSScreen};
#[cfg(target_os = "macos")]
use objc2_foundation::NSData;
#[cfg(target_os = "windows")]
use windows::Win32::{
    Foundation::{LPARAM, POINT, RECT},
    Graphics::Gdi::{EnumDisplayMonitors, HDC, HMONITOR, MONITOR_DEFAULTTONULL, MonitorFromPoint},
    UI::WindowsAndMessaging::GetCursorPos,
};
#[cfg(target_os = "windows")]
use windows::core::BOOL;

#[cfg(target_os = "macos")]
pub fn record_frontmost_app() -> Option<Retained<NSRunningApplication>> {
    use objc2_app_kit::NSWorkspace;

    NSWorkspace::sharedWorkspace().frontmostApplication()
}

#[cfg(not(target_os = "macos"))]
pub fn record_frontmost_app() {}

#[cfg(target_os = "macos")]
pub fn restore_frontmost_app(prev_app: &Option<Retained<NSRunningApplication>>) {
    const NSAPPLICATION_ACTIVATE_IGNORING_OTHER_APPS: usize = 1 << 1;

    if let Some(app) = prev_app.as_ref() {
        use objc2_app_kit::NSApplicationActivationOptions;

        app.activateWithOptions(NSApplicationActivationOptions(
            NSAPPLICATION_ACTIVATE_IGNORING_OTHER_APPS,
        ));
    }
}

#[cfg(not(target_os = "macos"))]
pub fn restore_frontmost_app(_: &()) {}

#[cfg(target_os = "macos")]
pub fn current_mouse_location() -> Option<(f32, f32)> {
    let _ = MainThreadMarker::new()?;
    let point = NSEvent::mouseLocation();
    Some((point.x as f32, point.y as f32))
}

#[cfg(target_os = "windows")]
pub fn current_mouse_location() -> Option<(f32, f32)> {
    let mut point = POINT::default();
    unsafe { GetCursorPos(&mut point).ok()? };
    Some((point.x as f32, point.y as f32))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn current_mouse_location() -> Option<(f32, f32)> {
    None
}

#[cfg(target_os = "macos")]
pub fn current_mouse_display_id() -> Option<u32> {
    let mtm = MainThreadMarker::new()?;
    let point = NSEvent::mouseLocation();
    let screens = NSScreen::screens(mtm);
    for screen in &screens {
        let frame = screen.frame();
        let max_x = frame.origin.x + frame.size.width;
        let max_y = frame.origin.y + frame.size.height;
        if point.x >= frame.origin.x
            && point.x < max_x
            && point.y >= frame.origin.y
            && point.y < max_y
        {
            return Some(screen.CGDirectDisplayID());
        }
    }
    None
}

#[cfg(target_os = "windows")]
pub fn current_mouse_display_id() -> Option<u32> {
    let mut point = POINT::default();
    unsafe { GetCursorPos(&mut point).ok()? };
    let monitor = unsafe { MonitorFromPoint(point, MONITOR_DEFAULTTONULL) };
    if monitor.is_invalid() {
        return None;
    }

    let mut monitors = Vec::new();
    unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(enum_monitors),
            LPARAM(&mut monitors as *mut Vec<HMONITOR> as isize),
        )
        .ok()
        .ok()?;
    }

    monitors
        .iter()
        .position(|candidate: &HMONITOR| candidate.0 == monitor.0)
        .map(|index| index as u32)
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn enum_monitors(
    monitor: HMONITOR,
    _hdc: HDC,
    _rect: *mut RECT,
    data: LPARAM,
) -> BOOL {
    let monitors = unsafe { &mut *(data.0 as *mut Vec<HMONITOR>) };
    monitors.push(monitor);
    BOOL(1)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn current_mouse_display_id() -> Option<u32> {
    None
}

#[cfg(target_os = "macos")]
pub fn set_application_icon_from_bytes(icon_bytes: &[u8]) -> Result<(), PlatformExtError> {
    let ns_app = NSApplication::sharedApplication(
        MainThreadMarker::new().ok_or(PlatformExtError::MainThreadUnavailable)?,
    );
    let data = NSData::with_bytes(icon_bytes);
    let image =
        NSImage::initWithData(NSImage::alloc(), &data).ok_or(PlatformExtError::FailedToLoadIcon)?;
    unsafe {
        ns_app.setApplicationIconImage(Some(&image));
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn set_application_icon_from_bytes(_: &[u8]) -> Result<(), PlatformExtError> {
    Ok(())
}

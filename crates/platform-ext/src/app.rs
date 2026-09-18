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
    Foundation::POINT,
    Graphics::Gdi::{MONITOR_DEFAULTTONULL, MonitorFromPoint},
    UI::WindowsAndMessaging::GetCursorPos,
};

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
pub fn current_mouse_display_id() -> Option<u64> {
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
            return Some(u64::from(screen.CGDirectDisplayID()));
        }
    }
    None
}

#[cfg(target_os = "windows")]
pub fn current_mouse_display_id() -> Option<u64> {
    let mut point = POINT::default();
    unsafe { GetCursorPos(&mut point).ok()? };
    let monitor = unsafe { MonitorFromPoint(point, MONITOR_DEFAULTTONULL) };
    if monitor.is_invalid() {
        return None;
    }

    // GPUI's Windows DisplayId is the native handle, not an enumeration index.
    Some(monitor.0 as u64)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn current_mouse_display_id() -> Option<u64> {
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

#[cfg(target_os = "macos")]
pub fn set_windows_menu_from_main_menu_index(index: usize) -> Result<(), PlatformExtError> {
    let ns_app = NSApplication::sharedApplication(
        MainThreadMarker::new().ok_or(PlatformExtError::MainThreadUnavailable)?,
    );
    let main_menu = ns_app
        .mainMenu()
        .ok_or(PlatformExtError::MainMenuUnavailable)?;
    let item = main_menu
        .itemAtIndex(index as isize)
        .ok_or(PlatformExtError::MenuItemUnavailable(index))?;
    let submenu = item
        .submenu()
        .ok_or(PlatformExtError::MenuItemHasNoSubmenu(index))?;

    ns_app.setWindowsMenu(Some(&submenu));
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn set_windows_menu_from_main_menu_index(_: usize) -> Result<(), PlatformExtError> {
    Ok(())
}

/// A retained foreground target for restoring focus after a utility window hides.
#[derive(Clone)]
pub struct FrontmostApp {
    #[cfg(target_os = "macos")]
    app: Retained<NSRunningApplication>,
    #[cfg(target_os = "windows")]
    window: windows::Win32::Foundation::HWND,
}
pub fn capture_frontmost() -> Option<FrontmostApp> {
    #[cfg(target_os = "macos")]
    {
        let app = record_frontmost_app()?;
        if app.processIdentifier() == std::process::id() as i32 {
            return None;
        }
        Some(FrontmostApp { app })
    }
    #[cfg(target_os = "windows")]
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{
            GetForegroundWindow, GetWindowThreadProcessId,
        };
        let window = GetForegroundWindow();
        let mut process = 0;
        GetWindowThreadProcessId(window, Some(&mut process));
        if window.is_invalid() || process == std::process::id() {
            None
        } else {
            Some(FrontmostApp { window })
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        None
    }
}
impl FrontmostApp {
    pub fn restore(&self) {
        #[cfg(target_os = "macos")]
        restore_frontmost_app(&Some(self.app.clone()));
        #[cfg(target_os = "windows")]
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(self.window);
        }
    }
}

/// Fail before hiding the UI if system event injection is unavailable.
pub fn check_paste_access() -> Result<(), &'static str> {
    #[cfg(target_os = "macos")]
    if !objc2_core_graphics::CGPreflightPostEventAccess() {
        return Err("Accessibility permission is required to paste");
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    return Err("Pasting into another application is unsupported");
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    Ok(())
}
impl FrontmostApp {
    pub fn is_frontmost(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            record_frontmost_app()
                .is_some_and(|app| app.processIdentifier() == self.app.processIdentifier())
        }
        #[cfg(target_os = "windows")]
        {
            unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() == self.window }
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            false
        }
    }
    /// The caller restores and waits for this target first. Never inject into
    /// whichever application happened to steal focus during that wait.
    pub fn paste(&self) -> Result<(), &'static str> {
        check_paste_access()?;
        if !self.is_frontmost() {
            return Err("The paste target is no longer focused");
        }
        #[cfg(target_os = "macos")]
        {
            use objc2_core_graphics::{CGEvent, CGEventFlags};
            let down =
                CGEvent::new_keyboard_event(None, 9, true).ok_or("Cannot create paste event")?;
            let up =
                CGEvent::new_keyboard_event(None, 9, false).ok_or("Cannot create paste event")?;
            CGEvent::set_flags(Some(&down), CGEventFlags::MaskCommand);
            CGEvent::set_flags(Some(&up), CGEventFlags::MaskCommand);
            CGEvent::post_to_pid(self.app.processIdentifier(), Some(&down));
            CGEvent::post_to_pid(self.app.processIdentifier(), Some(&up));
            Ok(())
        }
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::UI::Input::KeyboardAndMouse::{
                INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_CONTROL,
                VK_V,
            };
            let key = |vk, flags| INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: vk,
                        dwFlags: flags,
                        ..Default::default()
                    },
                },
            };
            let inputs = [
                key(VK_CONTROL, Default::default()),
                key(VK_V, Default::default()),
                key(VK_V, KEYEVENTF_KEYUP),
                key(VK_CONTROL, KEYEVENTF_KEYUP),
            ];
            if unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) }
                != inputs.len() as u32
            {
                // Release modifiers even if injection was only partially accepted.
                unsafe {
                    SendInput(&inputs[2..], std::mem::size_of::<INPUT>() as i32);
                }
                return Err("The system rejected the paste event");
            }
            Ok(())
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        Err("Pasting into another application is unsupported")
    }
}

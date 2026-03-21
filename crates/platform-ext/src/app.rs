use crate::error::PlatformExtError;

#[cfg(target_os = "macos")]
pub use objc2::rc::Retained;
#[cfg(target_os = "macos")]
use objc2::{AnyThread, MainThreadMarker};
#[cfg(target_os = "macos")]
pub use objc2_app_kit::NSRunningApplication;
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSApplication, NSImage};
#[cfg(target_os = "macos")]
use objc2_foundation::NSData;

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

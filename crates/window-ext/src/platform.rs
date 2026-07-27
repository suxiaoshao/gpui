use super::*;

#[cfg(any(target_os = "macos", test))]
pub(super) fn macos_window_level_value(level: WindowLevel) -> i32 {
    match level {
        WindowLevel::Normal => NORMAL_WINDOW_LEVEL,
        WindowLevel::Floating => FLOATING_WINDOW_LEVEL,
        WindowLevel::ModalPanel => MODAL_PANEL_WINDOW_LEVEL,
        WindowLevel::PopUpMenu => POP_UP_MENU_WINDOW_LEVEL,
        WindowLevel::Custom(value) => value,
    }
}

#[cfg(any(target_os = "windows", test))]
pub(super) fn logical_bounds_to_device_rect(
    bounds: Bounds<Pixels>,
    scale_factor: f32,
) -> (i32, i32, i32, i32) {
    let bounds = bounds.to_device_pixels(scale_factor);
    (
        bounds.origin.x.0,
        bounds.origin.y.0,
        bounds.size.width.0,
        bounds.size.height.0,
    )
}

#[cfg(any(target_os = "windows", test))]
pub(super) fn resolve_target_scale_factor(
    fallback_scale_factor: f32,
    target_scale_factor: Option<f32>,
) -> f32 {
    target_scale_factor.unwrap_or(fallback_scale_factor)
}

#[cfg(target_os = "windows")]
pub(super) fn scale_factor_for_display(display_id: DisplayId) -> Option<f32> {
    let display_index = usize::try_from(u64::from(display_id)).ok()?;
    available_monitors()
        .into_iter()
        .nth(display_index)
        .and_then(|monitor| get_scale_factor_for_monitor(monitor).ok())
}

#[cfg(target_os = "windows")]
pub(super) fn available_monitors() -> Vec<HMONITOR> {
    let mut monitors = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(monitor_enum_proc),
            LPARAM(&mut monitors as *mut _ as _),
        );
    }
    monitors
}

#[cfg(target_os = "windows")]
pub(super) unsafe extern "system" fn monitor_enum_proc(
    monitor: HMONITOR,
    _hdc: HDC,
    _rect: *mut RECT,
    data: LPARAM,
) -> BOOL {
    let monitors = data.0 as *mut Vec<HMONITOR>;
    unsafe { (*monitors).push(monitor) };
    BOOL(1)
}

#[cfg(target_os = "windows")]
pub(super) fn get_scale_factor_for_monitor(monitor: HMONITOR) -> windows::core::Result<f32> {
    let mut dpi_x = 0;
    let mut dpi_y = 0;
    unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) }?;
    debug_assert_eq!(dpi_x, dpi_y);
    Ok(dpi_x as f32 / USER_DEFAULT_SCREEN_DPI as f32)
}

pub(super) fn get_raw_window(window: &Window) -> Result<RawWindowHandle, WindowExtError> {
    let raw_window = window
        .raw_window_handle()
        .map_err(WindowExtError::FailedToGetHandle)?;
    Ok(raw_window)
}

#[cfg(target_os = "macos")]
pub(super) fn get_ns_window(
    window: AppKitWindowHandle,
) -> Result<objc2::rc::Retained<NSWindow>, WindowExtError> {
    let ns_view = get_ns_view(window)?;
    let ns_window = ns_view
        .window()
        .ok_or(WindowExtError::FailedToGetNSWindow)?;

    Ok(ns_window)
}

#[cfg(target_os = "macos")]
pub(super) fn get_ns_view(
    window: AppKitWindowHandle,
) -> Result<objc2::rc::Retained<NSView>, WindowExtError> {
    let ns_view = window.ns_view.as_ptr();
    let ns_view: Id<NSView> =
        unsafe { Id::retain(ns_view.cast()) }.ok_or(WindowExtError::FailedToGetNSView)?;
    Ok(ns_view)
}

#[cfg(target_os = "macos")]
pub(super) fn resolve_screen_frame(
    ns_window: &NSWindow,
    display_id: Option<DisplayId>,
) -> Result<objc2_foundation::NSRect, WindowExtError> {
    if let Some(display_id) = display_id {
        let mtm = MainThreadMarker::new().ok_or(WindowExtError::FailedToGetNSApplication)?;
        let screens = NSScreen::screens(mtm);
        for screen in &screens {
            if u64::from(screen.CGDirectDisplayID()) == u64::from(display_id) {
                return Ok(screen.frame());
            }
        }
    }

    let screen = ns_window
        .screen()
        .ok_or(WindowExtError::FailedToGetNSWindow)?;
    Ok(screen.frame())
}

use gpui::{Pixels, Point};
use platform_ext::ocr::ImageFrame;
use thiserror::Error;

#[cfg(any(target_os = "windows", target_os = "macos"))]
use xcap::Monitor;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CaptureDisplay {
    pub(crate) id_hint: u64,
    pub(crate) origin: Point<Pixels>,
    pub(crate) width_px: u32,
    pub(crate) height_px: u32,
    pub(crate) scale_factor: f32,
    pub(crate) is_primary: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CaptureRect {
    pub(crate) x_px: u32,
    pub(crate) y_px: u32,
    pub(crate) width_px: u32,
    pub(crate) height_px: u32,
}

impl CaptureRect {
    pub(crate) fn is_empty(self) -> bool {
        self.width_px == 0 || self.height_px == 0
    }
}

#[derive(Debug, Error, PartialEq)]
pub(crate) enum CaptureError {
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    #[error("capture is unsupported on this platform")]
    UnsupportedPlatform,
    #[error("capture was cancelled")]
    Cancelled,
    #[error("capture permission was denied")]
    PermissionDenied,
    #[error("capture backend is unavailable: {0}")]
    BackendUnavailable(&'static str),
    #[error("invalid capture input: {0}")]
    InvalidInput(&'static str),
    #[error("capture failed: {0}")]
    SystemFailure(String),
}

pub(crate) fn capture_region(
    display: &CaptureDisplay,
    rect: CaptureRect,
) -> Result<ImageFrame, CaptureError> {
    if rect.is_empty() {
        return Err(CaptureError::Cancelled);
    }
    validate_capture_rect(display, rect)?;

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        let monitor = resolve_monitor(display)?;
        let image = monitor
            .capture_region(
                rect.x_px as _,
                rect.y_px as _,
                rect.width_px,
                rect.height_px,
            )
            .map_err(|err| map_capture_error(err.to_string()))?;
        image_frame_from_captured_image(image, display.scale_factor)
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = display;
        let _ = rect;
        Err(CaptureError::UnsupportedPlatform)
    }
}

fn validate_capture_rect(display: &CaptureDisplay, rect: CaptureRect) -> Result<(), CaptureError> {
    let max_x = rect
        .x_px
        .checked_add(rect.width_px)
        .ok_or(CaptureError::InvalidInput(
            "capture rect overflows display width",
        ))?;
    let max_y = rect
        .y_px
        .checked_add(rect.height_px)
        .ok_or(CaptureError::InvalidInput(
            "capture rect overflows display height",
        ))?;
    if max_x > display.width_px || max_y > display.height_px {
        return Err(CaptureError::InvalidInput(
            "capture rect exceeds display bounds",
        ));
    }
    Ok(())
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn resolve_monitor(display: &CaptureDisplay) -> Result<Monitor, CaptureError> {
    let mut monitors = Monitor::all().map_err(|err| map_capture_error(err.to_string()))?;
    if monitors.is_empty() {
        return Err(CaptureError::BackendUnavailable(
            "no displays are available",
        ));
    }

    #[cfg(target_os = "macos")]
    if let Some(index) = monitors.iter().position(|monitor| {
        monitor.id().ok().map(u64::from) == Some(display.id_hint)
            && monitor.width().ok() == Some(display.width_px)
            && monitor.height().ok() == Some(display.height_px)
    }) {
        return Ok(monitors.swap_remove(index));
    }

    if let Some(index) = unique_monitor_match(&monitors, display) {
        return Ok(monitors.swap_remove(index));
    }

    if let Ok(hint_index) = usize::try_from(display.id_hint)
        && hint_index < monitors.len()
        && monitor_matches_display(&monitors[hint_index], display)
    {
        return Ok(monitors.swap_remove(hint_index));
    }

    Err(CaptureError::BackendUnavailable(
        "failed to resolve the selected display",
    ))
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn unique_monitor_match(monitors: &[Monitor], display: &CaptureDisplay) -> Option<usize> {
    let matching = monitors
        .iter()
        .enumerate()
        .filter_map(|(index, monitor)| monitor_matches_display(monitor, display).then_some(index))
        .collect::<Vec<_>>();

    (matching.len() == 1).then(|| matching[0])
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn monitor_matches_display(monitor: &Monitor, display: &CaptureDisplay) -> bool {
    let Ok(width) = monitor.width() else {
        return false;
    };
    let Ok(height) = monitor.height() else {
        return false;
    };
    if width != display.width_px || height != display.height_px {
        return false;
    }

    let Ok(scale_factor) = monitor.scale_factor() else {
        return false;
    };
    if !approx_eq(scale_factor, display.scale_factor) {
        return false;
    }

    match monitor.is_primary() {
        Ok(is_primary) => is_primary == display.is_primary,
        Err(_) => false,
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn approx_eq(left: f32, right: f32) -> bool {
    (left - right).abs() < 0.01
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn image_frame_from_captured_image(
    image: impl CapturedImageExt,
    scale_factor: f32,
) -> Result<ImageFrame, CaptureError> {
    Ok(ImageFrame {
        width: image.width(),
        height: image.height(),
        scale_factor,
        bytes_rgba8: image.into_rgba8(),
    })
}

fn map_capture_error(message: String) -> CaptureError {
    let lowercase = message.to_ascii_lowercase();
    if lowercase.contains("permission") || lowercase.contains("denied") {
        CaptureError::PermissionDenied
    } else {
        CaptureError::SystemFailure(message)
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
trait CapturedImageExt {
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn into_rgba8(self) -> Vec<u8>;
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
impl CapturedImageExt for image::RgbaImage {
    fn width(&self) -> u32 {
        image::ImageBuffer::width(self)
    }

    fn height(&self) -> u32 {
        image::ImageBuffer::height(self)
    }

    fn into_rgba8(self) -> Vec<u8> {
        self.into_raw()
    }
}

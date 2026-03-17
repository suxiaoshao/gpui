use crate::error::CaptureError;
#[cfg(target_os = "macos")]
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
#[cfg(target_os = "macos")]
use objc2_core_graphics::{
    CGImage, CGImageAlphaInfo, CGImageByteOrderInfo, CGPreflightScreenCaptureAccess,
    CGRequestScreenCaptureAccess,
};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScreenRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl ScreenRect {
    #[must_use]
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    #[must_use]
    pub fn normalized(self) -> Self {
        let x = self.x.min(self.x + self.width);
        let y = self.y.min(self.y + self.height);
        Self {
            x,
            y,
            width: self.width.abs(),
            height: self.height.abs(),
        }
    }

    #[must_use]
    pub fn is_empty(self) -> bool {
        let rect = self.normalized();
        rect.width <= f64::EPSILON || rect.height <= f64::EPSILON
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DisplayId(pub u32);

#[derive(Clone, Debug, PartialEq)]
pub struct ImageFrame {
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
    pub bytes_rgba8: Vec<u8>,
}

pub fn capture_display(display_id: DisplayId) -> Result<ImageFrame, CaptureError> {
    capture_display_impl(display_id)
}

pub fn capture_rect(display_id: DisplayId, rect: ScreenRect) -> Result<ImageFrame, CaptureError> {
    let rect = rect.normalized();
    if rect.is_empty() {
        return Err(CaptureError::InvalidInput(
            "capture rectangle width and height must be positive",
        ));
    }

    capture_rect_impl(display_id, rect)
}

#[cfg(target_os = "macos")]
#[allow(deprecated)]
fn capture_display_impl(display_id: DisplayId) -> Result<ImageFrame, CaptureError> {
    ensure_capture_access()?;
    let image = objc2_core_graphics::CGDisplayCreateImage(display_id.0)
        .ok_or_else(|| CaptureError::SystemFailure("failed to capture display".into()))?;
    image_frame_from_cg_image(&image)
}

#[cfg(target_os = "windows")]
fn capture_display_impl(_: DisplayId) -> Result<ImageFrame, CaptureError> {
    Err(CaptureError::BackendUnavailable(
        "windows capture backend is not implemented yet",
    ))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn capture_display_impl(_: DisplayId) -> Result<ImageFrame, CaptureError> {
    Err(CaptureError::UnsupportedPlatform)
}

#[cfg(target_os = "macos")]
#[allow(deprecated)]
fn capture_rect_impl(display_id: DisplayId, rect: ScreenRect) -> Result<ImageFrame, CaptureError> {
    ensure_capture_access()?;
    let image = objc2_core_graphics::CGDisplayCreateImageForRect(display_id.0, cg_rect(rect))
        .ok_or_else(|| CaptureError::SystemFailure("failed to capture display rect".into()))?;
    image_frame_from_cg_image(&image)
}

#[cfg(target_os = "windows")]
fn capture_rect_impl(_: DisplayId, _: ScreenRect) -> Result<ImageFrame, CaptureError> {
    Err(CaptureError::BackendUnavailable(
        "windows capture backend is not implemented yet",
    ))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn capture_rect_impl(_: DisplayId, _: ScreenRect) -> Result<ImageFrame, CaptureError> {
    Err(CaptureError::UnsupportedPlatform)
}

#[cfg(target_os = "macos")]
fn ensure_capture_access() -> Result<(), CaptureError> {
    if CGPreflightScreenCaptureAccess() || CGRequestScreenCaptureAccess() {
        Ok(())
    } else {
        Err(CaptureError::PermissionDenied)
    }
}

#[cfg(target_os = "macos")]
fn cg_rect(rect: ScreenRect) -> CGRect {
    CGRect {
        origin: CGPoint {
            x: rect.x,
            y: rect.y,
        },
        size: CGSize {
            width: rect.width,
            height: rect.height,
        },
    }
}

#[cfg(target_os = "macos")]
fn image_frame_from_cg_image(image: &CGImage) -> Result<ImageFrame, CaptureError> {
    let width = CGImage::width(Some(image));
    let height = CGImage::height(Some(image));
    let bits_per_component = CGImage::bits_per_component(Some(image));
    let bits_per_pixel = CGImage::bits_per_pixel(Some(image));
    let bytes_per_row = CGImage::bytes_per_row(Some(image));
    let bitmap_info = CGImage::bitmap_info(Some(image));
    let provider = CGImage::data_provider(Some(image))
        .ok_or_else(|| CaptureError::SystemFailure("captured image has no provider".into()))?;
    let data = objc2_core_graphics::CGDataProvider::data(Some(&provider))
        .ok_or_else(|| CaptureError::SystemFailure("failed to copy captured image bytes".into()))?;
    let total_len = data.length() as usize;
    let bytes = unsafe { std::slice::from_raw_parts(data.byte_ptr(), total_len) };

    if bits_per_component != 8 || bits_per_pixel != 32 {
        return Err(CaptureError::BackendUnavailable(
            "unsupported cg image pixel format",
        ));
    }

    let required_len = bytes_per_row
        .checked_mul(height)
        .ok_or_else(|| CaptureError::SystemFailure("image row size overflowed".into()))?;
    if total_len < required_len {
        return Err(CaptureError::SystemFailure(
            "captured image bytes were truncated".into(),
        ));
    }

    let alpha_info = CGImageAlphaInfo(bitmap_info.bits() & 0x1F);
    let byte_order = bitmap_info.bits() & 0x7000;
    let is_bgra = byte_order == CGImageByteOrderInfo::Order32Little.0
        && matches!(
            alpha_info,
            CGImageAlphaInfo::PremultipliedFirst
                | CGImageAlphaInfo::First
                | CGImageAlphaInfo::NoneSkipFirst
        );
    let is_rgba = byte_order == CGImageByteOrderInfo::Order32Big.0
        && matches!(
            alpha_info,
            CGImageAlphaInfo::PremultipliedLast
                | CGImageAlphaInfo::Last
                | CGImageAlphaInfo::NoneSkipLast
        );

    if !is_bgra && !is_rgba {
        return Err(CaptureError::BackendUnavailable(
            "unsupported cg image channel order",
        ));
    }

    let mut rgba = Vec::with_capacity(width.saturating_mul(height).saturating_mul(4));
    for row in 0..height {
        let start = row * bytes_per_row;
        let row_bytes = &bytes[start..start + width * 4];
        for pixel in row_bytes.chunks_exact(4) {
            if is_bgra {
                rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
            } else {
                rgba.extend_from_slice(pixel);
            }
        }
    }

    Ok(ImageFrame {
        width: width as u32,
        height: height as u32,
        scale_factor: 1.0,
        bytes_rgba8: rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::ScreenRect;

    #[test]
    fn normalizes_negative_dimensions() {
        let rect = ScreenRect::new(16.0, 12.0, -6.0, -4.0).normalized();

        assert_eq!(rect.x, 10.0);
        assert_eq!(rect.y, 8.0);
        assert_eq!(rect.width, 6.0);
        assert_eq!(rect.height, 4.0);
    }

    #[test]
    fn empty_rect_detects_zero_dimensions() {
        assert!(ScreenRect::new(0.0, 0.0, 0.0, 5.0).is_empty());
    }
}

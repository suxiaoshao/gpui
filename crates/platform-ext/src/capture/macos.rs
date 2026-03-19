use crate::{
    CaptureError,
    capture::{DisplayId, ImageFrame, ScreenRect},
};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_core_graphics::{
    CGImage, CGImageAlphaInfo, CGImageByteOrderInfo, CGPreflightScreenCaptureAccess,
    CGRequestScreenCaptureAccess,
};

#[allow(deprecated)]
pub(super) fn capture_display(display_id: DisplayId) -> Result<ImageFrame, CaptureError> {
    ensure_capture_access()?;
    let image = objc2_core_graphics::CGDisplayCreateImage(display_id.0)
        .ok_or_else(|| CaptureError::SystemFailure("failed to capture display".into()))?;
    image_frame_from_cg_image(&image)
}

#[allow(deprecated)]
pub(super) fn capture_rect(
    display_id: DisplayId,
    rect: ScreenRect,
) -> Result<ImageFrame, CaptureError> {
    ensure_capture_access()?;
    let image = objc2_core_graphics::CGDisplayCreateImageForRect(display_id.0, cg_rect(rect))
        .ok_or_else(|| CaptureError::SystemFailure("failed to capture display rect".into()))?;
    image_frame_from_cg_image(&image)
}

fn ensure_capture_access() -> Result<(), CaptureError> {
    if CGPreflightScreenCaptureAccess() || CGRequestScreenCaptureAccess() {
        Ok(())
    } else {
        Err(CaptureError::PermissionDenied)
    }
}

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

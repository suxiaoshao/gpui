use crate::capture::{ImageFrame, ScreenRect};
use windows::{
    Graphics::Imaging::{BitmapAlphaMode, BitmapPixelFormat, SoftwareBitmap},
    Storage::Streams::Buffer,
    Win32::System::WinRT::IBufferByteAccess,
    core::Interface,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PixelRect {
    pub left: u32,
    pub top: u32,
    pub width: u32,
    pub height: u32,
}

impl PixelRect {
    fn byte_range(self, image_width: u32, row: u32) -> std::ops::Range<usize> {
        let row_start = usize::try_from(row)
            .unwrap_or(usize::MAX)
            .saturating_mul(usize::try_from(image_width).unwrap_or(usize::MAX))
            .saturating_mul(4);
        let start = row_start
            .saturating_add(usize::try_from(self.left).unwrap_or(usize::MAX).saturating_mul(4));
        let end = start
            .saturating_add(usize::try_from(self.width).unwrap_or(usize::MAX).saturating_mul(4));
        start..end
    }
}

pub(crate) fn clip_rect_to_image(
    image_width: u32,
    image_height: u32,
    rect: ScreenRect,
) -> Option<PixelRect> {
    let rect = rect.normalized();
    let left = rect.x.floor().max(0.0).min(f64::from(image_width)) as u32;
    let top = rect.y.floor().max(0.0).min(f64::from(image_height)) as u32;
    let right = (rect.x + rect.width).ceil().max(0.0).min(f64::from(image_width)) as u32;
    let bottom = (rect.y + rect.height)
        .ceil()
        .max(0.0)
        .min(f64::from(image_height)) as u32;
    if right <= left || bottom <= top {
        return None;
    }

    Some(PixelRect {
        left,
        top,
        width: right - left,
        height: bottom - top,
    })
}

pub(crate) fn crop_image_frame(image: &ImageFrame, rect: PixelRect) -> Result<ImageFrame, String> {
    let cropped_len = byte_len(rect.width, rect.height)?;
    let mut bytes = Vec::with_capacity(cropped_len);
    for row in rect.top..rect.top + rect.height {
        let range = rect.byte_range(image.width, row);
        bytes.extend_from_slice(
            image
                .bytes_rgba8
                .get(range)
                .ok_or_else(|| "cropped row was out of bounds".to_string())?,
        );
    }

    Ok(ImageFrame {
        width: rect.width,
        height: rect.height,
        scale_factor: image.scale_factor,
        bytes_rgba8: bytes,
    })
}

pub(crate) fn rgba_frame_from_software_bitmap(bitmap: &SoftwareBitmap) -> Result<ImageFrame, String> {
    let bitmap = SoftwareBitmap::ConvertWithAlpha(bitmap, BitmapPixelFormat::Rgba8, BitmapAlphaMode::Ignore)
        .map_err(|err| format!("failed to convert bitmap to rgba8: {err}"))?;
    let width = u32::try_from(bitmap.PixelWidth().map_err(|err| format!("failed to read bitmap width: {err}"))?)
        .map_err(|_| "bitmap width was negative".to_string())?;
    let height =
        u32::try_from(bitmap.PixelHeight().map_err(|err| format!("failed to read bitmap height: {err}"))?)
            .map_err(|_| "bitmap height was negative".to_string())?;
    let byte_len = byte_len(width, height)?;
    let buffer = Buffer::Create(width.saturating_mul(height).saturating_mul(4))
        .map_err(|err| format!("failed to allocate bitmap buffer: {err}"))?;
    buffer
        .SetLength(width.saturating_mul(height).saturating_mul(4))
        .map_err(|err| format!("failed to set bitmap buffer length: {err}"))?;
    bitmap
        .CopyToBuffer(&buffer)
        .map_err(|err| format!("failed to copy bitmap bytes: {err}"))?;
    let access: IBufferByteAccess = buffer
        .cast()
        .map_err(|err| format!("failed to access bitmap buffer bytes: {err}"))?;
    let ptr = unsafe { access.Buffer() }
        .map_err(|err| format!("failed to resolve bitmap buffer pointer: {err}"))?;
    let bytes = unsafe { std::slice::from_raw_parts(ptr, byte_len) }.to_vec();

    Ok(ImageFrame {
        width,
        height,
        scale_factor: 1.0,
        bytes_rgba8: bytes,
    })
}

pub(crate) fn software_bitmap_from_image_frame(image: &ImageFrame) -> Result<SoftwareBitmap, String> {
    let buffer = Buffer::Create(
        u32::try_from(image.bytes_rgba8.len()).map_err(|_| "image buffer was too large".to_string())?,
    )
    .map_err(|err| format!("failed to allocate source buffer: {err}"))?;
    buffer
        .SetLength(u32::try_from(image.bytes_rgba8.len()).map_err(|_| "image buffer was too large".to_string())?)
        .map_err(|err| format!("failed to set source buffer length: {err}"))?;
    let access: IBufferByteAccess = buffer
        .cast()
        .map_err(|err| format!("failed to access source buffer bytes: {err}"))?;
    let ptr = unsafe { access.Buffer() }
        .map_err(|err| format!("failed to resolve source buffer pointer: {err}"))?;
    unsafe {
        std::ptr::copy_nonoverlapping(image.bytes_rgba8.as_ptr(), ptr, image.bytes_rgba8.len());
    }

    SoftwareBitmap::CreateCopyWithAlphaFromBuffer(
        &buffer,
        BitmapPixelFormat::Rgba8,
        i32::try_from(image.width).map_err(|_| "image width was too large".to_string())?,
        i32::try_from(image.height).map_err(|_| "image height was too large".to_string())?,
        BitmapAlphaMode::Ignore,
    )
    .map_err(|err| format!("failed to create software bitmap from rgba bytes: {err}"))
}

fn byte_len(width: u32, height: u32) -> Result<usize, String> {
    usize::try_from(width)
        .map_err(|_| "image width was too large".to_string())?
        .checked_mul(usize::try_from(height).map_err(|_| "image height was too large".to_string())?)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| "image byte length overflowed".to_string())
}

#[cfg(test)]
mod tests {
    use super::{PixelRect, clip_rect_to_image};
    use crate::capture::ScreenRect;

    #[test]
    fn clip_rect_intersects_with_image_bounds() {
        let clipped = clip_rect_to_image(100, 80, ScreenRect::new(-10.0, 8.0, 25.2, 15.1))
            .expect("expected clipped rect");

        assert_eq!(
            clipped,
            PixelRect {
                left: 0,
                top: 8,
                width: 16,
                height: 16,
            }
        );
    }

    #[test]
    fn clip_rect_returns_none_when_outside_image() {
        assert!(clip_rect_to_image(32, 32, ScreenRect::new(48.0, 2.0, 4.0, 4.0)).is_none());
    }
}

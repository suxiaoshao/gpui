use crate::capture::ImageFrame;
use windows::{
    Graphics::Imaging::{BitmapAlphaMode, BitmapPixelFormat, SoftwareBitmap},
    Storage::Streams::Buffer,
    Win32::System::WinRT::IBufferByteAccess,
    core::Interface,
};

pub(crate) fn software_bitmap_from_image_frame(
    image: &ImageFrame,
) -> Result<SoftwareBitmap, String> {
    let buffer_len = u32::try_from(image.bytes_rgba8.len())
        .map_err(|_| "image buffer was too large".to_string())?;
    let buffer = Buffer::Create(buffer_len)
        .map_err(|err| format!("failed to allocate source buffer: {err}"))?;
    buffer
        .SetLength(buffer_len)
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

#[derive(Clone, Debug, PartialEq)]
pub struct ImageFrame {
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
    pub bytes_rgba8: Vec<u8>,
}

pub(crate) fn decode_image_bytes(bytes: &[u8]) -> Result<ImageFrame, String> {
    let image = image::load_from_memory(bytes)
        .map_err(|err| format!("failed to decode screenshot image: {err}"))?;
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();

    Ok(ImageFrame {
        width,
        height,
        scale_factor: 1.0,
        bytes_rgba8: rgba.into_raw(),
    })
}

pub(crate) fn decode_image_file(path: &std::path::Path) -> Result<ImageFrame, String> {
    let bytes = std::fs::read(path)
        .map_err(|err| format!("failed to read screenshot image {}: {err}", path.display()))?;
    decode_image_bytes(&bytes)
}

#[cfg(test)]
mod tests {
    use super::decode_image_bytes;

    #[test]
    fn decodes_png_into_rgba_frame() {
        let png = [
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9C, 0x63, 0xF8, 0xCF, 0xC0, 0xF0, 0x1F, 0x00, 0x05, 0x00, 0x01, 0xFF, 0x89, 0x99,
            0x3D, 0x1D, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];

        let frame = decode_image_bytes(&png).expect("png should decode");

        assert_eq!(frame.width, 1);
        assert_eq!(frame.height, 1);
        assert_eq!(frame.bytes_rgba8, vec![255, 0, 0, 255]);
    }
}

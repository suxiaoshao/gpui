use base64::{Engine as _, engine::general_purpose::STANDARD};
use gpui_kit::{Image, ImageFormat};
use std::{io::Cursor, path::PathBuf, sync::Arc};

#[derive(Clone)]
pub(crate) struct Attachment {
    pub id: String,
    pub name: String,
    pub content: Content,
}
#[derive(Clone)]
pub(crate) enum Content {
    File {
        path: PathBuf,
        byte_len: u64,
    },
    Image {
        image: pi_rpc::protocol::Image,
        preview: Arc<Image>,
    },
}
impl Attachment {
    pub fn file(path: PathBuf, byte_len: u64) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            content: Content::File { path, byte_len },
        }
    }
    pub fn byte_len(&self) -> u64 {
        match &self.content {
            Content::File { byte_len, .. } => *byte_len,
            Content::Image { preview, .. } => preview.bytes().len() as u64,
        }
    }
    pub fn format_name(&self) -> Option<String> {
        match &self.content {
            Content::File { path, .. } => path
                .extension()
                .filter(|extension| !extension.is_empty())
                .map(|extension| extension.to_string_lossy().to_uppercase()),
            Content::Image { image, .. } => {
                Some(image.mime_type.trim_start_matches("image/").to_uppercase())
            }
        }
    }
    pub fn from_image(name: String, bytes: &[u8]) -> Result<Self, String> {
        let format = image::guess_format(bytes).map_err(|e| e.to_string())?;
        let preview_format = match format {
            image::ImageFormat::Png => ImageFormat::Png,
            image::ImageFormat::Jpeg => ImageFormat::Jpeg,
            image::ImageFormat::Gif => ImageFormat::Gif,
            image::ImageFormat::WebP => ImageFormat::Webp,
            image::ImageFormat::Bmp => ImageFormat::Bmp,
            _ => return Err("Unsupported image format".into()),
        };
        // Read dimensions to validate the container without transforming its pixels.
        image::ImageReader::with_format(Cursor::new(bytes), format)
            .into_dimensions()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            content: Content::Image {
                image: pi_rpc::protocol::Image {
                    data: STANDARD.encode(bytes),
                    mime_type: format.to_mime_type().into(),
                },
                preview: Arc::new(Image::from_bytes(preview_format, bytes.to_vec())),
            },
        })
    }
}
pub(crate) fn from_paths(paths: Vec<PathBuf>) -> Result<Vec<Attachment>, String> {
    paths
        .into_iter()
        .map(|path| {
            let path = std::path::absolute(path).map_err(|e| e.to_string())?;
            let metadata = std::fs::metadata(&path).map_err(|e| e.to_string())?;
            if !metadata.is_file() {
                return Err(format!("Not a file: {}", path.display()));
            }
            let attachment = Attachment::file(path.clone(), metadata.len());
            let extension = path
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_ascii_lowercase();
            if matches!(
                extension.as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp"
            ) {
                let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
                Attachment::from_image(attachment.name, &bytes)
            } else {
                Ok(attachment)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn file_and_image_attachments_preserve_original_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a b中文.pdf");
        std::fs::write(&path, [0, 255, 127]).unwrap();
        let items = from_paths(vec![path.clone()]).unwrap();
        assert!(matches!(&items[0].content,Content::File { path: p, .. } if p==&path));
        assert_eq!(std::fs::read(&path).unwrap(), vec![0, 255, 127]);
        assert_eq!(items[0].byte_len(), 3);
        assert_eq!(items[0].format_name().as_deref(), Some("PDF"));
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(2400, 1200)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let image = Attachment::from_image("test".into(), bytes.get_ref()).unwrap();
        assert_eq!(image.byte_len(), bytes.get_ref().len() as u64);
        assert_eq!(image.format_name().as_deref(), Some("PNG"));
        let Content::Image { image, preview } = image.content else {
            panic!()
        };
        let payload = STANDARD.decode(image.data).unwrap();
        assert_eq!(&payload, bytes.get_ref());
        assert_eq!(preview.bytes(), bytes.get_ref());
        let decoded = image::load_from_memory(&payload).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (2400, 1200));
        assert_eq!(image.mime_type, "image/png");
    }
}

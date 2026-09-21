use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::{
    io::{Cursor, Read},
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Clone)]
pub(crate) struct Attachment {
    pub id: String,
    pub name: String,
    pub content: Content,
}
#[derive(Clone)]
pub(crate) enum Content {
    File { path: PathBuf, byte_len: u64 },
    Image { image: Arc<ImageAttachment> },
}

/// An immutable original shared by the draft, send task and open preview.
/// The temporary file is removed when its last owner releases it.
pub(crate) struct ImageAttachment {
    file: tempfile::TempPath,
    format: image::ImageFormat,
    dimensions: (u32, u32),
    byte_len: u64,
}
impl ImageAttachment {
    fn from_reader(mut reader: impl Read) -> Result<Self, String> {
        let mut file = tempfile::Builder::new()
            .prefix("gupi-attachment-")
            .tempfile()
            .map_err(|e| e.to_string())?;
        let byte_len = std::io::copy(&mut reader, &mut file).map_err(|e| e.to_string())?;
        let decoder = image::ImageReader::open(file.path())
            .map_err(|e| e.to_string())?
            .with_guessed_format()
            .map_err(|e| e.to_string())?;
        let format = decoder.format().ok_or("Unsupported image format")?;
        if !matches!(
            format,
            image::ImageFormat::Png
                | image::ImageFormat::Jpeg
                | image::ImageFormat::Gif
                | image::ImageFormat::WebP
                | image::ImageFormat::Bmp
        ) {
            return Err("Unsupported image format".into());
        }
        let dimensions = decoder.into_dimensions().map_err(|e| e.to_string())?;
        Ok(Self {
            file: file.into_temp_path(),
            format,
            dimensions,
            byte_len,
        })
    }
    pub fn path(&self) -> &Path {
        &self.file
    }
    pub fn dimensions(&self) -> (u32, u32) {
        self.dimensions
    }
    /// Read and encode on the sending worker, never while rendering the draft.
    pub fn to_rpc_image(&self) -> Result<pi_rpc::protocol::Image, String> {
        let bytes = std::fs::read(self.path()).map_err(|e| e.to_string())?;
        Ok(pi_rpc::protocol::Image {
            data: STANDARD.encode(bytes),
            mime_type: self.format.to_mime_type().into(),
        })
    }
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
            Content::Image { image } => image.byte_len,
        }
    }
    pub fn format_name(&self) -> Option<String> {
        match &self.content {
            Content::File { path, .. } => path
                .extension()
                .filter(|extension| !extension.is_empty())
                .map(|extension| extension.to_string_lossy().to_uppercase()),
            Content::Image { image, .. } => Some(
                image
                    .format
                    .to_mime_type()
                    .trim_start_matches("image/")
                    .to_uppercase(),
            ),
        }
    }
    pub fn from_image(name: String, bytes: &[u8]) -> Result<Self, String> {
        Self::from_image_reader(name, Cursor::new(bytes))
    }
    fn from_image_reader(name: String, reader: impl Read) -> Result<Self, String> {
        Ok(Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            content: Content::Image {
                image: Arc::new(ImageAttachment::from_reader(reader)?),
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
                let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
                Attachment::from_image_reader(attachment.name, file)
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
        let Content::Image { image } = image.content else {
            panic!()
        };
        let wire = image.to_rpc_image().unwrap();
        let payload = STANDARD.decode(wire.data).unwrap();
        assert_eq!(&payload, bytes.get_ref());
        assert_eq!(&std::fs::read(image.path()).unwrap(), bytes.get_ref());
        assert_eq!(image.dimensions(), (2400, 1200));
        let decoded = image::load_from_memory(&payload).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (2400, 1200));
        assert_eq!(wire.mime_type, "image/png");
    }

    #[test]
    fn image_file_is_shared_until_the_last_owner_releases_it() {
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(2, 2)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let attachment = Attachment::from_image("clipboard.png".into(), bytes.get_ref()).unwrap();
        let sending = attachment.clone();
        let Content::Image { image: preview } = attachment.content.clone() else {
            panic!()
        };
        let path = preview.path().to_owned();
        drop(attachment);
        assert!(path.exists());
        drop(sending);
        assert_eq!(std::fs::read(&path).unwrap(), *bytes.get_ref());
        drop(preview);
        assert!(!path.exists());
    }

    #[test]
    fn selected_image_uses_a_snapshot_without_owning_the_source_file() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("original.png");
        image::DynamicImage::new_rgb8(3, 2).save(&source).unwrap();
        let original = std::fs::read(&source).unwrap();
        let mut attachments = from_paths(vec![source.clone()]).unwrap();
        let Content::Image { image } = attachments.pop().unwrap().content else {
            panic!()
        };
        let snapshot = image.path().to_owned();
        assert_ne!(snapshot, source);
        std::fs::write(&source, "edited after attachment").unwrap();
        assert_eq!(
            STANDARD.decode(image.to_rpc_image().unwrap().data).unwrap(),
            original
        );
        drop(image);
        assert!(!snapshot.exists());
        assert_eq!(
            std::fs::read_to_string(&source).unwrap(),
            "edited after attachment"
        );
    }
}

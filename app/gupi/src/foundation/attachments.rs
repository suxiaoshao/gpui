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
            let attachment = Attachment::file(path.clone(), metadata.len());
            // Directories are path references even when named like an image.
            if metadata.is_file() && is_image_path(&path) {
                let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
                Attachment::from_image_reader(attachment.name, file)
            } else {
                Ok(attachment)
            }
        })
        .collect()
}

pub(crate) fn is_image_path(path: &Path) -> bool {
    matches!(
        path.extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_ascii_lowercase()
            .as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp"
    )
}

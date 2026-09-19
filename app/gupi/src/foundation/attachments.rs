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
    File(PathBuf),
    Image {
        image: pi_rpc::protocol::Image,
        preview: Arc<Image>,
    },
}
impl Attachment {
    pub fn file(path: PathBuf) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            content: Content::File(path),
        }
    }
    pub fn from_image(name: String, bytes: &[u8]) -> Result<Self, String> {
        let mut reader = image::ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|e| e.to_string())?;
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(128 * 1024 * 1024);
        reader.limits(limits);
        let decoded = reader.decode().map_err(|e| e.to_string())?;
        let mut resized = decoded.thumbnail(2000, 2000);
        let mut encoded = Cursor::new(Vec::new());
        resized
            .write_to(&mut encoded, image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        let mut format = ImageFormat::Png;
        let mut mime = "image/png";
        // Pi's default inline budget is 4.5 MiB after base64 encoding.
        let budget = 9 * 1024 * 1024 / 2;
        if encoded.get_ref().len().div_ceil(3) * 4 >= budget {
            format = ImageFormat::Jpeg;
            mime = "image/jpeg";
            for edge in [2000, 1500, 1000, 750] {
                resized = resized.thumbnail(edge, edge);
                encoded = Cursor::new(Vec::new());
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, 85)
                    .encode_image(&resized.to_rgb8())
                    .map_err(|e| e.to_string())?;
                if encoded.get_ref().len().div_ceil(3) * 4 < budget {
                    break;
                }
            }
        }
        if encoded.get_ref().len().div_ceil(3) * 4 >= budget {
            return Err("Image exceeds inline size limit".into());
        }
        let bytes = encoded.into_inner();
        Ok(Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            content: Content::Image {
                image: pi_rpc::protocol::Image {
                    data: STANDARD.encode(&bytes),
                    mime_type: mime.into(),
                },
                preview: Arc::new(Image::from_bytes(format, bytes)),
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
            let attachment = Attachment::file(path.clone());
            let extension = path
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_ascii_lowercase();
            if matches!(
                extension.as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp"
            ) {
                if metadata.len() > 64 * 1024 * 1024 {
                    return Err("Image source exceeds 64 MiB".into());
                }
                let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
                Attachment::from_image(attachment.name, &bytes)
            } else {
                Ok(attachment)
            }
        })
        .collect()
}

pub(crate) fn check_image_policy(cwd: &std::path::Path) -> Result<(), String> {
    let agent = super::pi_resources::agent_dir().map_err(|e| e.to_string())?;
    let mut blocked = false;
    for path in [agent.join("settings.json"), cwd.join(".pi/settings.json")] {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.to_string()),
        };
        let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if let Some(value) = value
            .pointer("/images/blockImages")
            .and_then(|v| v.as_bool())
        {
            blocked = value;
        }
    }
    if blocked {
        Err("Pi images.blockImages is enabled".into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn file_references_preserve_original_bytes_and_image_payload_is_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a b中文.pdf");
        std::fs::write(&path, [0, 255, 127]).unwrap();
        let items = from_paths(vec![path.clone()]).unwrap();
        assert!(matches!(&items[0].content,Content::File(p) if p==&path));
        assert_eq!(std::fs::read(&path).unwrap(), vec![0, 255, 127]);
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(2400, 1200)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let image = Attachment::from_image("test".into(), bytes.get_ref()).unwrap();
        let Content::Image { image, .. } = image.content else {
            panic!()
        };
        let decoded = image::load_from_memory(&STANDARD.decode(image.data).unwrap()).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (2000, 1000));
        assert_eq!(image.mime_type, "image/png");
    }
}

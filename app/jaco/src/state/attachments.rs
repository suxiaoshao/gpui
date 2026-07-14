use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use gpui::{App, ClipboardEntry, ClipboardItem, Image, ImageFormat};
use jaco_core::{
    AttachmentKind, AttachmentMetadata, AttachmentSource, AttachmentStorageKind, ConversationId,
};
use jaco_db::NewAttachment;
use tracing::{Level, event};

use crate::{
    errors::{JacoError, JacoResult},
    state::config,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ComposerAttachmentKind {
    Image,
    File,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ComposerAttachmentSource {
    LocalFile { path: PathBuf },
    GeneratedImage { image: Arc<Image> },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ComposerAttachment {
    pub(crate) local_id: u64,
    pub(crate) kind: ComposerAttachmentKind,
    pub(crate) source: ComposerAttachmentSource,
    pub(crate) name: String,
    pub(crate) mime_type: Option<String>,
    pub(crate) size_bytes: Option<u64>,
    pub(crate) width: Option<u32>,
    pub(crate) height: Option<u32>,
}

impl ComposerAttachment {
    pub(crate) fn local_file_path(&self) -> Option<&Path> {
        match &self.source {
            ComposerAttachmentSource::LocalFile { path } => Some(path),
            ComposerAttachmentSource::GeneratedImage { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RejectedAttachment {
    pub(crate) label: String,
    pub(crate) reason: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct AttachmentAddResult {
    pub(crate) attachments: Vec<ComposerAttachment>,
    pub(crate) rejected: Vec<RejectedAttachment>,
}

#[derive(Debug, Default)]
pub(crate) struct PreparedMessageAttachments {
    pub(crate) new_attachments: Vec<NewAttachment>,
    pub(crate) stored_paths: Vec<PathBuf>,
}

pub(crate) fn clipboard_item_has_attachments(item: &ClipboardItem) -> bool {
    item.entries().iter().any(|entry| {
        matches!(
            entry,
            ClipboardEntry::ExternalPaths(_) | ClipboardEntry::Image(_)
        )
    })
}

pub(crate) fn add_attachments_from_clipboard(
    item: ClipboardItem,
    next_local_id: &mut u64,
) -> JacoResult<AttachmentAddResult> {
    if let Some(paths) = item.entries().iter().find_map(|entry| match entry {
        ClipboardEntry::ExternalPaths(paths) => Some(paths.paths().to_vec()),
        ClipboardEntry::String(_) | ClipboardEntry::Image(_) => None,
    }) {
        return add_attachments_from_paths(paths, next_local_id);
    }

    if let Some(image) = item.entries().iter().find_map(|entry| match entry {
        ClipboardEntry::Image(image) => Some(image),
        ClipboardEntry::String(_) | ClipboardEntry::ExternalPaths(_) => None,
    }) {
        return add_attachment_from_clipboard_image(image, next_local_id);
    }

    Ok(AttachmentAddResult::default())
}

pub(crate) fn add_attachments_from_paths(
    paths: Vec<PathBuf>,
    next_local_id: &mut u64,
) -> JacoResult<AttachmentAddResult> {
    let mut result = AttachmentAddResult::default();
    for path in paths {
        match composer_attachment_from_path(path, next_local_id) {
            Ok(attachment) => result.attachments.push(attachment),
            Err(rejected) => result.rejected.push(rejected),
        }
    }
    Ok(result)
}

pub(crate) fn prepare_message_attachments(
    conversation_id: &ConversationId,
    storage_prefix: &str,
    attachments: &[ComposerAttachment],
    cx: &App,
) -> JacoResult<PreparedMessageAttachments> {
    if attachments.is_empty() {
        return Ok(PreparedMessageAttachments::default());
    }

    let attachment_dir = attachment_store_dir(conversation_id, cx)?;
    fs::create_dir_all(&attachment_dir)?;

    let mut prepared = PreparedMessageAttachments::default();
    for attachment in attachments {
        let stored_path = stored_attachment_path(&attachment_dir, storage_prefix, attachment);
        if let Err(err) = write_stored_attachment(attachment, &stored_path) {
            cleanup_stored_attachment_files(&prepared.stored_paths);
            let _ = fs::remove_file(&stored_path);
            return Err(err.into());
        }
        prepared.stored_paths.push(stored_path.clone());
        let path_string = stored_path.to_string_lossy().to_string();
        let kind = match attachment.kind {
            ComposerAttachmentKind::Image => AttachmentKind::Image,
            ComposerAttachmentKind::File => AttachmentKind::File,
        };
        prepared.new_attachments.push(NewAttachment {
            conversation_id: conversation_id.clone(),
            kind,
            storage_kind: AttachmentStorageKind::LocalFile,
            mime_type: attachment.mime_type.clone(),
            name: Some(attachment.name.clone()),
            path: Some(path_string.clone()),
            external_uri: None,
            provider_id: None,
            provider_file_id: None,
            sha256: None,
            size_bytes: attachment
                .size_bytes
                .and_then(|size| i64::try_from(size).ok()),
            metadata: AttachmentMetadata {
                source: attachment_source_for_record(attachment, path_string),
                width: attachment.width,
                height: attachment.height,
                duration_ms: None,
                preview_attachment_id: None,
            },
        });
    }

    Ok(prepared)
}

pub(crate) fn generated_image_attachment(
    name: String,
    image: Image,
    mime_type: String,
    dimensions: (u32, u32),
    local_id: u64,
) -> ComposerAttachment {
    let size_bytes = image.bytes().len() as u64;
    ComposerAttachment {
        local_id,
        kind: ComposerAttachmentKind::Image,
        source: ComposerAttachmentSource::GeneratedImage {
            image: Arc::new(image),
        },
        name,
        mime_type: Some(mime_type),
        size_bytes: Some(size_bytes),
        width: Some(dimensions.0),
        height: Some(dimensions.1),
    }
}

pub(crate) fn cleanup_stored_attachment_files(paths: &[PathBuf]) {
    for path in paths {
        if let Err(err) = fs::remove_file(path)
            && path.exists()
        {
            event!(
                Level::WARN,
                path = %path.display(),
                error = %err,
                "failed to clean up prepared attachment file"
            );
        }
    }
}

pub(crate) fn model_support_issue(
    attachments: &[ComposerAttachment],
    capabilities: Option<&jaco_core::ModelCapabilitiesSnapshot>,
) -> Option<ModelAttachmentSupportIssue> {
    let capabilities = capabilities?;
    let image_count = attachments
        .iter()
        .filter(|attachment| attachment.kind == ComposerAttachmentKind::Image)
        .count();
    let file_count = attachments
        .iter()
        .filter(|attachment| attachment.kind == ComposerAttachmentKind::File)
        .count();

    if image_count > 0 {
        let Some(image_input) = capabilities.image_input.as_ref() else {
            return Some(ModelAttachmentSupportIssue::ImagesUnsupported);
        };
        if image_input
            .max_images
            .is_some_and(|max_images| image_count > max_images)
        {
            return Some(ModelAttachmentSupportIssue::TooManyImages {
                max_images: image_input.max_images.unwrap_or_default(),
            });
        }
    }

    if file_count > 0 {
        let Some(file_input) = capabilities.file_input.as_ref() else {
            return Some(ModelAttachmentSupportIssue::FilesUnsupported);
        };
        if file_input
            .max_files
            .is_some_and(|max_files| file_count > max_files)
        {
            return Some(ModelAttachmentSupportIssue::TooManyFiles {
                max_files: file_input.max_files.unwrap_or_default(),
            });
        }
        if let Some(attachment) = attachments
            .iter()
            .find(|attachment| !file_attachment_model_supported(attachment))
        {
            return Some(ModelAttachmentSupportIssue::UnsupportedFileType {
                name: attachment.name.clone(),
            });
        }
    }

    None
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ModelAttachmentSupportIssue {
    ImagesUnsupported,
    FilesUnsupported,
    TooManyImages { max_images: usize },
    TooManyFiles { max_files: usize },
    UnsupportedFileType { name: String },
}

fn add_attachment_from_clipboard_image(
    image: &Image,
    next_local_id: &mut u64,
) -> JacoResult<AttachmentAddResult> {
    let Some((extension, mime_type)) = clipboard_image_format(image.format()) else {
        return Ok(AttachmentAddResult {
            attachments: Vec::new(),
            rejected: vec![RejectedAttachment {
                label: "clipboard image".to_string(),
                reason: "unsupported clipboard image format".to_string(),
            }],
        });
    };
    let (width, height) = image_dimensions_from_bytes(image.bytes(), image.format())
        .map_err(|err| JacoError::Attachment(format!("decode clipboard image failed: {err}")))?;
    let local_id = allocate_local_id(next_local_id);
    let name = format!("clipboard-image-{local_id}.{extension}");
    let attachment = generated_image_attachment(
        name.clone(),
        image.clone(),
        mime_type.to_string(),
        (width, height),
        local_id,
    );

    Ok(AttachmentAddResult {
        attachments: vec![attachment],
        rejected: Vec::new(),
    })
}

fn composer_attachment_from_path(
    path: PathBuf,
    next_local_id: &mut u64,
) -> Result<ComposerAttachment, RejectedAttachment> {
    if !path.is_file() {
        return Err(RejectedAttachment {
            label: display_label(&path),
            reason: "not a regular file".to_string(),
        });
    }
    let metadata = fs::metadata(&path).map_err(|err| RejectedAttachment {
        label: display_label(&path),
        reason: err.to_string(),
    })?;
    let name = file_name(&path);
    let local_id = allocate_local_id(next_local_id);
    if let Some((mime_type, width, height)) = classify_image_path(&path) {
        return Ok(ComposerAttachment {
            local_id,
            kind: ComposerAttachmentKind::Image,
            source: ComposerAttachmentSource::LocalFile { path },
            name,
            mime_type: Some(mime_type.to_string()),
            size_bytes: Some(metadata.len()),
            width: Some(width),
            height: Some(height),
        });
    }

    Ok(ComposerAttachment {
        local_id,
        kind: ComposerAttachmentKind::File,
        mime_type: mime_type_for_path(&path).map(str::to_string),
        source: ComposerAttachmentSource::LocalFile { path },
        name,
        size_bytes: Some(metadata.len()),
        width: None,
        height: None,
    })
}

fn classify_image_path(path: &Path) -> Option<(&'static str, u32, u32)> {
    let mime_type = image_mime_type_for_extension(path.extension()?)?;
    let (width, height) = image::image_dimensions(path).ok()?;
    Some((mime_type, width, height))
}

fn image_dimensions_from_bytes(
    bytes: &[u8],
    format: ImageFormat,
) -> image::ImageResult<(u32, u32)> {
    use image::GenericImageView as _;

    let image_format = match format {
        ImageFormat::Png => image::ImageFormat::Png,
        ImageFormat::Jpeg => image::ImageFormat::Jpeg,
        ImageFormat::Webp => image::ImageFormat::WebP,
        ImageFormat::Gif => image::ImageFormat::Gif,
        ImageFormat::Svg
        | ImageFormat::Bmp
        | ImageFormat::Tiff
        | ImageFormat::Ico
        | ImageFormat::Pnm => image::ImageFormat::Png,
    };
    let image = image::load_from_memory_with_format(bytes, image_format)?;
    Ok(image.dimensions())
}

fn image_mime_type_for_extension(extension: &OsStr) -> Option<&'static str> {
    match extension.to_string_lossy().to_ascii_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

fn mime_type_for_path(path: &Path) -> Option<&'static str> {
    match path
        .extension()?
        .to_string_lossy()
        .to_ascii_lowercase()
        .as_str()
    {
        "txt" | "text" | "rs" => Some("text/plain"),
        "md" | "markdown" => Some("text/markdown"),
        "toml" => Some("application/toml"),
        "json" => Some("application/json"),
        "yaml" | "yml" => Some("application/yaml"),
        "pdf" => Some("application/pdf"),
        "csv" => Some("text/csv"),
        "html" | "htm" => Some("text/html"),
        "css" => Some("text/css"),
        "rtf" => Some("text/rtf"),
        "xml" => Some("text/xml"),
        "js" | "mjs" | "cjs" => Some("application/x-javascript"),
        "py" => Some("application/x-python"),
        "zip" => Some("application/zip"),
        "doc" => Some("application/msword"),
        "docx" => Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
        "xls" => Some("application/vnd.ms-excel"),
        "xlsx" => Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
        "ppt" => Some("application/vnd.ms-powerpoint"),
        "pptx" => Some("application/vnd.openxmlformats-officedocument.presentationml.presentation"),
        _ => None,
    }
}

fn file_attachment_model_supported(attachment: &ComposerAttachment) -> bool {
    if attachment.kind != ComposerAttachmentKind::File {
        return true;
    }
    attachment
        .mime_type
        .as_deref()
        .is_some_and(file_mime_type_model_supported)
        || attachment
            .local_file_path()
            .and_then(Path::extension)
            .and_then(OsStr::to_str)
            .is_some_and(file_extension_model_supported)
}

fn file_mime_type_model_supported(mime_type: &str) -> bool {
    mime_type == "application/pdf"
        || mime_type.starts_with("text/")
        || matches!(
            mime_type,
            "application/json"
                | "application/toml"
                | "application/yaml"
                | "application/xml"
                | "application/x-javascript"
                | "application/x-python"
        )
}

fn file_extension_model_supported(extension: &str) -> bool {
    matches!(
        extension.to_ascii_lowercase().as_str(),
        "pdf"
            | "txt"
            | "text"
            | "rtf"
            | "html"
            | "htm"
            | "css"
            | "md"
            | "markdown"
            | "csv"
            | "xml"
            | "js"
            | "mjs"
            | "cjs"
            | "py"
            | "rs"
            | "toml"
            | "json"
            | "yaml"
            | "yml"
    )
}

fn clipboard_image_format(format: ImageFormat) -> Option<(&'static str, &'static str)> {
    match format {
        ImageFormat::Png => Some(("png", "image/png")),
        ImageFormat::Jpeg => Some(("jpg", "image/jpeg")),
        ImageFormat::Gif => Some(("gif", "image/gif")),
        ImageFormat::Webp => Some(("webp", "image/webp")),
        ImageFormat::Svg
        | ImageFormat::Bmp
        | ImageFormat::Tiff
        | ImageFormat::Ico
        | ImageFormat::Pnm => None,
    }
}

fn attachment_store_dir(conversation_id: &ConversationId, cx: &App) -> JacoResult<PathBuf> {
    Ok(config::data_dir(cx)?
        .join("attachments")
        .join(conversation_id))
}

fn stored_attachment_path(
    attachment_dir: &Path,
    trigger_entry_id: &str,
    attachment: &ComposerAttachment,
) -> PathBuf {
    attachment_dir.join(format!(
        "{}-{}-{}",
        trigger_entry_id,
        attachment.local_id,
        sanitize_file_name(&attachment.name)
    ))
}

fn write_stored_attachment(
    attachment: &ComposerAttachment,
    stored_path: &Path,
) -> std::io::Result<()> {
    match &attachment.source {
        ComposerAttachmentSource::LocalFile { path } => fs::copy(path, stored_path).map(|_| ()),
        ComposerAttachmentSource::GeneratedImage { image } => fs::write(stored_path, image.bytes()),
    }
}

fn attachment_source_for_record(attachment: &ComposerAttachment, path: String) -> AttachmentSource {
    match &attachment.source {
        ComposerAttachmentSource::LocalFile { .. } => AttachmentSource::LocalFile { path },
        ComposerAttachmentSource::GeneratedImage { .. } => AttachmentSource::GeneratedFile { path },
    }
}

fn sanitize_file_name(name: &str) -> String {
    let sanitized = name
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '\0' => '-',
            _ => ch,
        })
        .collect::<String>();
    if sanitized.trim().is_empty() {
        "attachment".to_string()
    } else {
        sanitized
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(OsStr::to_str)
        .map(str::to_string)
        .unwrap_or_else(|| display_label(path))
}

fn display_label(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn allocate_local_id(next_local_id: &mut u64) -> u64 {
    let id = *next_local_id;
    *next_local_id = next_local_id.saturating_add(1);
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_common_decodable_image_extensions() {
        assert_eq!(
            image_mime_type_for_extension(OsStr::new("png")),
            Some("image/png")
        );
        assert_eq!(
            image_mime_type_for_extension(OsStr::new("JPG")),
            Some("image/jpeg")
        );
        assert_eq!(image_mime_type_for_extension(OsStr::new("svg")), None);
    }

    #[test]
    fn sanitizes_stored_file_names() {
        assert_eq!(sanitize_file_name("a/b:c\\d.txt"), "a-b-c-d.txt");
        assert_eq!(sanitize_file_name(""), "attachment");
    }

    #[test]
    fn detects_model_attachment_support() {
        let image = ComposerAttachment {
            local_id: 1,
            kind: ComposerAttachmentKind::Image,
            source: ComposerAttachmentSource::LocalFile {
                path: PathBuf::from("/tmp/a.png"),
            },
            name: "a.png".to_string(),
            mime_type: Some("image/png".to_string()),
            size_bytes: Some(1),
            width: Some(1),
            height: Some(1),
        };
        let capabilities = jaco_core::ModelCapabilitiesSnapshot {
            text_input: true,
            text_output: true,
            streaming: true,
            image_input: None,
            file_input: None,
            audio_input: false,
            image_generation: false,
            tool_calling: None,
            hosted_web_search: false,
            remote_mcp: false,
            reasoning: None,
            structured_output: false,
            stateful_response_continuation: false,
            extension: jaco_core::ProviderCapabilityExtensionSnapshot::None,
        };

        assert_eq!(
            model_support_issue(&[image], Some(&capabilities)),
            Some(ModelAttachmentSupportIssue::ImagesUnsupported)
        );
    }

    #[test]
    fn rejects_currently_unsupported_file_types_even_with_file_capability() {
        let zip = ComposerAttachment {
            local_id: 1,
            kind: ComposerAttachmentKind::File,
            source: ComposerAttachmentSource::LocalFile {
                path: PathBuf::from("/tmp/archive.zip"),
            },
            name: "archive.zip".to_string(),
            mime_type: Some("application/zip".to_string()),
            size_bytes: Some(1),
            width: None,
            height: None,
        };
        let capabilities = jaco_core::ModelCapabilitiesSnapshot {
            text_input: true,
            text_output: true,
            streaming: true,
            image_input: None,
            file_input: Some(jaco_core::FileInputCapabilitySnapshot { max_files: Some(4) }),
            audio_input: false,
            image_generation: false,
            tool_calling: None,
            hosted_web_search: false,
            remote_mcp: false,
            reasoning: None,
            structured_output: false,
            stateful_response_continuation: false,
            extension: jaco_core::ProviderCapabilityExtensionSnapshot::None,
        };

        assert_eq!(
            model_support_issue(&[zip], Some(&capabilities)),
            Some(ModelAttachmentSupportIssue::UnsupportedFileType {
                name: "archive.zip".to_string()
            })
        );
    }

    #[test]
    fn generated_image_attachment_keeps_bytes_in_memory() {
        let bytes = vec![1, 2, 3, 4];
        let attachment = generated_image_attachment(
            "clipboard.png".to_string(),
            Image::from_bytes(ImageFormat::Png, bytes.clone()),
            "image/png".to_string(),
            (2, 2),
            7,
        );

        assert_eq!(attachment.local_id, 7);
        assert_eq!(attachment.kind, ComposerAttachmentKind::Image);
        assert_eq!(attachment.local_file_path(), None);
        let ComposerAttachmentSource::GeneratedImage { image } = attachment.source else {
            panic!("generated image attachment should keep an in-memory image");
        };
        assert_eq!(image.bytes(), bytes);
    }

    #[gpui::test]
    fn prepare_message_attachments_writes_generated_image_to_conversation_store(
        cx: &mut gpui::TestAppContext,
    ) {
        use crate::state::{JacoConfig, config};

        let temp_root = std::env::temp_dir()
            .canonicalize()
            .unwrap_or_else(|_| std::env::temp_dir());
        let dir = tempfile::Builder::new()
            .prefix("jaco-attachments-")
            .tempdir_in(temp_root)
            .unwrap();
        cx.update(|cx| {
            let mut config =
                JacoConfig::load_from_path_for_test(&dir.path().join("config.toml")).unwrap();
            config.storage.data_dir = Some(dir.path().join("data"));
            config.save_for_test().unwrap();
            config::install_for_test(cx, config).unwrap();
        });

        let bytes = vec![137, 80, 78, 71];
        let attachment = generated_image_attachment(
            "clipboard.png".to_string(),
            Image::from_bytes(ImageFormat::Png, bytes.clone()),
            "image/png".to_string(),
            (1, 1),
            0,
        );
        let conversation_id = "conversation-1".to_string();
        let prepared = cx.update(|cx| {
            prepare_message_attachments(&conversation_id, "item-1", &[attachment], cx).unwrap()
        });

        assert_eq!(prepared.stored_paths.len(), 1);
        let stored_path = &prepared.stored_paths[0];
        assert!(stored_path.starts_with(dir.path().join("data/attachments/conversation-1")));
        assert!(!stored_path.to_string_lossy().contains("/pending/"));
        assert_eq!(fs::read(stored_path).unwrap(), bytes);
        assert_eq!(prepared.new_attachments.len(), 1);
        let new_attachment = &prepared.new_attachments[0];
        assert_eq!(
            new_attachment.path.as_deref(),
            Some(stored_path.to_string_lossy().as_ref())
        );
        assert_eq!(
            new_attachment.metadata.source,
            AttachmentSource::GeneratedFile {
                path: stored_path.to_string_lossy().to_string()
            }
        );
    }
}

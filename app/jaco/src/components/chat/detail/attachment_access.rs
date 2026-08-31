use std::{
    collections::HashSet,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use jaco_core::{
    AttachmentId, AttachmentKind, AttachmentSource, AttachmentStorageKind, ConversationAttachment,
    ConversationId,
};

use crate::features::conversation::attachments::managed_attachment_dir;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum AttachmentAction {
    Open,
    Reveal,
    SaveCopy,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum AttachmentAccessKind {
    Image,
    File,
    Attachment,
}

impl From<super::attachments::AttachmentCardKind> for AttachmentAccessKind {
    fn from(kind: super::attachments::AttachmentCardKind) -> Self {
        match kind {
            super::attachments::AttachmentCardKind::File => Self::File,
            super::attachments::AttachmentCardKind::Attachment => Self::Attachment,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct AttachmentActionTarget {
    pub(super) attachment_id: AttachmentId,
    pub(super) kind: super::attachments::AttachmentCardKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AttachmentSourceHint {
    Local,
    Generated,
    External,
    Provider,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResolvedAttachmentSource {
    Managed,
    Local,
    Generated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AttachmentSourceLabel {
    Managed,
    Local,
    Generated,
    External,
    Provider,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum AttachmentAccessProblem {
    MissingRecord,
    WrongConversation,
    KindMismatch,
    UnsupportedSource,
    MissingLocator,
    LocatorMismatch,
    MissingFile,
    NotRegularFile,
    UnsafeGeneratedPath,
    Io(ErrorKind),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ResolvedLocalAttachment {
    attachment_id: AttachmentId,
    kind: AttachmentAccessKind,
    path: PathBuf,
    source: ResolvedAttachmentSource,
}

pub(super) enum AttachmentAccessState {
    Checking,
    Available(ResolvedLocalAttachment),
    Unavailable(AttachmentAccessProblem),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum AttachmentAvailability {
    Checking,
    Available,
    Unavailable(AttachmentAccessProblem),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AttachmentAccessView {
    pub(super) availability: AttachmentAvailability,
    pub(super) source: AttachmentSourceLabel,
    pub(super) busy_actions: HashSet<AttachmentAction>,
    pub(super) resolved: Option<ResolvedLocalAttachment>,
}

impl ResolvedLocalAttachment {
    pub(super) fn attachment_id(&self) -> &AttachmentId {
        &self.attachment_id
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn kind(&self) -> AttachmentAccessKind {
        self.kind
    }

    pub(super) fn image_path(&self) -> Option<&Path> {
        (self.kind == AttachmentAccessKind::Image).then_some(self.path.as_path())
    }

    pub(super) fn source_label(&self) -> AttachmentSourceLabel {
        self.source.into()
    }
}

impl From<ResolvedAttachmentSource> for AttachmentSourceLabel {
    fn from(source: ResolvedAttachmentSource) -> Self {
        match source {
            ResolvedAttachmentSource::Managed => Self::Managed,
            ResolvedAttachmentSource::Local => Self::Local,
            ResolvedAttachmentSource::Generated => Self::Generated,
        }
    }
}

impl From<AttachmentSourceHint> for AttachmentSourceLabel {
    fn from(source: AttachmentSourceHint) -> Self {
        match source {
            AttachmentSourceHint::Local => Self::Local,
            AttachmentSourceHint::Generated => Self::Generated,
            AttachmentSourceHint::External => Self::External,
            AttachmentSourceHint::Provider => Self::Provider,
            AttachmentSourceHint::Unknown => Self::Unknown,
        }
    }
}

impl AttachmentAccessState {
    pub(super) fn availability(&self) -> AttachmentAvailability {
        match self {
            Self::Checking => AttachmentAvailability::Checking,
            Self::Available(_) => AttachmentAvailability::Available,
            Self::Unavailable(problem) => AttachmentAvailability::Unavailable(problem.clone()),
        }
    }

    pub(super) fn resolved(&self) -> Option<&ResolvedLocalAttachment> {
        match self {
            Self::Available(resolved) => Some(resolved),
            Self::Checking | Self::Unavailable(_) => None,
        }
    }
}

pub(super) fn attachment_source_hint(record: &ConversationAttachment) -> AttachmentSourceHint {
    match record.storage_kind {
        AttachmentStorageKind::ExternalUri => AttachmentSourceHint::External,
        AttachmentStorageKind::ProviderFile => AttachmentSourceHint::Provider,
        AttachmentStorageKind::LocalFile | AttachmentStorageKind::GeneratedFile => {
            match &record.metadata.source {
                AttachmentSource::LocalFile { .. } => AttachmentSourceHint::Local,
                AttachmentSource::GeneratedFile { .. } => AttachmentSourceHint::Generated,
                AttachmentSource::ExternalUri { .. } => AttachmentSourceHint::External,
                AttachmentSource::ProviderFile { .. } => AttachmentSourceHint::Provider,
            }
        }
    }
}

pub(super) fn safe_display_name(name: Option<&str>) -> Option<String> {
    let name = name?.rsplit(['/', '\\']).next().unwrap_or_default();
    let name = name
        .chars()
        .filter(|character| !character.is_control())
        .collect::<String>();
    let name = name.trim();
    if name.is_empty() || matches!(name, "." | "..") {
        return None;
    }

    Some(name.chars().take(160).collect())
}

pub(super) fn safe_mime_type(mime: Option<&str>) -> Option<String> {
    let mime = mime?;
    if mime.chars().any(char::is_control) {
        return None;
    }
    let mime = mime.trim();
    if mime.is_empty() || mime.chars().count() > 127 {
        return None;
    }

    let mut components = mime.split('/');
    let major = components.next().unwrap_or_default();
    let subtype = components.next().unwrap_or_default();
    if components.next().is_some()
        || major.is_empty()
        || subtype.is_empty()
        || major.chars().any(char::is_whitespace)
        || subtype.chars().any(char::is_whitespace)
    {
        return None;
    }

    Some(mime.to_string())
}

pub(super) fn format_persisted_size(size: Option<i64>) -> Option<String> {
    let size = u64::try_from(size?).ok()?;
    const UNIT: u64 = 1024;
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];

    if size < UNIT {
        return Some(format!("{size} B"));
    }

    let mut value = size as f64;
    let mut unit_index = 0;
    while value >= UNIT as f64 && unit_index < UNITS.len() - 1 {
        value /= UNIT as f64;
        unit_index += 1;
    }

    Some(format!("{value:.1} {}", UNITS[unit_index]))
}

pub(super) fn resolve_local_attachment(
    record: &ConversationAttachment,
    expected_conversation_id: &ConversationId,
    expected_kind: impl Into<AttachmentAccessKind>,
    database_data_dir: &Path,
) -> Result<ResolvedLocalAttachment, AttachmentAccessProblem> {
    let expected_kind = expected_kind.into();
    if record.conversation_id.as_str() != expected_conversation_id.as_str() {
        return Err(AttachmentAccessProblem::WrongConversation);
    }
    if !attachment_kind_matches(record.kind, expected_kind) {
        return Err(AttachmentAccessProblem::KindMismatch);
    }

    let generated_source = match record.storage_kind {
        AttachmentStorageKind::ExternalUri | AttachmentStorageKind::ProviderFile => {
            return Err(AttachmentAccessProblem::UnsupportedSource);
        }
        AttachmentStorageKind::LocalFile => match &record.metadata.source {
            AttachmentSource::LocalFile { .. } => false,
            AttachmentSource::GeneratedFile { .. } => true,
            AttachmentSource::ExternalUri { .. } | AttachmentSource::ProviderFile { .. } => {
                return Err(AttachmentAccessProblem::UnsupportedSource);
            }
        },
        AttachmentStorageKind::GeneratedFile => match &record.metadata.source {
            AttachmentSource::GeneratedFile { .. } => true,
            AttachmentSource::LocalFile { .. }
            | AttachmentSource::ExternalUri { .. }
            | AttachmentSource::ProviderFile { .. } => {
                return Err(AttachmentAccessProblem::UnsupportedSource);
            }
        },
    };

    let record_path = trimmed_locator(record.path.as_deref());
    let metadata_path = match &record.metadata.source {
        AttachmentSource::LocalFile { path } | AttachmentSource::GeneratedFile { path } => {
            trimmed_locator(Some(path.as_str()))
        }
        AttachmentSource::ExternalUri { .. } | AttachmentSource::ProviderFile { .. } => None,
    };
    let path = match (record_path, metadata_path) {
        (None, None) => return Err(AttachmentAccessProblem::MissingLocator),
        (Some(path), None) | (None, Some(path)) => canonicalize_locator(path)?,
        (Some(record_path), Some(metadata_path)) => {
            let record_path = canonicalize_locator(record_path)?;
            let metadata_path = canonicalize_locator(metadata_path)?;
            if record_path != metadata_path {
                return Err(AttachmentAccessProblem::LocatorMismatch);
            }
            record_path
        }
    };

    if !fs::metadata(&path).map_err(map_filesystem_error)?.is_file() {
        return Err(AttachmentAccessProblem::NotRegularFile);
    }

    let managed_root = canonical_managed_root(database_data_dir, &record.conversation_id);
    let source = if generated_source {
        let Some(managed_root) = managed_root? else {
            return Err(AttachmentAccessProblem::UnsafeGeneratedPath);
        };
        if !path.starts_with(managed_root) {
            return Err(AttachmentAccessProblem::UnsafeGeneratedPath);
        }
        ResolvedAttachmentSource::Generated
    } else if managed_root
        .ok()
        .flatten()
        .is_some_and(|root| path.starts_with(root))
    {
        ResolvedAttachmentSource::Managed
    } else {
        ResolvedAttachmentSource::Local
    };

    Ok(ResolvedLocalAttachment {
        attachment_id: record.id.clone(),
        kind: expected_kind,
        path,
        source,
    })
}

fn attachment_kind_matches(
    record_kind: AttachmentKind,
    expected_kind: AttachmentAccessKind,
) -> bool {
    match expected_kind {
        AttachmentAccessKind::Image => record_kind == AttachmentKind::Image,
        AttachmentAccessKind::File => record_kind == AttachmentKind::File,
        AttachmentAccessKind::Attachment => {
            matches!(
                record_kind,
                AttachmentKind::Attachment | AttachmentKind::Audio
            )
        }
    }
}

fn trimmed_locator(value: Option<&str>) -> Option<&Path> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(Path::new)
}

fn canonicalize_locator(path: &Path) -> Result<PathBuf, AttachmentAccessProblem> {
    fs::canonicalize(path).map_err(map_filesystem_error)
}

fn canonical_managed_root(
    database_data_dir: &Path,
    conversation_id: &str,
) -> Result<Option<PathBuf>, AttachmentAccessProblem> {
    if !crate::features::conversation::attachments::is_valid_managed_conversation_id(
        conversation_id,
    ) {
        return Err(AttachmentAccessProblem::UnsafeGeneratedPath);
    }
    match fs::canonicalize(database_data_dir) {
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(AttachmentAccessProblem::Io(error.kind())),
    }
    let attachments_dir = database_data_dir.join("attachments");
    let canonical_attachments_dir = match fs::canonicalize(&attachments_dir) {
        Ok(path) => path,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(AttachmentAccessProblem::Io(error.kind())),
    };

    let managed_root = managed_attachment_dir(database_data_dir, conversation_id);
    let managed_metadata = match fs::symlink_metadata(&managed_root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(AttachmentAccessProblem::Io(error.kind())),
    };
    if managed_metadata.file_type().is_symlink() {
        return Err(AttachmentAccessProblem::UnsafeGeneratedPath);
    }
    let canonical_root = fs::canonicalize(managed_root).map_err(map_filesystem_error)?;
    if canonical_root.parent() != Some(canonical_attachments_dir.as_path()) {
        return Err(AttachmentAccessProblem::UnsafeGeneratedPath);
    }
    Ok(Some(canonical_root))
}

fn map_filesystem_error(error: std::io::Error) -> AttachmentAccessProblem {
    if error.kind() == ErrorKind::NotFound {
        AttachmentAccessProblem::MissingFile
    } else {
        AttachmentAccessProblem::Io(error.kind())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use jaco_core::AttachmentMetadata;
    use time::OffsetDateTime;

    fn attachment_record(
        id: &str,
        conversation_id: &str,
        kind: AttachmentKind,
        storage_kind: AttachmentStorageKind,
        path: Option<String>,
        source: AttachmentSource,
    ) -> ConversationAttachment {
        let now = OffsetDateTime::UNIX_EPOCH;
        ConversationAttachment {
            id: id.to_string(),
            conversation_id: conversation_id.to_string(),
            kind,
            storage_kind,
            mime_type: None,
            name: Some("attachment.txt".to_string()),
            path,
            external_uri: None,
            provider_id: None,
            provider_file_id: None,
            sha256: None,
            size_bytes: None,
            metadata: AttachmentMetadata {
                source,
                width: None,
                height: None,
                duration_ms: None,
                preview_attachment_id: None,
            },
            created_at: now,
            updated_at: now,
        }
    }

    fn local_record(
        id: &str,
        conversation_id: &str,
        kind: AttachmentKind,
        storage_kind: AttachmentStorageKind,
        record_path: Option<&Path>,
        metadata_path: Option<&Path>,
    ) -> ConversationAttachment {
        let metadata_path = metadata_path
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_default();
        attachment_record(
            id,
            conversation_id,
            kind,
            storage_kind,
            record_path.map(|path| path.to_string_lossy().to_string()),
            if storage_kind == AttachmentStorageKind::GeneratedFile {
                AttachmentSource::GeneratedFile {
                    path: metadata_path,
                }
            } else {
                AttachmentSource::LocalFile {
                    path: metadata_path,
                }
            },
        )
    }

    fn managed_file(data_dir: &Path, conversation_id: &str, name: &str) -> PathBuf {
        let dir = managed_attachment_dir(data_dir, conversation_id);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        fs::write(&path, b"attachment").unwrap();
        path
    }

    #[test]
    fn sanitizes_display_names_without_exposing_path_segments() {
        assert_eq!(
            safe_display_name(Some(" /tmp/unsafe\\report.txt ")),
            Some("report.txt".to_string())
        );
        assert_eq!(
            safe_display_name(Some("a\u{0000}b\u{0001}")),
            Some("ab".to_string())
        );
        assert_eq!(safe_display_name(Some("/\\")), None);
        assert_eq!(safe_display_name(Some(".")), None);
        assert_eq!(safe_display_name(Some("..")), None);
        assert_eq!(safe_display_name(None), None);
        assert_eq!(
            safe_display_name(Some(&"x".repeat(161)))
                .unwrap()
                .chars()
                .count(),
            160
        );
    }

    #[test]
    fn validates_mime_types_and_persisted_sizes() {
        assert_eq!(
            safe_mime_type(Some(" text/plain ")),
            Some("text/plain".to_string())
        );
        assert_eq!(safe_mime_type(Some("text/plain/extra")), None);
        assert_eq!(safe_mime_type(Some("/plain")), None);
        assert_eq!(safe_mime_type(Some("text/")), None);
        assert_eq!(safe_mime_type(Some("text\n/plain")), None);
        assert_eq!(
            safe_mime_type(Some(&format!("text/{}", "x".repeat(123)))),
            None
        );

        assert_eq!(format_persisted_size(None), None);
        assert_eq!(format_persisted_size(Some(-1)), None);
        assert_eq!(format_persisted_size(Some(42)), Some("42 B".to_string()));
        assert_eq!(
            format_persisted_size(Some(1536)),
            Some("1.5 KiB".to_string())
        );
        assert_eq!(
            format_persisted_size(Some(1024 * 1024)),
            Some("1.0 MiB".to_string())
        );
    }

    #[test]
    fn resolves_local_and_managed_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let conversation_id = "conversation-1".to_string();
        let managed_path = managed_file(temp_dir.path(), &conversation_id, "managed.txt");
        let managed_record = local_record(
            "managed",
            &conversation_id,
            AttachmentKind::File,
            AttachmentStorageKind::LocalFile,
            Some(&managed_path),
            Some(&managed_path),
        );
        let resolved = resolve_local_attachment(
            &managed_record,
            &conversation_id,
            super::super::attachments::AttachmentCardKind::File,
            temp_dir.path(),
        )
        .unwrap();
        assert_eq!(resolved.attachment_id(), "managed");
        assert_eq!(resolved.path(), fs::canonicalize(&managed_path).unwrap());
        assert_eq!(resolved.source_label(), AttachmentSourceLabel::Managed);

        let outside_path = temp_dir.path().join("outside.txt");
        fs::write(&outside_path, b"outside").unwrap();
        let outside_record = local_record(
            "outside",
            &conversation_id,
            AttachmentKind::File,
            AttachmentStorageKind::LocalFile,
            Some(&outside_path),
            Some(&outside_path),
        );
        let resolved = resolve_local_attachment(
            &outside_record,
            &conversation_id,
            super::super::attachments::AttachmentCardKind::File,
            temp_dir.path(),
        )
        .unwrap();
        assert_eq!(resolved.source_label(), AttachmentSourceLabel::Local);
    }

    #[test]
    fn image_paths_are_exposed_only_for_image_validation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let conversation_id = "conversation-1".to_string();
        let image_path = managed_file(temp_dir.path(), &conversation_id, "image.png");
        let image_record = local_record(
            "image",
            &conversation_id,
            AttachmentKind::Image,
            AttachmentStorageKind::LocalFile,
            Some(&image_path),
            Some(&image_path),
        );

        let resolved = resolve_local_attachment(
            &image_record,
            &conversation_id,
            AttachmentAccessKind::Image,
            temp_dir.path(),
        )
        .unwrap();
        assert_eq!(resolved.kind(), AttachmentAccessKind::Image);
        assert_eq!(resolved.image_path(), Some(resolved.path()));
        assert_eq!(resolved.source_label(), AttachmentSourceLabel::Managed);

        assert_eq!(
            resolve_local_attachment(
                &image_record,
                &conversation_id,
                AttachmentAccessKind::File,
                temp_dir.path(),
            ),
            Err(AttachmentAccessProblem::KindMismatch)
        );
    }

    #[test]
    fn resolves_production_local_storage_with_generated_source_inside_managed_root() {
        let temp_dir = tempfile::tempdir().unwrap();
        let conversation_id = "conversation-1".to_string();
        let managed_path = managed_file(temp_dir.path(), &conversation_id, "generated.png");
        let mut record = local_record(
            "generated-image",
            &conversation_id,
            AttachmentKind::Attachment,
            AttachmentStorageKind::LocalFile,
            Some(&managed_path),
            Some(&managed_path),
        );
        record.metadata.source = AttachmentSource::GeneratedFile {
            path: managed_path.to_string_lossy().to_string(),
        };

        let resolved = resolve_local_attachment(
            &record,
            &conversation_id,
            super::super::attachments::AttachmentCardKind::Attachment,
            temp_dir.path(),
        )
        .unwrap();

        assert_eq!(resolved.source_label(), AttachmentSourceLabel::Generated);
        assert_eq!(resolved.path(), fs::canonicalize(managed_path).unwrap());
    }

    #[test]
    fn rejects_path_like_managed_conversation_id() {
        let temp_dir = tempfile::tempdir().unwrap();
        let outside = temp_dir.path().join("outside.bin");
        fs::write(&outside, b"outside").unwrap();
        let invalid_id = "../outside-conversation".to_string();
        let record = local_record(
            "generated",
            &invalid_id,
            AttachmentKind::Attachment,
            AttachmentStorageKind::GeneratedFile,
            Some(&outside),
            Some(&outside),
        );

        assert!(matches!(
            resolve_local_attachment(
                &record,
                &invalid_id,
                super::super::attachments::AttachmentCardKind::Attachment,
                temp_dir.path(),
            ),
            Err(AttachmentAccessProblem::UnsafeGeneratedPath)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symbolic_link_managed_conversation_root() {
        use std::os::unix::fs::symlink;

        let temp_dir = tempfile::tempdir().unwrap();
        let outside_dir = tempfile::tempdir().unwrap();
        let conversation_id = "conversation-1".to_string();
        let outside = outside_dir.path().join("generated.bin");
        fs::write(&outside, b"outside").unwrap();
        fs::create_dir_all(temp_dir.path().join("attachments")).unwrap();
        symlink(
            outside_dir.path(),
            managed_attachment_dir(temp_dir.path(), &conversation_id),
        )
        .unwrap();
        let record = local_record(
            "generated",
            &conversation_id,
            AttachmentKind::Attachment,
            AttachmentStorageKind::GeneratedFile,
            Some(&outside),
            Some(&outside),
        );

        assert!(matches!(
            resolve_local_attachment(
                &record,
                &conversation_id,
                super::super::attachments::AttachmentCardKind::Attachment,
                temp_dir.path(),
            ),
            Err(AttachmentAccessProblem::UnsafeGeneratedPath)
        ));

        let local_record = local_record(
            "local",
            &conversation_id,
            AttachmentKind::File,
            AttachmentStorageKind::LocalFile,
            Some(&outside),
            Some(&outside),
        );
        assert_eq!(
            resolve_local_attachment(
                &local_record,
                &conversation_id,
                super::super::attachments::AttachmentCardKind::File,
                temp_dir.path(),
            )
            .unwrap()
            .source_label(),
            AttachmentSourceLabel::Local
        );
    }

    #[cfg(unix)]
    #[test]
    fn accepts_symbolic_link_for_the_whole_managed_attachments_root() {
        use std::os::unix::fs::symlink;

        let temp_dir = tempfile::tempdir().unwrap();
        let external_attachments = tempfile::tempdir().unwrap();
        symlink(
            external_attachments.path(),
            temp_dir.path().join("attachments"),
        )
        .unwrap();
        let conversation_id = "conversation-1".to_string();
        let managed_path = managed_file(temp_dir.path(), &conversation_id, "generated.bin");
        let record = local_record(
            "generated",
            &conversation_id,
            AttachmentKind::Attachment,
            AttachmentStorageKind::GeneratedFile,
            Some(&managed_path),
            Some(&managed_path),
        );

        let resolved = resolve_local_attachment(
            &record,
            &conversation_id,
            super::super::attachments::AttachmentCardKind::Attachment,
            temp_dir.path(),
        )
        .unwrap();

        assert_eq!(resolved.source_label(), AttachmentSourceLabel::Generated);
        assert!(
            resolved
                .path()
                .starts_with(fs::canonicalize(external_attachments.path()).unwrap())
        );
    }

    #[test]
    fn enforces_generated_containment_and_source_storage_pairs() {
        let temp_dir = tempfile::tempdir().unwrap();
        let conversation_id = "conversation-1".to_string();
        let managed_path = managed_file(temp_dir.path(), &conversation_id, "generated.txt");
        let generated_record = local_record(
            "generated",
            &conversation_id,
            AttachmentKind::Attachment,
            AttachmentStorageKind::GeneratedFile,
            Some(&managed_path),
            Some(&managed_path),
        );
        assert_eq!(
            resolve_local_attachment(
                &generated_record,
                &conversation_id,
                super::super::attachments::AttachmentCardKind::Attachment,
                temp_dir.path(),
            )
            .unwrap()
            .source_label(),
            AttachmentSourceLabel::Generated
        );

        let outside_path = temp_dir.path().join("outside.txt");
        fs::write(&outside_path, b"outside").unwrap();
        let generated_outside = local_record(
            "generated-outside",
            &conversation_id,
            AttachmentKind::File,
            AttachmentStorageKind::LocalFile,
            Some(&outside_path),
            Some(&outside_path),
        );
        let mut generated_outside = generated_outside;
        generated_outside.metadata.source = AttachmentSource::GeneratedFile {
            path: outside_path.to_string_lossy().to_string(),
        };
        assert_eq!(
            resolve_local_attachment(
                &generated_outside,
                &conversation_id,
                super::super::attachments::AttachmentCardKind::File,
                temp_dir.path(),
            ),
            Err(AttachmentAccessProblem::UnsafeGeneratedPath)
        );

        let generated_with_local_source = local_record(
            "generated-local-source",
            &conversation_id,
            AttachmentKind::File,
            AttachmentStorageKind::GeneratedFile,
            Some(&managed_path),
            Some(&managed_path),
        );
        let mut generated_with_local_source = generated_with_local_source;
        generated_with_local_source.metadata.source = AttachmentSource::LocalFile {
            path: managed_path.to_string_lossy().to_string(),
        };
        assert_eq!(
            resolve_local_attachment(
                &generated_with_local_source,
                &conversation_id,
                super::super::attachments::AttachmentCardKind::File,
                temp_dir.path(),
            ),
            Err(AttachmentAccessProblem::UnsupportedSource)
        );
    }

    #[test]
    fn denies_external_and_provider_storage_before_stale_paths() {
        let temp_dir = tempfile::tempdir().unwrap();
        let stale_path = temp_dir.path().join("stale.txt");
        fs::write(&stale_path, b"stale").unwrap();
        let conversation_id = "conversation-1".to_string();
        let external = attachment_record(
            "external",
            &conversation_id,
            AttachmentKind::File,
            AttachmentStorageKind::ExternalUri,
            Some(stale_path.to_string_lossy().to_string()),
            AttachmentSource::ExternalUri {
                uri: "https://example.test/private?token=secret".to_string(),
            },
        );
        assert_eq!(
            resolve_local_attachment(
                &external,
                &conversation_id,
                super::super::attachments::AttachmentCardKind::File,
                temp_dir.path(),
            ),
            Err(AttachmentAccessProblem::UnsupportedSource)
        );

        let provider = attachment_record(
            "provider",
            &conversation_id,
            AttachmentKind::Attachment,
            AttachmentStorageKind::ProviderFile,
            Some(stale_path.to_string_lossy().to_string()),
            AttachmentSource::ProviderFile {
                provider_id: "provider-secret".to_string(),
                file_id: "file-secret".to_string(),
            },
        );
        assert_eq!(
            resolve_local_attachment(
                &provider,
                &conversation_id,
                super::super::attachments::AttachmentCardKind::Attachment,
                temp_dir.path(),
            ),
            Err(AttachmentAccessProblem::UnsupportedSource)
        );
    }

    #[test]
    fn rejects_invalid_conversation_kind_and_locators() {
        let temp_dir = tempfile::tempdir().unwrap();
        let conversation_id = "conversation-1".to_string();
        let path = temp_dir.path().join("file.txt");
        fs::write(&path, b"file").unwrap();
        let record = local_record(
            "file",
            "other-conversation",
            AttachmentKind::File,
            AttachmentStorageKind::LocalFile,
            Some(&path),
            Some(&path),
        );
        assert_eq!(
            resolve_local_attachment(
                &record,
                &conversation_id,
                super::super::attachments::AttachmentCardKind::File,
                temp_dir.path(),
            ),
            Err(AttachmentAccessProblem::WrongConversation)
        );

        let record = local_record(
            "image",
            &conversation_id,
            AttachmentKind::Image,
            AttachmentStorageKind::LocalFile,
            Some(&path),
            Some(&path),
        );
        assert_eq!(
            resolve_local_attachment(
                &record,
                &conversation_id,
                super::super::attachments::AttachmentCardKind::File,
                temp_dir.path(),
            ),
            Err(AttachmentAccessProblem::KindMismatch)
        );

        let missing_locator = local_record(
            "missing-locator",
            &conversation_id,
            AttachmentKind::File,
            AttachmentStorageKind::LocalFile,
            None,
            None,
        );
        assert_eq!(
            resolve_local_attachment(
                &missing_locator,
                &conversation_id,
                super::super::attachments::AttachmentCardKind::File,
                temp_dir.path(),
            ),
            Err(AttachmentAccessProblem::MissingLocator)
        );

        let other_path = temp_dir.path().join("other.txt");
        fs::write(&other_path, b"other").unwrap();
        let mismatch = local_record(
            "mismatch",
            &conversation_id,
            AttachmentKind::File,
            AttachmentStorageKind::LocalFile,
            Some(&path),
            Some(&other_path),
        );
        assert_eq!(
            resolve_local_attachment(
                &mismatch,
                &conversation_id,
                super::super::attachments::AttachmentCardKind::File,
                temp_dir.path(),
            ),
            Err(AttachmentAccessProblem::LocatorMismatch)
        );

        let directory = temp_dir.path().join("directory");
        fs::create_dir(&directory).unwrap();
        let directory_record = local_record(
            "directory",
            &conversation_id,
            AttachmentKind::File,
            AttachmentStorageKind::LocalFile,
            Some(&directory),
            Some(&directory),
        );
        assert_eq!(
            resolve_local_attachment(
                &directory_record,
                &conversation_id,
                super::super::attachments::AttachmentCardKind::File,
                temp_dir.path(),
            ),
            Err(AttachmentAccessProblem::NotRegularFile)
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_generated_symlink_escape() {
        use std::os::unix::fs::symlink;

        let temp_dir = tempfile::tempdir().unwrap();
        let conversation_id = "conversation-1".to_string();
        let outside_path = temp_dir.path().join("outside.txt");
        fs::write(&outside_path, b"outside").unwrap();
        let generated_dir = managed_attachment_dir(temp_dir.path(), &conversation_id);
        fs::create_dir_all(&generated_dir).unwrap();
        let symlink_path = generated_dir.join("escape.txt");
        symlink(&outside_path, &symlink_path).unwrap();
        let record = local_record(
            "escape",
            &conversation_id,
            AttachmentKind::File,
            AttachmentStorageKind::GeneratedFile,
            Some(&symlink_path),
            Some(&symlink_path),
        );

        assert_eq!(
            resolve_local_attachment(
                &record,
                &conversation_id,
                super::super::attachments::AttachmentCardKind::File,
                temp_dir.path(),
            ),
            Err(AttachmentAccessProblem::UnsafeGeneratedPath)
        );
    }
}

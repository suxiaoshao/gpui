use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use jaco_core::{AttachmentKind, AttachmentSource, AttachmentStorageKind};
use jaco_db::AttachmentRecord;
use tracing::{Level, event};

use super::attachments::{is_valid_managed_conversation_id, managed_attachment_dir};

type ArtifactKey = (String, String);
const GENERATED_FILE_PREFIX: &str = ".jaco-generated-";

#[derive(Clone, Debug)]
struct GeneratedAuthority {
    path: PathBuf,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct GeneratedArtifactReconciliation {
    pub(crate) removed_pending: usize,
    pub(crate) removed_orphans: usize,
    pub(crate) missing_managed_root: usize,
    pub(crate) missing_generated_files: usize,
    pub(crate) ambiguous_records_or_files: usize,
    pub(crate) unsafe_entries: usize,
    pub(crate) inspection_failures: usize,
    pub(crate) delete_failures: usize,
}

impl GeneratedArtifactReconciliation {
    pub(crate) fn emit_warnings(&self) {
        for (category, count) in [
            ("missing_managed_root", self.missing_managed_root),
            ("missing_generated_file", self.missing_generated_files),
            (
                "ambiguous_generated_artifact",
                self.ambiguous_records_or_files,
            ),
            ("unsafe_generated_artifact_entry", self.unsafe_entries),
            (
                "generated_artifact_inspection_failed",
                self.inspection_failures,
            ),
            ("generated_artifact_delete_failed", self.delete_failures),
        ] {
            if count > 0 {
                event!(
                    Level::WARN,
                    category,
                    count,
                    "generated artifact reconciliation completed with a degraded warning"
                );
            }
        }
    }
}

pub(crate) fn reconcile_generated_artifacts(
    data_dir: PathBuf,
    records: Vec<AttachmentRecord>,
) -> GeneratedArtifactReconciliation {
    let mut summary = GeneratedArtifactReconciliation::default();
    let indexed_record_count = records.len();
    let (authorities, protected_paths) = generated_authorities(&data_dir, records, &mut summary);
    let generated_authority_count = authorities
        .values()
        .filter(|authorities| authorities.len() == 1)
        .count();
    let root = data_dir.join("attachments");
    let root_metadata = match fs::symlink_metadata(&root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if indexed_record_count > 0 {
                summary.missing_managed_root = indexed_record_count;
                summary.missing_generated_files = generated_authority_count;
            }
            return summary;
        }
        Err(_) => {
            summary.inspection_failures += 1;
            summary.missing_generated_files = generated_authority_count;
            return summary;
        }
    };
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        summary.unsafe_entries += 1;
        summary.missing_generated_files = generated_authority_count;
        return summary;
    }

    let root_entries = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(_) => {
            summary.inspection_failures += 1;
            summary.missing_generated_files = generated_authority_count;
            return summary;
        }
    };
    let mut observed = HashSet::new();
    for entry in root_entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                summary.inspection_failures += 1;
                continue;
            }
        };
        let conversation_name = entry.file_name();
        let Some(conversation_id) = conversation_name.to_str() else {
            summary.unsafe_entries += 1;
            continue;
        };
        if !is_valid_managed_conversation_id(conversation_id) || !is_generated_id(conversation_id) {
            summary.unsafe_entries += 1;
            continue;
        }
        let conversation_dir = entry.path();
        let metadata = match fs::symlink_metadata(&conversation_dir) {
            Ok(metadata) => metadata,
            Err(_) => {
                summary.inspection_failures += 1;
                continue;
            }
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            summary.unsafe_entries += 1;
            continue;
        }

        reconcile_pending(&conversation_dir, &mut summary);
        reconcile_final_files(
            conversation_id,
            &conversation_dir,
            &authorities,
            &protected_paths,
            &mut observed,
            &mut summary,
        );
    }

    summary.missing_generated_files += authorities
        .iter()
        .filter(|(key, candidates)| candidates.len() == 1 && !observed.contains(*key))
        .count();
    summary
}

fn generated_authorities(
    data_dir: &Path,
    records: Vec<AttachmentRecord>,
    summary: &mut GeneratedArtifactReconciliation,
) -> (
    HashMap<ArtifactKey, Vec<GeneratedAuthority>>,
    HashSet<PathBuf>,
) {
    let mut authorities = HashMap::<ArtifactKey, Vec<GeneratedAuthority>>::new();
    let mut protected_paths = HashSet::new();
    for record in records {
        if let Some(path) = record.path.as_ref() {
            protected_paths.insert(PathBuf::from(path));
        }
        if let AttachmentSource::GeneratedFile { path } = &record.metadata.source {
            protected_paths.insert(PathBuf::from(path));
        }

        let key = (record.conversation_id.clone(), record.id.clone());
        let Some(authority) = generated_authority(data_dir, &record) else {
            summary.ambiguous_records_or_files += 1;
            authorities.entry(key).or_default();
            continue;
        };
        authorities.entry(key).or_default().push(authority);
    }
    for candidates in authorities.values() {
        if candidates.len() > 1 {
            summary.ambiguous_records_or_files += 1;
        }
    }
    (authorities, protected_paths)
}

fn generated_authority(data_dir: &Path, record: &AttachmentRecord) -> Option<GeneratedAuthority> {
    if record.kind != AttachmentKind::Image
        || record.storage_kind != AttachmentStorageKind::GeneratedFile
        || !is_valid_managed_conversation_id(&record.conversation_id)
        || !is_generated_id(&record.conversation_id)
        || !is_generated_id(&record.id)
    {
        return None;
    }
    let record_path = PathBuf::from(record.path.as_ref()?);
    let AttachmentSource::GeneratedFile { path: source_path } = &record.metadata.source else {
        return None;
    };
    if record_path.as_path() != Path::new(source_path) {
        return None;
    }
    let (attachment_id, _) = generated_final_name(record_path.file_name()?.to_str()?)?;
    if attachment_id != record.id {
        return None;
    }
    let expected_dir = managed_attachment_dir(data_dir, &record.conversation_id);
    if record_path.parent() != Some(expected_dir.as_path()) {
        return None;
    }
    Some(GeneratedAuthority { path: record_path })
}

fn reconcile_pending(conversation_dir: &Path, summary: &mut GeneratedArtifactReconciliation) {
    let pending_dir = conversation_dir.join(".pending");
    let metadata = match fs::symlink_metadata(&pending_dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(_) => {
            summary.inspection_failures += 1;
            return;
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        summary.unsafe_entries += 1;
        return;
    }
    let entries = match fs::read_dir(&pending_dir) {
        Ok(entries) => entries,
        Err(_) => {
            summary.inspection_failures += 1;
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                summary.inspection_failures += 1;
                continue;
            }
        };
        let path = entry.path();
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(_) => {
                summary.inspection_failures += 1;
                continue;
            }
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            summary.unsafe_entries += 1;
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            summary.unsafe_entries += 1;
            continue;
        };
        let Some(id) = name.strip_suffix(".part") else {
            continue;
        };
        if !is_generated_id(id) {
            continue;
        }
        match fs::remove_file(&path) {
            Ok(()) => summary.removed_pending += 1,
            Err(_) => summary.delete_failures += 1,
        }
    }
}

fn reconcile_final_files(
    conversation_id: &str,
    conversation_dir: &Path,
    authorities: &HashMap<ArtifactKey, Vec<GeneratedAuthority>>,
    protected_paths: &HashSet<PathBuf>,
    observed: &mut HashSet<ArtifactKey>,
    summary: &mut GeneratedArtifactReconciliation,
) {
    let entries = match fs::read_dir(conversation_dir) {
        Ok(entries) => entries,
        Err(_) => {
            summary.inspection_failures += 1;
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                summary.inspection_failures += 1;
                continue;
            }
        };
        let path = entry.path();
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(_) => {
                summary.inspection_failures += 1;
                continue;
            }
        };
        if metadata.file_type().is_symlink() {
            summary.unsafe_entries += 1;
            continue;
        }
        if !metadata.is_file() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            summary.unsafe_entries += 1;
            continue;
        };
        let Some((attachment_id, _)) = generated_final_name(&name) else {
            continue;
        };
        let key = (conversation_id.to_string(), attachment_id.to_string());
        match authorities.get(&key) {
            Some(candidates) if candidates.len() == 1 && candidates[0].path == path => {
                if exact_regular_file_inside(path.as_path(), conversation_dir) {
                    observed.insert(key);
                } else {
                    summary.unsafe_entries += 1;
                }
            }
            Some(_) => summary.ambiguous_records_or_files += 1,
            None if protected_paths.contains(&path) => {
                summary.ambiguous_records_or_files += 1;
            }
            None => match fs::remove_file(&path) {
                Ok(()) => summary.removed_orphans += 1,
                Err(_) => summary.delete_failures += 1,
            },
        }
    }
}

fn exact_regular_file_inside(path: &Path, conversation_dir: &Path) -> bool {
    let Ok(directory) = conversation_dir.canonicalize() else {
        return false;
    };
    let Ok(file) = path.canonicalize() else {
        return false;
    };
    file.parent() == Some(directory.as_path())
}

fn generated_final_name(name: &str) -> Option<(&str, &str)> {
    let (stem, extension) = name.rsplit_once('.')?;
    let id = stem.strip_prefix(GENERATED_FILE_PREFIX)?;
    if !is_generated_id(id) || !matches!(extension, "png" | "jpg" | "jpeg" | "gif" | "webp") {
        return None;
    }
    Some((id, extension))
}

fn is_generated_id(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use jaco_core::{AttachmentMetadata, ConversationAttachment};
    use time::OffsetDateTime;

    const CONVERSATION_A: &str = "019c0537-7fc4-7fdd-9711-ccf1e5a68fd5";
    const CONVERSATION_B: &str = "019c0537-7fc4-7fdd-9711-ccf1e5a68fd6";
    const ATTACHMENT_A: &str = "019c0537-7fc4-7fdd-9711-ccf1e5a68fd7";
    const ATTACHMENT_B: &str = "019c0537-7fc4-7fdd-9711-ccf1e5a68fd8";

    fn record(data_dir: &Path, conversation_id: &str, attachment_id: &str) -> AttachmentRecord {
        let path = managed_attachment_dir(data_dir, conversation_id)
            .join(format!("{GENERATED_FILE_PREFIX}{attachment_id}.png"))
            .to_string_lossy()
            .into_owned();
        let now = OffsetDateTime::UNIX_EPOCH;
        ConversationAttachment {
            id: attachment_id.to_string(),
            conversation_id: conversation_id.to_string(),
            kind: AttachmentKind::Image,
            storage_kind: AttachmentStorageKind::GeneratedFile,
            mime_type: Some("image/png".to_string()),
            name: Some("generated-image-1.png".to_string()),
            path: Some(path.clone()),
            external_uri: None,
            provider_id: Some("openrouter".to_string()),
            provider_file_id: None,
            sha256: None,
            size_bytes: Some(4),
            metadata: AttachmentMetadata {
                source: AttachmentSource::GeneratedFile { path },
                width: Some(1),
                height: Some(1),
                duration_ms: None,
                preview_attachment_id: None,
            },
            created_at: now,
            updated_at: now,
        }
    }

    fn write_generated(data_dir: &Path, conversation_id: &str, attachment_id: &str) -> PathBuf {
        let directory = managed_attachment_dir(data_dir, conversation_id);
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join(format!("{GENERATED_FILE_PREFIX}{attachment_id}.png"));
        fs::write(&path, b"file").unwrap();
        path
    }

    #[test]
    fn removes_only_prefixed_generated_orphans_and_preserves_unprefixed_uuid_files() {
        let directory = tempfile::tempdir().unwrap();
        let data_dir = directory.path();
        let kept = write_generated(data_dir, CONVERSATION_A, ATTACHMENT_A);
        let orphan = write_generated(data_dir, CONVERSATION_A, ATTACHMENT_B);
        let pending_dir = managed_attachment_dir(data_dir, CONVERSATION_A).join(".pending");
        fs::create_dir(&pending_dir).unwrap();
        let pending = pending_dir.join(format!("{ATTACHMENT_B}.part"));
        fs::write(&pending, b"part").unwrap();
        let composer = managed_attachment_dir(data_dir, CONVERSATION_A).join("message-1-1.txt");
        fs::write(&composer, b"user").unwrap();
        let unreserved_uuid_file =
            managed_attachment_dir(data_dir, CONVERSATION_A).join(format!("{ATTACHMENT_B}.png"));
        fs::write(&unreserved_uuid_file, b"user-owned").unwrap();

        let summary = reconcile_generated_artifacts(
            data_dir.to_path_buf(),
            vec![record(data_dir, CONVERSATION_A, ATTACHMENT_A)],
        );

        assert!(kept.exists());
        assert!(!orphan.exists());
        assert!(!pending.exists());
        assert!(composer.exists());
        assert!(unreserved_uuid_file.exists());
        assert_eq!(summary.removed_pending, 1);
        assert_eq!(summary.removed_orphans, 1);
        assert_eq!(summary.missing_generated_files, 0);
    }

    #[test]
    fn missing_root_and_reference_are_degraded_without_mutating_records() {
        let directory = tempfile::tempdir().unwrap();
        let data_dir = directory.path();

        let clean = reconcile_generated_artifacts(data_dir.to_path_buf(), Vec::new());
        assert_eq!(clean, GeneratedArtifactReconciliation::default());

        let degraded = reconcile_generated_artifacts(
            data_dir.to_path_buf(),
            vec![record(data_dir, CONVERSATION_A, ATTACHMENT_A)],
        );
        assert_eq!(degraded.missing_managed_root, 1);
        assert_eq!(degraded.missing_generated_files, 1);

        fs::create_dir(data_dir.join("attachments")).unwrap();
        let missing_reference = reconcile_generated_artifacts(
            data_dir.to_path_buf(),
            vec![record(data_dir, CONVERSATION_A, ATTACHMENT_A)],
        );
        assert_eq!(missing_reference.missing_generated_files, 1);
    }

    #[test]
    fn repeated_recovery_is_idempotent_and_conversations_do_not_cross_delete() {
        let directory = tempfile::tempdir().unwrap();
        let data_dir = directory.path();
        let a = write_generated(data_dir, CONVERSATION_A, ATTACHMENT_A);
        let b = write_generated(data_dir, CONVERSATION_B, ATTACHMENT_B);
        let records = vec![
            record(data_dir, CONVERSATION_A, ATTACHMENT_A),
            record(data_dir, CONVERSATION_B, ATTACHMENT_B),
        ];

        let first = reconcile_generated_artifacts(data_dir.to_path_buf(), records.clone());
        let second = reconcile_generated_artifacts(data_dir.to_path_buf(), records);

        assert!(a.exists());
        assert!(b.exists());
        assert_eq!(first, GeneratedArtifactReconciliation::default());
        assert_eq!(second, GeneratedArtifactReconciliation::default());
    }

    #[test]
    fn malformed_records_and_unsafe_entries_are_preserved() {
        let directory = tempfile::tempdir().unwrap();
        let data_dir = directory.path();
        let file = write_generated(data_dir, CONVERSATION_A, ATTACHMENT_A);
        let mut malformed = record(data_dir, CONVERSATION_A, ATTACHMENT_A);
        malformed.kind = AttachmentKind::File;

        let summary = reconcile_generated_artifacts(data_dir.to_path_buf(), vec![malformed]);

        assert!(file.exists());
        assert!(summary.ambiguous_records_or_files > 0);
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_never_followed_or_deleted() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        let external_file = external.path().join("external.png");
        fs::write(&external_file, b"external").unwrap();
        let conversation_dir = managed_attachment_dir(directory.path(), CONVERSATION_A);
        fs::create_dir_all(&conversation_dir).unwrap();
        let linked_file =
            conversation_dir.join(format!("{GENERATED_FILE_PREFIX}{ATTACHMENT_A}.png"));
        symlink(&external_file, &linked_file).unwrap();

        let summary = reconcile_generated_artifacts(directory.path().to_path_buf(), Vec::new());

        assert!(linked_file.symlink_metadata().is_ok());
        assert_eq!(fs::read(external_file).unwrap(), b"external");
        assert!(summary.unsafe_entries > 0);
    }

    #[cfg(unix)]
    #[test]
    fn linked_root_conversation_and_pending_directories_are_preserved() {
        use std::os::unix::fs::symlink;

        let linked_root_data = tempfile::tempdir().unwrap();
        let linked_root_target = tempfile::tempdir().unwrap();
        let linked_root_file =
            write_generated(linked_root_target.path(), CONVERSATION_A, ATTACHMENT_A);
        symlink(
            linked_root_target.path().join("attachments"),
            linked_root_data.path().join("attachments"),
        )
        .unwrap();
        let root_summary =
            reconcile_generated_artifacts(linked_root_data.path().to_path_buf(), Vec::new());
        assert!(linked_root_file.exists());
        assert!(root_summary.unsafe_entries > 0);

        let linked_conversation_data = tempfile::tempdir().unwrap();
        let linked_conversation_target = tempfile::tempdir().unwrap();
        fs::create_dir(linked_conversation_data.path().join("attachments")).unwrap();
        let linked_conversation_file = write_generated(
            linked_conversation_target.path(),
            CONVERSATION_A,
            ATTACHMENT_A,
        );
        symlink(
            managed_attachment_dir(linked_conversation_target.path(), CONVERSATION_A),
            managed_attachment_dir(linked_conversation_data.path(), CONVERSATION_A),
        )
        .unwrap();
        let conversation_summary = reconcile_generated_artifacts(
            linked_conversation_data.path().to_path_buf(),
            Vec::new(),
        );
        assert!(linked_conversation_file.exists());
        assert!(conversation_summary.unsafe_entries > 0);

        let linked_pending_data = tempfile::tempdir().unwrap();
        let conversation_dir = managed_attachment_dir(linked_pending_data.path(), CONVERSATION_A);
        fs::create_dir_all(&conversation_dir).unwrap();
        let linked_pending_target = tempfile::tempdir().unwrap();
        let linked_pending_file = linked_pending_target
            .path()
            .join(format!("{ATTACHMENT_A}.part"));
        fs::write(&linked_pending_file, b"part").unwrap();
        symlink(
            linked_pending_target.path(),
            conversation_dir.join(".pending"),
        )
        .unwrap();
        let pending_summary =
            reconcile_generated_artifacts(linked_pending_data.path().to_path_buf(), Vec::new());
        assert!(linked_pending_file.exists());
        assert!(pending_summary.unsafe_entries > 0);
    }

    #[test]
    fn invalid_conversation_directory_and_composer_files_are_unreachable_by_delete() {
        let directory = tempfile::tempdir().unwrap();
        let invalid_dir = directory.path().join("attachments").join("not a uuid");
        fs::create_dir_all(&invalid_dir).unwrap();
        let generated_shape =
            invalid_dir.join(format!("{GENERATED_FILE_PREFIX}{ATTACHMENT_A}.png"));
        fs::write(&generated_shape, b"preserve").unwrap();
        let valid_dir = managed_attachment_dir(directory.path(), CONVERSATION_A);
        fs::create_dir_all(&valid_dir).unwrap();
        let composer_file = valid_dir.join("message-attachment.png");
        fs::write(&composer_file, b"composer").unwrap();

        let summary = reconcile_generated_artifacts(directory.path().to_path_buf(), Vec::new());

        assert!(generated_shape.exists());
        assert!(composer_file.exists());
        assert!(summary.unsafe_entries > 0);
        assert_eq!(summary.removed_orphans, 0);
    }
}

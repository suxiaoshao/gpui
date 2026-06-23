use ai_chat_agent::{SkillCatalog, SkillCatalogEntry, SkillLoader};
use ai_chat_core::{ContentPart, SkillSourceKind};
use gpui::{App, AppContext};
use gpui_store::{SharedStore, StoreBackend, StoreBackendFuture, StoreBackendId, StoreState};
use std::{
    convert::Infallible,
    path::{Path, PathBuf},
};
use time::OffsetDateTime;

pub(crate) type GlobalSkillCatalogStore =
    SharedStore<GlobalSkillCatalogState, GlobalSkillCatalogBackend>;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct GlobalSkillCatalogState {
    entries: Vec<GlobalSkillEntry>,
    last_refreshed_at: Option<OffsetDateTime>,
    last_error: Option<String>,
}

impl GlobalSkillCatalogState {
    pub(crate) fn entries(&self) -> &[GlobalSkillEntry] {
        &self.entries
    }

    pub(crate) fn entry_records(&self) -> &Vec<GlobalSkillEntry> {
        &self.entries
    }

    pub(crate) fn last_refreshed_at(&self) -> Option<OffsetDateTime> {
        self.last_refreshed_at
    }

    pub(crate) fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }
}

impl StoreState for GlobalSkillCatalogState {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GlobalSkillEntry {
    pub(crate) name: String,
    pub(crate) description: Option<String>,
    pub(crate) source_kind: SkillSourceKind,
    pub(crate) skill_file_path: PathBuf,
    pub(crate) directory_path: PathBuf,
    pub(crate) search_text: String,
}

impl GlobalSkillEntry {
    fn from_catalog_entry(entry: &SkillCatalogEntry) -> Self {
        let skill_file_path = entry.skill_file_path.clone();
        let directory_path = entry.directory_path.clone();
        let source_kind = entry.source_kind;
        let name = entry.name.clone();
        let description = entry.description.clone();
        let search_text = skill_search_text(
            &name,
            description.as_deref(),
            source_kind,
            &skill_file_path,
            &directory_path,
        );

        Self {
            name,
            description,
            source_kind,
            skill_file_path,
            directory_path,
            search_text,
        }
    }

    fn to_catalog_entry(&self) -> SkillCatalogEntry {
        SkillCatalogEntry {
            name: self.name.clone(),
            description: self.description.clone(),
            skill_file_path: self.skill_file_path.clone(),
            directory_path: self.directory_path.clone(),
            source_kind: self.source_kind,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LoadedSkillContent {
    pub(crate) content: String,
    pub(crate) content_sha256: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GlobalSkillCatalogSnapshot {
    entries: Option<Vec<GlobalSkillEntry>>,
    last_refreshed_at: OffsetDateTime,
    last_error: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct GlobalSkillCatalogBackend;

impl StoreBackend<GlobalSkillCatalogState> for GlobalSkillCatalogBackend {
    type Snapshot = GlobalSkillCatalogSnapshot;
    type Event = ();
    type Subscription = ();
    type Error = Infallible;

    fn backend_id(&self) -> StoreBackendId {
        StoreBackendId::new("filesystem:global-skills")
    }

    fn load(&self) -> StoreBackendFuture<Option<Self::Snapshot>, Self::Error> {
        Ok(Some(load_global_skill_catalog_snapshot()))
    }

    fn reconcile(&self, state: &mut GlobalSkillCatalogState, snapshot: Self::Snapshot) -> bool {
        let next_entries = snapshot.entries.unwrap_or_else(|| state.entries.clone());
        if state.entries == next_entries
            && state.last_refreshed_at == Some(snapshot.last_refreshed_at)
            && state.last_error == snapshot.last_error
        {
            return false;
        }

        state.entries = next_entries;
        state.last_refreshed_at = Some(snapshot.last_refreshed_at);
        state.last_error = snapshot.last_error;
        true
    }
}

pub(crate) fn init(cx: &mut App) {
    GlobalSkillCatalogStore::install_global_with_backend(
        cx,
        GlobalSkillCatalogState::default(),
        GlobalSkillCatalogBackend,
    )
    .expect("global skill catalog backend is infallible");
}

pub(crate) fn catalog(cx: &impl AppContext) -> GlobalSkillCatalogStore {
    GlobalSkillCatalogStore::global(cx)
}

pub(crate) fn refresh_global_catalog(cx: &mut App) {
    catalog(cx)
        .refresh_from_backend(cx)
        .expect("global skill catalog backend is infallible");
}

pub(crate) fn load_skill_content(
    entry: GlobalSkillEntry,
) -> ai_chat_agent::Result<LoadedSkillContent> {
    let activation = SkillLoader::new().load(&entry.to_catalog_entry())?;
    let content = activation
        .content
        .into_iter()
        .filter_map(|part| match part {
            ContentPart::Text { text } => Some(text),
            ContentPart::Image { .. }
            | ContentPart::File { .. }
            | ContentPart::Audio { .. }
            | ContentPart::Attachment { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n");

    Ok(LoadedSkillContent {
        content,
        content_sha256: activation.content_sha256,
    })
}

fn load_global_skill_catalog_snapshot() -> GlobalSkillCatalogSnapshot {
    let last_refreshed_at = OffsetDateTime::now_utc();
    match SkillCatalog::scan(None) {
        Ok(catalog) => GlobalSkillCatalogSnapshot {
            entries: Some(entries_from_catalog(&catalog)),
            last_refreshed_at,
            last_error: None,
        },
        Err(err) => GlobalSkillCatalogSnapshot {
            entries: None,
            last_refreshed_at,
            last_error: Some(err.to_string()),
        },
    }
}

fn entries_from_catalog(catalog: &SkillCatalog) -> Vec<GlobalSkillEntry> {
    let mut entries = catalog
        .entries()
        .map(GlobalSkillEntry::from_catalog_entry)
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        skill_source_rank(left.source_kind)
            .cmp(&skill_source_rank(right.source_kind))
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.skill_file_path.cmp(&right.skill_file_path))
    });
    entries
}

fn skill_search_text(
    name: &str,
    description: Option<&str>,
    source_kind: SkillSourceKind,
    skill_file_path: &Path,
    directory_path: &Path,
) -> String {
    [
        name.to_lowercase(),
        description.unwrap_or_default().to_lowercase(),
        skill_source_keyword(source_kind).to_owned(),
        skill_file_path.to_string_lossy().to_lowercase(),
        directory_path.to_string_lossy().to_lowercase(),
    ]
    .join(" ")
}

fn skill_source_keyword(source_kind: SkillSourceKind) -> &'static str {
    match source_kind {
        SkillSourceKind::BuiltIn => "built-in builtin bundled system",
        SkillSourceKind::User => "user global",
        SkillSourceKind::Project => "project workspace",
        SkillSourceKind::Plugin => "plugin",
    }
}

fn skill_source_rank(source_kind: SkillSourceKind) -> u8 {
    match source_kind {
        SkillSourceKind::BuiltIn => 0,
        SkillSourceKind::User => 1,
        SkillSourceKind::Plugin => 2,
        SkillSourceKind::Project => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn catalog_entries_include_searchable_metadata() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let skill_root = temp.path().join("skills");
        let rust_skill = skill_root.join("rust");
        let browser_skill = skill_root.join("browser");
        fs::create_dir_all(&rust_skill).expect("create rust skill dir");
        fs::create_dir_all(&browser_skill).expect("create browser skill dir");
        fs::write(
            rust_skill.join("SKILL.md"),
            "---\nname: rust\ndescription: Rust workflow\n---\nUse cargo test.\n",
        )
        .expect("write rust skill");
        fs::write(browser_skill.join("SKILL.md"), "# Browser\n").expect("write browser skill");

        let mut catalog = SkillCatalog::default();
        catalog
            .scan_root(&skill_root, SkillSourceKind::User)
            .expect("scan temp skills");
        let entries = entries_from_catalog(&catalog);

        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            vec!["browser", "rust"]
        );
        let rust = entries
            .iter()
            .find(|entry| entry.name == "rust")
            .expect("rust skill exists");
        assert_eq!(rust.description.as_deref(), Some("Rust workflow"));
        assert_eq!(rust.source_kind, SkillSourceKind::User);
        assert!(rust.search_text.contains("rust workflow"));
        assert!(rust.search_text.contains("user global"));
        assert!(rust.search_text.contains("skill.md"));
    }

    #[test]
    fn load_skill_content_reads_raw_skill_file() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let skill_dir = temp.path().join("skills/rust");
        fs::create_dir_all(&skill_dir).expect("create skill dir");
        let skill_file_path = skill_dir.join("SKILL.md");
        fs::write(
            &skill_file_path,
            "---\nname: rust\n---\nUse cargo test.\nSecond line.\n",
        )
        .expect("write skill");
        let entry = GlobalSkillEntry {
            name: "rust".to_string(),
            description: None,
            source_kind: SkillSourceKind::User,
            skill_file_path,
            directory_path: skill_dir,
            search_text: "rust".to_string(),
        };

        let content = load_skill_content(entry).expect("load content");

        assert_eq!(
            content.content,
            "---\nname: rust\n---\nUse cargo test.\nSecond line.\n"
        );
        assert_eq!(content.content_sha256.len(), 64);
    }
}

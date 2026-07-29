use std::{
    collections::BTreeMap,
    fmt,
    path::{Path, PathBuf},
};

use gpui::Task;
use gpui_operation::refresh;
use jaco_agent::{SkillCatalog, SkillCatalogEntry, SkillLoader};
use jaco_core::{ContentPart, SkillSourceKind};

pub(crate) type SkillCatalogOperation =
    refresh::Operation<SkillCatalogData, SkillCatalogProblem, Task<()>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SkillCatalogScope {
    Global,
    Project { root: PathBuf },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SkillCatalogData {
    entries: Vec<GlobalSkillEntry>,
    details: BTreeMap<PathBuf, LoadedSkillContent>,
}

impl SkillCatalogData {
    pub(crate) fn entries(&self) -> &[GlobalSkillEntry] {
        &self.entries
    }

    pub(crate) fn details(&self) -> &BTreeMap<PathBuf, LoadedSkillContent> {
        &self.details
    }
}

#[derive(Debug)]
pub(crate) struct SkillCatalogProblem(jaco_agent::AgentRuntimeError);

impl fmt::Display for SkillCatalogProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for SkillCatalogProblem {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

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

pub(crate) fn load_skill_content(
    entry: GlobalSkillEntry,
) -> jaco_agent::Result<LoadedSkillContent> {
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

pub(crate) fn load_catalog(
    scope: SkillCatalogScope,
) -> Result<SkillCatalogData, SkillCatalogProblem> {
    let entries = load_catalog_entries(scope).map_err(SkillCatalogProblem)?;
    let details = entries
        .iter()
        .cloned()
        .map(|entry| {
            let path = entry.skill_file_path.clone();
            load_skill_content(entry)
                .map(|content| (path, content))
                .map_err(SkillCatalogProblem)
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    Ok(SkillCatalogData { entries, details })
}

pub(crate) fn load_catalog_entries(
    scope: SkillCatalogScope,
) -> jaco_agent::Result<Vec<GlobalSkillEntry>> {
    let catalog = match scope {
        SkillCatalogScope::Global => SkillCatalog::scan(None)?,
        SkillCatalogScope::Project { root } => SkillCatalog::scan(Some(root.as_path()))?,
    };
    Ok(entries_from_catalog(&catalog))
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
        SkillSourceKind::Project => 0,
        SkillSourceKind::User => 1,
        SkillSourceKind::Plugin => 2,
        SkillSourceKind::BuiltIn => 3,
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

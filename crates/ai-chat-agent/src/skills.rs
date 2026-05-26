use crate::Result;
use ai_chat_core::*;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillActivationRequest {
    pub name: String,
}

impl SkillActivationRequest {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillCatalogEntry {
    pub name: String,
    pub description: Option<String>,
    pub skill_file_path: PathBuf,
    pub directory_path: PathBuf,
    pub source_kind: SkillSourceKind,
}

#[derive(Debug, Clone, Default)]
pub struct SkillCatalog {
    entries: BTreeMap<String, SkillCatalogEntry>,
}

impl SkillCatalog {
    pub fn scan(project_root: Option<&Path>) -> Result<Self> {
        let mut catalog = Self::default();
        if let Some(home) = std::env::var_os("HOME") {
            catalog.scan_root(
                PathBuf::from(home).join(".agents/skills"),
                SkillSourceKind::User,
            )?;
        }
        if let Some(project_root) = project_root {
            catalog.scan_root(
                project_root.join(".agents/skills"),
                SkillSourceKind::Project,
            )?;
        }
        Ok(catalog)
    }

    pub fn scan_root(
        &mut self,
        root: impl AsRef<Path>,
        source_kind: SkillSourceKind,
    ) -> Result<()> {
        let root = root.as_ref();
        if !root.is_dir() {
            return Ok(());
        }

        for entry in fs::read_dir(root)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let skill_file_path = path.join("SKILL.md");
            if !skill_file_path.is_file() {
                continue;
            }
            let content = fs::read_to_string(&skill_file_path)?;
            let metadata = parse_frontmatter_metadata(&content);
            let fallback_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("skill")
                .to_string();
            let name = metadata.name.unwrap_or(fallback_name);
            self.entries.insert(
                name.clone(),
                SkillCatalogEntry {
                    name,
                    description: metadata.description,
                    skill_file_path,
                    directory_path: path,
                    source_kind,
                },
            );
        }

        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&SkillCatalogEntry> {
        self.entries.get(name)
    }

    pub fn entries(&self) -> impl Iterator<Item = &SkillCatalogEntry> {
        self.entries.values()
    }

    pub fn catalog_hash(&self) -> String {
        let mut hasher = Sha256::new();
        for entry in self.entries.values() {
            hasher.update(entry.name.as_bytes());
            hasher.update(b"\0");
            hasher.update(entry.skill_file_path.to_string_lossy().as_bytes());
            hasher.update(b"\0");
        }
        hex::encode(hasher.finalize())
    }
}

#[derive(Debug, Clone, Default)]
pub struct SkillLoader;

impl SkillLoader {
    pub fn new() -> Self {
        Self
    }

    pub fn load(&self, entry: &SkillCatalogEntry) -> Result<SkillActivationItem> {
        let content = fs::read_to_string(&entry.skill_file_path)?;
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let content_sha256 = hex::encode(hasher.finalize());
        Ok(SkillActivationItem {
            name: entry.name.clone(),
            source_kind: entry.source_kind,
            skill_file_path: entry.skill_file_path.to_string_lossy().to_string(),
            directory_path: entry.directory_path.to_string_lossy().to_string(),
            content_sha256,
            content: vec![ContentPart::Text { text: content }],
        })
    }
}

#[derive(Default)]
struct FrontmatterMetadata {
    name: Option<String>,
    description: Option<String>,
}

fn parse_frontmatter_metadata(content: &str) -> FrontmatterMetadata {
    let mut metadata = FrontmatterMetadata::default();
    let mut lines = content.lines();
    if lines.next() != Some("---") {
        return metadata;
    }

    for line in lines {
        if line == "---" {
            break;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();
        match key.trim() {
            "name" if !value.is_empty() => metadata.name = Some(value),
            "description" if !value.is_empty() => metadata.description = Some(value),
            _ => {}
        }
    }
    metadata
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_loader_keeps_snapshot_when_file_changes() {
        let temp = tempfile::tempdir().unwrap();
        let skill_dir = temp.path().join(".agents/skills/rust");
        std::fs::create_dir_all(&skill_dir).unwrap();
        let skill_file = skill_dir.join("SKILL.md");
        std::fs::write(
            &skill_file,
            "---\nname: rust\ndescription: Rust workflow\n---\nUse cargo test.\n",
        )
        .unwrap();

        let catalog = SkillCatalog::scan(Some(temp.path())).unwrap();
        let entry = catalog.get("rust").unwrap();
        let first = SkillLoader::new().load(entry).unwrap();
        std::fs::write(&skill_file, "---\nname: rust\n---\nUse cargo clippy.\n").unwrap();
        let second = SkillLoader::new().load(entry).unwrap();

        assert_ne!(first.content_sha256, second.content_sha256);
        assert_eq!(
            first.content,
            vec![ContentPart::Text {
                text: "---\nname: rust\ndescription: Rust workflow\n---\nUse cargo test.\n"
                    .to_string(),
            }]
        );
    }
}

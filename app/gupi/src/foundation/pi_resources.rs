//! Personal Pi resources. Never imports extension code or reads project settings.
mod discovery;

use super::persistence;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub(crate) use discovery::scan;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Extension,
    Skill,
    Prompt,
    Theme,
}
impl Kind {
    pub fn key(self) -> &'static str {
        match self {
            Self::Extension => "extensions",
            Self::Skill => "skills",
            Self::Prompt => "prompts",
            Self::Theme => "themes",
        }
    }
}
#[derive(Clone, Debug)]
pub(crate) struct Resource {
    pub kind: Kind,
    pub path: PathBuf,
    pub base: PathBuf,
    pub package: Option<String>,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub editable: bool,
}
#[derive(Clone, Debug)]
pub(crate) struct Package {
    pub source: String,
    pub path: PathBuf,
    pub version: Option<String>,
}
#[derive(Clone, Debug, Default)]
pub(crate) struct Catalog {
    pub root: PathBuf,
    pub packages: Vec<Package>,
    pub resources: Vec<Resource>,
    pub warnings: Vec<String>,
}
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub(crate) struct Error(pub String);
impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self(error.to_string())
    }
}
pub(crate) fn agent_dir() -> Result<PathBuf, Error> {
    // Share the session catalog's environment and relative-path convention.
    Ok(super::session_catalog::Discovery::environment()?.agent)
}
pub(super) fn resolve(base: &Path, text: &str) -> PathBuf {
    let path = if text == "~" {
        dirs_next::home_dir().unwrap_or_else(|| base.to_owned())
    } else if let Some(path) = text.strip_prefix("~/") {
        dirs_next::home_dir()
            .unwrap_or_else(|| base.to_owned())
            .join(path)
    } else {
        base.join(text)
    };
    let mut normalized = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            part => normalized.push(part.as_os_str()),
        }
    }
    normalized
}
pub(super) fn read_json(path: &Path) -> Result<Value, Error> {
    match persistence::read(path)? {
        None => Ok(json!({})),
        Some(bytes) => {
            serde_json::from_slice(&bytes).map_err(|e| Error(format!("{}: {e}", path.display())))
        }
    }
}
pub(super) fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}
fn write_settings(
    root: &Path,
    change: impl FnOnce(&mut Value) -> Result<(), Error>,
) -> Result<(), Error> {
    let path = root.join("settings.json");
    let mut value = read_json(&path)?;
    if !value.is_object() {
        return Err(Error("Pi settings must be an object".into()));
    }
    change(&mut value)?;
    let mut bytes = serde_json::to_vec_pretty(&value).map_err(|e| Error(e.to_string()))?;
    bytes.push(b'\n');
    persistence::write_atomic(&path, &bytes)?;
    Ok(())
}
pub(crate) fn set_enabled(root: &Path, resource: &Resource, enabled: bool) -> Result<(), Error> {
    write_settings(root, |settings| {
        let target = if let Some(source) = &resource.package {
            let packages = settings["packages"]
                .as_array_mut()
                .ok_or_else(|| Error("Package no longer configured".into()))?;
            let entry = packages
                .iter_mut()
                .find(|v| v.as_str().or_else(|| v["source"].as_str()) == Some(source))
                .ok_or_else(|| Error("Package no longer configured".into()))?;
            if entry.is_string() {
                *entry = json!({"source": source});
            }
            entry
        } else {
            settings
        };
        let key = resource.kind.key();
        let mut patterns = strings(&target[key]);
        // An explicitly empty package filter means all disabled, unlike an absent filter.
        if resource.package.is_some()
            && target[key].as_array().is_some_and(Vec::is_empty)
            && target["autoload"] != false
        {
            patterns.push("!**".into());
        }
        patterns.retain(|p| {
            !p.strip_prefix(['+', '-'])
                .is_some_and(|p| discovery::exact(&resource.path, p, &resource.base))
        });
        let path = resource
            .path
            .strip_prefix(&resource.base)
            .unwrap_or(&resource.path)
            .to_string_lossy()
            .replace('\\', "/");
        patterns.push(format!("{}{path}", if enabled { '+' } else { '-' }));
        target[key] = json!(patterns);
        Ok(())
    })
}
pub(crate) fn register_skill(root: &Path, path: &Path) -> Result<(), Error> {
    if !path.is_absolute() || !path.exists() {
        return Err(Error(
            "Choose an existing absolute file or directory".into(),
        ));
    }
    write_settings(root, |settings| {
        let mut paths = strings(&settings["skills"]);
        let path = path.to_string_lossy().into_owned();
        if !paths.contains(&path) {
            paths.push(path);
        }
        settings["skills"] = json!(paths);
        Ok(())
    })
}
pub(crate) fn create_path(root: &Path, kind: Kind, name: &str) -> Result<PathBuf, Error> {
    let name = name.trim();
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    {
        return Err(Error(
            "Use letters, digits, hyphens or underscores for the resource name".into(),
        ));
    }
    Ok(match kind {
        Kind::Skill => root.join("skills").join(name).join("SKILL.md"),
        Kind::Prompt => root.join("prompts").join(format!("{name}.md")),
        _ => return Err(Error("Resource type is not editable".into())),
    })
}
pub(crate) fn save_text(path: &Path, text: &str, create: bool) -> Result<(), Error> {
    if create {
        use std::io::Write;
        let parent = path
            .parent()
            .ok_or_else(|| Error("Missing parent".into()))?;
        std::fs::create_dir_all(parent)?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        file.write_all(text.as_bytes())?;
        file.as_file().sync_all()?;
        file.persist_noclobber(path)
            .map_err(|e| Error(e.to_string()))?;
    } else {
        persistence::write_atomic(path, text.as_bytes())?;
    }
    Ok(())
}
pub(crate) async fn package_action(
    command: PathBuf,
    root: PathBuf,
    action: &'static str,
    source: String,
) -> Result<(), Error> {
    let source = source.trim();
    if source.is_empty() || source.starts_with('-') {
        return Err(Error("Enter a package source".into()));
    }
    tokio::fs::create_dir_all(&root).await?;
    let mut cmd = tokio::process::Command::new(command);
    cmd.env("PI_CODING_AGENT_DIR", &root)
        .current_dir(&root)
        .arg(action);
    if action == "update" {
        cmd.arg("--extension");
    }
    let output = cmd
        .arg(source)
        .arg("--no-approve")
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true)
        .output()
        .await?;
    if output.status.success() {
        Ok(())
    } else {
        let message = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
        Err(Error(format!(
            "Pi {action} ({}): {}",
            output.status,
            message.chars().take(4000).collect::<String>()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn text_creation_never_overwrites_and_registration_keeps_external_file() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("agent");
        let path = create_path(&root, Kind::Skill, "sample").unwrap();
        save_text(&path, "original", true).unwrap();
        assert!(save_text(&path, "overwrite", true).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "original");
        let external = temp.path().join("external.md");
        std::fs::write(&external, "external").unwrap();
        register_skill(&root, &external).unwrap();
        let catalog = scan(root.clone(), None).unwrap();
        assert!(
            catalog
                .resources
                .iter()
                .any(|r| r.path == external && !r.editable)
        );
        assert_eq!(std::fs::read_to_string(external).unwrap(), "external");
        assert!(!root.join("skills/external.md").exists());
        assert!(create_path(&root, Kind::Prompt, "../escape").is_err());
        save_text(&root.join("SYSTEM.md"), "replacement", false).unwrap();
        save_text(&root.join("APPEND_SYSTEM.md"), "additional", false).unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join("SYSTEM.md")).unwrap(),
            "replacement"
        );
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn package_cli_uses_personal_directory_and_targeted_update() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("personal agent");
        let cli = temp.path().join("fake-pi");
        std::fs::write(
            &cli,
            "#!/bin/sh\nprintf '%s\\n' \"$PWD\" \"$PI_CODING_AGENT_DIR\" \"$@\" > args\n",
        )
        .unwrap();
        std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap();
        package_action(
            cli.clone(),
            root.clone(),
            "update",
            "npm:test;echo unexpected".into(),
        )
        .await
        .unwrap();
        let args = std::fs::read_to_string(root.join("args")).unwrap();
        let lines: Vec<_> = args.lines().collect();
        assert_eq!(
            PathBuf::from(lines[0]).canonicalize().unwrap(),
            root.canonicalize().unwrap()
        );
        assert_eq!(lines[1], root.to_string_lossy());
        assert_eq!(
            &lines[2..],
            &[
                "update",
                "--extension",
                "npm:test;echo unexpected",
                "--no-approve"
            ]
        );
        std::fs::write(&cli, "#!/bin/sh\necho failure >&2\nexit 4\n").unwrap();
        assert!(
            package_action(cli, root, "remove", "npm:test".into())
                .await
                .unwrap_err()
                .to_string()
                .contains("failure")
        );
    }
}

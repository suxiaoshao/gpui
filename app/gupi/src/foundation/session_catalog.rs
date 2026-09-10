//! Read-only discovery. Pi remains the authority for loading conversation content.
use serde_json::Value;
use std::{
    collections::{BTreeSet, VecDeque},
    fs,
    io::{self, BufRead, BufReader},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub(crate) struct SessionInfo {
    pub path: PathBuf,
    pub id: String,
    pub cwd: PathBuf,
    pub name: Option<String>,
    pub first_message: String,
    pub activity: String,
    pub parent_session: Option<String>,
}
impl SessionInfo {
    pub fn key(&self) -> String {
        self.path.to_string_lossy().into_owned()
    }
    pub fn title(&self) -> &str {
        self.name
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(&self.first_message)
    }
}
#[derive(Clone, Debug)]
pub(crate) struct Discovery {
    pub home: PathBuf,
    pub agent: PathBuf,
    pub current: PathBuf,
    pub session_override: Option<PathBuf>,
}
impl Discovery {
    pub fn environment() -> io::Result<Self> {
        let home =
            dirs_next::home_dir().ok_or_else(|| io::Error::other("home directory unavailable"))?;
        let current = std::env::current_dir()?;
        let agent = std::env::var("PI_CODING_AGENT_DIR")
            .ok()
            .filter(|v| !v.is_empty())
            .map(|v| expand(&v, &home, &current))
            .unwrap_or_else(|| home.join(".pi/agent"));
        let session_override = std::env::var("PI_CODING_AGENT_SESSION_DIR")
            .ok()
            .filter(|v| !v.is_empty())
            .map(|v| expand(&v, &home, &current));
        Ok(Self {
            home,
            agent,
            current,
            session_override,
        })
    }
}
#[derive(Default)]
pub(crate) struct Catalog {
    pub sessions: Vec<SessionInfo>,
    pub directories: BTreeSet<PathBuf>,
    pub warnings: Vec<String>,
}
fn expand(value: &str, home: &Path, cwd: &Path) -> PathBuf {
    let path = if value == "~" {
        home.to_path_buf()
    } else if let Some(rest) = value.strip_prefix("~/") {
        home.join(rest)
    } else {
        PathBuf::from(value)
    };
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}
fn configured_dir(path: &Path) -> Option<String> {
    let value: Value = serde_json::from_slice(&fs::read(path).ok()?).ok()?;
    value
        .get("sessionDir")?
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}
/// Additional known projects come from drafts and the current directory picker.
pub(crate) fn scan(options: &Discovery, known: &[PathBuf]) -> Catalog {
    let mut result = Catalog::default();
    let mut seen_files = BTreeSet::new();
    let mut seen_directories = BTreeSet::new();
    let mut seen_projects = BTreeSet::new();
    let mut directories = VecDeque::new();
    let mut projects = VecDeque::from([options.current.clone()]);
    projects.extend(known.iter().cloned());
    // The default root contains per-cwd directories, while an override is flat.
    let default_root = options.agent.join("sessions");
    result.directories.insert(default_root.clone());
    if let Ok(children) = fs::read_dir(&default_root) {
        for child in children.flatten() {
            if child.path().is_dir() {
                directories.push_back(child.path());
            }
        }
    }
    if let Some(path) = &options.session_override {
        directories.push_back(path.clone());
    }
    let global = configured_dir(&options.agent.join("settings.json"));
    while !projects.is_empty() || !directories.is_empty() {
        while let Some(cwd) = projects.pop_front() {
            if !seen_projects.insert(cwd.clone()) {
                continue;
            }
            // Resolve the effective global/project setting in each known cwd.
            if let Some(dir) =
                configured_dir(&cwd.join(".pi/settings.json")).or_else(|| global.clone())
            {
                directories.push_back(expand(&dir, &options.home, &cwd));
            }
        }
        let Some(dir) = directories.pop_front() else {
            continue;
        };
        let dir = fs::canonicalize(&dir).unwrap_or(dir);
        if !seen_directories.insert(dir.clone()) {
            continue;
        }
        result.directories.insert(dir.clone());
        let children = match fs::read_dir(&dir) {
            Ok(children) => children,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => {
                result.warnings.push(format!("{}: {error}", dir.display()));
                continue;
            }
        };
        for child in children.flatten() {
            let path = child.path();
            if path.extension().is_none_or(|s| s != "jsonl") || !path.is_file() {
                continue;
            }
            let path = fs::canonicalize(&path).unwrap_or(path);
            if !seen_files.insert(path.clone()) {
                continue;
            }
            match read_metadata(&path) {
                Ok(info) => {
                    projects.push_back(info.cwd.clone());
                    result.sessions.push(info);
                }
                Err(error) => result.warnings.push(format!("{}: {error}", path.display())),
            }
        }
    }
    result.sessions.sort_by(|a, b| {
        b.activity
            .cmp(&a.activity)
            .then_with(|| a.path.cmp(&b.path))
    });
    result
}
pub(crate) fn text_content(message: &Value) -> String {
    match message.get("content") {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter(|p| p.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|p| p.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}
pub(crate) fn summary(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(120)
        .collect()
}
pub(crate) fn read_metadata(path: &Path) -> io::Result<SessionInfo> {
    let file = fs::File::open(path)?;
    let mut lines = BufReader::new(file).lines();
    let first = lines
        .next()
        .transpose()?
        .ok_or_else(|| io::Error::other("empty session"))?;
    let header: Value =
        serde_json::from_str(first.trim_start_matches('\u{feff}')).map_err(io::Error::other)?;
    if header.get("type").and_then(Value::as_str) != Some("session") {
        return Err(io::Error::other("invalid session header"));
    }
    let string = |key: &str| {
        header
            .get(key)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| io::Error::other(format!("missing session {key}")))
    };
    let mut info = SessionInfo {
        path: path.to_path_buf(),
        id: string("id")?,
        cwd: PathBuf::from(string("cwd")?),
        name: None,
        first_message: String::new(),
        activity: String::new(),
        parent_session: header
            .get("parentSession")
            .and_then(Value::as_str)
            .map(str::to_owned),
    };
    for line in lines {
        let line = line?;
        let Ok(entry) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        match entry.get("type").and_then(Value::as_str) {
            Some("session_info") => {
                info.name = entry.get("name").and_then(Value::as_str).map(str::to_owned);
            }
            Some("message") => {
                let message = &entry["message"];
                let role = message.get("role").and_then(Value::as_str);
                if role == Some("user") && info.first_message.is_empty() {
                    info.first_message = summary(&text_content(message));
                }
                if matches!(role, Some("user" | "assistant"))
                    && let Some(timestamp) = entry.get("timestamp").and_then(Value::as_str)
                    && timestamp > info.activity.as_str()
                {
                    info.activity = timestamp.to_owned();
                }
            }
            _ => {}
        }
    }
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn discover_default_custom_and_deduplicate_without_using_file_mtime() {
        let root = tempfile::tempdir().unwrap();
        let agent = root.path().join("agent");
        let cwd = root.path().join("project");
        let default = agent.join("sessions/project");
        let custom = cwd.join("custom");
        fs::create_dir_all(&default).unwrap();
        fs::create_dir_all(&custom).unwrap();
        fs::create_dir_all(cwd.join(".pi")).unwrap();
        fs::write(cwd.join(".pi/settings.json"), r#"{"sessionDir":"custom"}"#).unwrap();
        let file = |name: &str, time: &str| {
            format!(
                "{}\n{}\n{}\n{}\n",
                json!({"type":"session","id":name,"cwd":cwd}),
                json!({"type":"message","message":{"role":"user","content":[{"type":"text","text":name}]},"timestamp":time}),
                json!({"type":"message","message":{"role":"toolResult"},"timestamp":"2099-01-01T00:00:00Z"}),
                json!({"type":"session_info","name":"renamed"})
            )
        };
        fs::write(default.join("a.jsonl"), file("a", "2026-01-01T00:00:00Z")).unwrap();
        fs::write(custom.join("b.jsonl"), file("b", "2026-02-01T00:00:00Z")).unwrap();
        fs::write(custom.join("broken.jsonl"), "bad").unwrap();
        let result = scan(
            &Discovery {
                home: root.path().into(),
                agent,
                current: cwd.clone(),
                session_override: Some(custom),
            },
            &[cwd],
        );
        assert_eq!(result.sessions.len(), 2);
        assert_eq!(result.sessions[0].id, "b");
        assert_eq!(result.sessions[0].title(), "renamed");
        assert_eq!(result.sessions[1].activity, "2026-01-01T00:00:00Z");
        assert_eq!(result.warnings.len(), 1);
    }
}

//! Read-only discovery. Pi remains the authority for loading conversation content.
use serde_json::Value;
use std::{
    collections::{BTreeSet, VecDeque},
    fs,
    io::{self, BufRead, BufReader},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

#[derive(Clone, Debug, PartialEq, Eq)]
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
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScanProgress {
    Discovering { files: usize },
    Reading { completed: usize, total: usize },
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
fn at_path(path: &Path, error: impl std::fmt::Display) -> io::Error {
    io::Error::other(format!("{}: {error}", path.display()))
}
fn configured_dir(path: &Path) -> io::Result<Option<String>> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(at_path(path, error)),
    };
    let value: Value = serde_json::from_slice(&bytes).map_err(|e| at_path(path, e))?;
    Ok(value
        .get("sessionDir")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned))
}
fn check_cancel(cancel: &AtomicBool) -> io::Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(io::Error::new(io::ErrorKind::Interrupted, "scan cancelled"))
    } else {
        Ok(())
    }
}
fn directory(path: &Path) -> io::Result<Option<fs::ReadDir>> {
    match fs::read_dir(path) {
        Ok(entries) => Ok(Some(entries)),
        // Pi creates session directories lazily. A missing directory is empty.
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(at_path(path, error)),
    }
}
/// Additional known projects come from drafts and the current directory picker.
pub(crate) fn scan(
    options: &Discovery,
    known: &[PathBuf],
    cancel: &AtomicBool,
    mut progress: impl FnMut(ScanProgress),
) -> io::Result<Catalog> {
    let mut last_report = Instant::now();
    let mut report = |value, force| {
        if force || last_report.elapsed() >= Duration::from_millis(100) {
            progress(value);
            last_report = Instant::now();
        }
    };
    report(ScanProgress::Discovering { files: 0 }, true);
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
    if let Some(children) = directory(&default_root)? {
        for child in children {
            check_cancel(cancel)?;
            let child = child.map_err(|e| at_path(&default_root, e))?;
            if child
                .path()
                .metadata()
                .map_err(|e| at_path(&child.path(), e))?
                .is_dir()
            {
                directories.push_back(child.path());
            }
        }
    }
    if let Some(path) = &options.session_override {
        directories.push_back(path.clone());
    }
    let global = configured_dir(&options.agent.join("settings.json"))?;
    while !projects.is_empty() || !directories.is_empty() {
        check_cancel(cancel)?;
        while let Some(cwd) = projects.pop_front() {
            if !seen_projects.insert(cwd.clone()) {
                continue;
            }
            // Resolve the effective global/project setting in each known cwd.
            if let Some(dir) =
                configured_dir(&cwd.join(".pi/settings.json"))?.or_else(|| global.clone())
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
        let Some(children) = directory(&dir)? else {
            continue;
        };
        for child in children {
            check_cancel(cancel)?;
            let child = child.map_err(|e| at_path(&dir, e))?;
            let path = child.path();
            if path.extension().is_none_or(|s| s != "jsonl") {
                continue;
            }
            if !path.metadata().map_err(|e| at_path(&path, e))?.is_file() {
                continue;
            }
            let path = fs::canonicalize(&path).unwrap_or(path);
            if !seen_files.insert(path.clone()) {
                continue;
            }
            let file = fs::File::open(&path).map_err(|e| at_path(&path, e))?;
            let info =
                read_header(&path, &mut BufReader::new(file)).map_err(|e| at_path(&path, e))?;
            projects.push_back(info.cwd);
            report(
                ScanProgress::Discovering {
                    files: seen_files.len(),
                },
                false,
            );
        }
    }
    check_cancel(cancel)?;
    let files: Vec<_> = seen_files.into_iter().collect();
    let total = files.len();
    report(
        ScanProgress::Reading {
            completed: 0,
            total,
        },
        true,
    );
    // Keep blocking file reads off the UI thread and bound simultaneous opens.
    let next = AtomicUsize::new(0);
    let stopped = AtomicBool::new(false);
    std::thread::scope(|scope| -> io::Result<()> {
        let (sender, receiver) = mpsc::sync_channel(8);
        let mut workers = Vec::new();
        for _ in 0..total.min(8) {
            let sender = sender.clone();
            let files = &files;
            let next = &next;
            let stopped = &stopped;
            workers.push(scope.spawn(move || {
                while !stopped.load(Ordering::Relaxed) && !cancel.load(Ordering::Relaxed) {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(path) = files.get(index) else { break };
                    let result = read_metadata_cancelled(path, cancel, stopped)
                        .map_err(|e| at_path(path, e));
                    if sender.send(result).is_err() {
                        break;
                    }
                }
            }));
        }
        drop(sender);
        let mut failure = None;
        for item in receiver {
            match item {
                Ok(info) if failure.is_none() => {
                    result.sessions.push(info);
                    report(
                        ScanProgress::Reading {
                            completed: result.sessions.len(),
                            total,
                        },
                        result.sessions.len() == total,
                    );
                }
                Err(error) if failure.is_none() => {
                    failure = Some(error);
                    stopped.store(true, Ordering::Relaxed);
                }
                _ => {}
            }
        }
        for worker in workers {
            if worker.join().is_err() && failure.is_none() {
                failure = Some(io::Error::other("session reader panicked"));
            }
        }
        if let Some(error) = failure {
            return Err(error);
        }
        check_cancel(cancel)
    })?;
    result.sessions.sort_by(|a, b| {
        b.activity
            .cmp(&a.activity)
            .then_with(|| a.path.cmp(&b.path))
    });
    Ok(result)
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
    read_metadata_cancelled(path, &AtomicBool::new(false), &AtomicBool::new(false))
}
fn read_header(path: &Path, reader: &mut impl BufRead) -> io::Result<SessionInfo> {
    let mut first = String::new();
    if reader.read_line(&mut first)? == 0 {
        return Err(io::Error::other("empty session"));
    }
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
    Ok(SessionInfo {
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
    })
}
fn read_metadata_cancelled(
    path: &Path,
    cancel: &AtomicBool,
    stopped: &AtomicBool,
) -> io::Result<SessionInfo> {
    check_cancel(cancel)?;
    let file = fs::File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut info = read_header(path, &mut reader)?;
    for line in reader.lines() {
        check_cancel(cancel)?;
        check_cancel(stopped)?;
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let entry: Value = serde_json::from_str(&line).map_err(io::Error::other)?;
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
        let mut progress = vec![];
        let result = scan(
            &Discovery {
                home: root.path().into(),
                agent,
                current: cwd.clone(),
                session_override: Some(custom),
            },
            &[cwd],
            &AtomicBool::new(false),
            |value| progress.push(value),
        )
        .unwrap();
        assert_eq!(result.sessions.len(), 2);
        assert_eq!(result.sessions[0].id, "b");
        assert_eq!(result.sessions[0].title(), "renamed");
        assert_eq!(result.sessions[1].activity, "2026-01-01T00:00:00Z");
        let readings: Vec<_> = progress
            .into_iter()
            .filter_map(|value| match value {
                ScanProgress::Reading { completed, total } => Some((completed, total)),
                _ => None,
            })
            .collect();
        assert_eq!(readings.first(), Some(&(0, 2)));
        assert_eq!(readings.last(), Some(&(2, 2)));
        assert!(
            readings
                .windows(2)
                .all(|pair| pair[0].0 <= pair[1].0 && pair[1].1 == 2)
        );
    }
    #[test]
    fn discovered_project_extends_file_set_before_total_is_fixed_and_read_failure_aborts() {
        let root = tempfile::tempdir().unwrap();
        let agent = root.path().join("agent");
        let cwd = root.path().join("discovered");
        let default = agent.join("sessions/origin");
        fs::create_dir_all(&default).unwrap();
        fs::create_dir_all(cwd.join(".pi")).unwrap();
        fs::create_dir_all(cwd.join("custom")).unwrap();
        fs::write(cwd.join(".pi/settings.json"), r#"{"sessionDir":"custom"}"#).unwrap();
        let header = format!("{}\n", json!({"type":"session","id":"a","cwd":cwd}));
        fs::write(default.join("a.jsonl"), &header).unwrap();
        let extra = cwd.join("custom/b.jsonl");
        fs::write(&extra, &header).unwrap();
        let options = Discovery {
            home: root.path().into(),
            agent,
            current: root.path().join("current"),
            session_override: None,
        };
        let cancel = AtomicBool::new(false);
        let mut totals = vec![];
        let result = scan(&options, &[], &cancel, |value| {
            if let ScanProgress::Reading { total, .. } = value {
                totals.push(total);
            }
        })
        .unwrap();
        assert_eq!(result.sessions.len(), 2);
        assert!(totals.iter().all(|total| *total == 2));
        fs::write(&extra, format!("{header}invalid JSON\n")).unwrap();
        let error = scan(&options, &[], &cancel, |_| {}).err().unwrap();
        assert!(error.to_string().contains("b.jsonl"));
        fs::write(&extra, "invalid header").unwrap();
        let mut reading_started = false;
        assert!(
            scan(&options, &[], &cancel, |value| reading_started |=
                matches!(value, ScanProgress::Reading { .. }))
            .is_err()
        );
        assert!(!reading_started);
        cancel.store(true, Ordering::Relaxed);
        assert_eq!(
            scan(&options, &[], &cancel, |_| {}).err().unwrap().kind(),
            io::ErrorKind::Interrupted
        );
    }
    #[test]
    fn missing_default_directory_is_a_successful_empty_catalog() {
        let root = tempfile::tempdir().unwrap();
        let options = Discovery {
            home: root.path().into(),
            agent: root.path().join("agent"),
            current: root.path().into(),
            session_override: None,
        };
        let mut last = None;
        let result = scan(&options, &[], &AtomicBool::new(false), |value| {
            last = Some(value)
        })
        .unwrap();
        assert!(result.sessions.is_empty());
        assert_eq!(
            last,
            Some(ScanProgress::Reading {
                completed: 0,
                total: 0
            })
        );
    }
}

//! Offline metadata edits using Pi's session entry format.
use super::session_catalog::SessionInfo;
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    fs::OpenOptions,
    io::{self, Read, Write},
};

/// Like Pi TUI's SessionManager.open(path).appendSessionInfo(name).
/// The caller must use RPC instead when it owns a live instance of this session.
pub(crate) fn rename(info: &SessionInfo, name: &str) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .read(true)
        .append(true)
        .open(&info.path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("session path is not a regular file"));
    }
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let mut entries = contents
        .trim_start_matches('\u{feff}')
        .lines()
        .filter(|line| !line.trim().is_empty());
    let header: Value = serde_json::from_str(
        entries
            .next()
            .ok_or_else(|| io::Error::other("empty session"))?,
    )?;
    if header["type"] != "session"
        || header["id"].as_str() != Some(info.id.as_str())
        || header["cwd"].as_str().map(std::path::Path::new) != Some(info.cwd.as_path())
    {
        return Err(io::Error::other(
            "session identity or working directory changed",
        ));
    }
    if header["version"] != 3 {
        return Err(io::Error::other(
            "open this session in Pi to upgrade its format before renaming",
        ));
    }
    let mut ids = HashSet::new();
    let mut leaf = None;
    for line in entries {
        let entry: Value = serde_json::from_str(line)?;
        let id = entry["id"]
            .as_str()
            .ok_or_else(|| io::Error::other("session entry is missing its id"))?
            .to_owned();
        ids.insert(id.clone());
        leaf = Some(id);
    }
    let id = loop {
        let id = uuid::Uuid::new_v4().simple().to_string()[..8].to_owned();
        if !ids.contains(&id) {
            break id;
        }
    };
    let timestamp = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(io::Error::other)?;
    let entry = json!({"type":"session_info", "id":id, "parentId":leaf, "timestamp":timestamp, "name":name});
    let mut append = if contents.ends_with('\n') {
        String::new()
    } else {
        "\n".into()
    };
    append.push_str(&entry.to_string());
    append.push('\n');
    file.write_all(append.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rename_appends_one_entry_to_latest_leaf_and_preserves_original_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        let original = format!(
            "{}\n{}\n{}",
            json!({"type":"session","version":3,"id":"fixture","cwd":dir.path()}),
            json!({"type":"message","id":"first","parentId":null}),
            json!({"type":"message","id":"branch","parentId":null})
        );
        std::fs::write(&path, &original).unwrap();
        let info = super::super::session_catalog::read_metadata(&path).unwrap();
        rename(&info, "new name").unwrap();
        let bytes = std::fs::read_to_string(&path).unwrap();
        assert!(bytes.starts_with(&original));
        let entries = bytes
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[3]["type"], "session_info");
        assert_eq!(entries[3]["parentId"], "branch");
        assert_eq!(entries[3]["id"].as_str().unwrap().len(), 8);
        assert_eq!(
            super::super::session_catalog::read_metadata(&path)
                .unwrap()
                .name
                .as_deref(),
            Some("new name")
        );
        let mut stale = info;
        stale.id = "replaced".into();
        assert!(rename(&stale, "wrong session").is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), bytes);
    }
}

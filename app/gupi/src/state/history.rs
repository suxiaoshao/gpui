//! Shared Pi entry projection. UI filtering never changes parent links or the execution leaf.
mod graph;
mod nodes;
use crate::foundation::session_catalog::text_content;
pub(crate) use graph::HistoryGraph;
pub(crate) use nodes::HistoryKind;
use pi_rpc::protocol::{Entries, SessionEntry};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum HistoryMode {
    #[default]
    Brief,
    Detailed,
}

#[derive(Clone, Default)]
pub(crate) struct History {
    pub entries: Vec<SessionEntry>,
    pub leaf: Option<String>,
    index: HashMap<String, usize>,
    labels: HashMap<String, String>,
    descriptions: HashMap<String, nodes::Description>,
}
#[derive(Clone, Debug)]
pub(crate) struct HistoryRow {
    pub id: String,
    pub title: String,
    pub kind: HistoryKind,
    pub tool: Option<String>,
    pub timestamp: String,
    pub label: Option<String>,
    pub parent: Option<String>,
    pub indirect_parent: bool,
    pub current: bool,
}
impl History {
    pub fn replace(&mut self, entries: Entries) {
        self.entries = entries.entries;
        self.leaf = entries.leaf_id;
        self.index = self
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| (e.id.clone(), i))
            .collect();
        self.descriptions = nodes::describe(&self.entries);
        self.labels.clear();
        for e in &self.entries {
            if e.kind == "label"
                && let Some(target) = e.data.get("targetId").and_then(Value::as_str)
            {
                if let Some(label) = e
                    .data
                    .get("label")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                {
                    self.labels.insert(target.to_owned(), label.to_owned());
                } else {
                    self.labels.remove(target);
                }
            }
        }
    }
    pub fn entry(&self, id: &str) -> Option<&SessionEntry> {
        self.index.get(id).map(|i| &self.entries[*i])
    }
    pub fn path(&self, leaf: Option<&str>) -> Vec<&SessionEntry> {
        let mut result = Vec::new();
        let mut seen = HashSet::new();
        let mut next = leaf;
        while let Some(id) = next {
            if !seen.insert(id) {
                break;
            }
            let Some(entry) = self.entry(id) else {
                break;
            };
            result.push(entry);
            next = entry.parent_id.as_deref();
        }
        result.reverse();
        result
    }
    pub fn on_current_path(&self, id: &str) -> bool {
        self.path(self.leaf.as_deref()).iter().any(|e| e.id == id)
    }
    pub fn preview_leaf(&self, id: &str) -> Option<String> {
        if self.on_current_path(id) {
            return self.leaf.clone();
        }
        // Preserve the selected branch, choosing its most recently appended descendant.
        let mut candidate = id.to_owned();
        let mut descendants = HashSet::from([id.to_owned()]);
        for entry in &self.entries {
            if entry
                .parent_id
                .as_ref()
                .is_some_and(|parent| descendants.contains(parent))
            {
                descendants.insert(entry.id.clone());
                candidate = entry.id.clone();
            }
        }
        Some(candidate)
    }
    pub fn messages(&self, leaf: Option<&str>) -> Vec<DisplayMessage> {
        self.path(leaf)
            .into_iter()
            .filter_map(DisplayMessage::from_entry)
            .collect()
    }
    pub fn visible_ancestor(&self, id: &str, mode: HistoryMode) -> Option<String> {
        self.path(Some(id))
            .into_iter()
            .rev()
            .find(|entry| visible(entry, self.leaf.as_deref(), mode))
            .map(|entry| entry.id.clone())
    }
    pub fn tree_rows(&self, mode: HistoryMode) -> Vec<HistoryRow> {
        // Pi appends parents before children. Carry the nearest visible ancestor
        // across filtered entries without changing the underlying parent links.
        let mut nearest: HashMap<String, Option<String>> = HashMap::new();
        let mut rows = Vec::new();
        for e in &self.entries {
            let parent = e
                .parent_id
                .as_ref()
                .and_then(|id| nearest.get(id))
                .cloned()
                .flatten();
            if visible(e, self.leaf.as_deref(), mode) {
                let mut row = self.row(e);
                row.indirect_parent = parent.is_some() && parent != e.parent_id;
                row.parent = parent;
                rows.push(row);
                nearest.insert(e.id.clone(), Some(e.id.clone()));
            } else {
                nearest.insert(e.id.clone(), parent);
            }
        }
        // Append order keeps parents above children and matches the conversation,
        // even if entry timestamps are equal or a clock moves backwards.
        rows
    }
    fn row(&self, e: &SessionEntry) -> HistoryRow {
        let description = &self.descriptions[&e.id];
        HistoryRow {
            id: e.id.clone(),
            title: description.title.clone(),
            kind: description.kind,
            tool: description.tool.clone(),
            timestamp: e.timestamp.clone(),
            label: self.labels.get(&e.id).cloned(),
            parent: None,
            indirect_parent: false,
            current: self.leaf.as_deref() == Some(e.id.as_str()),
        }
    }
}
fn visible(e: &SessionEntry, leaf: Option<&str>, mode: HistoryMode) -> bool {
    visible_by_default(e, leaf)
        && (mode == HistoryMode::Detailed
            || e.kind == "message"
                && matches!(
                    e.data["message"]["role"].as_str(),
                    Some("user" | "assistant")
                ))
}
fn visible_by_default(e: &SessionEntry, leaf: Option<&str>) -> bool {
    // Match Pi /tree's default filter, including its current-leaf exception for
    // assistants without text. Labels and branch points do not bypass filtering.
    if matches!(
        e.kind.as_str(),
        "label" | "custom" | "model_change" | "thinking_level_change" | "session_info"
    ) {
        return false;
    }
    if e.kind == "message"
        && leaf != Some(e.id.as_str())
        && let Some(message) = e.data.get("message").filter(|m| m["role"] == "assistant")
    {
        let exceptional_stop = message["stopReason"]
            .as_str()
            .is_some_and(|reason| !matches!(reason, "" | "stop" | "toolUse"));
        return exceptional_stop || !text_content(message).trim().is_empty();
    }
    true
}
#[derive(Clone, Debug)]
pub(crate) struct DisplayMessage {
    pub id: String,
    pub entry: Option<String>,
    pub value: Value,
    pub completed_at: Option<i64>,
}
impl DisplayMessage {
    pub fn from_entry(e: &SessionEntry) -> Option<Self> {
        let value = if e.kind == "message" {
            e.data.get("message")?.clone()
        } else if matches!(e.kind.as_str(), "compaction" | "branch_summary") {
            serde_json::json!({"role":e.kind,"content":e.data.get("summary").and_then(Value::as_str).unwrap_or("")})
        } else {
            return None;
        };
        Some(Self {
            id: e.id.clone(),
            entry: Some(e.id.clone()),
            value,
            completed_at: time::OffsetDateTime::parse(
                &e.timestamp,
                &time::format_description::well_known::Rfc3339,
            )
            .ok()
            .map(|time| (time.unix_timestamp_nanos() / 1_000_000) as i64),
        })
    }
    pub fn role(&self) -> &str {
        self.value["role"].as_str().unwrap_or("custom")
    }
    pub fn text(&self) -> String {
        text_content(&self.value)
    }
    pub fn signature(&self) -> String {
        format!(
            "{}:{}",
            self.role(),
            self.value
                .get("timestamp")
                .map(Value::to_string)
                .unwrap_or_else(|| self.text())
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn history() -> History {
        let entries = serde_json::from_value(serde_json::json!({"leafId":"b2","entries":[
            {"type":"message","id":"root","parentId":null,"timestamp":"1","message":{"role":"user","content":"question"}},
            {"type":"message","id":"tool","parentId":"root","timestamp":"2","message":{"role":"toolResult","content":"output"}},
            {"type":"message","id":"split","parentId":"tool","timestamp":"3","message":{"role":"assistant","content":"answer"}},
            {"type":"message","id":"a","parentId":"split","timestamp":"4","message":{"role":"user","content":"A"}},
            {"type":"message","id":"b","parentId":"split","timestamp":"5","message":{"role":"user","content":"B"}},
            {"type":"message","id":"b2","parentId":"b","timestamp":"6","message":{"role":"assistant","content":"B answer"}}
        ]})).unwrap();
        let mut h = History::default();
        h.replace(entries);
        h
    }
    #[test]
    fn projection_keeps_chains_flat_and_preview_does_not_change_leaf() {
        let h = history();
        let rows = h.tree_rows(HistoryMode::Detailed);
        assert_eq!(
            rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            vec!["root", "tool", "split", "a", "b", "b2"]
        );
        assert_eq!(rows[2].parent.as_deref(), Some("tool"));
        assert!(!rows[2].indirect_parent);
        assert_eq!(h.preview_leaf("a").as_deref(), Some("a"));
        assert_eq!(h.leaf.as_deref(), Some("b2"));
        let brief = h.tree_rows(HistoryMode::Brief);
        assert_eq!(
            brief.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
            vec!["root", "split", "a", "b", "b2"]
        );
        assert_eq!(brief[1].parent.as_deref(), Some("root"));
        assert!(brief[1].indirect_parent);
        assert_eq!(
            h.visible_ancestor("tool", HistoryMode::Brief).as_deref(),
            Some("root")
        );
        assert_eq!(
            h.visible_ancestor("tool", HistoryMode::Detailed).as_deref(),
            Some("tool")
        );
        assert_eq!(h.entry("split").unwrap().parent_id.as_deref(), Some("tool"));
    }
    #[test]
    fn brief_mode_excludes_tools_and_summaries_even_at_the_execution_leaf() {
        let mut h = history();
        let mut entries = h.entries.clone();
        entries.extend(serde_json::from_value::<Vec<SessionEntry>>(serde_json::json!([
            {"type":"compaction","id":"summary","parentId":"b2","timestamp":"0","summary":"compressed"},
            {"type":"custom_message","id":"notice","parentId":"summary","timestamp":"0","content":"notice"},
            {"type":"message","id":"latest-tool","parentId":"notice","timestamp":"0","message":{"role":"toolResult","content":"result"}}
        ])).unwrap());
        h.replace(Entries {
            entries,
            leaf_id: Some("latest-tool".into()),
        });
        let brief = h.tree_rows(HistoryMode::Brief);
        assert_eq!(
            brief.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
            vec!["root", "split", "a", "b", "b2"]
        );
        let detailed = h.tree_rows(HistoryMode::Detailed);
        assert_eq!(detailed.last().unwrap().id, "latest-tool");
        assert!(detailed.last().unwrap().current);
        assert_eq!(
            h.visible_ancestor("latest-tool", HistoryMode::Brief)
                .as_deref(),
            Some("b2")
        );
        assert_eq!(h.leaf.as_deref(), Some("latest-tool"));
    }
    #[test]
    fn default_filter_preserves_content_and_exceptional_assistants() {
        for (message, visible) in [
            (serde_json::json!({"role":"user","content":[]}), true),
            (
                serde_json::json!({"role":"toolResult","content":"output"}),
                true,
            ),
            (
                serde_json::json!({"role":"assistant","content":"checking"}),
                true,
            ),
            (
                serde_json::json!({"role":"assistant","content":[{"type":"text","text":"checking"},{"type":"toolCall","id":"c"}],"stopReason":"toolUse"}),
                true,
            ),
            (
                serde_json::json!({"role":"assistant","content":[{"type":"toolCall","id":"c"}],"stopReason":"toolUse"}),
                false,
            ),
            (
                serde_json::json!({"role":"assistant","content":[{"type":"thinking","thinking":"reason"}],"stopReason":"stop"}),
                false,
            ),
            (
                serde_json::json!({"role":"assistant","content":[{"type":"text","text":" \n "}]}),
                false,
            ),
            (
                serde_json::json!({"role":"assistant","content":[],"stopReason":"error"}),
                true,
            ),
            (
                serde_json::json!({"role":"assistant","content":[],"stopReason":"aborted"}),
                true,
            ),
            (
                serde_json::json!({"role":"assistant","content":[],"stopReason":"length"}),
                true,
            ),
        ] {
            let entry: SessionEntry = serde_json::from_value(serde_json::json!({
                "id":"entry","parentId":null,"timestamp":"t","type":"message","message":message
            }))
            .unwrap();
            assert_eq!(visible_by_default(&entry, None), visible, "{message}");
            assert!(visible_by_default(&entry, Some("entry")), "{message}");
        }
        for (kind, visible) in [
            ("model_change", false),
            ("thinking_level_change", false),
            ("session_info", false),
            ("custom", false),
            ("label", false),
            ("custom_message", true),
            ("compaction", true),
            ("branch_summary", true),
        ] {
            let entry: SessionEntry = serde_json::from_value(serde_json::json!({
                "id":"entry","parentId":null,"timestamp":"t","type":kind
            }))
            .unwrap();
            assert_eq!(visible_by_default(&entry, None), visible, "{kind}");
            assert_eq!(visible_by_default(&entry, Some("entry")), visible, "{kind}");
        }
    }
}

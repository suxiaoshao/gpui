//! Shared Pi entry projection. UI filtering never changes parent links or the execution leaf.
pub(crate) mod canvas;
mod graph;
mod nodes;
use crate::foundation::session_catalog::text_content;
pub(crate) use graph::HistoryGraph;
pub(crate) use nodes::HistoryKind;
use pi_rpc::protocol::{Entries, SessionEntry};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum HistoryDetail {
    #[default]
    Brief,
    Detailed,
    All,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum HistoryView {
    #[default]
    List,
    Tree,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum HistoryScope {
    #[default]
    All,
    Branch,
}

#[derive(Clone, Default)]
pub(crate) struct History {
    pub entries: Vec<SessionEntry>,
    pub leaf: Option<String>,
    pub revision: u64,
    index: HashMap<String, usize>,
    labels: HashMap<String, String>,
    descriptions: HashMap<String, nodes::Description>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
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
        self.revision = self.revision.wrapping_add(1);
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
    pub fn visible_ancestor(&self, id: &str, mode: HistoryDetail) -> Option<String> {
        self.path(Some(id))
            .into_iter()
            .rev()
            .find(|entry| self.visible(entry, mode))
            .map(|entry| entry.id.clone())
    }
    pub fn tree_rows(&self, mode: HistoryDetail) -> Vec<HistoryRow> {
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
            if self.visible(e, mode) {
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
        if let Some(current) = self
            .leaf
            .as_ref()
            .and_then(|id| nearest.get(id))
            .and_then(Option::as_ref)
            && let Some(row) = rows.iter_mut().find(|row| &row.id == current)
        {
            row.current = true;
        }
        rows
    }
    pub fn list_rows(
        &self,
        detail: HistoryDetail,
        scope: HistoryScope,
        preview: Option<&str>,
    ) -> Vec<HistoryRow> {
        let mut rows = self.tree_rows(detail);
        if scope == HistoryScope::Branch {
            let leaf = preview
                .and_then(|id| self.preview_leaf(id))
                .or_else(|| self.leaf.clone());
            let path: HashSet<_> = self
                .path(leaf.as_deref())
                .into_iter()
                .map(|e| e.id.as_str())
                .collect();
            rows.retain(|row| path.contains(row.id.as_str()));
        }
        rows
    }
    fn visible(&self, entry: &SessionEntry, detail: HistoryDetail) -> bool {
        let kind = self.descriptions[&entry.id].kind;
        match detail {
            HistoryDetail::Brief => matches!(kind, HistoryKind::User | HistoryKind::Assistant),
            HistoryDetail::Detailed => matches!(
                kind,
                HistoryKind::User
                    | HistoryKind::Assistant
                    | HistoryKind::AssistantProgress
                    | HistoryKind::Thinking
                    | HistoryKind::ToolCall
                    | HistoryKind::ToolResult
                    | HistoryKind::Failed
                    | HistoryKind::Stopped
                    | HistoryKind::Compaction
                    | HistoryKind::BranchSummary
            ),
            HistoryDetail::All => true,
        }
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
        let rows = h.tree_rows(HistoryDetail::Detailed);
        assert_eq!(
            rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            vec!["root", "tool", "split", "a", "b", "b2"]
        );
        assert_eq!(rows[2].parent.as_deref(), Some("tool"));
        assert!(!rows[2].indirect_parent);
        assert_eq!(h.preview_leaf("a").as_deref(), Some("a"));
        assert_eq!(h.leaf.as_deref(), Some("b2"));
        let brief = h.tree_rows(HistoryDetail::Brief);
        assert_eq!(
            brief.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
            vec!["root", "split", "a", "b", "b2"]
        );
        assert_eq!(brief[1].parent.as_deref(), Some("root"));
        assert!(brief[1].indirect_parent);
        assert_eq!(
            h.visible_ancestor("tool", HistoryDetail::Brief).as_deref(),
            Some("root")
        );
        assert_eq!(
            h.visible_ancestor("tool", HistoryDetail::Detailed)
                .as_deref(),
            Some("tool")
        );
        assert_eq!(h.entry("split").unwrap().parent_id.as_deref(), Some("tool"));
    }
    #[test]
    fn brief_excludes_tools_and_summaries_even_at_the_execution_leaf() {
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
        {
            let mode = HistoryDetail::Brief;
            let rows = h.tree_rows(mode);
            assert_eq!(
                rows.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
                vec!["root", "split", "a", "b", "b2"]
            );
            assert_eq!(
                h.visible_ancestor("latest-tool", mode).as_deref(),
                Some("b2")
            );
        }
        let detailed = h.tree_rows(HistoryDetail::Detailed);
        assert_eq!(detailed.last().unwrap().id, "latest-tool");
        assert!(detailed.last().unwrap().current);
        assert_eq!(h.leaf.as_deref(), Some("latest-tool"));
    }
    #[test]
    fn content_levels_preserve_semantics_and_reconnect_branches() {
        let entries: Entries = serde_json::from_value(serde_json::json!({"leafId":"empty", "entries":[
            {"type":"message","id":"u","parentId":null,"timestamp":"t","message":{"role":"user","content":"question"}},
            {"type":"message","id":"progress","parentId":"u","timestamp":"t","message":{"role":"assistant","content":"checking"}},
            {"type":"message","id":"call","parentId":"progress","timestamp":"t","message":{"role":"assistant","content":[{"type":"text","text":"reading"},{"type":"toolCall","id":"c","name":"read"}],"stopReason":"toolUse"}},
            {"type":"message","id":"silent-call","parentId":"call","timestamp":"t","message":{"role":"assistant","content":[{"type":"toolCall","id":"d","name":"read"}],"stopReason":"toolUse"}},
            {"type":"message","id":"result","parentId":"silent-call","timestamp":"t","message":{"role":"toolResult","content":"output"}},
            {"type":"message","id":"thinking","parentId":"result","timestamp":"t","message":{"role":"assistant","content":[{"type":"thinking","thinking":"reason"}]}},
            {"type":"message","id":"answer","parentId":"thinking","timestamp":"t","message":{"role":"assistant","content":"answer","stopReason":"stop"}},
            {"type":"compaction","id":"summary","parentId":"answer","timestamp":"t","summary":"compressed"},
            {"type":"branch_summary","id":"branch","parentId":"summary","timestamp":"t","summary":"branch summary"},
            {"type":"custom_message","id":"notice","parentId":"branch","timestamp":"t","content":"notice"},
            {"type":"message","id":"next","parentId":"notice","timestamp":"t","message":{"role":"user","content":"next question"}},
            {"type":"message","id":"failed","parentId":"next","timestamp":"t","message":{"role":"assistant","content":"failure","stopReason":"error"}},
            {"type":"message","id":"stopped","parentId":"next","timestamp":"t","message":{"role":"assistant","content":"interrupted","stopReason":"aborted"}},
            {"type":"message","id":"empty","parentId":"next","timestamp":"t","message":{"role":"assistant","content":[]}},
            {"type":"message","id":"alternative","parentId":"result","timestamp":"t","message":{"role":"assistant","content":"other answer","stopReason":"stop"}},
            {"type":"label","id":"label","parentId":"empty","timestamp":"t","targetId":"failed","label":"bookmark"},
            {"type":"model_change","id":"model","parentId":"label","timestamp":"t","provider":"provider","modelId":"model"},
            {"type":"thinking_level_change","id":"level","parentId":"model","timestamp":"t","thinkingLevel":"high"},
            {"type":"session_info","id":"name","parentId":"level","timestamp":"t","name":"session name"},
            {"type":"custom","id":"custom","parentId":"name","timestamp":"t","customType":"test"},
            {"type":"future_record","id":"future","parentId":"custom","timestamp":"t"}
        ]})).unwrap();
        let mut h = History::default();
        h.replace(entries);
        let brief = h.tree_rows(HistoryDetail::Brief);
        assert_eq!(
            brief
                .iter()
                .map(|r| (r.id.as_str(), r.parent.as_deref()))
                .collect::<Vec<_>>(),
            [
                ("u", None),
                ("answer", Some("u")),
                ("next", Some("answer")),
                ("alternative", Some("u"))
            ]
        );
        assert!(brief[1..].iter().all(|row| row.indirect_parent));
        assert_eq!(
            brief
                .iter()
                .filter(|row| row.current)
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>(),
            ["next"]
        );
        {
            let mode = HistoryDetail::Brief;
            for (hidden, ancestor) in [
                ("thinking", "u"),
                ("notice", "answer"),
                ("failed", "next"),
                ("stopped", "next"),
                ("empty", "next"),
            ] {
                assert_eq!(h.visible_ancestor(hidden, mode).as_deref(), Some(ancestor));
            }
        }
        let detailed = h.tree_rows(HistoryDetail::Detailed);
        for id in [
            "progress",
            "call",
            "silent-call",
            "result",
            "thinking",
            "summary",
            "branch",
            "failed",
            "stopped",
        ] {
            assert!(detailed.iter().any(|row| row.id == id), "missing {id}");
        }
        assert!(
            detailed
                .iter()
                .find(|row| row.id == "next")
                .unwrap()
                .current
        );
        for id in [
            "notice", "empty", "label", "model", "level", "name", "custom", "future",
        ] {
            assert!(detailed.iter().all(|row| row.id != id), "unexpected {id}");
        }
        let all = h.tree_rows(HistoryDetail::All);
        assert_eq!(all.len(), h.entries.len());
        for id in [
            "model", "level", "name", "label", "custom", "future", "notice",
        ] {
            assert!(
                !all.iter()
                    .find(|row| row.id == id)
                    .unwrap()
                    .title
                    .is_empty()
            );
        }
        assert!(all.iter().find(|row| row.id == "empty").unwrap().current);
        assert_eq!(h.leaf.as_deref(), Some("empty"));
        assert_eq!(
            h.entry("answer").unwrap().parent_id.as_deref(),
            Some("thinking")
        );
        // The canvas cannot unfold records removed by the shared content filter.
        let mut tree = canvas::Tree::default();
        tree.replace(h.tree_rows(HistoryDetail::Brief), None);
        assert_eq!(tree.nodes.len(), brief.len());
        assert!(tree.edges.iter().all(|edge| edge.hidden.is_empty()));
    }
    #[test]
    fn branch_scope_matches_body_path_including_shared_history_and_descendants() {
        let h = history();
        for detail in [
            HistoryDetail::Brief,
            HistoryDetail::Detailed,
            HistoryDetail::All,
        ] {
            let active = h.list_rows(detail, HistoryScope::Branch, None);
            assert_eq!(
                active,
                h.list_rows(detail, HistoryScope::Branch, Some("split"))
            );
            assert!(active.iter().any(|row| row.id == "root"));
            assert!(active.iter().any(|row| row.id == "b2" && row.current));
            assert!(active.iter().all(|row| row.id != "a"));
            let preview = h.list_rows(detail, HistoryScope::Branch, Some("a"));
            assert_eq!(preview.last().unwrap().id, "a");
            assert!(preview.iter().any(|row| row.id == "root"));
            assert!(
                preview
                    .iter()
                    .all(|row| !row.current && row.id != "b" && row.id != "b2")
            );
            assert_eq!(
                h.list_rows(detail, HistoryScope::All, Some("a")),
                h.tree_rows(detail)
            );
        }
        assert_eq!(h.leaf.as_deref(), Some("b2"));
        assert_eq!(h.entry("split").unwrap().parent_id.as_deref(), Some("tool"));
        // Previewing an older off-branch node includes the rest of that branch.
        let mut h = h;
        let mut entries = h.entries.clone();
        entries.push(serde_json::from_value(serde_json::json!({"type":"message","id":"a2","parentId":"a","timestamp":"t","message":{"role":"assistant","content":"A answer"}})).unwrap());
        h.replace(Entries {
            entries,
            leaf_id: Some("b2".into()),
        });
        let rows = h.list_rows(HistoryDetail::Brief, HistoryScope::Branch, Some("a"));
        assert_eq!(
            rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            ["root", "split", "a", "a2"]
        );
    }
}

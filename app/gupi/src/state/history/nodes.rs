use super::*;
use markdown::mdast::Node;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HistoryKind {
    User,
    Assistant,
    AssistantProgress,
    Thinking,
    ToolCall,
    ToolResult,
    Failed,
    Stopped,
    EmptyAssistant,
    Compaction,
    BranchSummary,
    ModelChange,
    ThinkingLevelChange,
    SessionInfo,
    Label,
    Custom,
    Event,
}

#[derive(Clone, Debug)]
pub(super) struct Description {
    pub kind: HistoryKind,
    pub title: String,
    pub tool: Option<String>,
}

pub(super) fn describe(entries: &[SessionEntry]) -> HashMap<String, Description> {
    // Follow parent links across metadata, never across unrelated branches.
    // Pi has no final/commentary channel: an assistant followed by more assistant
    // or tool execution before the next user message is an intermediate message.
    let mut preceding: HashMap<&str, Option<&SessionEntry>> = HashMap::new();
    let mut continued = HashSet::new();
    for entry in entries {
        let prior = entry
            .parent_id
            .as_deref()
            .and_then(|id| preceding.get(id))
            .copied()
            .flatten();
        let role = entry.data.get("message").and_then(|m| m["role"].as_str());
        if matches!(role, Some("assistant" | "toolResult"))
            && let Some(prior) = prior.filter(|e| {
                e.data
                    .get("message")
                    .is_some_and(|m| m["role"] == "assistant")
            })
        {
            continued.insert(prior.id.as_str());
        }
        let nearest = if entry.kind == "message" {
            Some(entry)
        } else if matches!(
            entry.kind.as_str(),
            "compaction" | "branch_summary" | "custom_message"
        ) {
            None
        } else {
            prior
        };
        preceding.insert(&entry.id, nearest);
    }
    let tool_summaries: HashMap<_, _> = entries
        .iter()
        .filter_map(|entry| entry.data.get("message"))
        .filter_map(|message| message["content"].as_array())
        .flatten()
        .filter(|part| part["type"] == "toolCall")
        .filter_map(|call| call["id"].as_str().map(|id| (id, tool_summary(call))))
        .collect();
    entries
        .iter()
        .map(|entry| {
            let mut description = describe_entry(entry, continued.contains(entry.id.as_str()));
            if description.kind == HistoryKind::ToolResult
                && let Some(title) = entry
                    .data
                    .get("message")
                    .and_then(|message| message["toolCallId"].as_str())
                    .and_then(|id| tool_summaries.get(id))
            {
                description.title = title.clone();
            }
            (entry.id.clone(), description)
        })
        .collect()
}

fn describe_entry(entry: &SessionEntry, continued: bool) -> Description {
    let message = entry.data.get("message").unwrap_or(&Value::Null);
    let parts = message["content"].as_array();
    let calls: Vec<_> = parts
        .into_iter()
        .flatten()
        .filter(|part| part["type"] == "toolCall")
        .collect();
    let thinking = parts
        .into_iter()
        .flatten()
        .any(|part| part["type"] == "thinking");
    let text = text_content(message);
    let kind = match message["role"].as_str() {
        Some("user") => HistoryKind::User,
        Some("toolResult") => HistoryKind::ToolResult,
        Some("assistant") => match message["stopReason"].as_str() {
            Some("error") => HistoryKind::Failed,
            Some("aborted") => HistoryKind::Stopped,
            _ if !calls.is_empty() || message["stopReason"] == "toolUse" => HistoryKind::ToolCall,
            _ if text.trim().is_empty() && thinking => HistoryKind::Thinking,
            _ if text.trim().is_empty() => HistoryKind::EmptyAssistant,
            _ if continued => HistoryKind::AssistantProgress,
            _ => HistoryKind::Assistant,
        },
        _ => match entry.kind.as_str() {
            "compaction" => HistoryKind::Compaction,
            "branch_summary" => HistoryKind::BranchSummary,
            "model_change" => HistoryKind::ModelChange,
            "thinking_level_change" => HistoryKind::ThinkingLevelChange,
            "session_info" => HistoryKind::SessionInfo,
            "label" => HistoryKind::Label,
            "custom" | "custom_message" => HistoryKind::Custom,
            _ => HistoryKind::Event,
        },
    };
    let tool = message["toolName"]
        .as_str()
        .or_else(|| calls.first().and_then(|call| call["name"].as_str()))
        .map(str::to_owned);
    let body = if text.trim().is_empty() {
        message["errorMessage"]
            .as_str()
            .or_else(|| entry.data.get("summary").and_then(Value::as_str))
            .unwrap_or_default()
            .to_owned()
    } else {
        text
    };
    let mut title = plain_summary(&body);
    if entry.kind != "message" && title.is_empty() {
        let data = &entry.data;
        let field = |key: &str| data.get(key).and_then(Value::as_str);
        let metadata = match kind {
            HistoryKind::ModelChange => [field("provider"), field("modelId")]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" / "),
            HistoryKind::ThinkingLevelChange => field("thinkingLevel").unwrap_or_default().into(),
            HistoryKind::SessionInfo => field("name").unwrap_or_default().into(),
            HistoryKind::Label => field("label").unwrap_or_default().into(),
            HistoryKind::Custom => {
                let content = text_content(&Value::Object(data.clone()));
                if content.trim().is_empty() {
                    field("customType").unwrap_or_default().into()
                } else {
                    content
                }
            }
            HistoryKind::Event => entry.kind.clone(),
            _ => String::new(),
        };
        title = plain_summary(&metadata);
    }
    if kind == HistoryKind::ToolResult {
        title = tool.clone().unwrap_or_default();
    } else if kind == HistoryKind::ToolCall && title.is_empty() {
        title = crate::foundation::session_catalog::summary(
            &calls
                .iter()
                .map(|call| tool_summary(call))
                .collect::<Vec<_>>()
                .join(" · "),
        );
    }
    Description { kind, title, tool }
}

fn tool_summary(call: &Value) -> String {
    let name = call["name"].as_str().unwrap_or_default();
    let args = &call["arguments"];
    let target = ["path", "file_path", "command", "pattern", "query"]
        .iter()
        .find_map(|key| args[*key].as_str());
    crate::foundation::session_catalog::summary(
        &target.map_or_else(|| name.to_owned(), |target| format!("{name} · {target}")),
    )
}

fn plain_summary(source: &str) -> String {
    fn text(node: &Node, output: &mut String) {
        if output.chars().count() >= 160 {
            return;
        }
        match node {
            Node::Text(value) => output.push_str(&value.value),
            Node::InlineCode(value) => output.push_str(&value.value),
            Node::Code(value) => output.push_str(&value.value),
            Node::Image(value) => output.push_str(&value.alt),
            Node::Break(_) => output.push(' '),
            Node::Html(_) | Node::Definition(_) => {}
            _ => {
                if let Some(children) = node.children() {
                    for child in children {
                        text(child, output);
                    }
                }
            }
        }
        if matches!(
            node,
            Node::Paragraph(_)
                | Node::Heading(_)
                | Node::ListItem(_)
                | Node::Code(_)
                | Node::TableCell(_)
        ) {
            output.push(' ');
        }
    }
    let Ok(node) = markdown::to_mdast(source, &markdown::ParseOptions::gfm()) else {
        return crate::foundation::session_catalog::summary(source);
    };
    let mut output = String::new();
    text(&node, &mut output);
    crate::foundation::session_catalog::summary(&output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distinguishes_loop_messages_without_crossing_branch_or_user_boundaries() {
        let entries: Entries = serde_json::from_value(serde_json::json!({"leafId":"b", "entries":[
            {"id":"u","parentId":null,"timestamp":"t","type":"message","message":{"role":"user","content":"question"}},
            {"id":"p","parentId":"u","timestamp":"t","type":"message","message":{"role":"assistant","content":"checking"}},
            {"id":"meta","parentId":"p","timestamp":"t","type":"model_change"},
            {"id":"call","parentId":"meta","timestamp":"t","type":"message","message":{"role":"assistant","content":[{"type":"toolCall","name":"read","id":"c","arguments":{"path":"settings.json"}}]}},
            {"id":"tool","parentId":"call","timestamp":"t","type":"message","message":{"role":"toolResult","toolName":"read","toolCallId":"c","content":"result"}},
            {"id":"a","parentId":"tool","timestamp":"t","type":"message","message":{"role":"assistant","content":"done"}},
            {"id":"u2","parentId":"a","timestamp":"t","type":"message","message":{"role":"user","content":"next"}},
            {"id":"b","parentId":"u","timestamp":"t","type":"message","message":{"role":"assistant","content":"another branch"}}
        ]})).unwrap();
        let descriptions = describe(&entries.entries);
        assert_eq!(descriptions["p"].kind, HistoryKind::AssistantProgress);
        assert_eq!(descriptions["call"].kind, HistoryKind::ToolCall);
        assert_eq!(descriptions["tool"].kind, HistoryKind::ToolResult);
        assert_eq!(descriptions["tool"].title, "read · settings.json");
        assert_eq!(descriptions["a"].kind, HistoryKind::Assistant);
        assert_eq!(descriptions["b"].kind, HistoryKind::Assistant);
        let mut history = History::default();
        history.replace(entries);
        let rows = history.tree_rows(HistoryDetail::Detailed);
        assert!(rows.iter().any(|row| row.id == "call"));
        assert!(rows.iter().all(|row| row.id != "meta"));
        let tool = rows.iter().find(|row| row.id == "tool").unwrap();
        assert_eq!(tool.title, "read · settings.json");
        assert_eq!(tool.parent.as_deref(), Some("call"));
        assert!(!tool.indirect_parent);
        assert_eq!(history.leaf.as_deref(), Some("b"));
    }
    #[test]
    fn summaries_strip_markup_and_empty_or_failed_assistants_have_semantic_kinds() {
        assert_eq!(
            plain_summary("## Heading\n\n**bold** and [link](https://example.test)\n\n`file.rs`"),
            "Heading bold and link file.rs"
        );
        for (message, kind) in [
            (
                serde_json::json!({"role":"assistant","content":[]}),
                HistoryKind::EmptyAssistant,
            ),
            (
                serde_json::json!({"role":"assistant","content":[{"type":"thinking","thinking":"reason"}]}),
                HistoryKind::Thinking,
            ),
            (
                serde_json::json!({"role":"assistant","content":[],"stopReason":"error","errorMessage":"Connection error."}),
                HistoryKind::Failed,
            ),
            (
                serde_json::json!({"role":"assistant","content":"partial","stopReason":"aborted"}),
                HistoryKind::Stopped,
            ),
        ] {
            let entry: SessionEntry = serde_json::from_value(serde_json::json!({"id":"a","parentId":null,"timestamp":"t","type":"message","message":message})).unwrap();
            assert_eq!(describe_entry(&entry, false).kind, kind);
            assert_ne!(describe_entry(&entry, false).title, "assistant");
        }
    }
}

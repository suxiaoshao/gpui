//! Pi content projection for the conversation activity stream.
use std::collections::{HashMap, HashSet};

use serde_json::Value;

use crate::{
    foundation::session_catalog::text_content,
    state::{
        conversation::{ToolActivity, execution::ToolExecution},
        history::DisplayMessage,
    },
};

pub(super) struct RunContent {
    pub activities: Vec<Activity>,
    pub answer: Option<usize>,
    pub interrupted: bool,
}

pub(super) enum Activity {
    Text {
        id: String,
        text: String,
        thinking: bool,
    },
    Tool(Tool),
}

pub(super) enum ActivityBlock<'a> {
    Message(&'a Activity),
    Group { id: String, items: &'a [Activity] },
}

pub(super) struct Tool {
    pub id: String,
    pub entries: Vec<String>,
    pub name: String,
    pub args: Option<Value>,
    pub output: String,
    pub status: ToolStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ToolStatus {
    Running,
    Complete,
    Failed,
    Unfinished,
}

impl RunContent {
    /// Assistant prose separates activity groups. Tool results and private
    /// thinking blocks continue the same group, including across message_end.
    pub fn blocks(&self) -> Vec<ActivityBlock<'_>> {
        let mut blocks = vec![];
        let mut start = 0;
        for (index, activity) in self.activities.iter().enumerate() {
            if matches!(
                activity,
                Activity::Text {
                    thinking: false,
                    ..
                }
            ) {
                if start < index {
                    blocks.push(activity_group(&self.activities[start..index]));
                }
                blocks.push(ActivityBlock::Message(activity));
                start = index + 1;
            }
        }
        if start < self.activities.len() {
            blocks.push(activity_group(&self.activities[start..]));
        }
        blocks
    }

    pub fn project(messages: &[DisplayMessage], live: &[ToolActivity], active: bool) -> Self {
        // Pi has no commentary/final channel. Only a trailing assistant text can
        // be the answer; never promote an older progress message across a tool.
        let answer = messages.len().checked_sub(1).filter(|&i| {
            let m = &messages[i];
            m.role() == "assistant"
                && !m.text().is_empty()
                && !m.value["content"]
                    .as_array()
                    .is_some_and(|parts| parts.iter().any(|part| part["type"] == "toolCall"))
        });
        let interrupted = messages
            .iter()
            .any(|m| matches!(m.value["stopReason"].as_str(), Some("error" | "aborted")));
        let results: HashMap<_, _> = messages
            .iter()
            .filter(|m| m.role() == "toolResult")
            .filter_map(|m| m.value["toolCallId"].as_str().map(|id| (id, m)))
            .collect();
        let live: HashMap<_, _> = live.iter().map(|tool| (tool.id.as_str(), tool)).collect();
        let call_ids: HashSet<_> = messages
            .iter()
            .filter_map(|m| m.value["content"].as_array())
            .flatten()
            .filter(|part| part["type"] == "toolCall")
            .filter_map(|part| part["id"].as_str())
            .collect();
        let mut activities = vec![];
        for (index, m) in messages.iter().enumerate() {
            if m.role() == "toolResult" {
                let call_id = m.value["toolCallId"].as_str();
                if call_id.is_some_and(|id| call_ids.contains(id)) {
                    continue;
                }
                // Partial/compacted history can legitimately lack the call.
                activities.push(Activity::Tool(Tool {
                    id: format!("tool-{}", call_id.unwrap_or(&m.id)),
                    entries: m.entry.iter().cloned().collect(),
                    name: m.value["toolName"].as_str().unwrap_or("tool").to_owned(),
                    args: None,
                    output: m.text(),
                    status: result_status(m),
                }));
                continue;
            }
            if let Some(parts) = m.value["content"].as_array() {
                for (i, part) in parts.iter().enumerate() {
                    let id = format!("part-{}-{i}", m.id);
                    match part["type"].as_str() {
                        Some("toolCall") => {
                            let call_id = part["id"].as_str();
                            let result = call_id.and_then(|id| results.get(id)).copied();
                            let update = call_id.and_then(|id| live.get(id)).copied();
                            let mut entries: Vec<_> = m.entry.iter().cloned().collect();
                            entries.extend(result.and_then(|m| m.entry.clone()));
                            let (output, status) = if let Some(result) = result {
                                (result.text(), result_status(result))
                            } else if let Some(update) = update {
                                match &update.execution {
                                    ToolExecution::Running(output) => (
                                        text_content(output),
                                        if active {
                                            ToolStatus::Running
                                        } else {
                                            ToolStatus::Unfinished
                                        },
                                    ),
                                    ToolExecution::Complete(output) => {
                                        (text_content(output), ToolStatus::Complete)
                                    }
                                    ToolExecution::Failed(output) => {
                                        (text_content(output), ToolStatus::Failed)
                                    }
                                }
                            } else {
                                (
                                    String::new(),
                                    if active {
                                        ToolStatus::Running
                                    } else {
                                        ToolStatus::Unfinished
                                    },
                                )
                            };
                            activities.push(Activity::Tool(Tool {
                                id: format!("tool-{}", call_id.unwrap_or(&id)),
                                entries,
                                name: update.map(|tool| tool.name.clone()).unwrap_or_else(|| {
                                    part["name"].as_str().unwrap_or("tool").to_owned()
                                }),
                                args: update
                                    .map(|tool| tool.args.clone())
                                    .or_else(|| part.get("arguments").cloned()),
                                output,
                                status,
                            }));
                        }
                        Some("text" | "thinking") => {
                            let thinking = part["type"] == "thinking";
                            if !thinking && Some(index) == answer {
                                continue;
                            }
                            let text = part[if thinking { "thinking" } else { "text" }]
                                .as_str()
                                .unwrap_or_default();
                            if !text.is_empty() {
                                activities.push(Activity::Text {
                                    id,
                                    text: text.to_owned(),
                                    thinking,
                                });
                            }
                        }
                        _ => {}
                    }
                }
            } else if Some(index) != answer && !m.text().is_empty() {
                activities.push(Activity::Text {
                    id: m.id.clone(),
                    text: m.text(),
                    thinking: false,
                });
            }
        }
        Self {
            activities,
            answer,
            interrupted,
        }
    }
}

fn activity_group(items: &[Activity]) -> ActivityBlock<'_> {
    let first = items
        .iter()
        .find_map(|item| match item {
            Activity::Tool(tool) => Some(tool.id.as_str()),
            _ => None,
        })
        .unwrap_or_else(|| match &items[0] {
            Activity::Text { id, .. } => id,
            Activity::Tool(tool) => &tool.id,
        });
    ActivityBlock::Group {
        id: format!("group-{first}"),
        items,
    }
}

fn result_status(result: &DisplayMessage) -> ToolStatus {
    if result.value["isError"] == true {
        ToolStatus::Failed
    } else {
        ToolStatus::Complete
    }
}

impl Tool {
    pub fn summary(&self) -> Option<&str> {
        let args = self.args.as_ref()?;
        let field = match self.name.as_str() {
            "bash" => "command",
            "read" | "write" | "edit" => "path",
            "grep" | "find" => "pattern",
            "ls" => "path",
            _ => return None,
        };
        args[field].as_str().filter(|text| !text.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn message(id: &str, value: Value) -> DisplayMessage {
        DisplayMessage {
            id: id.into(),
            entry: Some(id.into()),
            value,
            completed_at: None,
        }
    }
    fn call() -> DisplayMessage {
        message(
            "call",
            json!({"role":"assistant", "content":[
                {"type":"thinking", "thinking":"Inspect the file"},
                {"type":"text", "text":"I will read it."},
                {"type":"toolCall", "id":"read-1", "name":"read", "arguments":{"path":"a.rs"}}
            ]}),
        )
    }
    fn tool_result(text: &str, error: bool) -> DisplayMessage {
        message(
            "result",
            json!({"role":"toolResult", "toolCallId":"read-1", "toolName":"read",
            "content":[{"type":"text", "text":text}], "isError":error}),
        )
    }
    fn tools(content: &RunContent) -> Vec<&Tool> {
        content
            .activities
            .iter()
            .filter_map(|a| match a {
                Activity::Tool(t) => Some(t),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn call_updates_in_place_and_persisted_result_replaces_live_output() {
        let live = ToolActivity {
            id: "read-1".into(),
            name: "read".into(),
            args: json!({"path":"a.rs"}),
            execution: ToolExecution::Running(
                json!({"content":[{"type":"text", "text":"partial"}]}),
            ),
        };
        let running = RunContent::project(&[call()], std::slice::from_ref(&live), true);
        let running_tool = tools(&running)[0];
        assert_eq!(running_tool.output, "partial");
        assert_eq!(running_tool.status, ToolStatus::Running);
        let settled = RunContent::project(&[call(), tool_result("complete", false)], &[live], true);
        assert_eq!(tools(&settled).len(), 1);
        let settled_tool = tools(&settled)[0];
        assert_eq!(settled_tool.id, running_tool.id);
        assert_eq!(settled_tool.output, "complete");
        assert_eq!(settled_tool.status, ToolStatus::Complete);
        assert_eq!(settled_tool.entries, ["call", "result"]);
        assert_eq!(settled_tool.summary(), Some("a.rs"));
    }

    #[test]
    fn progress_before_tools_is_not_a_final_answer_and_content_order_is_preserved() {
        let progress = message(
            "progress",
            json!({"role":"assistant", "content":"Let me check."}),
        );
        let mut messages = vec![progress, call(), tool_result("", false)];
        let content = RunContent::project(&messages, &[], true);
        assert_eq!(content.answer, None);
        assert_eq!(tools(&content).len(), 1); // Empty results still complete their call.
        assert_eq!(tools(&content)[0].status, ToolStatus::Complete);
        assert!(matches!(
            &content.activities[1],
            Activity::Text { thinking: true, .. }
        ));
        assert!(matches!(
            &content.activities[2],
            Activity::Text {
                thinking: false,
                ..
            }
        ));
        assert!(matches!(&content.activities[3], Activity::Tool(_)));
        messages.push(message(
            "answer",
            json!({"role":"assistant", "content":[
                {"type":"thinking", "thinking":"Finish"}, {"type":"text", "text":"The answer"}
            ]}),
        ));
        let content = RunContent::project(&messages, &[], true);
        assert_eq!(content.answer, Some(3)); // Visible even while streaming.
        assert!(matches!(
            content.activities.last(),
            Some(Activity::Text { thinking: true, .. })
        ));
    }

    #[test]
    fn orphan_results_and_interrupted_runs_remain_inspectable() {
        let orphan = RunContent::project(&[tool_result("failure", true)], &[], false);
        assert_eq!(tools(&orphan).len(), 1);
        assert_eq!(tools(&orphan)[0].status, ToolStatus::Failed);
        assert!(tools(&orphan)[0].args.is_none());
        let stopped = RunContent::project(
            &[
                call(),
                message(
                    "stopped",
                    json!({
                        "role":"assistant", "content":"Partial answer", "stopReason":"aborted"
                    }),
                ),
            ],
            &[],
            false,
        );
        assert!(stopped.interrupted);
        assert_eq!(stopped.answer, Some(1));
        assert_eq!(tools(&stopped)[0].status, ToolStatus::Unfinished);
    }

    #[test]
    fn only_assistant_prose_splits_tool_groups() {
        let tool = |id: &str| {
            Activity::Tool(Tool {
                id: id.into(),
                entries: vec![],
                name: "read".into(),
                args: None,
                output: String::new(),
                status: ToolStatus::Complete,
            })
        };
        let mut content = RunContent {
            activities: vec![
                tool("first"),
                Activity::Text {
                    id: "thought".into(),
                    text: "Thinking".into(),
                    thinking: true,
                },
                tool("second"),
                Activity::Text {
                    id: "progress".into(),
                    text: "Next step".into(),
                    thinking: false,
                },
                tool("third"),
            ],
            answer: None,
            interrupted: false,
        };
        let blocks = content.blocks();
        assert_eq!(blocks.len(), 3);
        assert!(
            matches!(&blocks[0], ActivityBlock::Group { id, items } if id == "group-first" && items.len() == 3)
        );
        assert!(matches!(&blocks[1], ActivityBlock::Message(_)));
        assert!(
            matches!(&blocks[2], ActivityBlock::Group { id, items } if id == "group-third" && items.len() == 1)
        );
        content.activities.push(tool("fourth"));
        assert!(
            matches!(content.blocks().last(), Some(ActivityBlock::Group { id, items }) if id == "group-third" && items.len() == 2)
        );
    }
}

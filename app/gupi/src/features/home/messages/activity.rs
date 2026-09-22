//! Pi content projection for the conversation activity stream.
use std::collections::{HashMap, HashSet};

use crate::foundation::tool_presentation::{ToolKind, read_path, skill_name};
use serde_json::Value;

use crate::state::{
    conversation::{ToolActivity, execution::ToolExecution},
    history::DisplayMessage,
};

pub(super) struct RunContent {
    pub activities: Vec<Activity>,
    pub answer: Option<usize>,
    pub answer_text: String,
    pub final_started: bool,
    pub interrupted: bool,
}

pub(super) enum Activity {
    Text {
        id: String,
        text: String,
        thinking: bool,
        running: bool,
    },
    Tool(Tool),
}

pub(super) enum ActivityBlock<'a> {
    Message(&'a Activity),
    Group { id: String, items: &'a [Activity] },
}

#[derive(Clone, PartialEq)]
pub(super) struct Tool {
    pub id: String,
    pub entries: Vec<String>,
    pub name: String,
    pub args: Option<Value>,
    pub result: Value,
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
    pub fn has_process(&self) -> bool {
        self.activities.iter().any(|activity| {
            matches!(
                activity,
                Activity::Tool(_)
                    | Activity::Text {
                        thinking: false,
                        ..
                    }
            )
        })
    }

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
        let answer = messages.len().checked_sub(1).filter(|&i| {
            let m = &messages[i];
            m.role() == "assistant"
                && (m.final_part().is_some() || (!m.text().is_empty() && !has_calls(&m.value)))
        });
        let final_part = answer.and_then(|i| messages[i].final_part());
        let answer_text = answer
            .map(|i| {
                let m = &messages[i];
                if let Some(parts) = m.value["content"].as_array() {
                    parts
                        .iter()
                        .enumerate()
                        .filter(|(index, part)| {
                            part["type"] == "text" && final_part.is_none_or(|start| *index >= start)
                        })
                        .filter_map(|(_, part)| part["text"].as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                } else {
                    m.text()
                }
            })
            .unwrap_or_default();
        let final_started = answer.is_some_and(|i| {
            let m = &messages[i];
            // `stop` may arrive during text_start, or only at message_end.
            // Pending prose remains visible but must not unlock the outer disclosure.
            m.value["stopReason"] == "stop" || (!active && m.value.get("stopReason").is_none())
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
                    result: m.value.clone(),
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
                            let (tool_result, status) = if let Some(result) = result {
                                (result.value.clone(), result_status(result))
                            } else if let Some(update) = update {
                                match &update.execution {
                                    ToolExecution::Running(output) => (
                                        output.clone(),
                                        if active {
                                            ToolStatus::Running
                                        } else {
                                            ToolStatus::Unfinished
                                        },
                                    ),
                                    ToolExecution::Complete(output) => {
                                        (output.clone(), ToolStatus::Complete)
                                    }
                                    ToolExecution::Failed(output) => {
                                        (output.clone(), ToolStatus::Failed)
                                    }
                                }
                            } else {
                                (
                                    Value::Null,
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
                                result: tool_result,
                                status,
                            }));
                        }
                        Some("text" | "thinking") => {
                            let thinking = part["type"] == "thinking";
                            if !thinking
                                && Some(index) == answer
                                && final_part.is_none_or(|start| i >= start)
                            {
                                continue;
                            }
                            let text = part[if thinking { "thinking" } else { "text" }]
                                .as_str()
                                .unwrap_or_default();
                            let running = thinking
                                && active
                                && index + 1 == messages.len()
                                && i + 1 == parts.len()
                                && m.completed_at.is_none()
                                && m.value["stopReason"] == "pending";
                            // Pi emits thinking_start with an empty block before
                            // its first delta. Keep that activity visible too.
                            if !text.is_empty() || running {
                                activities.push(Activity::Text {
                                    id,
                                    text: text.to_owned(),
                                    thinking,
                                    running,
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
                    running: false,
                });
            }
        }
        Self {
            activities,
            answer,
            answer_text,
            final_started,
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

fn has_calls(message: &Value) -> bool {
    message["content"]
        .as_array()
        .is_some_and(|parts| parts.iter().any(|p| p["type"] == "toolCall"))
}

fn result_status(result: &DisplayMessage) -> ToolStatus {
    if result.value["isError"] == true {
        ToolStatus::Failed
    } else {
        ToolStatus::Complete
    }
}

impl Tool {
    pub fn kind(&self) -> ToolKind {
        ToolKind::classify(&self.name, self.args.as_ref())
    }
    pub fn summary(&self) -> Option<&str> {
        let args = self.args.as_ref()?;
        if self.name == "read" {
            return skill_name(args).or_else(|| read_path(args));
        }
        let field = match self.name.as_str() {
            "bash" | "powershell" => "command",
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
    use crate::foundation::session_catalog::text_content;
    use serde_json::json;

    fn message(id: &str, value: Value) -> DisplayMessage {
        DisplayMessage {
            id: id.into(),
            entry: Some(id.into()),
            value,
            final_answer_part: None,
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
    fn thinking_alone_does_not_create_a_process_before_or_after_a_direct_answer() {
        let mut reply = message(
            "reply",
            json!({"role":"assistant", "stopReason":"pending", "content":[
                {"type":"thinking", "thinking":"First thought"},
                {"type":"thinking", "thinking":"Second thought"}
            ]}),
        );
        assert!(!RunContent::project(&[], &[], true).has_process());
        let project = |reply: &DisplayMessage, active| {
            RunContent::project(std::slice::from_ref(reply), &[], active)
        };
        assert!(!project(&reply, true).has_process());
        reply.value["content"]
            .as_array_mut()
            .unwrap()
            .push(json!({"type":"text", "text":"Direct answer"}));
        let pending = project(&reply, true);
        assert_eq!(pending.answer_text, "Direct answer");
        assert!(!pending.has_process());
        reply.value["stopReason"] = json!("stop");
        for active in [true, false] {
            let content = project(&reply, active);
            assert!(content.final_started);
            assert_eq!(content.answer_text, "Direct answer");
            assert!(!content.has_process());
        }
    }

    #[test]
    fn tools_or_intermediate_prose_reveal_the_process_and_preserve_thinking() {
        let thought = message(
            "thought",
            json!({"role":"assistant", "stopReason":"pending", "content":[
                {"type":"thinking", "thinking":"Initial thought"}
            ]}),
        );
        let with_tool = RunContent::project(&[thought.clone(), call()], &[], true);
        assert!(with_tool.has_process());
        assert!(matches!(&with_tool.activities[0], Activity::Text {
            thinking: true, text, ..
        } if text == "Initial thought"));

        let progress = message(
            "progress",
            json!({"role":"assistant", "content":"Let me check."}),
        );
        let mut messages = vec![thought.clone(), progress, thought];
        assert!(RunContent::project(&messages, &[], true).has_process());
        messages.push(message(
            "answer",
            json!({"role":"assistant", "stopReason":"stop", "content":"Done"}),
        ));
        let completed = RunContent::project(&messages, &[], false);
        assert!(completed.has_process());
        assert!(completed.final_started);
        assert_eq!(completed.answer_text, "Done");
    }

    #[test]
    fn call_updates_in_place_and_persisted_result_replaces_live_output() {
        let live = ToolActivity {
            id: "read-1".into(),
            name: "read".into(),
            args: json!({"path":"a.rs"}),
            execution: ToolExecution::Running(
                json!({"content":[{"type":"text", "text":"partial"}], "details":{"truncation":{"truncated":true}}}),
            ),
        };
        let running = RunContent::project(&[call()], std::slice::from_ref(&live), true);
        let running_tool = tools(&running)[0];
        assert_eq!(text_content(&running_tool.result), "partial");
        assert_eq!(running_tool.status, ToolStatus::Running);
        assert_eq!(
            running_tool.result["details"]["truncation"]["truncated"],
            true
        );
        let mut result = tool_result("complete", false);
        result.value["details"] = json!({"patch":"actual patch"});
        result.value["content"]
            .as_array_mut()
            .unwrap()
            .push(json!({"type":"image", "mimeType":"image/png", "data":"bytes"}));
        let settled = RunContent::project(&[call(), result.clone()], &[live], true);
        assert_eq!(tools(&settled).len(), 1);
        let settled_tool = tools(&settled)[0];
        assert_eq!(settled_tool.id, running_tool.id);
        assert_eq!(text_content(&settled_tool.result), "complete");
        assert_eq!(settled_tool.status, ToolStatus::Complete);
        assert_eq!(settled_tool.result["details"]["patch"], "actual patch");
        assert_eq!(settled_tool.result["content"][1]["data"], "bytes");
        assert_eq!(settled_tool.entries, ["call", "result"]);
        assert_eq!(settled_tool.summary(), Some("a.rs"));
        let history = RunContent::project(&[call(), result.clone()], &[], false);
        assert_eq!(tools(&history)[0].result, settled_tool.result);
        let orphan = RunContent::project(&[result], &[], false);
        assert_eq!(tools(&orphan)[0].result, settled_tool.result);
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
    fn thinking_start_is_visible_before_text_and_does_not_mark_old_work_as_running() {
        let mut messages = vec![
            call(),
            tool_result("File contents", false),
            message(
                "next",
                json!({"role":"assistant", "stopReason":"pending", "content":[]}),
            ),
        ];
        let has_running_thought = |content: &RunContent| {
            content.activities.iter().any(|item| {
                matches!(
                    item,
                    Activity::Text {
                        thinking: true,
                        running: true,
                        ..
                    }
                )
            })
        };
        assert!(!has_running_thought(&RunContent::project(
            &messages,
            &[],
            true
        )));

        messages[2].value["content"] = json!([{"type":"thinking", "thinking":""}]);
        let content = RunContent::project(&messages, &[], true);
        assert!(matches!(content.activities.last(), Some(Activity::Text {
            id, text, thinking: true, running: true,
        }) if id == "part-next-0" && text.is_empty()));
        assert!(
            matches!(content.blocks().last(), Some(ActivityBlock::Group { items, .. })
            if items.len() == 2 && matches!(items.last(), Some(Activity::Text { running: true, .. })))
        );

        messages[2].value["content"][0]["thinking"] = json!("Checking the next step");
        assert!(has_running_thought(&RunContent::project(
            &messages,
            &[],
            true
        )));
        assert!(!has_running_thought(&RunContent::project(
            &messages,
            &[],
            false
        )));

        messages[2].completed_at = Some(1);
        assert!(!has_running_thought(&RunContent::project(
            &messages,
            &[],
            true
        )));
        messages[2].completed_at = None;
        messages[2].value["content"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "type":"text", "text":"Here is the next step"
            }));
        let content = RunContent::project(&messages, &[], true);
        assert!(!has_running_thought(&content));
        assert!(matches!(content.activities.last(), Some(Activity::Text {
            text, thinking: true, running: false, ..
        }) if text == "Checking the next step"));
    }

    #[test]
    fn only_assistant_prose_splits_tool_groups() {
        let tool = |id: &str| {
            Activity::Tool(Tool {
                id: id.into(),
                entries: vec![],
                name: "read".into(),
                args: None,
                result: Value::Null,
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
                    running: false,
                },
                tool("second"),
                Activity::Text {
                    id: "progress".into(),
                    text: "Next step".into(),
                    thinking: false,
                    running: false,
                },
                tool("third"),
            ],
            answer: None,
            answer_text: String::new(),
            final_started: false,
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
    #[test]
    fn final_phase_unlocks_without_hiding_earlier_text_in_the_same_message() {
        let mut m = message(
            "stream",
            json!({"role":"assistant", "stopReason":"pending", "content":[
                {"type":"text", "text":"Checking files"},
                {"type":"toolCall", "id":"r", "name":"read", "arguments":{"path":"a"}},
                {"type":"thinking", "thinking":"Consider the result"},
                {"type":"text", "text":""}
            ]}),
        );
        assert!(!RunContent::project(&[m.clone()], &[], true).final_started);
        m.value["stopReason"] = json!("stop");
        m.final_answer_part = Some(3);
        let content = RunContent::project(&[m.clone()], &[], true);
        assert!(content.final_started); // text_start arrives before any final text delta.
        assert!(content.answer_text.is_empty());
        assert!(
            matches!(&content.activities[0], Activity::Text { text, .. } if text == "Checking files")
        );
        m.value["content"][3]["text"] = json!("The result");
        let content = RunContent::project(&[m.clone()], &[], true);
        assert_eq!(content.answer_text, "The result");
        m.final_answer_part = None;
        m.value["content"][3]["textSignature"] =
            json!(r#"{"v":1,"id":"final","phase":"final_answer"}"#);
        assert_eq!(
            RunContent::project(&[m.clone()], &[], false).answer_text,
            "The result"
        );
        m.value["stopReason"] = json!("error");
        let failed = RunContent::project(&[m], &[], true);
        assert!(!failed.final_started);
        assert!(failed.interrupted);
    }

    #[test]
    fn pending_text_remains_visible_but_only_confirmed_answer_unlocks() {
        let mut m = message(
            "stream",
            json!({"role":"assistant", "stopReason":"pending", "content":[
                {"type":"thinking", "thinking":"Reasoning"}, {"type":"text", "text":"Reply"}
            ]}),
        );
        let pending = RunContent::project(&[m.clone()], &[], true);
        assert_eq!(pending.answer_text, "Reply");
        assert!(!pending.final_started);
        m.value["stopReason"] = json!("toolUse");
        m.value["content"]
            .as_array_mut()
            .unwrap()
            .push(json!({"type":"toolCall", "id":"next", "name":"ls", "arguments":{}}));
        let continued = RunContent::project(&[m.clone()], &[], true);
        assert_eq!(continued.answer, None);
        assert!(!continued.final_started);
        m.value["content"].as_array_mut().unwrap().pop();
        m.value["stopReason"] = json!("stop");
        m.completed_at = Some(123);
        assert!(RunContent::project(&[m], &[], true).final_started);
    }
}

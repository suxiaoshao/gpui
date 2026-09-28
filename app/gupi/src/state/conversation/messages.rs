//! Assemble RPC deltas into the same live messages consumed by the UI.
use super::{DisplayMessage, Session};
use serde_json::{Value, json};
use std::collections::HashMap;

pub(super) struct MessageStream {
    message_id: String,
    arguments: HashMap<usize, String>,
}

impl Session {
    pub(super) fn receive_message(&mut self, kind: &str, raw: &Value) {
        // Custom messages gain their identity only in Pi's persisted entries.
        // Keep the assistant stream intact until the authoritative history read.
        if raw["message"]["role"] == "custom" {
            return;
        }
        let index = if let Some(value) = raw.get("message") {
            let candidate = DisplayMessage {
                id: format!(
                    "live-{}-{}",
                    value["role"].as_str().unwrap_or("message"),
                    value
                        .get("timestamp")
                        .map(Value::to_string)
                        .unwrap_or_default()
                ),
                entry: None,
                value: value.clone(),
                final_answer_part: (raw["assistantMessageEvent"]["type"] == "text_start"
                    && value["stopReason"] == "stop")
                    .then(|| raw["assistantMessageEvent"]["contentIndex"].as_u64())
                    .flatten()
                    .map(|i| i as usize),
                completed_at: (kind == "message_end").then(|| {
                    (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64
                }),
            };
            // A final snapshot (notably faux abort) can change the timestamp.
            // It still ends the assistant message opened by message_start.
            let stream_id = self
                .message_stream
                .as_ref()
                .filter(|_| kind != "message_start" && candidate.role() == "assistant")
                .map(|stream| stream.message_id.as_str());
            let signature = candidate.signature();
            let index = self
                .live
                .iter()
                .position(|m| stream_id == Some(m.id.as_str()))
                .or_else(|| self.live.iter().position(|m| m.signature() == signature));
            let index = if let Some(index) = index {
                let existing = &mut self.live[index];
                existing.value = candidate.value;
                existing.final_answer_part =
                    candidate.final_answer_part.or(existing.final_answer_part);
                existing.completed_at = candidate.completed_at;
                index
            } else {
                self.live.push(candidate);
                self.live.len() - 1
            };
            if self.live[index].role() == "assistant" {
                if kind == "message_start" {
                    self.message_stream = Some(MessageStream {
                        message_id: self.live[index].id.clone(),
                        arguments: HashMap::new(),
                    });
                } else if kind == "message_end" {
                    self.message_stream = None;
                }
            }
            index
        } else if kind == "message_update" {
            let Some(stream) = self.message_stream.as_mut() else {
                return;
            };
            let Some(index) = self.live.iter().position(|m| m.id == stream.message_id) else {
                return;
            };
            stream.update(&mut self.live[index].value, raw);
            index
        } else {
            return;
        };

        let message = &self.live[index];
        self.transcript.receive_message();
        self.run.record_message(message.signature());
        if message.value["stopReason"] == "error" {
            self.error = message.value["errorMessage"].as_str().map(str::to_owned);
        }
        if message.value["stopReason"] == "aborted" {
            self.interrupted = true;
        }
    }
}

impl MessageStream {
    fn update(&mut self, message: &mut Value, raw: &Value) {
        if let Some(usage) = raw.get("usage") {
            message["usage"] = usage.clone();
        }
        let event = &raw["assistantMessageEvent"];
        let Some(index) = event["contentIndex"]
            .as_u64()
            .and_then(|i| usize::try_from(i).ok())
        else {
            return;
        };
        let Some(parts) = message["content"].as_array_mut() else {
            return;
        };
        let kind = event["type"].as_str().unwrap_or_default();
        let start = match kind {
            "text_start" => Some(json!({"type":"text", "text":""})),
            "thinking_start" => Some(json!({"type":"thinking", "thinking":""})),
            "toolcall_start" => {
                self.arguments.insert(index, String::new());
                Some(
                    json!({"type":"toolCall", "id":event["id"], "name":event["toolName"], "arguments":{}}),
                )
            }
            _ => None,
        };
        if let Some(part) = start {
            if index == parts.len() {
                parts.push(part);
            } else if let Some(existing) = parts.get_mut(index) {
                *existing = part;
            }
            return;
        }
        let Some(part) = parts.get_mut(index) else {
            return;
        };
        match kind {
            "text_delta" | "thinking_delta" => {
                let field = if kind == "text_delta" {
                    "text"
                } else {
                    "thinking"
                };
                if let (Value::String(text), Some(delta)) =
                    (&mut part[field], event["delta"].as_str())
                {
                    text.push_str(delta);
                }
            }
            "text_end" | "thinking_end" => {
                let field = if kind == "text_end" {
                    "text"
                } else {
                    "thinking"
                };
                if let Some(content) = event["content"].as_str() {
                    part[field] = content.into();
                }
            }
            "toolcall_delta" => {
                if let (Some(buffer), Some(delta)) =
                    (self.arguments.get_mut(&index), event["delta"].as_str())
                {
                    buffer.push_str(delta);
                    if let Ok(value) = serde_json::from_str::<Value>(buffer) {
                        part["arguments"] = value;
                    }
                }
            }
            "toolcall_end" => {
                if let Some(call) = event.get("toolCall").filter(|call| call.is_object()) {
                    *part = call.clone();
                }
                self.arguments.remove(&index);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
impl Session {
    pub(crate) fn from_rpc_messages(events: &[Value]) -> Self {
        let mut session = Self::new(
            super::SessionInfo {
                path: Default::default(),
                id: "rpc-stream".into(),
                cwd: Default::default(),
                name: None,
                first_message: String::new(),
                activity: String::new(),
                parent_session: None,
            },
            String::new(),
        );
        session.run.start();
        for event in events {
            session.receive_message(event["type"].as_str().unwrap(), event);
        }
        session
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_events_do_not_overlay_messages_or_interrupt_the_assistant_stream() {
        let mut session = Session::from_rpc_messages(&[
            json!({"type":"message_start", "message":{"role":"assistant", "timestamp":1, "content":[]}}),
            json!({"type":"message_update", "assistantMessageEvent":{"type":"text_start", "contentIndex":0}}),
            json!({"type":"message_start", "message":{"role":"custom", "timestamp":1, "content":"notice", "display":true}}),
            json!({"type":"message_end", "message":{"role":"custom", "timestamp":1, "content":"notice", "display":true}}),
            json!({"type":"message_end", "message":{"role":"custom", "timestamp":1, "content":"hidden", "display":false}}),
        ]);
        session.receive_message(
            "message_update",
            &json!({"assistantMessageEvent":{
                "type":"text_delta", "contentIndex":0, "delta":"still streaming"
            }}),
        );
        assert_eq!(session.live.len(), 1);
        assert_eq!(session.live[0].role(), "assistant");
        assert_eq!(session.live[0].text(), "still streaming");
        assert!(session.message_stream.is_some());
    }
    #[test]
    fn final_snapshot_replaces_stream_even_when_abort_changes_timestamp() {
        let mut session = Session::from_rpc_messages(&[
            json!({"type":"message_start", "message":{"role":"assistant", "timestamp":1, "stopReason":"pending", "content":[]}}),
            json!({"type":"message_update", "assistantMessageEvent":{"type":"text_start", "contentIndex":0}}),
            json!({"type":"message_update", "usage":{"output":2}, "assistantMessageEvent":{"type":"text_delta", "contentIndex":0, "delta":"未完成"}}),
        ]);
        let id = session.live[0].id.clone();
        session.receive_message("message_end", &json!({"message":{
            "role":"assistant", "timestamp":2, "stopReason":"aborted", "content":[{"type":"text", "text":"停止前的内容"}], "usage":{"output":3}
        }}));
        assert_eq!(session.live.len(), 1);
        assert_eq!(session.live[0].id, id);
        assert_eq!(session.live[0].text(), "停止前的内容");
        assert_eq!(session.live[0].value["usage"]["output"], 3);
        assert!(session.live[0].completed_at.is_some());
        assert!(session.interrupted);
        assert!(session.message_stream.is_none());

        session.receive_message(
            "message_update",
            &json!({"assistantMessageEvent":{
                "type":"text_delta", "contentIndex":0, "delta":"不能追加到已结束消息"
            }}),
        );
        assert_eq!(session.live[0].text(), "停止前的内容");

        session.receive_message(
            "message_start",
            &json!({"message":{
                "role":"assistant", "timestamp":3, "stopReason":"pending", "content":[]
            }}),
        );
        session.receive_message(
            "message_update",
            &json!({"assistantMessageEvent":{
                "type":"thinking_start", "contentIndex":0
            }}),
        );
        assert_eq!(session.live.len(), 2);
        assert_eq!(session.live[1].value["content"][0]["type"], "thinking");
        assert!(session.live[1].completed_at.is_none());
    }
}

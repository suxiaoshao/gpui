use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The initial typed command surface. Commands are Pi's own JSONL protocol.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    GetState,
    GetCommands,
    Prompt(Prompt),
    Abort,
    ClearQueue,
    GetEntries,
    GetForkMessages,
    Fork {
        #[serde(rename = "entryId")]
        entry_id: String,
    },
    SetSessionName {
        name: String,
    },
    GetAvailableModels,
    SetModel {
        provider: String,
        #[serde(rename = "modelId")]
        model_id: String,
    },
    GetAvailableThinkingLevels,
    SetThinkingLevel {
        level: String,
    },
    GetSessionStats,
}
impl Command {
    pub fn name(&self) -> &'static str {
        match self {
            Self::GetState => "get_state",
            Self::GetCommands => "get_commands",
            Self::Prompt(_) => "prompt",
            Self::Abort => "abort",
            Self::ClearQueue => "clear_queue",
            Self::GetEntries => "get_entries",
            Self::GetForkMessages => "get_fork_messages",
            Self::Fork { .. } => "fork",
            Self::SetSessionName { .. } => "set_session_name",
            Self::GetAvailableModels => "get_available_models",
            Self::SetModel { .. } => "set_model",
            Self::GetAvailableThinkingLevels => "get_available_thinking_levels",
            Self::SetThinkingLevel { .. } => "set_thinking_level",
            Self::GetSessionStats => "get_session_stats",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Prompt {
    pub message: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<Image>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub streaming_behavior: Option<StreamingBehavior>,
}
impl Prompt {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            images: Vec::new(),
            streaming_behavior: None,
        }
    }
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StreamingBehavior {
    Steer,
    FollowUp,
}
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename = "image", rename_all = "camelCase")]
pub struct Image {
    pub data: String,
    pub mime_type: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionState {
    pub session_id: String,
    pub is_streaming: bool,
    pub is_compacting: bool,
    pub session_file: Option<String>,
    pub session_name: Option<String>,
    pub model: Option<Model>,
    #[serde(default)]
    pub thinking_level: String,
    #[serde(default)]
    pub auto_compaction_enabled: bool,
    #[serde(default)]
    pub pending_message_count: usize,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Entry identity is typed; kind-specific fields preserve Pi's extensible format.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEntry {
    pub id: String,
    pub parent_id: Option<String>,
    pub timestamp: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(flatten)]
    pub data: Map<String, Value>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entries {
    pub entries: Vec<SessionEntry>,
    pub leaf_id: Option<String>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForkMessage {
    pub entry_id: String,
    pub text: String,
}
#[derive(Clone, Debug, Deserialize)]
pub struct ForkMessages {
    pub messages: Vec<ForkMessage>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct ForkResult {
    #[serde(default)]
    pub text: String,
    pub cancelled: bool,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    pub id: String,
    pub name: String,
    pub provider: String,
    #[serde(default)]
    pub reasoning: bool,
    #[serde(default)]
    pub context_window: u64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Models {
    pub models: Vec<Model>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct ThinkingLevels {
    pub levels: Vec<String>,
}
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    #[serde(default)]
    pub input: u64,
    #[serde(default)]
    pub output: u64,
    #[serde(default)]
    pub cache_read: u64,
    #[serde(default)]
    pub cache_write: u64,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextUsage {
    pub tokens: Option<u64>,
    pub context_window: u64,
    pub percent: Option<f64>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStats {
    pub tokens: TokenUsage,
    pub cost: f64,
    pub context_usage: Option<ContextUsage>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlashCommand {
    pub name: String,
    pub description: Option<String>,
    pub source: String,
    pub source_info: Value,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Commands {
    pub commands: Vec<SlashCommand>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearedQueue {
    pub steering: Vec<String>,
    pub follow_up: Vec<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Successful response envelope, including fields added by newer Pi versions.
#[derive(Clone, Debug, Deserialize)]
pub struct Response {
    pub id: Option<String>,
    pub command: String,
    pub success: bool,
    #[serde(default)]
    pub data: Value,
    pub error: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ExtensionRequest {
    pub id: String,
    #[serde(flatten)]
    pub method: UiMethod,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "method")]
pub enum UiMethod {
    #[serde(rename = "select")]
    Select {
        title: String,
        options: Vec<String>,
        timeout: Option<u64>,
    },
    #[serde(rename = "confirm")]
    Confirm {
        title: String,
        message: String,
        timeout: Option<u64>,
    },
    #[serde(rename = "input")]
    Input {
        title: String,
        placeholder: Option<String>,
        timeout: Option<u64>,
    },
    #[serde(rename = "editor")]
    Editor {
        title: String,
        prefill: Option<String>,
    },
    #[serde(rename = "notify")]
    Notify {
        message: String,
        #[serde(rename = "notifyType")]
        notify_type: Option<String>,
    },
    #[serde(rename = "setStatus")]
    SetStatus {
        #[serde(rename = "statusKey")]
        key: String,
        #[serde(rename = "statusText")]
        text: Option<String>,
    },
    #[serde(rename = "setWidget")]
    SetWidget {
        #[serde(rename = "widgetKey")]
        key: String,
        #[serde(rename = "widgetLines")]
        lines: Option<Vec<String>>,
        #[serde(rename = "widgetPlacement")]
        placement: Option<String>,
    },
    #[serde(rename = "setTitle")]
    SetTitle { title: String },
    #[serde(rename = "set_editor_text")]
    SetEditorText { text: String },
    #[serde(other)]
    Unknown,
}
#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum UiReply {
    Value { value: String },
    Confirmed { confirmed: bool },
    Cancelled { cancelled: bool },
}
impl UiReply {
    pub fn cancelled() -> Self {
        Self::Cancelled { cancelled: true }
    }
}

/// Raw payload is retained even for typed extension requests.
#[derive(Clone, Debug)]
pub enum Event {
    ExtensionUi {
        request: ExtensionRequest,
        raw: Value,
    },
    Agent {
        kind: String,
        raw: Value,
    },
}
impl Event {
    pub fn raw(&self) -> &Value {
        match self {
            Self::ExtensionUi { raw, .. } | Self::Agent { raw, .. } => raw,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn follow_up_prompt_uses_the_native_camel_case_value() {
        let mut prompt = super::Prompt::new("next");
        prompt.streaming_behavior = Some(super::StreamingBehavior::FollowUp);
        let value = serde_json::to_value(prompt).unwrap();
        assert_eq!(value["streamingBehavior"], "followUp");
    }
    use super::*;
    #[test]
    fn unknown_extension_method_and_image_envelope_remain_compatible() {
        let value = serde_json::json!({"type":"extension_ui_request","id":"u","method":"future_dialog","payload":{"a":1}});
        let request: ExtensionRequest = serde_json::from_value(value).unwrap();
        assert!(matches!(request.method, UiMethod::Unknown));
        let image = Image {
            data: "abc".into(),
            mime_type: "image/png".into(),
        };
        assert_eq!(
            serde_json::to_value(image).unwrap(),
            serde_json::json!({"type":"image","data":"abc","mimeType":"image/png"})
        );
    }
}

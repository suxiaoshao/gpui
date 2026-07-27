use super::*;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum ConversationEntryPayload {
    Message {
        role: TranscriptRole,
        content: Vec<ContentPart>,
    },
    SkillActivation(SkillActivationEntry),
    Reasoning {
        text: String,
        summary: Option<String>,
    },
    ToolCall(ToolCallEntry),
    ToolResult(ToolResultEntry),
    ApprovalRequest(ApprovalRequestEntry),
    ApprovalDecision(ApprovalDecisionEntry),
    Status(ConversationStatusEntry),
    Error(RunErrorPayload),
}

impl ConversationEntryPayload {
    pub fn kind(&self) -> ConversationEntryKind {
        match self {
            Self::Message { .. } => ConversationEntryKind::Message,
            Self::SkillActivation(_) => ConversationEntryKind::SkillActivation,
            Self::Reasoning { .. } => ConversationEntryKind::Reasoning,
            Self::ToolCall(_) => ConversationEntryKind::ToolCall,
            Self::ToolResult(_) => ConversationEntryKind::ToolResult,
            Self::ApprovalRequest(_) => ConversationEntryKind::ApprovalRequest,
            Self::ApprovalDecision(_) => ConversationEntryKind::ApprovalDecision,
            Self::Status(_) => ConversationEntryKind::Status,
            Self::Error(_) => ConversationEntryKind::Error,
        }
    }

    pub fn search_text(&self) -> String {
        match self {
            Self::Message { content, .. } => content_parts_search_text(content),
            Self::SkillActivation(skill) => {
                format!(
                    "{} {}",
                    skill.name,
                    content_parts_search_text(&skill.content)
                )
            }
            Self::Reasoning { text, summary } => join_search_parts([Some(text), summary.as_ref()]),
            Self::ToolCall(call) => {
                format!("{} {}", call.name, call.runtime_tool_name)
            }
            Self::ToolResult(result) => content_parts_search_text(&result.content),
            Self::ApprovalRequest(item) => {
                format!("{} {}", item.request.tool_name, item.request.reason)
            }
            Self::ApprovalDecision(item) => item.decision.reason.clone().unwrap_or_default(),
            Self::Status(status) => {
                let code = match status.code {
                    ConversationStatusCode::Canceled => "canceled",
                    ConversationStatusCode::MaxStepsReached => "max_steps_reached",
                    ConversationStatusCode::CompletedWithoutOutput => "completed_without_output",
                }
                .to_string();
                join_search_parts([Some(&code), status.message.as_ref()])
            }
            Self::Error(error) => format!("{} {}", error.code, error.message),
        }
    }
}

fn content_parts_search_text(content: &[ContentPart]) -> String {
    content
        .iter()
        .filter_map(ContentPart::search_text)
        .collect::<Vec<_>>()
        .join("\n")
}

fn join_search_parts<'a>(parts: impl IntoIterator<Item = Option<&'a String>>) -> String {
    parts
        .into_iter()
        .flatten()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillActivationEntry {
    pub name: String,
    pub source_kind: SkillSourceKind,
    pub skill_file_path: String,
    pub directory_path: String,
    pub content_sha256: String,
    pub content: Vec<ContentPart>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillSourceKind {
    BuiltIn,
    User,
    Project,
    Plugin,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolCallEntry {
    pub tool_invocation_id: Option<ToolInvocationId>,
    pub call_id: String,
    pub source: ToolSource,
    pub name: String,
    pub runtime_tool_name: String,
    pub arguments: ToolArguments,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolResultEntry {
    pub tool_invocation_id: Option<ToolInvocationId>,
    pub call_id: String,
    pub content: Vec<ContentPart>,
    pub is_error: bool,
    pub structured_output: Option<StructuredOutput>,
    pub raw_output: Option<ProviderRawPayload>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum ToolSource {
    Local,
    Mcp { server_id: String },
    ProviderHosted { provider_id: ProviderId },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolArguments {
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructuredOutput {
    pub value: serde_json::Value,
}

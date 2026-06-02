use crate::{
    AgentRunId, ApprovalDecisionId, AttachmentId, ConversationId, ConversationItemId, ProjectId,
    ProviderId, ProviderModelId, ProviderStepId, ToolInvocationId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectKind {
    Normal,
    Scratch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationStatus {
    Active,
    Archived,
    Deleted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationItemKind {
    Message,
    SkillActivation,
    Reasoning,
    ToolCall,
    ToolResult,
    ApprovalRequest,
    ApprovalDecision,
    Status,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationItemStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Canceled,
    WaitingForApproval,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentKind {
    Image,
    File,
    Audio,
    Attachment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentStorageKind {
    LocalFile,
    ExternalUri,
    ProviderFile,
    GeneratedFile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRunTriggerKind {
    User,
    Shortcut,
    Resume,
    Retry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRunStatus {
    Queued,
    Running,
    WaitingForApproval,
    Completed,
    Failed,
    Canceled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderStepStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Canceled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolInvocationStatus {
    Requested,
    AwaitingApproval,
    Running,
    Succeeded,
    Failed,
    Denied,
    Canceled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Denied,
    Expired,
    Canceled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShortcutInputSource {
    SelectionOrClipboard,
    Screenshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TranscriptRole {
    System,
    Developer,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum ContentPart {
    Text { text: String },
    Image { attachment_id: AttachmentId },
    File { attachment_id: AttachmentId },
    Audio { attachment_id: AttachmentId },
    Attachment { attachment_id: AttachmentId },
}

impl ContentPart {
    pub fn search_text(&self) -> Option<&str> {
        match self {
            Self::Text { text } => Some(text),
            Self::Image { .. }
            | Self::File { .. }
            | Self::Audio { .. }
            | Self::Attachment { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum AttachmentSource {
    LocalFile {
        path: String,
    },
    ExternalUri {
        uri: String,
    },
    ProviderFile {
        provider_id: ProviderId,
        file_id: String,
    },
    GeneratedFile {
        path: String,
    },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttachmentMetadata {
    pub source: AttachmentSource,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_ms: Option<u64>,
    pub preview_attachment_id: Option<AttachmentId>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum ConversationItemPayload {
    Message {
        role: TranscriptRole,
        content: Vec<ContentPart>,
    },
    SkillActivation(SkillActivationItem),
    Reasoning {
        text: String,
        summary: Option<String>,
    },
    ToolCall(ToolCallItem),
    ToolResult(ToolResultItem),
    ApprovalRequest(ApprovalRequestItem),
    ApprovalDecision(ApprovalDecisionItem),
    Status(StatusItem),
    Error(RunErrorPayload),
}

impl ConversationItemPayload {
    pub fn kind(&self) -> ConversationItemKind {
        match self {
            Self::Message { .. } => ConversationItemKind::Message,
            Self::SkillActivation(_) => ConversationItemKind::SkillActivation,
            Self::Reasoning { .. } => ConversationItemKind::Reasoning,
            Self::ToolCall(_) => ConversationItemKind::ToolCall,
            Self::ToolResult(_) => ConversationItemKind::ToolResult,
            Self::ApprovalRequest(_) => ConversationItemKind::ApprovalRequest,
            Self::ApprovalDecision(_) => ConversationItemKind::ApprovalDecision,
            Self::Status(_) => ConversationItemKind::Status,
            Self::Error(_) => ConversationItemKind::Error,
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
                join_search_parts([Some(&status.label), status.message.as_ref()])
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
pub struct SkillActivationItem {
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
pub struct ToolCallItem {
    pub tool_invocation_id: Option<ToolInvocationId>,
    pub call_id: String,
    pub source: ToolSource,
    pub name: String,
    pub runtime_tool_name: String,
    pub arguments: ToolArguments,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolResultItem {
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

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentRunInput {
    pub user_item_id: ConversationItemId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_agent_run_id: Option<AgentRunId>,
    pub prompt_snapshot: Option<PromptContent>,
    pub provider_id: ProviderId,
    pub model_id: ProviderModelId,
    pub settings_snapshot: RunSettingsSnapshot,
    pub runtime_snapshot: AgentRuntimeSnapshot,
    pub max_steps: u32,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentRuntimeSnapshot {
    pub engine: AgentEngineKind,
    pub engine_version: String,
    pub skill_catalog_hash: Option<String>,
    pub mcp_config_hash: Option<String>,
    pub tool_name_strategy: ToolNameStrategy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentEngineKind {
    Rig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolNameStrategy {
    Direct,
    Namespaced,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentRunOutput {
    pub final_item_id: Option<ConversationItemId>,
    pub stopped_reason: AgentStoppedReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStoppedReason {
    Completed,
    MaxSteps,
    Canceled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum AgentRunEvent {
    Started {
        agent_run_id: AgentRunId,
    },
    ProviderStepStarted {
        provider_step_id: ProviderStepId,
    },
    ProviderStepEvent {
        provider_step_id: ProviderStepId,
        event: ProviderStepEvent,
    },
    ToolInvocationRequested {
        tool_invocation_id: ToolInvocationId,
    },
    ApprovalRequested {
        approval_decision_id: ApprovalDecisionId,
    },
    ToolInvocationFinished {
        tool_invocation_id: ToolInvocationId,
    },
    Completed {
        output: AgentRunOutput,
    },
    Failed {
        error: RunErrorPayload,
    },
    Canceled,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentRunState {
    pub agent_run_id: AgentRunId,
    pub status: AgentRunStatus,
    pub current_step_id: Option<ProviderStepId>,
    pub pending_tool_ids: Vec<ToolInvocationId>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderStepRequestSnapshot {
    pub provider_id: ProviderId,
    pub model_id: ProviderModelId,
    pub input_item_ids: Vec<ConversationItemId>,
    pub snapshot_kind: ProviderStepSnapshotKind,
    pub request_body: ProviderRawPayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderStepSnapshotKind {
    ProviderWire,
    RigCompletionRequest,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderStepResponseSnapshot {
    pub provider_run_id: Option<String>,
    pub output_item_ids: Vec<String>,
    pub response_body: Option<ProviderRawPayload>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderRunStateSnapshot {
    pub provider_id: ProviderId,
    pub provider_run_id: Option<String>,
    pub output_item_ids: Vec<String>,
    pub continuation: Option<ProviderRawPayload>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum ProviderStepEvent {
    OutputItemStarted {
        provider_item_id: Option<String>,
        item: ConversationItemPayload,
    },
    TextDelta {
        provider_item_id: Option<String>,
        text: String,
    },
    ReasoningDelta {
        provider_item_id: Option<String>,
        text: String,
    },
    OutputItemCompleted {
        provider_item_id: Option<String>,
        item: ConversationItemPayload,
    },
    ToolCallRequested {
        call: ToolCallItem,
    },
    UsageUpdated {
        usage: ProviderUsageSnapshot,
    },
    Completed {
        state: Option<ProviderRunStateSnapshot>,
    },
    Failed {
        error: RunErrorPayload,
    },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunSettingsSnapshot {
    pub prompt: Option<PromptContent>,
    pub provider_id: ProviderId,
    pub model_id: ProviderModelId,
    pub model_capabilities: ModelCapabilitiesSnapshot,
    pub provider_settings: ProviderSettingsPayload,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_selection: Option<ReasoningSelectionSnapshot>,
    pub tool_policy: ToolPolicySnapshot,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PromptContent {
    pub messages: Vec<PromptMessage>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PromptMessage {
    pub role: TranscriptRole,
    pub content: Vec<ContentPart>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderSettingsPayload {
    pub provider_kind: String,
    pub fields: Vec<ProviderSettingFieldValue>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderSecretRefs {
    pub refs: Vec<ProviderSecretRef>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelCapabilitiesSnapshot {
    pub text_input: bool,
    pub text_output: bool,
    pub streaming: bool,
    pub image_input: Option<ImageInputCapabilitySnapshot>,
    pub file_input: Option<FileInputCapabilitySnapshot>,
    pub audio_input: bool,
    pub image_generation: bool,
    pub tool_calling: Option<ToolCallingCapabilitySnapshot>,
    pub hosted_web_search: bool,
    pub remote_mcp: bool,
    pub reasoning: Option<ReasoningCapabilitySnapshot>,
    pub structured_output: bool,
    pub stateful_response_continuation: bool,
    pub extension: ProviderCapabilityExtensionSnapshot,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SchemaMetadataPayload {
    pub store_kind: String,
    pub legacy_policy: LegacyStorePolicy,
    pub feature_flags: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyStorePolicy {
    Ignore,
    BackupOnly,
    ReadOnly,
    ManualImport,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectMetadata {
    pub scratch_reason: Option<String>,
    pub git_root: Option<String>,
    pub last_active_conversation_id: Option<ConversationId>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationMetadata {
    pub summary: Option<String>,
    pub tags: Vec<String>,
    pub pinned: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationSettingsSnapshot {
    pub prompt: Option<PromptContent>,
    pub provider_id: Option<ProviderId>,
    pub model_id: Option<ProviderModelId>,
    pub model_capabilities: Option<ModelCapabilitiesSnapshot>,
    pub tool_policy: ToolPolicySnapshot,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunErrorPayload {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub provider: Option<String>,
    pub raw: Option<ProviderRawPayload>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StatusItem {
    pub label: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolInvocationInput {
    pub source: ToolSource,
    pub namespace: Option<String>,
    pub tool_name: String,
    pub runtime_tool_name: String,
    pub call_id: String,
    pub arguments: ToolArguments,
    pub approval_policy: ToolApprovalPolicy,
    pub execution_policy: ToolExecutionPolicy,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolInvocationOutput {
    pub content: Vec<ContentPart>,
    pub structured_output: Option<StructuredOutput>,
    pub raw_output: Option<ProviderRawPayload>,
    pub is_error: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolApprovalPolicy {
    Never,
    OnRequest,
    Always,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolExecutionPolicy {
    Foreground,
    Background,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovalRequestItem {
    pub approval_decision_id: ApprovalDecisionId,
    pub tool_invocation_id: ToolInvocationId,
    pub request: ApprovalRequestPayload,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovalDecisionItem {
    pub approval_decision_id: ApprovalDecisionId,
    pub decision: ApprovalDecisionPayload,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovalRequestPayload {
    pub reason: String,
    pub tool_source: ToolSource,
    pub tool_name: String,
    pub arguments_preview: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovalDecisionPayload {
    pub approved: bool,
    pub decided_by: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderUsageSnapshot {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_write_input_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_tokens: u64,
    pub metadata: Option<ProviderRawPayload>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum ShortcutAction {
    OpenTemporaryConversation,
    SendToConversation {
        conversation_id: Option<ConversationId>,
    },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolPolicySnapshot {
    pub approval_policy: ToolApprovalPolicy,
    pub enabled_sources: Vec<ToolSource>,
    pub max_steps: u32,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderSettingFieldValue {
    pub key: String,
    pub value: ProviderSettingValue,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum ProviderSettingValue {
    String { value: String },
    Bool { value: bool },
    Number { value: f64 },
    Object { value: ProviderRawPayload },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderSecretRef {
    pub key: String,
    pub storage: String,
    pub ref_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageInputCapabilitySnapshot {
    pub max_images: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileInputCapabilitySnapshot {
    pub max_files: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolCallingCapabilitySnapshot {
    pub parallel_tool_calls: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReasoningCapabilitySnapshot {
    pub default_effort: String,
    pub efforts: Vec<String>,
    pub summaries: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<ReasoningControlSnapshot>,
    #[serde(default = "default_capability_source")]
    pub source: CapabilitySourceSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum CapabilitySourceSnapshot {
    ApiDiscovered {
        provider: String,
        endpoint: String,
    },
    OfficialDocs {
        provider: String,
        url: String,
        checked_at: String,
    },
    Heuristic {
        reason: String,
    },
    Manual {
        source: String,
    },
    OpenRouterNormalized,
}

impl Default for CapabilitySourceSnapshot {
    fn default() -> Self {
        default_capability_source()
    }
}

fn default_capability_source() -> CapabilitySourceSnapshot {
    CapabilitySourceSnapshot::Heuristic {
        reason: "legacy capability payload".to_string(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum ReasoningControlSnapshot {
    None,
    Boolean {
        default_enabled: Option<bool>,
    },
    Levels {
        values: Vec<String>,
        default_value: Option<String>,
    },
    TokenBudget {
        min: Option<u32>,
        max: Option<u32>,
        default_value: Option<i32>,
        dynamic_supported: bool,
        off_supported: bool,
    },
    AdaptiveLevels {
        values: Vec<String>,
        default_value: Option<String>,
    },
    AlwaysOn {
        visible_summary_supported: bool,
    },
    Composite {
        controls: Vec<ReasoningControlSnapshot>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum ReasoningSelectionSnapshot {
    Boolean {
        enabled: bool,
    },
    Level {
        value: String,
    },
    TokenBudget {
        mode: TokenBudgetSelectionMode,
        value: Option<u32>,
    },
    Composite {
        selections: Vec<ReasoningSelectionSnapshot>,
    },
    AlwaysOn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenBudgetSelectionMode {
    Off,
    Dynamic,
    Custom,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "provider", rename_all = "camelCase", deny_unknown_fields)]
pub enum ProviderCapabilityExtensionSnapshot {
    None,
    OpenAi {
        responses_api: bool,
        raw: Option<ProviderRawPayload>,
    },
    Ollama {
        raw_capabilities: Vec<String>,
        family: String,
        #[serde(default)]
        families: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        thinking: Option<OllamaThinkingCapabilitySnapshot>,
        #[serde(default)]
        local_web_tools: bool,
        raw: Option<ProviderRawPayload>,
    },
    Gemini {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        thinking: Option<bool>,
        raw: Option<ProviderRawPayload>,
    },
    OpenRouter {
        #[serde(default)]
        supported_parameters: Vec<String>,
        raw: Option<ProviderRawPayload>,
    },
    Other {
        raw: ProviderRawPayload,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OllamaThinkingCapabilitySnapshot {
    Boolean,
    Levels,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderModelMetadata {
    pub display_name: Option<String>,
    pub family: Option<String>,
    pub raw: Option<ProviderRawPayload>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AppLanguage {
    #[serde(rename = "en-US", alias = "en")]
    English,
    #[serde(rename = "zh-CN", alias = "zh")]
    Chinese,
    #[default]
    #[serde(other, rename = "system")]
    System,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppThemeMode {
    Light,
    Dark,
    #[default]
    #[serde(other)]
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppThemeSettings {
    #[serde(default)]
    pub mode: AppThemeMode,
    pub light_theme: Option<String>,
    pub dark_theme: Option<String>,
    #[serde(default)]
    pub custom_theme_colors: Vec<String>,
}

impl Default for AppThemeSettings {
    fn default() -> Self {
        Self {
            mode: AppThemeMode::System,
            light_theme: None,
            dark_theme: None,
            custom_theme_colors: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppSettingsPayload {
    #[serde(default)]
    pub language: AppLanguage,
    #[serde(default)]
    pub theme: AppThemeSettings,
    #[serde(default)]
    pub temporary_hotkey: Option<String>,
    #[serde(default)]
    pub http_proxy: Option<String>,
    #[serde(default)]
    pub default_project_id: Option<ProjectId>,
}

impl Default for AppSettingsPayload {
    fn default() -> Self {
        Self {
            language: AppLanguage::System,
            theme: AppThemeSettings::default(),
            temporary_hotkey: None,
            http_proxy: None,
            default_project_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderRawPayload {
    pub provider_kind: String,
    pub value: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ids_are_uuid_v7_strings() {
        let id = crate::new_id();
        assert_eq!(id.len(), 36);
        assert_eq!(id.chars().nth(14), Some('7'));
    }

    #[test]
    fn app_settings_payload_roundtrips_as_typed_settings() {
        let payload = AppSettingsPayload {
            language: AppLanguage::Chinese,
            theme: AppThemeSettings {
                mode: AppThemeMode::Dark,
                light_theme: Some("preset:Default Light".to_string()),
                dark_theme: Some("material-you:#3271AE".to_string()),
                custom_theme_colors: vec!["#3271AE".to_string()],
            },
            temporary_hotkey: Some("cmd+shift+j".to_string()),
            http_proxy: Some("http://127.0.0.1:8080".to_string()),
            default_project_id: Some("project_1".to_string()),
        };

        let value = serde_json::to_value(&payload).unwrap();
        assert_eq!(value["language"], "zh-CN");
        assert_eq!(value["theme"]["mode"], "dark");
        assert_eq!(value["temporaryHotkey"], "cmd+shift+j");
        assert_eq!(value["httpProxy"], "http://127.0.0.1:8080");
        assert_eq!(
            serde_json::from_value::<AppSettingsPayload>(value).unwrap(),
            payload
        );
    }

    #[test]
    fn app_settings_payload_defaults_unknown_language_and_theme_mode_to_system() {
        let payload: AppSettingsPayload = serde_json::from_value(json!({
            "language": "fr-FR",
            "theme": {
                "mode": "auto"
            }
        }))
        .unwrap();

        assert_eq!(payload.language, AppLanguage::System);
        assert_eq!(payload.theme.mode, AppThemeMode::System);
        assert_eq!(payload.temporary_hotkey, None);
        assert_eq!(payload.http_proxy, None);
        assert_eq!(payload.default_project_id, None);
    }

    #[test]
    fn skill_activation_roundtrips_with_file_snapshot() {
        let payload = ConversationItemPayload::SkillActivation(SkillActivationItem {
            name: "rust".to_string(),
            source_kind: SkillSourceKind::Project,
            skill_file_path: "/repo/.agents/skills/rust/SKILL.md".to_string(),
            directory_path: "/repo/.agents/skills/rust".to_string(),
            content_sha256: "abc123".to_string(),
            content: vec![ContentPart::Text {
                text: "Use cargo test.".to_string(),
            }],
        });

        let value = serde_json::to_value(&payload).unwrap();
        assert_eq!(value["type"], "skillActivation");
        assert_eq!(
            serde_json::from_value::<ConversationItemPayload>(value).unwrap(),
            payload
        );
        assert_eq!(payload.kind(), ConversationItemKind::SkillActivation);
    }

    #[test]
    fn tool_runtime_name_roundtrips() {
        let payload = ToolInvocationInput {
            source: ToolSource::Mcp {
                server_id: "filesystem".to_string(),
            },
            namespace: Some("filesystem".to_string()),
            tool_name: "read_file".to_string(),
            runtime_tool_name: "filesystem__read_file".to_string(),
            call_id: "call-1".to_string(),
            arguments: ToolArguments {
                value: json!({ "path": "/tmp/a" }),
            },
            approval_policy: ToolApprovalPolicy::OnRequest,
            execution_policy: ToolExecutionPolicy::Foreground,
        };

        let value = serde_json::to_value(&payload).unwrap();
        assert_eq!(value["runtimeToolName"], "filesystem__read_file");
        assert_eq!(
            serde_json::from_value::<ToolInvocationInput>(value).unwrap(),
            payload
        );
    }

    #[test]
    fn rig_runtime_snapshot_roundtrips() {
        let snapshot = AgentRuntimeSnapshot {
            engine: AgentEngineKind::Rig,
            engine_version: "0.22.0".to_string(),
            skill_catalog_hash: Some("skills".to_string()),
            mcp_config_hash: Some("mcp".to_string()),
            tool_name_strategy: ToolNameStrategy::Namespaced,
        };

        let value = serde_json::to_value(&snapshot).unwrap();
        assert_eq!(value["engine"], "rig");
        assert_eq!(
            serde_json::from_value::<AgentRuntimeSnapshot>(value).unwrap(),
            snapshot
        );
    }

    #[test]
    fn provider_step_snapshot_kind_roundtrips() {
        let snapshot = ProviderStepRequestSnapshot {
            provider_id: "openai".to_string(),
            model_id: "gpt-5.2".to_string(),
            input_item_ids: vec!["item-1".to_string()],
            snapshot_kind: ProviderStepSnapshotKind::RigCompletionRequest,
            request_body: ProviderRawPayload {
                provider_kind: "openai".to_string(),
                value: json!({ "messages": [] }),
            },
        };

        let value = serde_json::to_value(&snapshot).unwrap();
        assert_eq!(value["snapshotKind"], "rig_completion_request");
        assert_eq!(
            serde_json::from_value::<ProviderStepRequestSnapshot>(value).unwrap(),
            snapshot
        );
    }

    #[test]
    fn legacy_reasoning_capability_defaults_to_heuristic_source() {
        let payload = json!({
            "defaultEffort": "medium",
            "efforts": ["low", "medium", "high"],
            "summaries": true
        });

        let snapshot: ReasoningCapabilitySnapshot = serde_json::from_value(payload).unwrap();

        assert_eq!(snapshot.control, None);
        assert!(matches!(
            snapshot.source,
            CapabilitySourceSnapshot::Heuristic { .. }
        ));
    }

    #[test]
    fn run_settings_defaults_missing_reasoning_selection() {
        let payload = json!({
            "prompt": null,
            "providerId": "provider",
            "modelId": "model",
            "modelCapabilities": {
                "textInput": true,
                "textOutput": true,
                "streaming": true,
                "imageInput": null,
                "fileInput": null,
                "audioInput": false,
                "imageGeneration": false,
                "toolCalling": null,
                "hostedWebSearch": false,
                "remoteMcp": false,
                "reasoning": null,
                "structuredOutput": false,
                "statefulResponseContinuation": false,
                "extension": { "provider": "none" }
            },
            "providerSettings": {
                "providerKind": "openai",
                "fields": []
            },
            "toolPolicy": {
                "approvalPolicy": "never",
                "enabledSources": [],
                "maxSteps": 8
            }
        });

        let snapshot: RunSettingsSnapshot = serde_json::from_value(payload).unwrap();

        assert_eq!(snapshot.reasoning_selection, None);
    }
}

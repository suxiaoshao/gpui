use ai_chat_core::*;
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq)]
pub struct SchemaMetadataRecord {
    pub schema_version: i32,
    pub created_app_version: Option<String>,
    pub last_opened_app_version: Option<String>,
    pub payload: SchemaMetadataPayload,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectRecord {
    pub id: ProjectId,
    pub path: String,
    pub display_name: String,
    pub kind: ProjectKind,
    pub pinned: bool,
    pub removed: bool,
    pub metadata: ProjectMetadata,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub last_opened_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewProject {
    pub path: String,
    pub display_name: String,
    pub kind: ProjectKind,
    pub pinned: bool,
    pub removed: bool,
    pub metadata: ProjectMetadata,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConversationRecord {
    pub id: ConversationId,
    pub project_id: ProjectId,
    pub title: String,
    pub status: ConversationStatus,
    pub pinned: bool,
    pub prompt_id: Option<PromptId>,
    pub default_provider_id: Option<ProviderId>,
    pub default_model_id: Option<ProviderModelId>,
    pub last_item_seq: i32,
    pub metadata: ConversationMetadata,
    pub settings_snapshot: ConversationSettingsSnapshot,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub archived_at: Option<OffsetDateTime>,
    pub deleted_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewConversation {
    pub project_id: ProjectId,
    pub title: String,
    pub pinned: bool,
    pub prompt_id: Option<PromptId>,
    pub default_provider_id: Option<ProviderId>,
    pub default_model_id: Option<ProviderModelId>,
    pub metadata: ConversationMetadata,
    pub settings_snapshot: ConversationSettingsSnapshot,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewConversationWithUserItem {
    pub conversation: NewConversation,
    pub user_item: NewConversationItem,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConversationWithUserItemRecord {
    pub conversation: ConversationRecord,
    pub user_item: ConversationItemRecord,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConversationTimelineRecords {
    pub conversation: ConversationRecord,
    pub project: ProjectRecord,
    pub items: Vec<ConversationItemRecord>,
    pub attachments: Vec<AttachmentRecord>,
    pub runs: Vec<AgentRunRecord>,
    pub tool_invocations: Vec<ToolInvocationRecord>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConversationItemRecord {
    pub id: ConversationItemId,
    pub conversation_id: ConversationId,
    pub seq: i32,
    pub kind: ConversationItemKind,
    pub status: ConversationItemStatus,
    pub agent_run_id: Option<AgentRunId>,
    pub provider_step_id: Option<ProviderStepId>,
    pub tool_invocation_id: Option<ToolInvocationId>,
    pub provider_item_id: Option<String>,
    pub payload: ConversationItemPayload,
    pub search_text: String,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewConversationItem {
    pub conversation_id: ConversationId,
    pub status: ConversationItemStatus,
    pub agent_run_id: Option<AgentRunId>,
    pub provider_step_id: Option<ProviderStepId>,
    pub tool_invocation_id: Option<ToolInvocationId>,
    pub provider_item_id: Option<String>,
    pub payload: ConversationItemPayload,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttachmentRecord {
    pub id: AttachmentId,
    pub conversation_id: ConversationId,
    pub kind: AttachmentKind,
    pub storage_kind: AttachmentStorageKind,
    pub mime_type: Option<String>,
    pub name: Option<String>,
    pub path: Option<String>,
    pub external_uri: Option<String>,
    pub provider_id: Option<ProviderId>,
    pub provider_file_id: Option<String>,
    pub sha256: Option<String>,
    pub size_bytes: Option<i64>,
    pub metadata: AttachmentMetadata,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewAttachment {
    pub conversation_id: ConversationId,
    pub kind: AttachmentKind,
    pub storage_kind: AttachmentStorageKind,
    pub mime_type: Option<String>,
    pub name: Option<String>,
    pub path: Option<String>,
    pub external_uri: Option<String>,
    pub provider_id: Option<ProviderId>,
    pub provider_file_id: Option<String>,
    pub sha256: Option<String>,
    pub size_bytes: Option<i64>,
    pub metadata: AttachmentMetadata,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AgentRunRecord {
    pub id: AgentRunId,
    pub conversation_id: ConversationId,
    pub trigger_kind: AgentRunTriggerKind,
    pub status: AgentRunStatus,
    pub input: AgentRunInput,
    pub output: Option<AgentRunOutput>,
    pub error: Option<RunErrorPayload>,
    pub created_at: OffsetDateTime,
    pub started_at: Option<OffsetDateTime>,
    pub completed_at: Option<OffsetDateTime>,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewAgentRun {
    pub trigger_kind: AgentRunTriggerKind,
    pub status: AgentRunStatus,
    pub input: AgentRunInput,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpdateAgentRunStatus {
    pub status: AgentRunStatus,
    pub output: Option<AgentRunOutput>,
    pub error: Option<RunErrorPayload>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderStepRecord {
    pub id: ProviderStepId,
    pub agent_run_id: AgentRunId,
    pub seq: i32,
    pub provider_id: ProviderId,
    pub model_id: ProviderModelId,
    pub status: ProviderStepStatus,
    pub request_snapshot: ProviderStepRequestSnapshot,
    pub response_snapshot: Option<ProviderStepResponseSnapshot>,
    pub state_snapshot: Option<ProviderRunStateSnapshot>,
    pub settings_snapshot: RunSettingsSnapshot,
    pub error: Option<RunErrorPayload>,
    pub created_at: OffsetDateTime,
    pub started_at: Option<OffsetDateTime>,
    pub completed_at: Option<OffsetDateTime>,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewProviderStep {
    pub agent_run_id: AgentRunId,
    pub seq: i32,
    pub status: ProviderStepStatus,
    pub request_snapshot: ProviderStepRequestSnapshot,
    pub response_snapshot: Option<ProviderStepResponseSnapshot>,
    pub state_snapshot: Option<ProviderRunStateSnapshot>,
    pub settings_snapshot: RunSettingsSnapshot,
    pub error: Option<RunErrorPayload>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpdateProviderStepStatus {
    pub status: ProviderStepStatus,
    pub response_snapshot: Option<ProviderStepResponseSnapshot>,
    pub state_snapshot: Option<ProviderRunStateSnapshot>,
    pub error: Option<RunErrorPayload>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolInvocationRecord {
    pub id: ToolInvocationId,
    pub agent_run_id: AgentRunId,
    pub provider_step_id: Option<ProviderStepId>,
    pub call_id: String,
    pub source: ToolSource,
    pub namespace: Option<String>,
    pub server_id: Option<String>,
    pub tool_name: String,
    pub runtime_tool_name: String,
    pub status: ToolInvocationStatus,
    pub input: ToolInvocationInput,
    pub output: Option<ToolInvocationOutput>,
    pub error: Option<RunErrorPayload>,
    pub approval: Option<ToolInvocationApproval>,
    pub created_at: OffsetDateTime,
    pub started_at: Option<OffsetDateTime>,
    pub completed_at: Option<OffsetDateTime>,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewToolInvocation {
    pub agent_run_id: AgentRunId,
    pub provider_step_id: Option<ProviderStepId>,
    pub status: ToolInvocationStatus,
    pub input: ToolInvocationInput,
    pub output: Option<ToolInvocationOutput>,
    pub error: Option<RunErrorPayload>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpdateToolInvocationStatus {
    pub status: ToolInvocationStatus,
    pub output: Option<ToolInvocationOutput>,
    pub error: Option<RunErrorPayload>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolInvocationApproval {
    pub status: ApprovalStatus,
    pub request: ApprovalRequestPayload,
    pub decision: Option<ApprovalDecisionPayload>,
    pub requested_at: OffsetDateTime,
    pub decided_at: Option<OffsetDateTime>,
    pub expires_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewToolInvocationApproval {
    pub request: ApprovalRequestPayload,
    pub expires_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ToolInvocationApprovalOutcome {
    Approved {
        decided_by: String,
        reason: Option<String>,
    },
    Denied {
        decided_by: String,
        reason: Option<String>,
    },
    Expired,
    Canceled,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UsageEventRecord {
    pub id: UsageEventId,
    pub provider_step_id: ProviderStepId,
    pub conversation_id: ConversationId,
    pub provider_id: ProviderId,
    pub model_id: ProviderModelId,
    pub date_key: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cached_input_tokens: i64,
    pub cache_write_input_tokens: i64,
    pub reasoning_tokens: i64,
    pub total_tokens: i64,
    pub usage: ProviderUsageSnapshot,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewUsageEvent {
    pub provider_step_id: ProviderStepId,
    pub date_key: String,
    pub usage: ProviderUsageSnapshot,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PromptRecord {
    pub id: PromptId,
    pub name: String,
    pub content: PromptContent,
    pub enabled: bool,
    pub sort_order: i32,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewPrompt {
    pub name: String,
    pub content: PromptContent,
    pub enabled: bool,
    pub sort_order: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpdatePrompt {
    pub name: String,
    pub content: PromptContent,
    pub enabled: bool,
    pub sort_order: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShortcutRecord {
    pub id: ShortcutId,
    pub hotkey: String,
    pub enabled: bool,
    pub prompt_id: Option<PromptId>,
    pub provider_id: Option<ProviderId>,
    pub model_id: Option<ProviderModelId>,
    pub input_source: ShortcutInputSource,
    pub action: ShortcutAction,
    pub settings_snapshot: RunSettingsSnapshot,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewShortcut {
    pub hotkey: String,
    pub enabled: bool,
    pub prompt_id: Option<PromptId>,
    pub provider_id: Option<ProviderId>,
    pub model_id: Option<ProviderModelId>,
    pub input_source: ShortcutInputSource,
    pub action: ShortcutAction,
    pub settings_snapshot: RunSettingsSnapshot,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpdateShortcut {
    pub hotkey: String,
    pub enabled: bool,
    pub prompt_id: Option<PromptId>,
    pub provider_id: Option<ProviderId>,
    pub model_id: Option<ProviderModelId>,
    pub input_source: ShortcutInputSource,
    pub action: ShortcutAction,
    pub settings_snapshot: RunSettingsSnapshot,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderRecord {
    pub id: ProviderId,
    pub kind: String,
    pub display_name: String,
    pub enabled: bool,
    pub settings: ProviderSettingsPayload,
    pub secret_refs: ProviderSecretRefs,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewProvider {
    pub kind: String,
    pub display_name: String,
    pub enabled: bool,
    pub settings: ProviderSettingsPayload,
    pub secret_refs: ProviderSecretRefs,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpdateProvider {
    pub display_name: String,
    pub enabled: bool,
    pub settings: ProviderSettingsPayload,
    pub secret_refs: ProviderSecretRefs,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderModelRecord {
    pub id: ProviderModelId,
    pub provider_id: ProviderId,
    pub model_id: String,
    pub display_name: Option<String>,
    pub enabled: bool,
    pub capabilities: ModelCapabilitiesSnapshot,
    pub metadata: ProviderModelMetadata,
    pub fetched_at: OffsetDateTime,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewProviderModel {
    pub provider_id: ProviderId,
    pub model_id: String,
    pub display_name: Option<String>,
    pub enabled: bool,
    pub capabilities: ModelCapabilitiesSnapshot,
    pub metadata: ProviderModelMetadata,
}

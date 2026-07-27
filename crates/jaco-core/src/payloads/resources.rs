use super::*;

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
    pub text: String,
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
pub struct ConversationStatusEntry {
    pub code: ConversationStatusCode,
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
pub struct ApprovalRequestEntry {
    pub tool_invocation_id: ToolInvocationId,
    pub request: ApprovalRequestPayload,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovalDecisionEntry {
    pub tool_invocation_id: ToolInvocationId,
    pub decision: ApprovalDecisionPayload,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovalRequestPayload {
    pub reason: String,
    pub tool_source: ToolSource,
    pub tool_name: String,
    pub arguments_preview: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub access_requests: Vec<ToolAccessRequestPayload>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovalDecisionPayload {
    pub approved: bool,
    pub decided_by: String,
    pub reason: Option<String>,
}

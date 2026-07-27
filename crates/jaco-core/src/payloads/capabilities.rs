use super::*;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolApprovalMode {
    AutoApprove,
    RequestApproval,
    FullAccess,
}

pub fn default_tool_approval_mode() -> ToolApprovalMode {
    ToolApprovalMode::RequestApproval
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolPermissionScopeSnapshot {
    pub project_roots: Vec<String>,
    pub external_read_requires_approval: bool,
    pub external_write_requires_approval: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolAccessKind {
    Read,
    Write,
    Execute,
    Network,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolAccessRequestPayload {
    pub kind: ToolAccessKind,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized_path: Option<String>,
    #[serde(default)]
    pub within_project: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolPolicySnapshot {
    pub approval_policy: ToolApprovalPolicy,
    pub enabled_sources: Vec<ToolSource>,
    pub max_steps: u32,
    #[serde(default = "default_tool_approval_mode")]
    pub approval_mode: ToolApprovalMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_scope: Option<ToolPermissionScopeSnapshot>,
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

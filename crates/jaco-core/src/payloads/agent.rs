use super::*;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentRunInput {
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
    pub tool_name_strategy: ToolNameStrategy,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpToolApprovalModeSnapshot {
    Auto,
    Prompt,
    Deny,
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
    pub final_entry_id: ConversationEntryId,
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
        tool_invocation_id: ToolInvocationId,
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
    pub input_item_ids: Vec<ConversationEntryId>,
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
        item: ConversationEntryPayload,
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
        item: ConversationEntryPayload,
    },
    ToolCallRequested {
        call: ToolCallEntry,
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

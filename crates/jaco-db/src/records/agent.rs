use super::*;

pub type AgentRunRecord = AgentRun;

#[derive(Debug, Clone, PartialEq)]
pub struct NewAgentRun {
    pub conversation_id: ConversationId,
    pub trigger_entry_id: ConversationEntryId,
    pub trigger_kind: AgentRunTriggerKind,
    pub input: AgentRunInput,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AgentRunFinalEntry {
    Existing(ConversationEntryId),
    Append(Box<NewConversationEntry>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FinishAgentRun {
    pub status: AgentRunStatus,
    pub stopped_reason: AgentStoppedReason,
    pub error: Option<RunErrorPayload>,
    pub final_entry: AgentRunFinalEntry,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FinishedAgentRun {
    pub run: AgentRunRecord,
    pub final_entry: ConversationEntryRecord,
    pub appended_final_entry: bool,
    pub request_usage: Option<AgentMessageRequestUsage>,
    pub context_request_usage: Option<ConversationContextRequestUsage>,
}

pub type ProviderStepRecord = ProviderStep;

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
pub struct CompleteProviderStep {
    pub response_snapshot: ProviderStepResponseSnapshot,
    pub state_snapshot: ProviderRunStateSnapshot,
    pub continuation: Option<ProviderContinuationSnapshot>,
    pub usage: ProviderUsageSnapshot,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompletedProviderStep {
    pub step: ProviderStepRecord,
    pub usage: UsageEventRecord,
}

pub type ToolInvocationRecord = ToolInvocation;

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

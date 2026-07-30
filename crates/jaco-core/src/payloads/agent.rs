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
    pub transport: ProviderTransportSnapshot,
    pub context_mode: ProviderRequestContextSnapshot,
    pub previous_response_id: Option<String>,
    pub request_body: ProviderRawPayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderTransportSnapshot {
    ProviderDefault,
    Http,
    ServerSentEvents,
    WebSocket,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderRequestContextSnapshot {
    FullHistory,
    PreviousResponse,
    FullHistoryFallback,
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
    pub provider_outputs: Vec<ProviderRawPayload>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderRunStateSnapshot {
    pub provider_id: ProviderId,
    pub provider_run_id: Option<String>,
    pub output_item_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderContinuationKind {
    OpenAiResponses,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderContinuationSnapshot {
    pub kind: ProviderContinuationKind,
    pub response_id: String,
    pub reasoning_context: String,
    pub expires_at: time::OffsetDateTime,
    pub invalidated_at: Option<time::OffsetDateTime>,
    pub invalidation_error: Option<RunErrorPayload>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderContinuationError {
    EmptyResponseId,
    EmptyReasoningContext,
    ExpirationOverflow,
    AlreadyInvalidated,
}

impl ProviderContinuationSnapshot {
    pub fn openai_responses(
        response_id: String,
        reasoning_context: String,
        completed_at: time::OffsetDateTime,
    ) -> Result<Self, ProviderContinuationError> {
        let response_id = response_id.trim().to_string();
        if response_id.is_empty() {
            return Err(ProviderContinuationError::EmptyResponseId);
        }
        let reasoning_context = reasoning_context.trim().to_string();
        if reasoning_context.is_empty() {
            return Err(ProviderContinuationError::EmptyReasoningContext);
        }
        let expires_at = completed_at
            .checked_add(time::Duration::days(30))
            .ok_or(ProviderContinuationError::ExpirationOverflow)?;
        Ok(Self {
            kind: ProviderContinuationKind::OpenAiResponses,
            response_id,
            reasoning_context,
            expires_at,
            invalidated_at: None,
            invalidation_error: None,
        })
    }

    pub fn is_available(&self, now: time::OffsetDateTime) -> bool {
        !self.response_id.trim().is_empty()
            && self.invalidated_at.is_none()
            && now < self.expires_at
    }

    pub fn invalidate(
        &mut self,
        at: time::OffsetDateTime,
        error: RunErrorPayload,
    ) -> Result<(), ProviderContinuationError> {
        if self.invalidated_at.is_some() {
            return Err(ProviderContinuationError::AlreadyInvalidated);
        }
        self.invalidated_at = Some(at);
        self.invalidation_error = Some(error);
        Ok(())
    }
}

impl std::fmt::Display for ProviderContinuationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::EmptyResponseId => "provider continuation response ID cannot be empty",
            Self::EmptyReasoningContext => {
                "provider continuation reasoning context cannot be empty"
            }
            Self::ExpirationOverflow => "provider continuation expiration overflows",
            Self::AlreadyInvalidated => "provider continuation is already invalidated",
        })
    }
}

impl std::error::Error for ProviderContinuationError {}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_continuation_availability_honors_ttl_and_invalidation() {
        let completed_at = time::OffsetDateTime::UNIX_EPOCH;
        let mut continuation = ProviderContinuationSnapshot::openai_responses(
            "resp_1".to_string(),
            "all_turns".to_string(),
            completed_at,
        )
        .unwrap();
        assert!(continuation.is_available(continuation.expires_at - time::Duration::NANOSECOND));
        assert!(!continuation.is_available(continuation.expires_at));
        assert!(!continuation.is_available(continuation.expires_at + time::Duration::NANOSECOND));

        let error = RunErrorPayload {
            code: "previous_response_id_rejected".to_string(),
            message: "expired".to_string(),
            retryable: true,
            provider: Some("openai".to_string()),
            raw: None,
        };
        continuation
            .invalidate(completed_at + time::Duration::SECOND, error.clone())
            .unwrap();
        assert!(!continuation.is_available(completed_at));
        assert_eq!(continuation.invalidation_error, Some(error.clone()));
        assert_eq!(
            continuation.invalidate(completed_at + time::Duration::seconds(2), error),
            Err(ProviderContinuationError::AlreadyInvalidated)
        );
    }
}

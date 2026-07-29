use crate::{
    AgentRuntimeError, AgentRuntimeEvent, AgentRuntimeObserver, AgentStep,
    RegisteredToolDefinition, Result, ToolApprovalBroker,
};
use jaco_core::*;
use jaco_db::{
    AgentRunFinalEntry, AgentRunRecord, FinishAgentRun, FinishedAgentRun, NewAgentRun,
    NewConversationEntry,
};
mod conversation_entries;
mod model;
mod port;
mod provider_step;
mod tool_hook;

pub use model::PersistingCompletionModel;
pub use port::AgentPersistence;

#[cfg(test)]
pub(crate) fn direct_agent_persistence(
    repository: jaco_db::FreshRepository,
) -> Arc<dyn AgentPersistence> {
    Arc::new(port::DirectAgentPersistence::new(repository))
}

use self::tool_hook::PersistingAgentHook;
use rig::completion::Usage;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum AgentRunOutcome {
    Completed {
        final_entry_id: Option<ConversationEntryId>,
    },
    MaxSteps {
        final_entry_id: Option<ConversationEntryId>,
    },
    Failed {
        error: RunErrorPayload,
    },
    Canceled {
        final_entry_id: Option<ConversationEntryId>,
    },
}

pub(crate) fn finish_agent_run_spec(
    run: &AgentRunRecord,
    outcome: AgentRunOutcome,
) -> FinishAgentRun {
    let (status, stopped_reason, error, final_entry_id) = match outcome {
        AgentRunOutcome::Completed { final_entry_id } => (
            AgentRunStatus::Completed,
            AgentStoppedReason::Completed,
            None,
            final_entry_id,
        ),
        AgentRunOutcome::MaxSteps { final_entry_id } => (
            AgentRunStatus::Completed,
            AgentStoppedReason::MaxSteps,
            None,
            final_entry_id,
        ),
        AgentRunOutcome::Failed { error } => (
            AgentRunStatus::Failed,
            AgentStoppedReason::Failed,
            Some(error),
            None,
        ),
        AgentRunOutcome::Canceled { final_entry_id } => (
            AgentRunStatus::Canceled,
            AgentStoppedReason::Canceled,
            None,
            final_entry_id,
        ),
    };

    let final_entry = if let Some(final_entry_id) = final_entry_id {
        AgentRunFinalEntry::Existing(final_entry_id)
    } else if let Some(error_payload) = error.clone() {
        AgentRunFinalEntry::Append(Box::new(NewConversationEntry {
            conversation_id: run.conversation_id.clone(),
            status: ConversationEntryStatus::Failed,
            agent_run_id: Some(run.id.clone()),
            provider_step_id: None,
            tool_invocation_id: None,
            provider_item_id: None,
            payload: ConversationEntryPayload::Error(error_payload),
        }))
    } else {
        let code = match stopped_reason {
            AgentStoppedReason::Canceled => ConversationStatusCode::Canceled,
            AgentStoppedReason::MaxSteps => ConversationStatusCode::MaxStepsReached,
            AgentStoppedReason::Completed => ConversationStatusCode::CompletedWithoutOutput,
            AgentStoppedReason::Failed => unreachable!("failed runs always provide an error"),
        };
        let status = match status {
            AgentRunStatus::Completed => ConversationEntryStatus::Completed,
            AgentRunStatus::Canceled => ConversationEntryStatus::Canceled,
            AgentRunStatus::Failed => ConversationEntryStatus::Failed,
            AgentRunStatus::Queued | AgentRunStatus::Running => {
                unreachable!("finish_agent_run_spec requires a terminal status")
            }
        };
        AgentRunFinalEntry::Append(Box::new(NewConversationEntry {
            conversation_id: run.conversation_id.clone(),
            status,
            agent_run_id: Some(run.id.clone()),
            provider_step_id: None,
            tool_invocation_id: None,
            provider_item_id: None,
            payload: ConversationEntryPayload::Status(ConversationStatusEntry {
                code,
                message: None,
            }),
        }))
    };

    FinishAgentRun {
        status,
        stopped_reason,
        error,
        final_entry,
    }
}

#[derive(Clone)]
pub(crate) struct PersistenceContext {
    persistence: Arc<dyn AgentPersistence>,
    agent_run_id: AgentRunId,
    conversation_id: ConversationId,
    provider_id: ProviderId,
    model_id: ProviderModelId,
    settings_snapshot: RunSettingsSnapshot,
    input_item_ids: Arc<Mutex<Vec<ConversationEntryId>>>,
    last_provider_step_id: Arc<Mutex<Option<ProviderStepId>>>,
    final_entry_id: Arc<Mutex<Option<ConversationEntryId>>>,
    events: Arc<Mutex<Vec<AgentRunEvent>>>,
    steps: Arc<Mutex<Vec<AgentStep>>>,
    tool_definitions: Arc<HashMap<String, RegisteredToolDefinition>>,
    tool_calls: Arc<Mutex<HashMap<String, ToolInvocationId>>>,
    repeated_tool_calls: Arc<Mutex<HashMap<String, u32>>>,
    max_tool_calls: u32,
    repeated_tool_call_limit: u32,
    cancellation_token: CancellationToken,
    observer: Option<AgentRuntimeObserver>,
    approval_broker: Option<Arc<dyn ToolApprovalBroker>>,
}

impl PersistenceContext {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        persistence: Arc<dyn AgentPersistence>,
        agent_run_id: AgentRunId,
        conversation_id: ConversationId,
        provider_id: ProviderId,
        model_id: ProviderModelId,
        settings_snapshot: RunSettingsSnapshot,
        input_item_ids: Vec<ConversationEntryId>,
        tool_definitions: Vec<RegisteredToolDefinition>,
        max_tool_calls: u32,
        repeated_tool_call_limit: u32,
        cancellation_token: CancellationToken,
        observer: Option<AgentRuntimeObserver>,
        approval_broker: Option<Arc<dyn ToolApprovalBroker>>,
    ) -> Self {
        Self {
            persistence,
            agent_run_id,
            conversation_id,
            provider_id,
            model_id,
            settings_snapshot,
            input_item_ids: Arc::new(Mutex::new(input_item_ids)),
            last_provider_step_id: Arc::new(Mutex::new(None)),
            final_entry_id: Arc::new(Mutex::new(None)),
            events: Arc::new(Mutex::new(Vec::new())),
            steps: Arc::new(Mutex::new(Vec::new())),
            tool_definitions: Arc::new(
                tool_definitions
                    .into_iter()
                    .map(|definition| (definition.runtime_tool_name.clone(), definition))
                    .collect(),
            ),
            tool_calls: Arc::new(Mutex::new(HashMap::new())),
            repeated_tool_calls: Arc::new(Mutex::new(HashMap::new())),
            max_tool_calls,
            repeated_tool_call_limit,
            cancellation_token,
            observer,
            approval_broker,
        }
    }

    pub(crate) fn hook(&self) -> PersistingAgentHook {
        PersistingAgentHook {
            context: self.clone(),
        }
    }

    pub(crate) fn events(&self) -> Vec<AgentRunEvent> {
        mutex_clone(&self.events)
    }

    pub(crate) fn steps(&self) -> Vec<AgentStep> {
        mutex_clone(&self.steps)
    }

    pub(crate) fn final_entry_id(&self) -> Option<ConversationEntryId> {
        mutex_clone(&self.final_entry_id)
    }

    pub(crate) async fn finish_run(&self, outcome: AgentRunOutcome) -> Result<FinishedAgentRun> {
        let run = self
            .persistence
            .get_agent_run(self.agent_run_id.clone())
            .await?
            .ok_or_else(|| {
                AgentRuntimeError::Invariant(format!("agent run {} disappeared", self.agent_run_id))
            })?;
        let commit = self
            .persistence
            .finish_agent_run(
                self.agent_run_id.clone(),
                finish_agent_run_spec(&run, outcome.clone()),
            )
            .await?;
        let mut changes = vec![jaco_core::ConversationChange::RunStatusChanged {
            run: Box::new(commit.value.run.clone()),
        }];
        if commit.value.appended_final_entry {
            changes.push(jaco_core::ConversationChange::EntryAppended {
                entry: Box::new(commit.value.final_entry.clone()),
            });
        }
        self.emit_conversation_commit_with_changes(&commit, changes);
        let finished = commit.value;
        self.set_final_entry_id(Some(finished.final_entry.id.clone()));
        self.push_step(AgentStep::ConversationEntry(
            finished.final_entry.id.clone(),
        ));
        match outcome {
            AgentRunOutcome::Completed { .. } | AgentRunOutcome::MaxSteps { .. } => {
                let output = finished.run.output.clone().ok_or_else(|| {
                    AgentRuntimeError::Invariant("finished run has no output".to_string())
                })?;
                self.push_event(AgentRunEvent::Completed { output });
            }
            AgentRunOutcome::Failed { error } => {
                self.push_event(AgentRunEvent::Failed { error });
            }
            AgentRunOutcome::Canceled { .. } => self.push_event(AgentRunEvent::Canceled),
        }
        self.emit_runtime(AgentRuntimeEvent::AgentRunStatusChanged {
            agent_run_id: finished.run.id.clone(),
            status: finished.run.status,
        });
        Ok(finished)
    }
}

pub(crate) fn new_agent_run_input(request: &crate::AgentRunRequest) -> NewAgentRun {
    NewAgentRun {
        conversation_id: request.conversation_id.clone(),
        trigger_entry_id: request.trigger_entry_id.clone(),
        trigger_kind: request.trigger_kind,
        status: AgentRunStatus::Queued,
        input: AgentRunInput {
            prompt_snapshot: request.prompt_snapshot.clone(),
            provider_id: request.provider_id.clone(),
            model_id: request.model_id.clone(),
            settings_snapshot: request.settings_snapshot.clone(),
            runtime_snapshot: request.runtime_snapshot.clone(),
            max_steps: request.guards.max_steps,
        },
    }
}

pub(crate) fn run_error(
    code: impl Into<String>,
    message: impl Into<String>,
    retryable: bool,
    raw: Option<ProviderRawPayload>,
) -> RunErrorPayload {
    RunErrorPayload {
        code: code.into(),
        message: message.into(),
        retryable,
        provider: None,
        raw,
    }
}

fn error_tool_output(message: impl Into<String>) -> ToolInvocationOutput {
    ToolInvocationOutput {
        content: vec![ContentPart::Text {
            text: message.into(),
        }],
        structured_output: None,
        raw_output: None,
        is_error: true,
    }
}

pub(crate) fn provider_usage(usage: Usage) -> ProviderUsageSnapshot {
    ProviderUsageSnapshot {
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cached_input_tokens: usage.cached_input_tokens,
        cache_write_input_tokens: usage.cache_creation_input_tokens,
        reasoning_tokens: usage.reasoning_tokens,
        total_tokens: usage.total_tokens,
        metadata: None,
    }
}

fn completion_request_error(error: AgentRuntimeError) -> rig::completion::CompletionError {
    rig::completion::CompletionError::RequestError(Box::new(error))
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn mutex_clone<T: Clone>(mutex: &Mutex<T>) -> T {
    lock(mutex).clone()
}

fn mutex_replace<T>(mutex: &Mutex<T>, value: T) {
    *lock(mutex) = value;
}

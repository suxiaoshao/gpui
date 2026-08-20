use crate::{
    AgentRuntimeError, AgentRuntimeObserver,
    persistence::{AgentRunOutcome, run_error},
};
use gpui_operation::Transition;
use jaco_core::{AgentRunStatus, ConversationEntryId};
use jaco_db::{AgentRunRecord, FinishedAgentRun};

struct AgentRunLifecycle<State> {
    run: AgentRunRecord,
    observer: Option<AgentRuntimeObserver>,
    state: State,
}

pub struct PreparingAgentRun {
    lifecycle: AgentRunLifecycle<Preparing>,
}

struct Preparing;

pub(crate) struct ExecutingAgentRun {
    lifecycle: AgentRunLifecycle<Executing>,
}

struct Executing;

pub(crate) struct PersistedActiveAgentRun {
    lifecycle: AgentRunLifecycle<PersistedActive>,
}

struct PersistedActive;

pub(crate) struct FinalizingAgentRun {
    lifecycle: AgentRunLifecycle<Finalizing>,
}

struct Finalizing {
    outcome: AgentRunOutcome,
}

pub(crate) struct FinishedAgentRunState {
    lifecycle: AgentRunLifecycle<Finished>,
}

struct Finished {
    finished: FinishedAgentRun,
}

pub(crate) struct PersistenceUncertainAgentRun {
    lifecycle: AgentRunLifecycle<PersistenceUncertain>,
}

struct PersistenceUncertain {
    intended_outcome: AgentRunOutcome,
    error: AgentRuntimeError,
}

pub(crate) struct BeginExecution;

pub(crate) struct SetupFailed(pub(crate) String);

pub(crate) struct PreparationCanceled;

pub(crate) struct ExecutionFinished(pub(crate) AgentRunOutcome);

pub(crate) struct CancelPersistedActive {
    pub(crate) final_entry_id: Option<ConversationEntryId>,
}

pub(crate) struct InterruptPersistedActive {
    pub(crate) error: jaco_core::RunErrorPayload,
}

pub(crate) struct FinishCommitted(pub(crate) FinishedAgentRun);

pub(crate) struct FinishCommitFailed(pub(crate) AgentRuntimeError);

impl PreparingAgentRun {
    pub(crate) fn new(
        run: AgentRunRecord,
        observer: Option<AgentRuntimeObserver>,
    ) -> crate::Result<Self> {
        validate_active_run(&run)?;
        Ok(Self {
            lifecycle: AgentRunLifecycle {
                run,
                observer,
                state: Preparing,
            },
        })
    }

    pub(crate) fn record(&self) -> &AgentRunRecord {
        &self.lifecycle.run
    }

    pub(crate) fn observer(&self) -> Option<&AgentRuntimeObserver> {
        self.lifecycle.observer.as_ref()
    }
}

impl ExecutingAgentRun {
    pub(crate) fn record(&self) -> &AgentRunRecord {
        &self.lifecycle.run
    }
}

impl PersistedActiveAgentRun {
    pub(crate) fn new(
        run: AgentRunRecord,
        observer: Option<AgentRuntimeObserver>,
    ) -> crate::Result<Self> {
        validate_active_run(&run)?;
        Ok(Self {
            lifecycle: AgentRunLifecycle {
                run,
                observer,
                state: PersistedActive,
            },
        })
    }

    pub(crate) fn record(&self) -> &AgentRunRecord {
        &self.lifecycle.run
    }
}

impl FinalizingAgentRun {
    pub(crate) fn record(&self) -> &AgentRunRecord {
        &self.lifecycle.run
    }

    pub(crate) fn observer(&self) -> Option<&AgentRuntimeObserver> {
        self.lifecycle.observer.as_ref()
    }

    pub(crate) fn outcome(&self) -> &AgentRunOutcome {
        &self.lifecycle.state.outcome
    }
}

impl FinishedAgentRunState {
    pub(crate) fn into_finished(self) -> FinishedAgentRun {
        self.lifecycle.state.finished
    }
}

impl PersistenceUncertainAgentRun {
    pub(crate) fn into_error(self) -> AgentRuntimeError {
        let status = outcome_status(&self.lifecycle.state.intended_outcome);
        let intended_error = match &self.lifecycle.state.intended_outcome {
            AgentRunOutcome::Failed { error } => {
                Some((error.code.as_str(), error.message.as_str()))
            }
            AgentRunOutcome::Completed { .. }
            | AgentRunOutcome::MaxSteps { .. }
            | AgentRunOutcome::Canceled { .. } => None,
        };
        let error = self.lifecycle.state.error;
        tracing::error!(
            agent_run_id = %self.lifecycle.run.id,
            conversation_id = %self.lifecycle.run.conversation_id,
            intended_status = ?status,
            intended_error_code = intended_error.map(|(code, _)| code),
            intended_error = intended_error.map(|(_, message)| message),
            error = %error,
            "agent run finalization failed; persisted lifecycle state is uncertain"
        );
        error
    }
}

impl Transition<BeginExecution> for PreparingAgentRun {
    type Output = ExecutingAgentRun;

    fn transition(self, _message: BeginExecution) -> Self::Output {
        ExecutingAgentRun {
            lifecycle: AgentRunLifecycle {
                run: self.lifecycle.run,
                observer: self.lifecycle.observer,
                state: Executing,
            },
        }
    }
}

impl Transition<SetupFailed> for PreparingAgentRun {
    type Output = FinalizingAgentRun;

    fn transition(self, message: SetupFailed) -> Self::Output {
        let error = run_error("setup_error", message.0, true, None);
        self.into_finalizing(AgentRunOutcome::Failed { error })
    }
}

impl Transition<PreparationCanceled> for PreparingAgentRun {
    type Output = FinalizingAgentRun;

    fn transition(self, _message: PreparationCanceled) -> Self::Output {
        self.into_finalizing(AgentRunOutcome::Canceled {
            final_entry_id: None,
        })
    }
}

impl PreparingAgentRun {
    fn into_finalizing(self, outcome: AgentRunOutcome) -> FinalizingAgentRun {
        FinalizingAgentRun {
            lifecycle: AgentRunLifecycle {
                run: self.lifecycle.run,
                observer: self.lifecycle.observer,
                state: Finalizing { outcome },
            },
        }
    }
}

impl Transition<ExecutionFinished> for ExecutingAgentRun {
    type Output = FinalizingAgentRun;

    fn transition(self, message: ExecutionFinished) -> Self::Output {
        FinalizingAgentRun {
            lifecycle: AgentRunLifecycle {
                run: self.lifecycle.run,
                observer: self.lifecycle.observer,
                state: Finalizing { outcome: message.0 },
            },
        }
    }
}

impl Transition<CancelPersistedActive> for PersistedActiveAgentRun {
    type Output = FinalizingAgentRun;

    fn transition(self, message: CancelPersistedActive) -> Self::Output {
        FinalizingAgentRun {
            lifecycle: AgentRunLifecycle {
                run: self.lifecycle.run,
                observer: self.lifecycle.observer,
                state: Finalizing {
                    outcome: AgentRunOutcome::Canceled {
                        final_entry_id: message.final_entry_id,
                    },
                },
            },
        }
    }
}

impl Transition<InterruptPersistedActive> for PersistedActiveAgentRun {
    type Output = FinalizingAgentRun;

    fn transition(self, message: InterruptPersistedActive) -> Self::Output {
        FinalizingAgentRun {
            lifecycle: AgentRunLifecycle {
                run: self.lifecycle.run,
                observer: self.lifecycle.observer,
                state: Finalizing {
                    outcome: AgentRunOutcome::Failed {
                        error: message.error,
                    },
                },
            },
        }
    }
}

impl Transition<FinishCommitted> for FinalizingAgentRun {
    type Output = FinishedAgentRunState;

    fn transition(self, message: FinishCommitted) -> Self::Output {
        FinishedAgentRunState {
            lifecycle: AgentRunLifecycle {
                run: self.lifecycle.run,
                observer: self.lifecycle.observer,
                state: Finished {
                    finished: message.0,
                },
            },
        }
    }
}

impl Transition<FinishCommitFailed> for FinalizingAgentRun {
    type Output = PersistenceUncertainAgentRun;

    fn transition(self, message: FinishCommitFailed) -> Self::Output {
        PersistenceUncertainAgentRun {
            lifecycle: AgentRunLifecycle {
                run: self.lifecycle.run,
                observer: self.lifecycle.observer,
                state: PersistenceUncertain {
                    intended_outcome: self.lifecycle.state.outcome,
                    error: message.0,
                },
            },
        }
    }
}

fn validate_active_run(run: &AgentRunRecord) -> crate::Result<()> {
    if run.status != AgentRunStatus::Running {
        return Err(AgentRuntimeError::Invariant(format!(
            "agent run {} must be running before entering the runtime lifecycle, got {:?}",
            run.id, run.status
        )));
    }
    Ok(())
}

fn outcome_status(outcome: &AgentRunOutcome) -> AgentRunStatus {
    match outcome {
        AgentRunOutcome::Completed { .. } | AgentRunOutcome::MaxSteps { .. } => {
            AgentRunStatus::Completed
        }
        AgentRunOutcome::Failed { .. } => AgentRunStatus::Failed,
        AgentRunOutcome::Canceled { .. } => AgentRunStatus::Canceled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jaco_core::{
        AgentEngineKind, AgentRun, AgentRunInput, AgentRunTriggerKind, AgentRuntimeSnapshot,
        ModelCapabilitiesSnapshot, ProviderCapabilityExtensionSnapshot, ProviderSettingsPayload,
        RunSettingsSnapshot, ToolApprovalMode, ToolApprovalPolicy, ToolNameStrategy,
        ToolPolicySnapshot, ToolSource,
    };
    use time::OffsetDateTime;

    fn run() -> AgentRunRecord {
        let now = OffsetDateTime::now_utc();
        AgentRun {
            id: "run".to_string(),
            conversation_id: "conversation".to_string(),
            trigger_entry_id: "entry".to_string(),
            trigger_kind: AgentRunTriggerKind::User,
            status: AgentRunStatus::Running,
            input: AgentRunInput {
                prompt_snapshot: None,
                provider_id: "provider".to_string(),
                model_id: "model".to_string(),
                settings_snapshot: RunSettingsSnapshot {
                    prompt: None,
                    provider_id: "provider".to_string(),
                    model_id: "model".to_string(),
                    model_capabilities: ModelCapabilitiesSnapshot {
                        text_input: true,
                        text_output: true,
                        streaming: true,
                        context_window: None,
                        image_input: None,
                        file_input: None,
                        audio_input: false,
                        image_generation: false,
                        tool_calling: None,
                        hosted_web_search: false,
                        remote_mcp: false,
                        reasoning: None,
                        structured_output: false,
                        stateful_response_continuation: false,
                        extension: ProviderCapabilityExtensionSnapshot::None,
                    },
                    provider_settings: ProviderSettingsPayload {
                        provider_kind: "test".to_string(),
                        fields: Vec::new(),
                    },
                    reasoning_selection: None,
                    tool_policy: ToolPolicySnapshot {
                        approval_policy: ToolApprovalPolicy::Never,
                        enabled_sources: vec![ToolSource::Local],
                        max_steps: 8,
                        approval_mode: ToolApprovalMode::RequestApproval,
                        permission_scope: None,
                    },
                },
                runtime_snapshot: AgentRuntimeSnapshot {
                    engine: AgentEngineKind::Rig,
                    engine_version: "test".to_string(),
                    skill_catalog_hash: None,
                    tool_name_strategy: ToolNameStrategy::Direct,
                },
                max_steps: 8,
            },
            output: None,
            error: None,
            created_at: now,
            started_at: Some(now),
            completed_at: None,
            updated_at: now,
        }
    }

    #[test]
    fn preparing_setup_failure_requires_finalization() {
        let preparing = PreparingAgentRun::new(run(), None).unwrap();
        let finalizing = preparing.transition(SetupFailed("setup failed".to_string()));

        assert!(matches!(
            finalizing.outcome(),
            AgentRunOutcome::Failed { error }
                if error.code == "setup_error" && error.message == "setup failed"
        ));
    }

    #[test]
    fn execution_completion_preserves_explicit_outcome() {
        let executing = PreparingAgentRun::new(run(), None)
            .unwrap()
            .transition(BeginExecution);
        let finalizing = executing.transition(ExecutionFinished(AgentRunOutcome::Completed {
            final_entry_id: Some("assistant".to_string()),
        }));

        assert_eq!(
            finalizing.outcome(),
            &AgentRunOutcome::Completed {
                final_entry_id: Some("assistant".to_string())
            }
        );
    }
}

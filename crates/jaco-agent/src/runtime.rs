mod finalization;
mod history;
pub(crate) mod lifecycle;
mod reasoning;
mod streaming;
#[cfg(test)]
mod tests;
pub(crate) mod types;

pub use lifecycle::PreparingAgentRun;

use self::{
    history::{PromptHistoryOptions, build_prompt_history_with_options},
    lifecycle::{
        BeginExecution, CancelPersistedActive, ExecutionFinished, FinishCommitFailed,
        FinishCommitted, InterruptPersistedActive, PersistedActiveAgentRun, PreparationCanceled,
        SetupFailed,
    },
    reasoning::{merge_additional_params, reasoning_additional_params},
    streaming::StreamingOutputAccumulator,
};
use crate::{
    AgentRunHandle, AgentRunHandleStatus, AgentRunRequest, AgentRuntimeError, AgentRuntimeEvent,
    AgentRuntimeObserver, AgentStep, McpSessionManager, ProviderSecretValues, Result, SkillCatalog,
    SkillLoader, ToolApprovalBroker,
    persistence::{
        AgentPersistence, AgentRunOutcome, PersistenceContext, PersistingCompletionModel,
        finish_agent_run_spec, new_agent_run_input, run_error,
    },
    providers::run_saved_provider_model,
};
use futures::StreamExt;
use gpui_operation::Transition;
use jaco_core::*;
use jaco_db::{AgentRunRecord, FinishedAgentRun, NewConversationEntry, ProviderRecord};
use rig::{
    agent::{AgentBuilder, MultiTurnStreamItem, StreamingError},
    completion::{CompletionModel, Prompt, PromptError, Usage},
    streaming::{StreamedAssistantContent, StreamingPrompt},
};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct AgentRuntime {
    persistence: Arc<dyn AgentPersistence>,
    skill_loader: SkillLoader,
    mcp_session_manager: Option<Arc<Mutex<McpSessionManager>>>,
    approval_broker: Option<Arc<dyn ToolApprovalBroker>>,
    openai_sessions: crate::providers::openai::OpenAiResponsesSessionPool,
}

impl AgentRuntime {
    pub fn new(persistence: Arc<dyn AgentPersistence>) -> Self {
        Self {
            persistence,
            skill_loader: SkillLoader::new(),
            mcp_session_manager: None,
            approval_broker: None,
            openai_sessions: crate::providers::openai::OpenAiResponsesSessionPool::new(),
        }
    }

    #[cfg(test)]
    fn from_repository(repository: jaco_db::FreshRepository) -> Self {
        Self::new(crate::persistence::direct_agent_persistence(repository))
    }

    pub fn with_skill_loader(mut self, skill_loader: SkillLoader) -> Self {
        self.skill_loader = skill_loader;
        self
    }

    pub fn with_mcp_session_manager(mut self, manager: Arc<Mutex<McpSessionManager>>) -> Self {
        self.mcp_session_manager = Some(manager);
        self
    }

    pub fn with_approval_broker(mut self, broker: Arc<dyn ToolApprovalBroker>) -> Self {
        self.approval_broker = Some(broker);
        self
    }

    pub fn with_openai_session_pool(
        mut self,
        pool: crate::providers::openai::OpenAiResponsesSessionPool,
    ) -> Self {
        self.openai_sessions = pool;
        self
    }

    pub(crate) fn openai_session_pool(
        &self,
    ) -> crate::providers::openai::OpenAiResponsesSessionPool {
        self.openai_sessions.clone()
    }

    pub(crate) fn persistence(&self) -> Arc<dyn AgentPersistence> {
        self.persistence.clone()
    }

    pub async fn run_with_model<M>(
        &self,
        request: AgentRunRequest,
        model: M,
    ) -> Result<AgentRunHandle>
    where
        M: CompletionModel + 'static,
        M::Response: serde::Serialize + serde::de::DeserializeOwned,
        M::StreamingResponse: Clone
            + Unpin
            + Send
            + Sync
            + serde::Serialize
            + serde::de::DeserializeOwned
            + rig::completion::GetTokenUsage,
    {
        self.run_with_model_observed(request, model, None).await
    }

    pub async fn run_with_model_observed<M>(
        &self,
        mut request: AgentRunRequest,
        model: M,
        observer: Option<AgentRuntimeObserver>,
    ) -> Result<AgentRunHandle>
    where
        M: CompletionModel + 'static,
        M::Response: serde::Serialize + serde::de::DeserializeOwned,
        M::StreamingResponse: Clone
            + Unpin
            + Send
            + Sync
            + serde::Serialize
            + serde::de::DeserializeOwned
            + rig::completion::GetTokenUsage,
    {
        let agent_run = self.begin_run(&mut request, observer).await?;
        self.run_started_with_model_observed(agent_run, request, model)
            .await
    }

    pub async fn begin_run(
        &self,
        request: &mut AgentRunRequest,
        observer: Option<AgentRuntimeObserver>,
    ) -> Result<PreparingAgentRun> {
        if request.cancellation_token.is_cancelled() {
            return Err(AgentRuntimeError::Canceled);
        }
        crate::tools::builtin::registry::register_enabled_builtin_tools(
            &mut request.tool_registry,
            &request.settings_snapshot.tool_policy,
            request.project_root.as_deref(),
        )?;
        request.tool_registry.finalize_names();
        let agent_run = self
            .persistence
            .insert_agent_run(new_agent_run_input(request))
            .await?;
        emit_runtime(
            observer.as_ref(),
            AgentRuntimeEvent::AgentRunStarted {
                agent_run_id: agent_run.id.clone(),
                conversation_id: agent_run.conversation_id.clone(),
            },
        );
        emit_runtime(
            observer.as_ref(),
            AgentRuntimeEvent::AgentRunStatusChanged {
                agent_run_id: agent_run.id.clone(),
                status: AgentRunStatus::Running,
            },
        );
        emit_runtime(
            observer.as_ref(),
            AgentRuntimeEvent::ConversationTimelineChanged {
                conversation_id: agent_run.conversation_id.clone(),
                changes: vec![jaco_core::ConversationChange::RunStatusChanged {
                    run: Box::new(agent_run.clone()),
                }],
            },
        );
        PreparingAgentRun::new(agent_run, observer)
    }

    pub(crate) async fn run_started_with_model_observed<M>(
        &self,
        agent_run: PreparingAgentRun,
        request: AgentRunRequest,
        model: M,
    ) -> Result<AgentRunHandle>
    where
        M: CompletionModel + 'static,
        M::Response: serde::Serialize + serde::de::DeserializeOwned,
        M::StreamingResponse: Clone
            + Unpin
            + Send
            + Sync
            + serde::Serialize
            + serde::de::DeserializeOwned
            + rig::completion::GetTokenUsage,
    {
        self.run_started_with_model_observed_inner(agent_run, request, model, None)
            .await
    }

    pub(crate) async fn run_started_with_openai_websocket_observed<M>(
        &self,
        agent_run: PreparingAgentRun,
        request: AgentRunRequest,
        model: M,
        attempts: crate::providers::openai::OpenAiAttemptCoordinator,
    ) -> Result<AgentRunHandle>
    where
        M: CompletionModel + 'static,
        M::Response: serde::Serialize + serde::de::DeserializeOwned,
        M::StreamingResponse: Clone
            + Unpin
            + Send
            + Sync
            + serde::Serialize
            + serde::de::DeserializeOwned
            + rig::completion::GetTokenUsage,
    {
        self.run_started_with_model_observed_inner(agent_run, request, model, Some(attempts))
            .await
    }

    async fn run_started_with_model_observed_inner<M>(
        &self,
        agent_run: PreparingAgentRun,
        request: AgentRunRequest,
        model: M,
        openai_attempts: Option<crate::providers::openai::OpenAiAttemptCoordinator>,
    ) -> Result<AgentRunHandle>
    where
        M: CompletionModel + 'static,
        M::Response: serde::Serialize + serde::de::DeserializeOwned,
        M::StreamingResponse: Clone
            + Unpin
            + Send
            + Sync
            + serde::Serialize
            + serde::de::DeserializeOwned
            + rig::completion::GetTokenUsage,
    {
        if request.cancellation_token.is_cancelled() {
            return self
                .finish_preparation(agent_run.transition(PreparationCanceled))
                .await;
        }

        if let Err(error) = self.activate_skills(&request, &agent_run.record().id).await {
            return self.record_setup_failed_started_run(agent_run, error).await;
        }

        let timeline = match self
            .persistence
            .conversation_timeline(request.conversation_id.clone())
            .await
        {
            Ok(Some(timeline)) => timeline,
            Ok(None) => {
                return self
                    .record_setup_failed_started_run(
                        agent_run,
                        AgentRuntimeError::Invariant(format!(
                            "conversation {} is missing",
                            request.conversation_id
                        )),
                    )
                    .await;
            }
            Err(error) => {
                return self
                    .record_setup_failed_started_run(agent_run, AgentRuntimeError::from(error))
                    .await;
            }
        };
        let prompt_history = match build_prompt_history_with_options(
            &timeline.items,
            &timeline.attachments,
            &request.trigger_entry_id,
            &agent_run.record().id,
            PromptHistoryOptions {
                include_reasoning: true,
                preserve_tool_protocol: true,
            },
        ) {
            Ok(prompt_history) => prompt_history,
            Err(error) => {
                return self.record_setup_failed_started_run(agent_run, error).await;
            }
        };

        let tool_bundle = request
            .tool_registry
            .clone()
            .into_rig_tool_bundle(request.guards.tool_timeout);
        let registered_definitions = tool_bundle.definitions().to_vec();
        let context = PersistenceContext::new(
            self.persistence.clone(),
            agent_run.record().id.clone(),
            request.conversation_id.clone(),
            request.provider_id.clone(),
            request.model_id.clone(),
            request.settings_snapshot.clone(),
            prompt_history.input_item_ids,
            registered_definitions,
            request.guards.max_tool_calls,
            request.guards.repeated_tool_call_limit,
            request.cancellation_token.clone(),
            agent_run.observer().cloned(),
            self.approval_broker.clone(),
        );
        let model = match openai_attempts {
            Some(attempts) => PersistingCompletionModel::new_with_openai_attempts(
                model,
                context.clone(),
                attempts,
            ),
            None => PersistingCompletionModel::new(model, context.clone()),
        };
        let hook = context.hook();

        let mut builder = AgentBuilder::new(model)
            .name("jaco-agent")
            .add_hook(hook)
            .default_max_turns(request.guards.max_steps as usize);
        if let Some(prompt) = prompt_preamble(request.prompt_snapshot.as_ref()) {
            builder = builder.preamble(&prompt);
        }
        let reasoning_params = reasoning_additional_params(&request.settings_snapshot);
        let additional_params = merge_additional_params(
            reasoning_params,
            (!request.provider_tools.is_empty()).then(|| {
                serde_json::json!({
                    "tools": request.provider_tools,
                })
            }),
        );
        if let Some(additional_params) = additional_params {
            builder = builder.additional_params(additional_params);
        }
        let agent = tool_bundle.install(builder).build();
        let agent_run = agent_run.transition(BeginExecution);

        let outcome: Result<AgentRunOutcome> = async {
            let execution = if request.settings_snapshot.model_capabilities.streaming {
                let stream = tokio::select! {
                    biased;
                    _ = request.cancellation_token.cancelled() => None,
                    stream = agent
                        .stream_prompt(prompt_history.prompt)
                        .history(prompt_history.history)
                        .without_memory() => Some(stream),
                };
                match stream {
                    None => {
                        let _ = context
                            .cancel_current_provider_step(run_error(
                                "canceled",
                                "runtime canceled",
                                false,
                                None,
                            ))
                            .await;
                        Ok(AgentStoppedReason::Canceled)
                    }
                    Some(mut stream) => {
                        let mut accumulator = StreamingOutputAccumulator::new(context.clone());
                        let mut final_response = None;
                        let mut final_raw_response = None;

                        loop {
                            let next = tokio::select! {
                                biased;
                                _ = request.cancellation_token.cancelled() => {
                                    accumulator
                                        .finish(ConversationEntryStatus::Canceled, None)
                                        .await?;
                                    let _ = context
                                        .cancel_current_provider_step(run_error(
                                            "canceled",
                                            "runtime canceled",
                                            false,
                                            None,
                                        ))
                                        .await;
                                    break Ok(AgentStoppedReason::Canceled);
                                }
                                next = stream.next() => next,
                            };
                            match next {
                                Some(Ok(MultiTurnStreamItem::StreamAssistantItem(item))) => {
                                    match item {
                                        StreamedAssistantContent::Text(text) => {
                                            accumulator.append_text(&text.text).await?;
                                        }
                                        StreamedAssistantContent::Reasoning(reasoning) => {
                                            accumulator
                                                .replace_reasoning(reasoning.display_text())
                                                .await?;
                                        }
                                        StreamedAssistantContent::ReasoningDelta {
                                            reasoning,
                                            ..
                                        } => {
                                            accumulator.append_reasoning(&reasoning).await?;
                                        }
                                        StreamedAssistantContent::Final(response) => {
                                            final_raw_response = Some(response);
                                        }
                                        StreamedAssistantContent::Unknown(output) => {
                                            context.record_provider_output(output)?;
                                        }
                                        StreamedAssistantContent::ToolCall { .. }
                                        | StreamedAssistantContent::ToolCallDelta { .. } => {}
                                    }
                                }
                                Some(Ok(MultiTurnStreamItem::StreamUserItem(_))) => {}
                                Some(Ok(MultiTurnStreamItem::FinalResponse(response))) => {
                                    final_response = Some(response);
                                }
                                Some(Ok(_)) => {}
                                Some(Err(error)) => {
                                    if request.cancellation_token.is_cancelled() {
                                        accumulator
                                            .finish(ConversationEntryStatus::Canceled, None)
                                            .await?;
                                        let _ = context
                                            .cancel_current_provider_step(run_error(
                                                "canceled",
                                                "runtime canceled",
                                                false,
                                                None,
                                            ))
                                            .await;
                                    } else {
                                        accumulator
                                            .finish(ConversationEntryStatus::Failed, None)
                                            .await?;
                                        let _ = context
                                            .fail_current_provider_step(run_error(
                                                "prompt_error",
                                                error.to_string(),
                                                true,
                                                None,
                                            ))
                                            .await;
                                    }
                                    break Err(PromptExecutionError::streaming(error));
                                }
                                None => {
                                    let final_text = final_response
                                        .as_ref()
                                        .map(|response| response.output.clone())
                                        .filter(|text| !text.is_empty());
                                    if request.cancellation_token.is_cancelled() {
                                        accumulator
                                            .finish(
                                                ConversationEntryStatus::Canceled,
                                                final_text.as_deref(),
                                            )
                                            .await?;
                                        let _ = context
                                            .cancel_current_provider_step(run_error(
                                                "canceled",
                                                "runtime canceled",
                                                false,
                                                None,
                                            ))
                                            .await;
                                        break Ok(AgentStoppedReason::Canceled);
                                    }
                                    accumulator
                                        .finish(
                                            ConversationEntryStatus::Completed,
                                            final_text.as_deref(),
                                        )
                                        .await?;
                                    let usage = final_response
                                        .as_ref()
                                        .map(|response| response.usage())
                                        .unwrap_or_else(Usage::new);
                                    context
                                        .finish_current_streaming_provider_step(
                                            final_raw_response.as_ref(),
                                            usage,
                                        )
                                        .await?;
                                    break Ok(AgentStoppedReason::Completed);
                                }
                            }
                        }
                    }
                }
            } else {
                let response = tokio::select! {
                    biased;
                    _ = request.cancellation_token.cancelled() => {
                        let _ = context
                            .cancel_current_provider_step(run_error(
                                "canceled",
                                "runtime canceled",
                                false,
                                None,
                            ))
                            .await;
                        None
                    }
                    response = agent
                        .prompt(prompt_history.prompt)
                        .history(prompt_history.history)
                        .tool_concurrency(request.guards.tool_concurrency)
                        .without_memory()
                        .extended_details() => Some(response),
                };
                match response {
                    None => Ok(AgentStoppedReason::Canceled),
                    Some(Ok(_response)) => Ok(AgentStoppedReason::Completed),
                    Some(Err(error)) => Err(PromptExecutionError::prompt(error)),
                }
            };

            match execution {
                Ok(stopped_reason) => {
                    let canceled = stopped_reason == AgentStoppedReason::Canceled
                        || request.cancellation_token.is_cancelled();
                    if canceled {
                        self.finalize_active_tool_invocations(
                            &agent_run.record().id,
                            &request.conversation_id,
                            ToolInvocationStatus::Canceled,
                            run_error("canceled", "runtime canceled", false, None),
                            context.observer(),
                        )
                        .await?;
                    }
                    let final_entry_id = if canceled {
                        context.final_entry_id().or(self
                            .latest_assistant_entry_id_for_run(agent_run.record())
                            .await?)
                    } else {
                        context.final_entry_id()
                    };
                    if canceled {
                        Ok(AgentRunOutcome::Canceled { final_entry_id })
                    } else if stopped_reason == AgentStoppedReason::MaxSteps {
                        Ok(AgentRunOutcome::MaxSteps { final_entry_id })
                    } else {
                        Ok(AgentRunOutcome::Completed { final_entry_id })
                    }
                }
                Err(error) if error.max_steps => Ok(AgentRunOutcome::MaxSteps {
                    final_entry_id: context.final_entry_id(),
                }),
                Err(error) => {
                    let canceled = request.cancellation_token.is_cancelled();
                    let payload = if canceled {
                        run_error("canceled", "runtime canceled", false, None)
                    } else {
                        run_error("prompt_error", error.message, true, None)
                    };
                    self.finalize_active_tool_invocations(
                        &agent_run.record().id,
                        &request.conversation_id,
                        if canceled {
                            ToolInvocationStatus::Canceled
                        } else {
                            ToolInvocationStatus::Failed
                        },
                        payload.clone(),
                        context.observer(),
                    )
                    .await?;
                    if canceled {
                        let final_entry_id = context.final_entry_id().or(self
                            .latest_assistant_entry_id_for_run(agent_run.record())
                            .await?);
                        Ok(AgentRunOutcome::Canceled { final_entry_id })
                    } else {
                        Ok(AgentRunOutcome::Failed { error: payload })
                    }
                }
            }
        }
        .await;

        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(error) => {
                let payload = run_error("runtime_error", error.to_string(), true, None);
                let _ = context.fail_current_provider_step(payload.clone()).await;
                let _ = self
                    .fail_active_provider_steps(&agent_run.record().id, payload.clone())
                    .await;
                let _ = self
                    .finalize_active_tool_invocations(
                        &agent_run.record().id,
                        &request.conversation_id,
                        ToolInvocationStatus::Failed,
                        payload.clone(),
                        context.observer(),
                    )
                    .await;
                AgentRunOutcome::Failed { error: payload }
            }
        };
        self.finish_execution(agent_run.transition(ExecutionFinished(outcome)), &context)
            .await
    }

    pub async fn run_with_saved_provider_observed(
        &self,
        request: AgentRunRequest,
        provider: ProviderRecord,
        secrets: ProviderSecretValues,
        observer: Option<AgentRuntimeObserver>,
    ) -> Result<AgentRunHandle> {
        let mut request = request;
        let agent_run = self.begin_run(&mut request, observer).await?;
        self.run_started_with_saved_provider(agent_run, request, provider, secrets)
            .await
    }

    pub async fn run_started_with_saved_provider(
        &self,
        agent_run: PreparingAgentRun,
        request: AgentRunRequest,
        provider: ProviderRecord,
        secrets: ProviderSecretValues,
    ) -> Result<AgentRunHandle> {
        run_saved_provider_model(self, agent_run, request, provider, secrets).await
    }

    pub async fn record_setup_failed_run(
        &self,
        mut request: AgentRunRequest,
        error: impl ToString,
        observer: Option<&AgentRuntimeObserver>,
    ) -> Result<AgentRunHandle> {
        let agent_run = self.begin_run(&mut request, observer.cloned()).await?;
        self.record_setup_failed_started_run(agent_run, error).await
    }

    pub async fn record_setup_failed_started_run(
        &self,
        agent_run: PreparingAgentRun,
        error: impl ToString,
    ) -> Result<AgentRunHandle> {
        self.finish_preparation(agent_run.transition(SetupFailed(error.to_string())))
            .await
    }

    pub async fn cancel_non_terminal_runs_for_conversation(
        &self,
        conversation_id: &str,
        observer: Option<&AgentRuntimeObserver>,
    ) -> Result<Vec<AgentRunRecord>> {
        let mut canceled = Vec::new();
        for run in self
            .persistence
            .agent_runs_for_conversation(conversation_id.to_string())
            .await?
        {
            if is_terminal_agent_run_status(run.status) {
                continue;
            }
            if let Some(run) = self.cancel_run(&run.id, observer).await? {
                canceled.push(run);
            }
        }
        Ok(canceled)
    }

    pub async fn cancel_run(
        &self,
        agent_run_id: &str,
        observer: Option<&AgentRuntimeObserver>,
    ) -> Result<Option<AgentRunRecord>> {
        let Some(run) = self
            .persistence
            .get_agent_run(agent_run_id.to_string())
            .await?
        else {
            return Ok(None);
        };
        if is_terminal_agent_run_status(run.status) {
            return Ok(Some(run));
        }

        let active = PersistedActiveAgentRun::new(run, observer.cloned())?;
        let cleanup = async {
            let error = run_error("canceled", "runtime canceled", false, None);
            self.finalize_active_provider_steps(
                &active.record().id,
                ProviderStepStatus::Canceled,
                error.clone(),
            )
            .await?;
            self.finalize_active_tool_invocations(
                &active.record().id,
                &active.record().conversation_id,
                ToolInvocationStatus::Canceled,
                error,
                observer,
            )
            .await?;
            self.latest_assistant_entry_id_for_run(active.record())
                .await
        }
        .await;
        let finalizing = match cleanup {
            Ok(final_entry_id) => active.transition(CancelPersistedActive { final_entry_id }),
            Err(error) => active.transition(InterruptPersistedActive {
                error: run_error("runtime_error", error.to_string(), true, None),
            }),
        };
        let finished = self.finish_agent_run_with_observer(finalizing).await?;
        Ok(Some(finished.run))
    }

    pub async fn recover_interrupted_runs(&self) -> Result<Vec<AgentRunRecord>> {
        let mut recovered = Vec::new();
        for run in self
            .persistence
            .agent_runs_by_status(AgentRunStatus::Running)
            .await?
        {
            let interrupted = run_error(
                "interrupted",
                "agent run was interrupted before reaching a terminal state",
                true,
                None,
            );
            let active = PersistedActiveAgentRun::new(run, None)?;
            let cleanup = async {
                self.fail_active_provider_steps(&active.record().id, interrupted.clone())
                    .await?;
                self.finalize_active_tool_invocations(
                    &active.record().id,
                    &active.record().conversation_id,
                    ToolInvocationStatus::Failed,
                    interrupted.clone(),
                    None,
                )
                .await
            }
            .await;
            let error = match cleanup {
                Ok(()) => interrupted,
                Err(error) => run_error("recovery_error", error.to_string(), true, None),
            };
            let finished = self
                .finish_agent_run_with_observer(
                    active.transition(InterruptPersistedActive { error }),
                )
                .await?;
            recovered.push(finished.run);
        }
        Ok(recovered)
    }

    async fn activate_skills(&self, request: &AgentRunRequest, agent_run_id: &str) -> Result<()> {
        if request.skill_requests.is_empty() {
            return Ok(());
        }
        let catalog = SkillCatalog::scan(request.project_root.as_deref())?;
        for skill_request in &request.skill_requests {
            let entry = catalog.get(&skill_request.name).ok_or_else(|| {
                AgentRuntimeError::Invariant(format!("skill {} is missing", skill_request.name))
            })?;
            let activation = self.skill_loader.load(entry)?;
            self.persistence
                .append_conversation_entry(NewConversationEntry {
                    conversation_id: request.conversation_id.clone(),
                    status: ConversationEntryStatus::Completed,
                    agent_run_id: Some(agent_run_id.to_string()),
                    provider_step_id: None,
                    tool_invocation_id: None,
                    provider_item_id: None,
                    payload: ConversationEntryPayload::SkillActivation(activation),
                })
                .await?;
        }
        Ok(())
    }
}

impl AgentRuntime {
    pub(super) async fn finish_agent_run_with_observer(
        &self,
        finalizing: lifecycle::FinalizingAgentRun,
    ) -> Result<FinishedAgentRun> {
        let finish = finish_agent_run_spec(finalizing.record(), finalizing.outcome().clone());
        let observer = finalizing.observer().cloned();
        let commit = match self
            .persistence
            .finish_agent_run(finalizing.record().id.clone(), finish)
            .await
        {
            Ok(commit) => commit,
            Err(error) => {
                return Err(finalizing
                    .transition(FinishCommitFailed(AgentRuntimeError::from(error)))
                    .into_error());
            }
        };
        emit_runtime(
            observer.as_ref(),
            AgentRuntimeEvent::ConversationCommitted {
                conversation: Box::new(commit.conversation.clone()),
                changes: {
                    let mut changes = vec![jaco_core::ConversationChange::RunStatusChanged {
                        run: Box::new(commit.value.run.clone()),
                    }];
                    if commit.value.appended_final_entry {
                        changes.push(jaco_core::ConversationChange::EntryAppended {
                            entry: Box::new(commit.value.final_entry.clone()),
                        });
                    }
                    changes
                },
            },
        );
        let finished = finalizing
            .transition(FinishCommitted(commit.value))
            .into_finished();
        emit_runtime(
            observer.as_ref(),
            AgentRuntimeEvent::AgentRunStatusChanged {
                agent_run_id: finished.run.id.clone(),
                status: finished.run.status,
            },
        );
        Ok(finished)
    }

    async fn finish_preparation(
        &self,
        finalizing: lifecycle::FinalizingAgentRun,
    ) -> Result<AgentRunHandle> {
        let finished = self.finish_agent_run_with_observer(finalizing).await?;
        let output = finished.run.output.clone().ok_or_else(|| {
            AgentRuntimeError::Invariant("prepared run finalization has no output".to_string())
        })?;
        let event = match finished.run.status {
            AgentRunStatus::Completed => AgentRunEvent::Completed {
                output: output.clone(),
            },
            AgentRunStatus::Failed => AgentRunEvent::Failed {
                error: finished.run.error.clone().ok_or_else(|| {
                    AgentRuntimeError::Invariant("failed prepared run has no error".to_string())
                })?,
            },
            AgentRunStatus::Canceled => AgentRunEvent::Canceled,
            AgentRunStatus::Running => {
                return Err(AgentRuntimeError::Invariant(
                    "prepared run finalization remains active".to_string(),
                ));
            }
        };
        Ok(AgentRunHandle {
            agent_run: finished.run,
            output: Some(output),
            status: AgentRunHandleStatus::Finished,
            events: vec![event],
            steps: vec![AgentStep::ConversationEntry(finished.final_entry.id)],
        })
    }

    async fn finish_execution(
        &self,
        finalizing: lifecycle::FinalizingAgentRun,
        context: &PersistenceContext,
    ) -> Result<AgentRunHandle> {
        let conversation_id = finalizing.record().conversation_id.clone();
        let result = async {
            let finished = context.finish_run(finalizing).await?;
            let output = finished.run.output.clone().ok_or_else(|| {
                AgentRuntimeError::Invariant("executed run finalization has no output".to_string())
            })?;
            Ok(AgentRunHandle {
                agent_run: finished.run,
                output: Some(output),
                status: AgentRunHandleStatus::Finished,
                events: context.events(),
                steps: context.steps(),
            })
        }
        .await;
        let session_is_reusable = result
            .as_ref()
            .is_ok_and(|handle| handle.agent_run.status == AgentRunStatus::Completed);
        if !session_is_reusable {
            self.openai_sessions
                .close_conversation(&conversation_id)
                .await;
        }
        result
    }
}

fn emit_runtime(observer: Option<&AgentRuntimeObserver>, event: AgentRuntimeEvent) {
    if let Some(observer) = observer {
        observer.emit(event);
    }
}

#[derive(Debug)]
struct PromptExecutionError {
    message: String,
    max_steps: bool,
}

impl PromptExecutionError {
    fn prompt(error: PromptError) -> Self {
        let max_steps = matches!(&error, PromptError::MaxTurnsError { .. });
        Self {
            message: error.to_string(),
            max_steps,
        }
    }

    fn streaming(error: StreamingError) -> Self {
        let max_steps = matches!(
            &error,
            StreamingError::Prompt(prompt)
                if matches!(prompt.as_ref(), PromptError::MaxTurnsError { .. })
        );
        Self {
            message: error.to_string(),
            max_steps,
        }
    }
}

fn is_terminal_agent_run_status(status: AgentRunStatus) -> bool {
    matches!(
        status,
        AgentRunStatus::Completed | AgentRunStatus::Failed | AgentRunStatus::Canceled
    )
}

fn prompt_preamble(prompt: Option<&PromptContent>) -> Option<String> {
    let prompt = prompt?;
    let text = prompt.text.trim().to_string();
    (!text.is_empty()).then_some(text)
}

mod finalization;
mod history;
mod reasoning;
mod streaming;
#[cfg(test)]
mod tests;
pub(crate) mod types;

use self::{
    history::{PromptHistoryOptions, build_prompt_history_with_options},
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
use jaco_core::*;
use jaco_db::{
    AgentRunRecord, FinishAgentRun, FinishedAgentRun, NewConversationEntry, ProviderRecord,
    UpdateAgentRunStatus,
};
use rig_core::{
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
}

impl AgentRuntime {
    pub fn new(persistence: Arc<dyn AgentPersistence>) -> Self {
        Self {
            persistence,
            skill_loader: SkillLoader::new(),
            mcp_session_manager: None,
            approval_broker: None,
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
            + rig_core::completion::GetTokenUsage,
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
            + rig_core::completion::GetTokenUsage,
    {
        let agent_run = self.begin_run(&mut request, observer.as_ref()).await?;
        self.run_started_with_model_observed(agent_run, request, model, observer)
            .await
    }

    pub async fn begin_run(
        &self,
        request: &mut AgentRunRequest,
        observer: Option<&AgentRuntimeObserver>,
    ) -> Result<AgentRunRecord> {
        if request.cancellation_token.is_cancelled() {
            return Err(AgentRuntimeError::Canceled);
        }
        crate::tools::builtin::registry::register_enabled_builtin_tools(
            &mut request.tool_registry,
            &request.settings_snapshot.tool_policy,
            request.project_root.as_deref(),
        )?;
        request.tool_registry.finalize_names();
        let mut agent_run = self
            .persistence
            .insert_agent_run(new_agent_run_input(request))
            .await?;
        emit_runtime(
            observer,
            AgentRuntimeEvent::AgentRunStarted {
                agent_run_id: agent_run.id.clone(),
                conversation_id: agent_run.conversation_id.clone(),
            },
        );
        agent_run = self
            .persistence
            .update_agent_run_status(
                agent_run.id.clone(),
                UpdateAgentRunStatus {
                    status: AgentRunStatus::Running,
                    error: None,
                },
            )
            .await?;
        emit_runtime(
            observer,
            AgentRuntimeEvent::AgentRunStatusChanged {
                agent_run_id: agent_run.id.clone(),
                status: AgentRunStatus::Running,
            },
        );
        emit_runtime(
            observer,
            AgentRuntimeEvent::ConversationTimelineChanged {
                conversation_id: agent_run.conversation_id.clone(),
                changes: vec![jaco_core::ConversationChange::RunStatusChanged {
                    run: Box::new(agent_run.clone()),
                }],
            },
        );
        Ok(agent_run)
    }

    pub(crate) async fn run_started_with_model_observed<M>(
        &self,
        agent_run: AgentRunRecord,
        request: AgentRunRequest,
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
            + rig_core::completion::GetTokenUsage,
    {
        if request.cancellation_token.is_cancelled() {
            let agent_run = self
                .cancel_run(&agent_run.id, observer.as_ref())
                .await?
                .ok_or_else(|| AgentRuntimeError::Invariant("agent run disappeared".to_string()))?;
            return Ok(AgentRunHandle {
                agent_run,
                output: None,
                status: AgentRunHandleStatus::Finished,
                events: vec![AgentRunEvent::Canceled],
                steps: Vec::new(),
            });
        }

        if let Err(error) = self.activate_skills(&request, &agent_run.id).await {
            return Err(self
                .mark_setup_failed(&agent_run.id, error, observer.as_ref())
                .await?);
        }

        let timeline = match self
            .persistence
            .conversation_timeline(request.conversation_id.clone())
            .await
        {
            Ok(Some(timeline)) => timeline,
            Ok(None) => {
                return Err(self
                    .mark_setup_failed(
                        &agent_run.id,
                        AgentRuntimeError::Invariant(format!(
                            "conversation {} is missing",
                            request.conversation_id
                        )),
                        observer.as_ref(),
                    )
                    .await?);
            }
            Err(error) => {
                return Err(self
                    .mark_setup_failed(
                        &agent_run.id,
                        AgentRuntimeError::from(error),
                        observer.as_ref(),
                    )
                    .await?);
            }
        };
        let prompt_history = match build_prompt_history_with_options(
            &timeline.items,
            &timeline.attachments,
            &request.trigger_entry_id,
            &agent_run.id,
            PromptHistoryOptions {
                include_reasoning: true,
                preserve_tool_protocol: true,
            },
        ) {
            Ok(prompt_history) => prompt_history,
            Err(error) => {
                return Err(self
                    .mark_setup_failed(&agent_run.id, error, observer.as_ref())
                    .await?);
            }
        };

        let rig_tools = request
            .tool_registry
            .clone()
            .into_rig_tools(request.guards.tool_timeout);
        let registered_definitions = request.tool_registry.registered_definitions();
        let runtime_tools = request
            .tool_registry
            .runtime_tools(request.guards.tool_timeout);
        let context = PersistenceContext::new(
            self.persistence.clone(),
            agent_run.id.clone(),
            request.conversation_id.clone(),
            request.provider_id.clone(),
            request.model_id.clone(),
            request.settings_snapshot.clone(),
            prompt_history.input_item_ids,
            registered_definitions,
            runtime_tools,
            request.guards.max_tool_calls,
            request.guards.repeated_tool_call_limit,
            request.cancellation_token.clone(),
            observer.clone(),
            self.approval_broker.clone(),
        );
        let model = PersistingCompletionModel::new(model, context.clone());
        let hook = context.hook();

        let mut builder = AgentBuilder::new(model)
            .name("jaco-agent")
            .hook(hook)
            .tools(rig_tools)
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
        let agent = builder.build();

        let execution = if request.settings_snapshot.model_capabilities.streaming {
            let stream = tokio::select! {
                biased;
                _ = request.cancellation_token.cancelled() => None,
                stream = agent
                    .stream_prompt(prompt_history.prompt)
                    .with_history(prompt_history.history)
                    .without_memory() => Some(stream),
            };
            let Some(mut stream) = stream else {
                let _ = context
                    .cancel_current_provider_step(run_error(
                        "canceled",
                        "runtime canceled",
                        false,
                        None,
                    ))
                    .await;
                return Ok(AgentRunHandle {
                    agent_run: self
                        .cancel_run(&agent_run.id, observer.as_ref())
                        .await?
                        .ok_or_else(|| {
                            AgentRuntimeError::Invariant("agent run disappeared".to_string())
                        })?,
                    output: None,
                    status: AgentRunHandleStatus::Finished,
                    events: vec![AgentRunEvent::Canceled],
                    steps: context.steps(),
                });
            };
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
                    Some(Ok(MultiTurnStreamItem::StreamAssistantItem(item))) => match item {
                        StreamedAssistantContent::Text(text) => {
                            accumulator.append_text(&text.text).await?;
                        }
                        StreamedAssistantContent::Reasoning(reasoning) => {
                            accumulator
                                .replace_reasoning(reasoning.display_text())
                                .await?;
                        }
                        StreamedAssistantContent::ReasoningDelta { reasoning, .. } => {
                            accumulator.append_reasoning(&reasoning).await?;
                        }
                        StreamedAssistantContent::Final(response) => {
                            final_raw_response = Some(response);
                        }
                        StreamedAssistantContent::ToolCall { .. }
                        | StreamedAssistantContent::ToolCallDelta { .. } => {}
                    },
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
                            .map(|response| response.response())
                            .filter(|text| !text.is_empty());
                        if request.cancellation_token.is_cancelled() {
                            accumulator
                                .finish(ConversationEntryStatus::Canceled, final_text)
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
                        } else {
                            accumulator
                                .finish(ConversationEntryStatus::Completed, final_text)
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
                    .with_history(prompt_history.history)
                    .with_tool_concurrency(request.guards.tool_concurrency)
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
                let final_status = if stopped_reason == AgentStoppedReason::Canceled
                    || request.cancellation_token.is_cancelled()
                {
                    AgentRunStatus::Canceled
                } else {
                    AgentRunStatus::Completed
                };
                let final_stopped_reason = if final_status == AgentRunStatus::Canceled {
                    AgentStoppedReason::Canceled
                } else {
                    stopped_reason
                };
                if final_status == AgentRunStatus::Canceled {
                    self.finalize_active_tool_invocations(
                        &agent_run.id,
                        &request.conversation_id,
                        ToolInvocationStatus::Canceled,
                        run_error("canceled", "runtime canceled", false, None),
                    )
                    .await?;
                }
                let final_entry_id = if final_status == AgentRunStatus::Canceled {
                    context
                        .final_entry_id()
                        .or(self.latest_assistant_entry_id_for_run(&agent_run).await?)
                } else {
                    context.final_entry_id()
                };
                let outcome = match final_status {
                    AgentRunStatus::Canceled => AgentRunOutcome::Canceled { final_entry_id },
                    AgentRunStatus::Completed
                        if final_stopped_reason == AgentStoppedReason::MaxSteps =>
                    {
                        AgentRunOutcome::MaxSteps { final_entry_id }
                    }
                    AgentRunStatus::Completed => AgentRunOutcome::Completed { final_entry_id },
                    AgentRunStatus::Queued | AgentRunStatus::Running | AgentRunStatus::Failed => {
                        return Err(AgentRuntimeError::Invariant(
                            "invalid successful run final status".to_string(),
                        ));
                    }
                };
                let finished = context.finish_run(outcome).await?;
                let output = finished.run.output.clone().ok_or_else(|| {
                    AgentRuntimeError::Invariant("finished run has no output".to_string())
                })?;
                Ok(AgentRunHandle {
                    agent_run: finished.run,
                    output: Some(output),
                    status: AgentRunHandleStatus::Finished,
                    events: context.events(),
                    steps: context.steps(),
                })
            }
            Err(error) => {
                if error.max_steps {
                    let finished = context
                        .finish_run(AgentRunOutcome::MaxSteps {
                            final_entry_id: context.final_entry_id(),
                        })
                        .await?;
                    let output = finished.run.output.clone().ok_or_else(|| {
                        AgentRuntimeError::Invariant("max steps run has no output".to_string())
                    })?;
                    return Ok(AgentRunHandle {
                        agent_run: finished.run,
                        output: Some(output),
                        status: AgentRunHandleStatus::Finished,
                        events: context.events(),
                        steps: context.steps(),
                    });
                }

                let payload = if request.cancellation_token.is_cancelled() {
                    run_error("canceled", "runtime canceled", false, None)
                } else {
                    run_error("prompt_error", error.message, true, None)
                };
                let final_status = if request.cancellation_token.is_cancelled() {
                    AgentRunStatus::Canceled
                } else {
                    AgentRunStatus::Failed
                };
                self.finalize_active_tool_invocations(
                    &agent_run.id,
                    &request.conversation_id,
                    if final_status == AgentRunStatus::Canceled {
                        ToolInvocationStatus::Canceled
                    } else {
                        ToolInvocationStatus::Failed
                    },
                    payload.clone(),
                )
                .await?;
                let final_entry_id = if final_status == AgentRunStatus::Canceled {
                    context
                        .final_entry_id()
                        .or(self.latest_assistant_entry_id_for_run(&agent_run).await?)
                } else {
                    None
                };
                let outcome = if final_status == AgentRunStatus::Canceled {
                    AgentRunOutcome::Canceled { final_entry_id }
                } else {
                    AgentRunOutcome::Failed {
                        error: payload.clone(),
                    }
                };
                let finished = context.finish_run(outcome).await?;
                let output = Some(finished.run.output.clone().ok_or_else(|| {
                    AgentRuntimeError::Invariant("finished run has no output".to_string())
                })?);
                Ok(AgentRunHandle {
                    agent_run: finished.run,
                    output,
                    status: AgentRunHandleStatus::Finished,
                    events: context.events(),
                    steps: context.steps(),
                })
            }
        }
    }

    pub async fn run_with_saved_provider_observed(
        &self,
        request: AgentRunRequest,
        provider: ProviderRecord,
        secrets: ProviderSecretValues,
        observer: Option<AgentRuntimeObserver>,
    ) -> Result<AgentRunHandle> {
        let mut request = request;
        let agent_run = self.begin_run(&mut request, observer.as_ref()).await?;
        self.run_started_with_saved_provider_observed(
            agent_run, request, provider, secrets, observer,
        )
        .await
    }

    pub async fn run_started_with_saved_provider_observed(
        &self,
        agent_run: AgentRunRecord,
        request: AgentRunRequest,
        provider: ProviderRecord,
        secrets: ProviderSecretValues,
        observer: Option<AgentRuntimeObserver>,
    ) -> Result<AgentRunHandle> {
        run_saved_provider_model(self, agent_run, request, provider, secrets, observer).await
    }

    pub async fn record_setup_failed_run(
        &self,
        mut request: AgentRunRequest,
        error: impl ToString,
        observer: Option<&AgentRuntimeObserver>,
    ) -> Result<AgentRunHandle> {
        let agent_run = self.begin_run(&mut request, observer).await?;
        self.record_setup_failed_started_run(&agent_run, error, observer)
            .await
    }

    pub async fn record_setup_failed_started_run(
        &self,
        agent_run: &AgentRunRecord,
        error: impl ToString,
        observer: Option<&AgentRuntimeObserver>,
    ) -> Result<AgentRunHandle> {
        let payload = run_error("setup_error", error.to_string(), true, None);
        let finished = self
            .finish_agent_run_with_observer(
                &agent_run.id,
                finish_agent_run_spec(
                    agent_run,
                    AgentRunOutcome::Failed {
                        error: payload.clone(),
                    },
                ),
                observer,
            )
            .await?;
        let output = finished.run.output.clone().ok_or_else(|| {
            AgentRuntimeError::Invariant("setup failure has no output".to_string())
        })?;
        Ok(AgentRunHandle {
            agent_run: finished.run,
            output: Some(output),
            status: AgentRunHandleStatus::Finished,
            events: vec![AgentRunEvent::Failed { error: payload }],
            steps: vec![AgentStep::ConversationEntry(finished.final_entry.id)],
        })
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

        let error = run_error("canceled", "runtime canceled", false, None);
        self.finalize_active_provider_steps(&run.id, ProviderStepStatus::Canceled, error.clone())
            .await?;
        self.finalize_active_tool_invocations(
            &run.id,
            &run.conversation_id,
            ToolInvocationStatus::Canceled,
            error,
        )
        .await?;
        let _ = self
            .finish_agent_run_with_observer(
                &run.id,
                finish_agent_run_spec(
                    &run,
                    AgentRunOutcome::Canceled {
                        final_entry_id: self.latest_assistant_entry_id_for_run(&run).await?,
                    },
                ),
                observer,
            )
            .await?;
        let run = self
            .persistence
            .get_agent_run(run.id.clone())
            .await?
            .ok_or_else(|| {
                AgentRuntimeError::Invariant("canceled agent run disappeared".to_string())
            })?;
        Ok(Some(run))
    }

    pub async fn recover_interrupted_runs(&self) -> Result<Vec<AgentRunRecord>> {
        let mut recovered = Vec::new();
        for status in [AgentRunStatus::Queued, AgentRunStatus::Running] {
            for run in self.persistence.agent_runs_by_status(status).await? {
                let error = run_error(
                    "interrupted",
                    "agent run was interrupted before reaching a terminal state",
                    true,
                    None,
                );
                self.fail_active_provider_steps(&run.id, error.clone())
                    .await?;
                self.finalize_active_tool_invocations(
                    &run.id,
                    &run.conversation_id,
                    ToolInvocationStatus::Failed,
                    error.clone(),
                )
                .await?;
                let finished = self
                    .finish_agent_run_with_observer(
                        &run.id,
                        finish_agent_run_spec(&run, AgentRunOutcome::Failed { error }),
                        None,
                    )
                    .await?;
                recovered.push(finished.run);
            }
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
        agent_run_id: &str,
        finish: FinishAgentRun,
        observer: Option<&AgentRuntimeObserver>,
    ) -> Result<FinishedAgentRun> {
        let commit = self
            .persistence
            .finish_agent_run(agent_run_id.to_string(), finish)
            .await?;
        emit_runtime(
            observer,
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
        let finished = commit.value;
        emit_runtime(
            observer,
            AgentRuntimeEvent::AgentRunStatusChanged {
                agent_run_id: finished.run.id.clone(),
                status: finished.run.status,
            },
        );
        Ok(finished)
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

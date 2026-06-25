use crate::{
    AgentRunHandle, AgentRunHandleStatus, AgentRunRequest, AgentRuntimeError, AgentRuntimeEvent,
    AgentRuntimeObserver, AgentStep, McpSessionManager, ProviderSecretValues, Result, SkillCatalog,
    SkillLoader,
    history::{PromptHistoryOptions, build_prompt_history_with_options},
    persistence::{PersistenceContext, PersistingCompletionModel, new_agent_run_input, run_error},
    provider_models::run_saved_provider_model,
    reasoning_params::{merge_additional_params, reasoning_additional_params},
};
use ai_chat_core::*;
use ai_chat_db::{
    AgentRunRecord, FreshRepository, NewConversationItem, ProviderRecord, UpdateAgentRunStatus,
};
use futures::StreamExt;
use rig_core::{
    agent::{AgentBuilder, MultiTurnStreamItem, StreamingError},
    completion::{CompletionModel, Prompt, PromptError, Usage},
    streaming::{StreamedAssistantContent, StreamingPrompt},
};
mod approval_resume;
mod finalization;
mod streaming;
#[cfg(test)]
mod tests;

use self::streaming::StreamingOutputAccumulator;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct AgentRuntime {
    repo: FreshRepository,
    skill_loader: SkillLoader,
    mcp_session_manager: Option<Arc<Mutex<McpSessionManager>>>,
}

impl AgentRuntime {
    pub fn new(repo: FreshRepository) -> Self {
        Self {
            repo,
            skill_loader: SkillLoader::new(),
            mcp_session_manager: None,
        }
    }

    pub fn with_skill_loader(mut self, skill_loader: SkillLoader) -> Self {
        self.skill_loader = skill_loader;
        self
    }

    pub fn with_mcp_session_manager(mut self, manager: Arc<Mutex<McpSessionManager>>) -> Self {
        self.mcp_session_manager = Some(manager);
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
        let agent_run = self.begin_run(&mut request, observer.as_ref())?;
        self.run_started_with_model_observed(agent_run, request, model, observer)
            .await
    }

    pub fn begin_run(
        &self,
        request: &mut AgentRunRequest,
        observer: Option<&AgentRuntimeObserver>,
    ) -> Result<AgentRunRecord> {
        if request.cancellation_token.is_cancelled() {
            return Err(AgentRuntimeError::Canceled);
        }
        crate::builtin_tools::registry::register_enabled_builtin_tools(
            &mut request.tool_registry,
            &request.settings_snapshot.tool_policy,
            request.project_root.as_deref(),
        )?;
        request.tool_registry.finalize_names();
        let mut agent_run = self.repo.insert_agent_run(new_agent_run_input(request))?;
        emit_runtime(
            observer,
            AgentRuntimeEvent::AgentRunStarted {
                agent_run_id: agent_run.id.clone(),
                conversation_id: agent_run.conversation_id.clone(),
            },
        );
        agent_run = self.repo.update_agent_run_status(
            &agent_run.id,
            UpdateAgentRunStatus {
                status: AgentRunStatus::Running,
                output: None,
                error: None,
            },
        )?;
        emit_runtime(
            observer,
            AgentRuntimeEvent::AgentRunStatusChanged {
                agent_run_id: agent_run.id.clone(),
                status: AgentRunStatus::Running,
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
                .cancel_run(&agent_run.id, observer.as_ref())?
                .ok_or_else(|| AgentRuntimeError::Invariant("agent run disappeared".to_string()))?;
            return Ok(AgentRunHandle {
                agent_run,
                output: None,
                status: AgentRunHandleStatus::Finished,
                events: vec![AgentRunEvent::Canceled],
                steps: Vec::new(),
            });
        }

        if let Err(error) = self.activate_skills(&request, &agent_run.id) {
            return Err(self.mark_setup_failed(&agent_run.id, error, observer.as_ref())?);
        }

        let timeline = match self
            .repo
            .conversation_timeline_records(&request.conversation_id)
        {
            Ok(Some(timeline)) => timeline,
            Ok(None) => {
                return Err(self.mark_setup_failed(
                    &agent_run.id,
                    AgentRuntimeError::Invariant(format!(
                        "conversation {} is missing",
                        request.conversation_id
                    )),
                    observer.as_ref(),
                )?);
            }
            Err(error) => {
                return Err(self.mark_setup_failed(
                    &agent_run.id,
                    AgentRuntimeError::from(error),
                    observer.as_ref(),
                )?);
            }
        };
        let deepseek_text_resume = needs_deepseek_text_resume(&request);
        // DeepSeek thinking requires provider-native reasoning_content on replayed assistant
        // tool-call messages; Rig history does not preserve that field, so resume as text.
        let prompt_history = match build_prompt_history_with_options(
            &timeline.items,
            &timeline.attachments,
            &request.user_item_id,
            &agent_run.id,
            request.parent_agent_run_id.as_deref(),
            PromptHistoryOptions {
                include_reasoning: !deepseek_text_resume,
                preserve_tool_protocol: !deepseek_text_resume,
            },
        ) {
            Ok(prompt_history) => prompt_history,
            Err(error) => {
                return Err(self.mark_setup_failed(&agent_run.id, error, observer.as_ref())?);
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
            self.repo.clone(),
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
        );
        let model = PersistingCompletionModel::new(model, context.clone());
        let hook = context.hook();

        let mut builder = AgentBuilder::new(model)
            .name("ai-chat-agent")
            .hook(hook)
            .tools(rig_tools)
            .default_max_turns(request.guards.max_steps as usize);
        if let Some(prompt) = prompt_preamble(request.prompt_snapshot.as_ref()) {
            builder = builder.preamble(&prompt);
        }
        let reasoning_params = if deepseek_text_resume {
            None
        } else {
            reasoning_additional_params(&request.settings_snapshot)
        };
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
                let _ = context.cancel_current_provider_step(run_error(
                    "canceled",
                    "runtime canceled",
                    false,
                    None,
                ));
                return Ok(AgentRunHandle {
                    agent_run: self
                        .cancel_run(&agent_run.id, observer.as_ref())?
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
                        accumulator.finish(ConversationItemStatus::Canceled, None)?;
                        let _ = context.cancel_current_provider_step(run_error(
                            "canceled",
                            "runtime canceled",
                            false,
                            None,
                        ));
                        break Ok(AgentStoppedReason::Canceled);
                    }
                    next = stream.next() => next,
                };
                match next {
                    Some(Ok(MultiTurnStreamItem::StreamAssistantItem(item))) => match item {
                        StreamedAssistantContent::Text(text) => {
                            accumulator.append_text(&text.text)?;
                        }
                        StreamedAssistantContent::Reasoning(reasoning) => {
                            accumulator.replace_reasoning(reasoning.display_text())?;
                        }
                        StreamedAssistantContent::ReasoningDelta { reasoning, .. } => {
                            accumulator.append_reasoning(&reasoning)?;
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
                        if context.waiting_tool_invocation_id().is_some() {
                            accumulator.finish(ConversationItemStatus::Completed, None)?;
                            context.finish_current_streaming_provider_step(
                                final_raw_response.as_ref(),
                                Usage::new(),
                            )?;
                        } else if request.cancellation_token.is_cancelled() {
                            accumulator.finish(ConversationItemStatus::Canceled, None)?;
                            let _ = context.cancel_current_provider_step(run_error(
                                "canceled",
                                "runtime canceled",
                                false,
                                None,
                            ));
                        } else {
                            accumulator.finish(ConversationItemStatus::Failed, None)?;
                            let _ = context.fail_current_provider_step(run_error(
                                "prompt_error",
                                error.to_string(),
                                true,
                                None,
                            ));
                        }
                        break Err(PromptExecutionError::streaming(error));
                    }
                    None => {
                        let final_text = final_response
                            .as_ref()
                            .map(|response| response.response())
                            .filter(|text| !text.is_empty());
                        if request.cancellation_token.is_cancelled() {
                            accumulator.finish(ConversationItemStatus::Canceled, final_text)?;
                            let _ = context.cancel_current_provider_step(run_error(
                                "canceled",
                                "runtime canceled",
                                false,
                                None,
                            ));
                            break Ok(AgentStoppedReason::Canceled);
                        } else {
                            accumulator.finish(ConversationItemStatus::Completed, final_text)?;
                            let usage = final_response
                                .as_ref()
                                .map(|response| response.usage())
                                .unwrap_or_else(Usage::new);
                            context.finish_current_streaming_provider_step(
                                final_raw_response.as_ref(),
                                usage,
                            )?;
                            break Ok(AgentStoppedReason::Completed);
                        }
                    }
                }
            }
        } else {
            let response = tokio::select! {
                biased;
                _ = request.cancellation_token.cancelled() => {
                    let _ = context.cancel_current_provider_step(run_error(
                        "canceled",
                        "runtime canceled",
                        false,
                        None,
                    ));
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
                    )?;
                }
                let output = AgentRunOutput {
                    final_item_id: context.final_item_id(),
                    stopped_reason: final_stopped_reason,
                };
                let agent_run = self.repo.update_agent_run_status(
                    &agent_run.id,
                    UpdateAgentRunStatus {
                        status: final_status,
                        output: Some(output.clone()),
                        error: None,
                    },
                )?;
                emit_runtime(
                    observer.as_ref(),
                    AgentRuntimeEvent::AgentRunStatusChanged {
                        agent_run_id: agent_run.id.clone(),
                        status: final_status,
                    },
                );
                let mut events = context.events();
                if final_status == AgentRunStatus::Canceled {
                    events.push(AgentRunEvent::Canceled);
                } else {
                    events.push(AgentRunEvent::Completed {
                        output: output.clone(),
                    });
                }
                Ok(AgentRunHandle {
                    agent_run,
                    output: Some(output),
                    status: AgentRunHandleStatus::Finished,
                    events,
                    steps: context.steps(),
                })
            }
            Err(error) => {
                if let Some(tool_invocation_id) = context.waiting_tool_invocation_id() {
                    let events = context.events();
                    let agent_run = self.repo.get_agent_run(&agent_run.id)?.ok_or_else(|| {
                        AgentRuntimeError::Invariant("agent run disappeared".to_string())
                    })?;
                    return Ok(AgentRunHandle {
                        agent_run,
                        output: None,
                        status: AgentRunHandleStatus::WaitingForApproval { tool_invocation_id },
                        events,
                        steps: context.steps(),
                    });
                }

                if error.max_steps {
                    let output = AgentRunOutput {
                        final_item_id: context.final_item_id(),
                        stopped_reason: AgentStoppedReason::MaxSteps,
                    };
                    let agent_run = self.repo.update_agent_run_status(
                        &agent_run.id,
                        UpdateAgentRunStatus {
                            status: AgentRunStatus::Completed,
                            output: Some(output.clone()),
                            error: None,
                        },
                    )?;
                    emit_runtime(
                        observer.as_ref(),
                        AgentRuntimeEvent::AgentRunStatusChanged {
                            agent_run_id: agent_run.id.clone(),
                            status: AgentRunStatus::Completed,
                        },
                    );
                    let mut events = context.events();
                    events.push(AgentRunEvent::Completed {
                        output: output.clone(),
                    });
                    return Ok(AgentRunHandle {
                        agent_run,
                        output: Some(output),
                        status: AgentRunHandleStatus::Finished,
                        events,
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
                )?;
                let output = (final_status == AgentRunStatus::Canceled).then_some(AgentRunOutput {
                    final_item_id: context.final_item_id(),
                    stopped_reason: AgentStoppedReason::Canceled,
                });
                let agent_run = self.repo.update_agent_run_status(
                    &agent_run.id,
                    UpdateAgentRunStatus {
                        status: final_status,
                        output: output.clone(),
                        error: (final_status == AgentRunStatus::Failed).then_some(payload.clone()),
                    },
                )?;
                emit_runtime(
                    observer.as_ref(),
                    AgentRuntimeEvent::AgentRunStatusChanged {
                        agent_run_id: agent_run.id.clone(),
                        status: final_status,
                    },
                );
                let mut events = context.events();
                if final_status == AgentRunStatus::Canceled {
                    events.push(AgentRunEvent::Canceled);
                } else {
                    events.push(AgentRunEvent::Failed { error: payload });
                }
                Ok(AgentRunHandle {
                    agent_run,
                    output,
                    status: AgentRunHandleStatus::Finished,
                    events,
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
        let agent_run = self.begin_run(&mut request, observer.as_ref())?;
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

    pub fn record_setup_failed_run(
        &self,
        mut request: AgentRunRequest,
        error: impl ToString,
        observer: Option<&AgentRuntimeObserver>,
    ) -> Result<AgentRunHandle> {
        let agent_run = self.begin_run(&mut request, observer)?;
        self.record_setup_failed_started_run(&agent_run, error, observer)
    }

    pub fn record_setup_failed_started_run(
        &self,
        agent_run: &AgentRunRecord,
        error: impl ToString,
        observer: Option<&AgentRuntimeObserver>,
    ) -> Result<AgentRunHandle> {
        let payload = run_error("setup_error", error.to_string(), true, None);
        let item = self.repo.append_conversation_item(NewConversationItem {
            conversation_id: agent_run.conversation_id.clone(),
            status: ConversationItemStatus::Completed,
            agent_run_id: Some(agent_run.id.clone()),
            provider_step_id: None,
            tool_invocation_id: None,
            provider_item_id: None,
            payload: ConversationItemPayload::Error(payload.clone()),
        })?;
        let output = AgentRunOutput {
            final_item_id: Some(item.id.clone()),
            stopped_reason: AgentStoppedReason::Failed,
        };
        let agent_run = self.repo.update_agent_run_status(
            &agent_run.id,
            UpdateAgentRunStatus {
                status: AgentRunStatus::Failed,
                output: Some(output.clone()),
                error: Some(payload.clone()),
            },
        )?;
        emit_runtime(
            observer,
            AgentRuntimeEvent::AgentRunStatusChanged {
                agent_run_id: agent_run.id.clone(),
                status: AgentRunStatus::Failed,
            },
        );
        Ok(AgentRunHandle {
            agent_run,
            output: Some(output),
            status: AgentRunHandleStatus::Finished,
            events: vec![AgentRunEvent::Failed { error: payload }],
            steps: vec![AgentStep::ConversationItem(item.id)],
        })
    }

    pub fn cancel_non_terminal_runs_for_conversation(
        &self,
        conversation_id: &str,
        observer: Option<&AgentRuntimeObserver>,
    ) -> Result<Vec<AgentRunRecord>> {
        let mut canceled = Vec::new();
        for run in self.repo.agent_runs_for_conversation(conversation_id)? {
            if is_terminal_agent_run_status(run.status) {
                continue;
            }
            if let Some(run) = self.cancel_run(&run.id, observer)? {
                canceled.push(run);
            }
        }
        Ok(canceled)
    }

    pub fn cancel_run(
        &self,
        agent_run_id: &str,
        observer: Option<&AgentRuntimeObserver>,
    ) -> Result<Option<AgentRunRecord>> {
        let Some(run) = self.repo.get_agent_run(agent_run_id)? else {
            return Ok(None);
        };
        if is_terminal_agent_run_status(run.status) {
            return Ok(Some(run));
        }

        let error = run_error("canceled", "runtime canceled", false, None);
        self.finalize_active_provider_steps(&run.id, ProviderStepStatus::Canceled, error.clone())?;
        self.finalize_active_tool_invocations(
            &run.id,
            &run.conversation_id,
            ToolInvocationStatus::Canceled,
            error,
        )?;
        let output = AgentRunOutput {
            final_item_id: self.latest_assistant_item_id_for_run(&run)?,
            stopped_reason: AgentStoppedReason::Canceled,
        };
        let run = self.repo.update_agent_run_status(
            &run.id,
            UpdateAgentRunStatus {
                status: AgentRunStatus::Canceled,
                output: Some(output),
                error: None,
            },
        )?;
        emit_runtime(
            observer,
            AgentRuntimeEvent::AgentRunStatusChanged {
                agent_run_id: run.id.clone(),
                status: AgentRunStatus::Canceled,
            },
        );
        Ok(Some(run))
    }

    pub fn recover_interrupted_runs(&self) -> Result<Vec<AgentRunRecord>> {
        let mut recovered = Vec::new();
        for status in [AgentRunStatus::Queued, AgentRunStatus::Running] {
            for run in self.repo.agent_runs_by_status(status)? {
                let error = run_error(
                    "interrupted",
                    "agent run was interrupted before reaching a terminal state",
                    true,
                    None,
                );
                self.fail_active_provider_steps(&run.id, error.clone())?;
                self.finalize_active_tool_invocations(
                    &run.id,
                    &run.conversation_id,
                    ToolInvocationStatus::Failed,
                    error.clone(),
                )?;
                recovered.push(self.repo.update_agent_run_status(
                    &run.id,
                    UpdateAgentRunStatus {
                        status: AgentRunStatus::Failed,
                        output: None,
                        error: Some(error),
                    },
                )?);
            }
        }
        Ok(recovered)
    }

    fn activate_skills(&self, request: &AgentRunRequest, agent_run_id: &str) -> Result<()> {
        if request.skill_requests.is_empty() {
            return Ok(());
        }
        let catalog = SkillCatalog::scan(request.project_root.as_deref())?;
        for skill_request in &request.skill_requests {
            let entry = catalog.get(&skill_request.name).ok_or_else(|| {
                AgentRuntimeError::Invariant(format!("skill {} is missing", skill_request.name))
            })?;
            let activation = self.skill_loader.load(entry)?;
            self.repo.append_conversation_item(NewConversationItem {
                conversation_id: request.conversation_id.clone(),
                status: ConversationItemStatus::Completed,
                agent_run_id: Some(agent_run_id.to_string()),
                provider_step_id: None,
                tool_invocation_id: None,
                provider_item_id: None,
                payload: ConversationItemPayload::SkillActivation(activation),
            })?;
        }
        Ok(())
    }
}

fn needs_deepseek_text_resume(request: &AgentRunRequest) -> bool {
    request.parent_agent_run_id.is_some()
        && request.settings_snapshot.provider_settings.provider_kind == "deepseek"
        && reasoning_selection_enables_deepseek_thinking(
            request.settings_snapshot.reasoning_selection.as_ref(),
        )
}

fn reasoning_selection_enables_deepseek_thinking(
    selection: Option<&ReasoningSelectionSnapshot>,
) -> bool {
    match selection {
        Some(ReasoningSelectionSnapshot::Level { value }) => value == "high" || value == "max",
        Some(ReasoningSelectionSnapshot::Composite { selections }) => selections
            .iter()
            .any(|selection| reasoning_selection_enables_deepseek_thinking(Some(selection))),
        _ => false,
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

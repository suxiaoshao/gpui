use crate::{
    AgentRunHandle, AgentRunRequest, AgentRuntimeError, AgentRuntimeEvent, AgentRuntimeObserver,
    ProviderSecretValues, Result, SkillCatalog, SkillLoader,
    history::build_prompt_history,
    persistence::{PersistenceContext, PersistingCompletionModel, new_agent_run_input, run_error},
    provider_models::run_saved_provider_model,
    reasoning_params::{merge_additional_params, reasoning_additional_params},
};
use ai_chat_core::*;
use ai_chat_db::{
    AgentRunRecord, ApprovalDecisionRecord, FreshRepository, NewApprovalDecisionOutcome,
    NewConversationItem, ProviderRecord, ToolInvocationRecord, UpdateAgentRunStatus,
    UpdateProviderStepStatus, UpdateToolInvocationStatus,
};
use rig_core::{
    agent::AgentBuilder,
    completion::{CompletionModel, Prompt},
    tool::{ToolSet, server::ToolServer},
};

#[derive(Clone)]
pub struct AgentRuntime {
    repo: FreshRepository,
    skill_loader: SkillLoader,
}

impl AgentRuntime {
    pub fn new(repo: FreshRepository) -> Self {
        Self {
            repo,
            skill_loader: SkillLoader::new(),
        }
    }

    pub fn with_skill_loader(mut self, skill_loader: SkillLoader) -> Self {
        self.skill_loader = skill_loader;
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
        if request.cancellation_token.is_cancelled() {
            return Err(AgentRuntimeError::Canceled);
        }

        request.tool_registry.finalize_names();
        let mut agent_run = self.repo.insert_agent_run(new_agent_run_input(&request))?;
        emit_runtime(
            observer.as_ref(),
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
            observer.as_ref(),
            AgentRuntimeEvent::AgentRunStatusChanged {
                agent_run_id: agent_run.id.clone(),
                status: AgentRunStatus::Running,
            },
        );

        if let Err(error) = self.activate_skills(&request, &agent_run.id) {
            return Err(self.mark_setup_failed(&agent_run.id, error, observer.as_ref())?);
        }

        let items = match self.repo.conversation_items(&request.conversation_id) {
            Ok(items) => items,
            Err(error) => {
                return Err(self.mark_setup_failed(
                    &agent_run.id,
                    AgentRuntimeError::from(error),
                    observer.as_ref(),
                )?);
            }
        };
        let prompt_history =
            match build_prompt_history(&items, &request.user_item_id, &agent_run.id) {
                Ok(prompt_history) => prompt_history,
                Err(error) => {
                    return Err(self.mark_setup_failed(&agent_run.id, error, observer.as_ref())?);
                }
            };

        let tool_server = ToolServer::new().run();
        let mut toolset = ToolSet::default();
        for tool in request
            .tool_registry
            .clone()
            .into_rig_tools(request.guards.tool_timeout)
        {
            toolset.add_tool_boxed(tool);
        }
        if let Err(error) = tool_server.append_toolset(toolset).await {
            return Err(self.mark_setup_failed(
                &agent_run.id,
                AgentRuntimeError::from(error),
                observer.as_ref(),
            )?);
        }

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
            .tool_server_handle(tool_server)
            .default_max_turns(request.guards.max_steps as usize);
        if let Some(prompt) = prompt_preamble(request.prompt_snapshot.as_ref()) {
            builder = builder.preamble(&prompt);
        }
        let additional_params = merge_additional_params(
            reasoning_additional_params(&request.settings_snapshot),
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

        let response = agent
            .prompt(prompt_history.prompt)
            .with_history(prompt_history.history)
            .with_tool_concurrency(request.guards.tool_concurrency)
            .without_memory()
            .extended_details()
            .await;

        match response {
            Ok(_response) => {
                let output = AgentRunOutput {
                    final_item_id: context.final_item_id(),
                    stopped_reason: AgentStoppedReason::Completed,
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
                Ok(AgentRunHandle {
                    agent_run,
                    output: Some(output),
                    events,
                    steps: context.steps(),
                })
            }
            Err(error) => {
                let status = self
                    .repo
                    .get_agent_run(&agent_run.id)?
                    .map(|run| run.status)
                    .unwrap_or(AgentRunStatus::Failed);
                if status == AgentRunStatus::WaitingForApproval {
                    let events = context.events();
                    let agent_run = self.repo.get_agent_run(&agent_run.id)?.ok_or_else(|| {
                        AgentRuntimeError::Invariant("agent run disappeared".to_string())
                    })?;
                    return Ok(AgentRunHandle {
                        agent_run,
                        output: None,
                        events,
                        steps: context.steps(),
                    });
                }

                if matches!(
                    &error,
                    rig_core::completion::PromptError::MaxTurnsError { .. }
                ) {
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
                        events,
                        steps: context.steps(),
                    });
                }

                let payload = if request.cancellation_token.is_cancelled() {
                    run_error("canceled", "runtime canceled", false, None)
                } else {
                    run_error("prompt_error", error.to_string(), true, None)
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
        run_saved_provider_model(self, request, provider, secrets, observer).await
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

    fn finalize_active_tool_invocations(
        &self,
        agent_run_id: &str,
        conversation_id: &str,
        status: ToolInvocationStatus,
        error: RunErrorPayload,
    ) -> Result<()> {
        for invocation in self.repo.tool_invocations_for_run(agent_run_id)? {
            if !matches!(
                invocation.status,
                ToolInvocationStatus::Requested
                    | ToolInvocationStatus::AwaitingApproval
                    | ToolInvocationStatus::Running
            ) {
                continue;
            }

            self.append_error_tool_result_and_update_tool_invocation(
                conversation_id,
                &invocation,
                status,
                error.clone(),
            )?;
        }
        Ok(())
    }

    fn append_error_tool_result_and_update_tool_invocation(
        &self,
        conversation_id: &str,
        invocation: &ToolInvocationRecord,
        status: ToolInvocationStatus,
        error: RunErrorPayload,
    ) -> Result<ConversationItemId> {
        let output = ToolInvocationOutput {
            content: vec![ContentPart::Text {
                text: error.message.clone(),
            }],
            structured_output: None,
            raw_output: None,
            is_error: true,
        };
        let payload = ConversationItemPayload::ToolResult(ToolResultItem {
            tool_invocation_id: Some(invocation.id.clone()),
            call_id: invocation.call_id.clone(),
            content: output.content.clone(),
            is_error: true,
            structured_output: None,
            raw_output: None,
        });
        let (item, _) = self
            .repo
            .append_conversation_item_and_update_tool_invocation(
                NewConversationItem {
                    conversation_id: conversation_id.to_string(),
                    status: ConversationItemStatus::Completed,
                    agent_run_id: Some(invocation.agent_run_id.clone()),
                    provider_step_id: invocation.provider_step_id.clone(),
                    tool_invocation_id: Some(invocation.id.clone()),
                    provider_item_id: None,
                    payload,
                },
                &invocation.id,
                UpdateToolInvocationStatus {
                    status,
                    output: Some(output),
                    error: Some(error),
                },
            )?;
        Ok(item.id)
    }

    fn finalize_active_provider_steps(
        &self,
        agent_run_id: &str,
        status: ProviderStepStatus,
        error: RunErrorPayload,
    ) -> Result<()> {
        for step in self.repo.provider_steps_for_run(agent_run_id)? {
            if !matches!(
                step.status,
                ProviderStepStatus::Queued | ProviderStepStatus::Running
            ) {
                continue;
            }

            self.repo.update_provider_step_status(
                &step.id,
                UpdateProviderStepStatus {
                    status,
                    response_snapshot: None,
                    state_snapshot: None,
                    error: Some(error.clone()),
                },
            )?;
        }
        Ok(())
    }

    fn fail_active_provider_steps(&self, agent_run_id: &str, error: RunErrorPayload) -> Result<()> {
        self.finalize_active_provider_steps(agent_run_id, ProviderStepStatus::Failed, error)
    }

    fn latest_assistant_item_id_for_run(
        &self,
        run: &AgentRunRecord,
    ) -> Result<Option<ConversationItemId>> {
        Ok(self
            .repo
            .conversation_items(&run.conversation_id)?
            .into_iter()
            .rev()
            .find(|item| {
                item.agent_run_id.as_deref() == Some(run.id.as_str())
                    && matches!(
                        item.payload,
                        ConversationItemPayload::Message {
                            role: TranscriptRole::Assistant,
                            ..
                        }
                    )
            })
            .map(|item| item.id))
    }

    fn mark_setup_failed(
        &self,
        agent_run_id: &str,
        error: AgentRuntimeError,
        observer: Option<&AgentRuntimeObserver>,
    ) -> Result<AgentRuntimeError> {
        self.repo.update_agent_run_status(
            agent_run_id,
            UpdateAgentRunStatus {
                status: AgentRunStatus::Failed,
                output: None,
                error: Some(run_error("setup_error", error.to_string(), true, None)),
            },
        )?;
        emit_runtime(
            observer,
            AgentRuntimeEvent::AgentRunStatusChanged {
                agent_run_id: agent_run_id.to_string(),
                status: AgentRunStatus::Failed,
            },
        );
        Ok(error)
    }

    pub fn decide_approval(
        &self,
        approval_decision_id: &str,
        outcome: NewApprovalDecisionOutcome,
    ) -> Result<ApprovalDecisionRecord> {
        enum TerminalApproval {
            Denied { message: String },
            Canceled,
            Expired,
            Pending,
        }

        let terminal = match &outcome {
            NewApprovalDecisionOutcome::Approved { .. } => {
                return Err(AgentRuntimeError::Unsupported(
                    "approved tool resume is not implemented in v1".to_string(),
                ));
            }
            NewApprovalDecisionOutcome::Denied { reason, .. } => TerminalApproval::Denied {
                message: reason
                    .clone()
                    .unwrap_or_else(|| "Tool call denied by user".to_string()),
            },
            NewApprovalDecisionOutcome::Canceled => TerminalApproval::Canceled,
            NewApprovalDecisionOutcome::Expired => TerminalApproval::Expired,
            NewApprovalDecisionOutcome::Pending { .. } => TerminalApproval::Pending,
        };
        let approval = self
            .repo
            .get_approval_decision(approval_decision_id)?
            .ok_or_else(|| {
                AgentRuntimeError::Invariant(format!(
                    "approval decision {approval_decision_id} is missing"
                ))
            })?;
        let invocation = self
            .repo
            .get_tool_invocation(&approval.tool_invocation_id)?
            .ok_or_else(|| {
                AgentRuntimeError::Invariant(format!(
                    "tool invocation {} is missing",
                    approval.tool_invocation_id
                ))
            })?;
        let agent_run = self
            .repo
            .get_agent_run(&invocation.agent_run_id)?
            .ok_or_else(|| {
                AgentRuntimeError::Invariant(format!(
                    "agent run {} is missing",
                    invocation.agent_run_id
                ))
            })?;
        let updated = self
            .repo
            .update_approval_decision(approval_decision_id, outcome)?;
        if let Some(decision) = updated.decision.as_ref() {
            let payload = ConversationItemPayload::ApprovalDecision(ApprovalDecisionItem {
                approval_decision_id: updated.id.clone(),
                decision: decision.clone(),
            });
            self.repo.append_conversation_item(NewConversationItem {
                conversation_id: agent_run.conversation_id.clone(),
                status: ConversationItemStatus::Completed,
                agent_run_id: Some(invocation.agent_run_id.clone()),
                provider_step_id: invocation.provider_step_id.clone(),
                tool_invocation_id: Some(invocation.id.clone()),
                provider_item_id: None,
                payload,
            })?;
        }

        match terminal {
            TerminalApproval::Denied { message } => {
                let error = run_error("approval_denied", message, false, None);
                let item_id = self.append_error_tool_result_and_update_tool_invocation(
                    &agent_run.conversation_id,
                    &invocation,
                    ToolInvocationStatus::Denied,
                    error.clone(),
                )?;
                let output = AgentRunOutput {
                    final_item_id: Some(item_id),
                    stopped_reason: AgentStoppedReason::Failed,
                };
                self.repo.update_agent_run_status(
                    &agent_run.id,
                    UpdateAgentRunStatus {
                        status: AgentRunStatus::Failed,
                        output: Some(output),
                        error: Some(error),
                    },
                )?;
            }
            TerminalApproval::Canceled => {
                let error = run_error("approval_canceled", "Tool approval canceled", false, None);
                let item_id = self.append_error_tool_result_and_update_tool_invocation(
                    &agent_run.conversation_id,
                    &invocation,
                    ToolInvocationStatus::Canceled,
                    error,
                )?;
                let output = AgentRunOutput {
                    final_item_id: Some(item_id),
                    stopped_reason: AgentStoppedReason::Canceled,
                };
                self.repo.update_agent_run_status(
                    &agent_run.id,
                    UpdateAgentRunStatus {
                        status: AgentRunStatus::Canceled,
                        output: Some(output),
                        error: None,
                    },
                )?;
            }
            TerminalApproval::Expired => {
                let error = run_error("approval_expired", "Tool approval expired", true, None);
                let item_id = self.append_error_tool_result_and_update_tool_invocation(
                    &agent_run.conversation_id,
                    &invocation,
                    ToolInvocationStatus::Failed,
                    error.clone(),
                )?;
                let output = AgentRunOutput {
                    final_item_id: Some(item_id),
                    stopped_reason: AgentStoppedReason::Failed,
                };
                self.repo.update_agent_run_status(
                    &agent_run.id,
                    UpdateAgentRunStatus {
                        status: AgentRunStatus::Failed,
                        output: Some(output),
                        error: Some(error),
                    },
                )?;
            }
            TerminalApproval::Pending => {}
        }
        Ok(updated)
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

fn emit_runtime(observer: Option<&AgentRuntimeObserver>, event: AgentRuntimeEvent) {
    if let Some(observer) = observer {
        observer.emit(event);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LocalTool, McpConnector, ToolDefinition, ToolExecutor, ToolRunPolicy};
    use ai_chat_db::{
        ConversationItemRecord, ConversationRecord, FreshStore, NewApprovalDecision,
        NewConversation, NewConversationItem, NewProject, NewProvider, NewProviderModel,
        NewProviderStep, NewToolInvocation, ProviderModelRecord, ProviderRecord,
        ProviderStepRecord, ToolInvocationRecord,
    };
    use async_trait::async_trait;
    use rig_core::{
        completion::{AssistantContent, Message as RigMessage},
        message::UserContent,
        test_utils::{MockCompletionModel, MockTurn},
    };
    use rmcp::{
        RoleServer, ServerHandler, ServiceExt,
        model::{
            CallToolRequestParams, CallToolResult, Content, ErrorData, Implementation,
            ListToolsResult, PaginatedRequestParams, ProtocolVersion, ServerCapabilities,
            ServerInfo, Tool,
        },
        service::RequestContext,
    };
    use serde_json::json;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::RwLock;

    #[tokio::test]
    async fn no_tool_run_persists_provider_step_and_final_message() {
        let fixture = Fixture::new("no-tool");
        let runtime = AgentRuntime::new(fixture.repo.clone());
        let model = MockCompletionModel::text("hello from model");
        let handle = runtime
            .run_with_model(fixture.request(), model)
            .await
            .unwrap();

        assert_eq!(handle.agent_run.status, AgentRunStatus::Completed);
        assert_eq!(
            handle.output.unwrap().stopped_reason,
            AgentStoppedReason::Completed
        );
        assert_eq!(
            fixture
                .repo
                .provider_steps_for_run(&handle.agent_run.id)
                .unwrap()
                .len(),
            1
        );
        let items = fixture
            .repo
            .conversation_items(&fixture.conversation.id)
            .unwrap();
        assert!(items.iter().any(|item| matches!(
            &item.payload,
            ConversationItemPayload::Message {
                role: TranscriptRole::Assistant,
                content,
            } if content[0].search_text() == Some("hello from model")
        )));
    }

    #[tokio::test]
    async fn rig_tool_call_persists_tool_call_and_result() {
        let fixture = Fixture::new("tool-run");
        let runtime = AgentRuntime::new(fixture.repo.clone());
        let mut request = fixture.request();
        request
            .tool_registry
            .register_local_tool(EchoTool::new(ToolApprovalPolicy::Never))
            .unwrap();
        let model = MockCompletionModel::new([
            MockTurn::tool_call("call_1", "echo", json!({"text": "hi"})),
            MockTurn::text("done"),
        ]);

        let handle = runtime.run_with_model(request, model).await.unwrap();
        assert_eq!(handle.agent_run.status, AgentRunStatus::Completed);
        let invocations = fixture
            .repo
            .tool_invocations_for_run(&handle.agent_run.id)
            .unwrap();
        assert_eq!(invocations.len(), 1);
        assert_eq!(invocations[0].status, ToolInvocationStatus::Succeeded);
        assert_eq!(invocations[0].runtime_tool_name, "echo");
        let output = invocations[0].output.as_ref().unwrap();
        assert_eq!(
            output.content,
            vec![ContentPart::Text {
                text: "hi".to_string()
            }]
        );
        assert_eq!(
            output.structured_output.as_ref().unwrap().value,
            json!({"text": "hi"})
        );

        let items = fixture
            .repo
            .conversation_items(&fixture.conversation.id)
            .unwrap();
        assert!(
            items
                .iter()
                .any(|item| matches!(item.payload, ConversationItemPayload::ToolCall(_)))
        );
        assert!(
            items
                .iter()
                .any(|item| matches!(item.payload, ConversationItemPayload::ToolResult(_)))
        );
        let tool_result = items
            .iter()
            .find_map(|item| match &item.payload {
                ConversationItemPayload::ToolResult(result) => Some(result),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            tool_result.content,
            vec![ContentPart::Text {
                text: "hi".to_string()
            }]
        );
        assert_eq!(
            tool_result.structured_output.as_ref().unwrap().value,
            json!({"text": "hi"})
        );
        assert!(!tool_result.is_error);
    }

    #[tokio::test]
    async fn tool_error_output_is_persisted_without_reconstructing_from_model_text() {
        let fixture = Fixture::new("tool-error-output");
        let runtime = AgentRuntime::new(fixture.repo.clone());
        let mut request = fixture.request();
        request
            .tool_registry
            .register_local_tool(ErrorTool)
            .unwrap();
        let model = MockCompletionModel::new([
            MockTurn::tool_call("call_1", "error_tool", json!({"code": "E_BAD"})),
            MockTurn::text("handled"),
        ]);

        let handle = runtime.run_with_model(request, model).await.unwrap();
        assert_eq!(handle.agent_run.status, AgentRunStatus::Completed);
        let invocations = fixture
            .repo
            .tool_invocations_for_run(&handle.agent_run.id)
            .unwrap();
        assert_eq!(invocations.len(), 1);
        assert_eq!(invocations[0].status, ToolInvocationStatus::Failed);
        assert_eq!(invocations[0].error.as_ref().unwrap().code, "tool_error");
        let output = invocations[0].output.as_ref().unwrap();
        assert!(output.is_error);
        assert_eq!(
            output.content,
            vec![ContentPart::Text {
                text: "human readable error".to_string()
            }]
        );
        assert_eq!(
            output.structured_output.as_ref().unwrap().value,
            json!({"code": "E_BAD"})
        );
        assert_eq!(
            output.raw_output.as_ref().unwrap().value,
            json!({"raw": "details"})
        );

        let items = fixture
            .repo
            .conversation_items(&fixture.conversation.id)
            .unwrap();
        let tool_result = items
            .iter()
            .find_map(|item| match &item.payload {
                ConversationItemPayload::ToolResult(result) => Some(result),
                _ => None,
            })
            .unwrap();
        assert!(tool_result.is_error);
        assert_eq!(
            tool_result.content,
            vec![ContentPart::Text {
                text: "human readable error".to_string()
            }]
        );
        assert_eq!(
            tool_result.structured_output.as_ref().unwrap().value,
            json!({"code": "E_BAD"})
        );
        assert_eq!(
            tool_result.raw_output.as_ref().unwrap().value,
            json!({"raw": "details"})
        );
    }

    #[tokio::test]
    async fn max_turns_is_persisted_as_max_steps_stop() {
        let fixture = Fixture::new("max-steps");
        let runtime = AgentRuntime::new(fixture.repo.clone());
        let mut request = fixture.request();
        request.guards.max_steps = 1;
        request
            .tool_registry
            .register_local_tool(EchoTool::new(ToolApprovalPolicy::Never))
            .unwrap();
        let model = MockCompletionModel::new([
            MockTurn::tool_call("call_1", "echo", json!({"text": "one"})),
            MockTurn::tool_call("call_2", "echo", json!({"text": "two"})),
            MockTurn::tool_call("call_3", "echo", json!({"text": "three"})),
        ]);

        let handle = runtime.run_with_model(request, model).await.unwrap();

        assert_eq!(handle.agent_run.status, AgentRunStatus::Completed);
        assert_eq!(handle.agent_run.error, None);
        assert_eq!(
            handle.output.as_ref().unwrap().stopped_reason,
            AgentStoppedReason::MaxSteps
        );
        assert!(
            handle
                .events
                .iter()
                .any(|event| matches!(event, AgentRunEvent::Completed { output } if output.stopped_reason == AgentStoppedReason::MaxSteps))
        );
        assert!(
            !handle
                .events
                .iter()
                .any(|event| matches!(event, AgentRunEvent::Failed { .. }))
        );
        let invocations = fixture
            .repo
            .tool_invocations_for_run(&handle.agent_run.id)
            .unwrap();
        assert_eq!(invocations.len(), 3);
        assert!(
            invocations
                .iter()
                .all(|invocation| invocation.status == ToolInvocationStatus::Succeeded)
        );
    }

    #[tokio::test]
    async fn prompt_error_fails_active_tool_invocations() {
        let fixture = Fixture::new("tool-failure");
        let runtime = AgentRuntime::new(fixture.repo.clone());
        let agent_run = fixture
            .repo
            .insert_agent_run(new_agent_run_input(&fixture.request()))
            .unwrap();
        let invocation = fixture
            .repo
            .insert_tool_invocation(NewToolInvocation {
                agent_run_id: agent_run.id.clone(),
                provider_step_id: None,
                status: ToolInvocationStatus::Running,
                input: ToolInvocationInput {
                    source: ToolSource::Local,
                    namespace: None,
                    tool_name: "echo".to_string(),
                    runtime_tool_name: "echo".to_string(),
                    call_id: "call_1".to_string(),
                    arguments: ToolArguments {
                        value: json!({"text": "hi"}),
                    },
                    approval_policy: ToolApprovalPolicy::Never,
                    execution_policy: ToolExecutionPolicy::Foreground,
                },
                output: None,
                error: None,
            })
            .unwrap();

        runtime
            .finalize_active_tool_invocations(
                &agent_run.id,
                &fixture.conversation.id,
                ToolInvocationStatus::Failed,
                run_error("prompt_error", "tool failed", true, None),
            )
            .unwrap();

        let invocation = fixture
            .repo
            .get_tool_invocation(&invocation.id)
            .unwrap()
            .unwrap();
        assert_eq!(invocation.status, ToolInvocationStatus::Failed);
        let error = invocation.error.as_ref().unwrap();
        assert_eq!(error.code, "prompt_error");
        assert_eq!(error.message, "tool failed");
        let invocations = fixture
            .repo
            .tool_invocations_for_run(&agent_run.id)
            .unwrap();
        assert_eq!(invocations.len(), 1);

        let tool_results = fixture
            .repo
            .conversation_items(&fixture.conversation.id)
            .unwrap()
            .into_iter()
            .filter_map(|item| match item.payload {
                ConversationItemPayload::ToolResult(result) => Some(result),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(tool_results.len(), 1);
        assert_eq!(tool_results[0].call_id, "call_1");
        assert!(tool_results[0].is_error);
    }

    #[tokio::test]
    async fn rmcp_tool_call_is_registered_and_persisted_with_source_server() {
        let fixture = Fixture::new("mcp-tool-run");
        let runtime = AgentRuntime::new(fixture.repo.clone());
        let mcp_service = start_mcp_server(vec![make_mcp_tool("mcp_echo", "Echo over MCP")]).await;
        let tools = mcp_service.peer().list_all_tools().await.unwrap();

        let mut request = fixture.request();
        McpConnector::new()
            .register_rmcp_tools(
                &mut request.tool_registry,
                "test-server",
                tools,
                mcp_service.peer().clone(),
                ToolApprovalPolicy::Never,
                ToolExecutionPolicy::Foreground,
            )
            .unwrap();
        let model = MockCompletionModel::new([
            MockTurn::tool_call("call_1", "mcp_echo", json!({"text": "hi"})),
            MockTurn::text("done"),
        ]);

        let handle = runtime.run_with_model(request, model).await.unwrap();
        let invocations = fixture
            .repo
            .tool_invocations_for_run(&handle.agent_run.id)
            .unwrap();
        assert_eq!(invocations.len(), 1);
        assert_eq!(
            invocations[0].source,
            ToolSource::Mcp {
                server_id: "test-server".to_string(),
            }
        );
        assert_eq!(invocations[0].tool_name, "mcp_echo");
        assert_eq!(invocations[0].runtime_tool_name, "mcp_echo");
        assert_eq!(invocations[0].status, ToolInvocationStatus::Succeeded);
    }

    #[tokio::test]
    async fn approval_policy_pauses_run_with_pending_decision() {
        let fixture = Fixture::new("approval");
        let runtime = AgentRuntime::new(fixture.repo.clone());
        let mut request = fixture.request();
        request
            .tool_registry
            .register_local_tool(EchoTool::new(ToolApprovalPolicy::OnRequest))
            .unwrap();
        let model = MockCompletionModel::new([MockTurn::tool_call(
            "call_1",
            "echo",
            json!({"text": "hi"}),
        )]);

        let handle = runtime.run_with_model(request, model).await.unwrap();
        assert_eq!(handle.agent_run.status, AgentRunStatus::WaitingForApproval);
        let pending = fixture.repo.pending_approval_decisions().unwrap();
        assert_eq!(pending.len(), 1);
        assert!(
            handle
                .events
                .iter()
                .any(|event| matches!(event, AgentRunEvent::ApprovalRequested { .. }))
        );
        assert!(
            !handle
                .events
                .iter()
                .any(|event| matches!(event, AgentRunEvent::Failed { .. }))
        );
        assert_eq!(handle.agent_run.error, None);
    }

    #[test]
    fn approved_approval_decision_is_unsupported_without_mutating_state() {
        let fixture = Fixture::new("approved-unsupported");
        let runtime = AgentRuntime::new(fixture.repo.clone());
        let (agent_run, invocation, approval) = insert_waiting_approval(&fixture);

        let error = runtime
            .decide_approval(
                &approval.id,
                NewApprovalDecisionOutcome::Approved {
                    decided_by: "user".to_string(),
                    reason: Some("ok".to_string()),
                },
            )
            .unwrap_err();
        assert!(matches!(
            error,
            AgentRuntimeError::Unsupported(message)
                if message == "approved tool resume is not implemented in v1"
        ));

        let approval = fixture
            .repo
            .get_approval_decision(&approval.id)
            .unwrap()
            .unwrap();
        assert_eq!(approval.status, ApprovalStatus::Pending);
        let invocation = fixture
            .repo
            .get_tool_invocation(&invocation.id)
            .unwrap()
            .unwrap();
        assert_eq!(invocation.status, ToolInvocationStatus::AwaitingApproval);
        let agent_run = fixture.repo.get_agent_run(&agent_run.id).unwrap().unwrap();
        assert_eq!(agent_run.status, AgentRunStatus::WaitingForApproval);
    }

    #[test]
    fn denied_approval_terminalizes_tool_and_run() {
        let fixture = Fixture::new("approval-denied");
        let runtime = AgentRuntime::new(fixture.repo.clone());
        let (agent_run, invocation, approval) = insert_waiting_approval(&fixture);

        let updated = runtime
            .decide_approval(
                &approval.id,
                NewApprovalDecisionOutcome::Denied {
                    decided_by: "user".to_string(),
                    reason: Some("not allowed".to_string()),
                },
            )
            .unwrap();
        assert_eq!(updated.status, ApprovalStatus::Denied);

        let invocation = fixture
            .repo
            .get_tool_invocation(&invocation.id)
            .unwrap()
            .unwrap();
        assert_eq!(invocation.status, ToolInvocationStatus::Denied);
        assert_eq!(invocation.error.as_ref().unwrap().code, "approval_denied");
        assert!(invocation.output.as_ref().unwrap().is_error);

        let agent_run = fixture.repo.get_agent_run(&agent_run.id).unwrap().unwrap();
        assert_eq!(agent_run.status, AgentRunStatus::Failed);
        assert_eq!(agent_run.error.as_ref().unwrap().code, "approval_denied");
        assert_eq!(
            agent_run.output.as_ref().unwrap().stopped_reason,
            AgentStoppedReason::Failed
        );

        let items = fixture
            .repo
            .conversation_items(&fixture.conversation.id)
            .unwrap();
        assert!(items.iter().any(|item| {
            matches!(
                item.payload,
                ConversationItemPayload::ApprovalDecision(ApprovalDecisionItem { .. })
            )
        }));
        assert!(items.iter().any(|item| {
            matches!(
                &item.payload,
                ConversationItemPayload::ToolResult(result)
                    if result.call_id == "call_approval"
                        && result.is_error
                        && result.content[0].search_text() == Some("not allowed")
            )
        }));
    }

    #[test]
    fn canceled_approval_terminalizes_tool_and_run() {
        let fixture = Fixture::new("approval-canceled");
        let runtime = AgentRuntime::new(fixture.repo.clone());
        let (agent_run, invocation, approval) = insert_waiting_approval(&fixture);

        let updated = runtime
            .decide_approval(&approval.id, NewApprovalDecisionOutcome::Canceled)
            .unwrap();
        assert_eq!(updated.status, ApprovalStatus::Canceled);

        let invocation = fixture
            .repo
            .get_tool_invocation(&invocation.id)
            .unwrap()
            .unwrap();
        assert_eq!(invocation.status, ToolInvocationStatus::Canceled);
        assert_eq!(invocation.error.as_ref().unwrap().code, "approval_canceled");
        let agent_run = fixture.repo.get_agent_run(&agent_run.id).unwrap().unwrap();
        assert_eq!(agent_run.status, AgentRunStatus::Canceled);
        assert_eq!(agent_run.error, None);
        assert_eq!(
            agent_run.output.as_ref().unwrap().stopped_reason,
            AgentStoppedReason::Canceled
        );
        assert_eq!(
            tool_result_texts(&fixture),
            vec!["Tool approval canceled".to_string()]
        );
    }

    #[test]
    fn expired_approval_terminalizes_tool_and_run() {
        let fixture = Fixture::new("approval-expired");
        let runtime = AgentRuntime::new(fixture.repo.clone());
        let (agent_run, invocation, approval) = insert_waiting_approval(&fixture);

        let updated = runtime
            .decide_approval(&approval.id, NewApprovalDecisionOutcome::Expired)
            .unwrap();
        assert_eq!(updated.status, ApprovalStatus::Expired);

        let invocation = fixture
            .repo
            .get_tool_invocation(&invocation.id)
            .unwrap()
            .unwrap();
        assert_eq!(invocation.status, ToolInvocationStatus::Failed);
        assert_eq!(invocation.error.as_ref().unwrap().code, "approval_expired");
        let agent_run = fixture.repo.get_agent_run(&agent_run.id).unwrap().unwrap();
        assert_eq!(agent_run.status, AgentRunStatus::Failed);
        assert_eq!(agent_run.error.as_ref().unwrap().code, "approval_expired");
        assert_eq!(
            tool_result_texts(&fixture),
            vec!["Tool approval expired".to_string()]
        );
    }

    #[test]
    fn recovery_fails_active_child_execution_rows() {
        let fixture = Fixture::new("recovery-children");
        let runtime = AgentRuntime::new(fixture.repo.clone());
        let agent_run = insert_agent_run_with_status(&fixture, AgentRunStatus::Running);
        let provider_step =
            insert_provider_step(&fixture, &agent_run.id, ProviderStepStatus::Running);
        let invocation = insert_tool_invocation(
            &fixture,
            &agent_run.id,
            Some(provider_step.id.clone()),
            ToolInvocationStatus::Running,
        );

        let recovered = runtime.recover_interrupted_runs().unwrap();
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].status, AgentRunStatus::Failed);
        assert_eq!(recovered[0].error.as_ref().unwrap().code, "interrupted");

        let provider_step = fixture
            .repo
            .get_provider_step(&provider_step.id)
            .unwrap()
            .unwrap();
        assert_eq!(provider_step.status, ProviderStepStatus::Failed);
        assert_eq!(provider_step.error.as_ref().unwrap().code, "interrupted");
        let invocation = fixture
            .repo
            .get_tool_invocation(&invocation.id)
            .unwrap()
            .unwrap();
        assert_eq!(invocation.status, ToolInvocationStatus::Failed);
        assert_eq!(invocation.error.as_ref().unwrap().code, "interrupted");
        assert_eq!(
            tool_result_texts(&fixture),
            vec!["agent run was interrupted before reaching a terminal state".to_string()]
        );
    }

    #[test]
    fn recovery_keeps_waiting_for_approval_runs_resumable() {
        let fixture = Fixture::new("recovery-waiting");
        let runtime = AgentRuntime::new(fixture.repo.clone());
        let (agent_run, invocation, approval) = insert_waiting_approval(&fixture);

        let recovered = runtime.recover_interrupted_runs().unwrap();
        assert!(recovered.is_empty());

        let agent_run = fixture.repo.get_agent_run(&agent_run.id).unwrap().unwrap();
        assert_eq!(agent_run.status, AgentRunStatus::WaitingForApproval);
        let invocation = fixture
            .repo
            .get_tool_invocation(&invocation.id)
            .unwrap()
            .unwrap();
        assert_eq!(invocation.status, ToolInvocationStatus::AwaitingApproval);
        let approval = fixture
            .repo
            .get_approval_decision(&approval.id)
            .unwrap()
            .unwrap();
        assert_eq!(approval.status, ApprovalStatus::Pending);
        assert!(tool_result_texts(&fixture).is_empty());
    }

    #[test]
    fn cancel_running_run_terminalizes_active_children_without_run_error() {
        let fixture = Fixture::new("cancel-running");
        let runtime = AgentRuntime::new(fixture.repo.clone());
        let agent_run = insert_agent_run_with_status(&fixture, AgentRunStatus::Running);
        let provider_step =
            insert_provider_step(&fixture, &agent_run.id, ProviderStepStatus::Running);
        let invocation = insert_tool_invocation(
            &fixture,
            &agent_run.id,
            Some(provider_step.id.clone()),
            ToolInvocationStatus::Running,
        );
        let assistant_item = fixture
            .repo
            .append_conversation_item(NewConversationItem {
                conversation_id: fixture.conversation.id.clone(),
                status: ConversationItemStatus::Completed,
                agent_run_id: Some(agent_run.id.clone()),
                provider_step_id: Some(provider_step.id.clone()),
                tool_invocation_id: None,
                provider_item_id: None,
                payload: ConversationItemPayload::Message {
                    role: TranscriptRole::Assistant,
                    content: vec![ContentPart::Text {
                        text: "partial answer".to_string(),
                    }],
                },
            })
            .unwrap();
        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        let observer = AgentRuntimeObserver::new({
            let events = events.clone();
            move |event| {
                events.lock().unwrap().push(event);
            }
        });

        let canceled = runtime
            .cancel_run(&agent_run.id, Some(&observer))
            .unwrap()
            .unwrap();

        assert_eq!(canceled.status, AgentRunStatus::Canceled);
        assert!(canceled.error.is_none());
        assert_eq!(
            canceled.output.as_ref().unwrap().stopped_reason,
            AgentStoppedReason::Canceled
        );
        assert_eq!(
            canceled.output.as_ref().unwrap().final_item_id.as_deref(),
            Some(assistant_item.id.as_str())
        );
        assert_eq!(
            *events.lock().unwrap(),
            vec![AgentRuntimeEvent::AgentRunStatusChanged {
                agent_run_id: agent_run.id.clone(),
                status: AgentRunStatus::Canceled,
            }]
        );

        let provider_step = fixture
            .repo
            .get_provider_step(&provider_step.id)
            .unwrap()
            .unwrap();
        assert_eq!(provider_step.status, ProviderStepStatus::Canceled);
        assert_eq!(provider_step.error.as_ref().unwrap().code, "canceled");
        let invocation = fixture
            .repo
            .get_tool_invocation(&invocation.id)
            .unwrap()
            .unwrap();
        assert_eq!(invocation.status, ToolInvocationStatus::Canceled);
        assert_eq!(invocation.error.as_ref().unwrap().code, "canceled");
        assert_eq!(tool_result_texts(&fixture), vec!["runtime canceled"]);
    }

    #[test]
    fn cancel_terminal_run_is_idempotent() {
        let fixture = Fixture::new("cancel-terminal");
        let runtime = AgentRuntime::new(fixture.repo.clone());
        let agent_run = insert_agent_run_with_status(&fixture, AgentRunStatus::Completed);
        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        let observer = AgentRuntimeObserver::new({
            let events = events.clone();
            move |event| {
                events.lock().unwrap().push(event);
            }
        });

        let unchanged = runtime
            .cancel_run(&agent_run.id, Some(&observer))
            .unwrap()
            .unwrap();

        assert_eq!(unchanged.status, AgentRunStatus::Completed);
        assert!(events.lock().unwrap().is_empty());
        assert!(tool_result_texts(&fixture).is_empty());
    }

    #[tokio::test]
    async fn setup_failure_marks_agent_run_failed() {
        let fixture = Fixture::new("setup-failure");
        let runtime = AgentRuntime::new(fixture.repo.clone());
        let mut request = fixture.request();
        request.project_root = Some(fixture.dir.path().to_path_buf());
        request.skill_requests = vec![crate::SkillActivationRequest::new("missing-skill")];

        let error = runtime
            .run_with_model(request, MockCompletionModel::text("unused"))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("missing-skill"));

        assert!(
            fixture
                .repo
                .agent_runs_by_status(AgentRunStatus::Running)
                .unwrap()
                .is_empty()
        );
        let failed = fixture
            .repo
            .agent_runs_by_status(AgentRunStatus::Failed)
            .unwrap();
        assert_eq!(failed.len(), 1);
        let payload = failed[0].error.as_ref().unwrap();
        assert_eq!(payload.code, "setup_error");
        assert!(payload.message.contains("missing-skill"));
        assert!(payload.retryable);
    }

    #[tokio::test]
    async fn skill_activation_is_persisted_as_snapshot() {
        let fixture = Fixture::new("skills");
        let skill_dir = fixture.dir.path().join(".agents/skills/rust");
        std::fs::create_dir_all(&skill_dir).unwrap();
        let skill_file = skill_dir.join("SKILL.md");
        std::fs::write(
            &skill_file,
            "---\nname: rust\ndescription: Rust workflow\n---\nUse cargo test.\n",
        )
        .unwrap();

        let runtime = AgentRuntime::new(fixture.repo.clone());
        let mut request = fixture.request();
        request.project_root = Some(fixture.dir.path().to_path_buf());
        request.skill_requests = vec![crate::SkillActivationRequest::new("rust")];
        let model = MockCompletionModel::text("ok");
        let handle = runtime
            .run_with_model(request, model.clone())
            .await
            .unwrap();
        std::fs::write(&skill_file, "---\nname: rust\n---\nUse cargo clippy.\n").unwrap();

        let items = fixture
            .repo
            .conversation_items(&fixture.conversation.id)
            .unwrap();
        let skill = items
            .iter()
            .find_map(|item| match &item.payload {
                ConversationItemPayload::SkillActivation(skill) => Some(skill),
                _ => None,
            })
            .unwrap();
        assert_eq!(skill.name, "rust");
        assert_eq!(
            skill.content,
            vec![ContentPart::Text {
                text: "---\nname: rust\ndescription: Rust workflow\n---\nUse cargo test.\n"
                    .to_string(),
            }]
        );
        let skill_item = items
            .iter()
            .find(|item| matches!(item.payload, ConversationItemPayload::SkillActivation(_)))
            .unwrap();
        let provider_steps = fixture
            .repo
            .provider_steps_for_run(&handle.agent_run.id)
            .unwrap();
        assert_eq!(provider_steps.len(), 1);
        assert_eq!(
            provider_steps[0].request_snapshot.input_item_ids,
            vec![fixture.user_item.id.clone(), skill_item.id.clone()]
        );

        let requests = model.requests();
        assert_eq!(requests.len(), 1);
        let messages = requests[0].chat_history.iter().collect::<Vec<_>>();
        let last_message_text = rig_message_text(messages.last().unwrap());
        assert!(last_message_text.starts_with("hello\n\n<skill>\n<name>rust</name>"));
        assert!(last_message_text.contains("Use cargo test."));
        assert!(
            messages[..messages.len() - 1]
                .iter()
                .all(|message| !rig_message_text(message).contains("<skill>"))
        );
    }

    #[tokio::test]
    async fn tool_history_replay_preserves_provider_call_ids() {
        let fixture = Fixture::new("tool-history");
        fixture
            .repo
            .append_conversation_item(NewConversationItem {
                conversation_id: fixture.conversation.id.clone(),
                status: ConversationItemStatus::Completed,
                agent_run_id: None,
                provider_step_id: None,
                tool_invocation_id: None,
                provider_item_id: None,
                payload: ConversationItemPayload::ToolCall(ToolCallItem {
                    tool_invocation_id: None,
                    call_id: "call_previous".to_string(),
                    source: ToolSource::Local,
                    name: "echo".to_string(),
                    runtime_tool_name: "echo".to_string(),
                    arguments: ToolArguments {
                        value: json!({"text": "hi"}),
                    },
                }),
            })
            .unwrap();
        fixture
            .repo
            .append_conversation_item(NewConversationItem {
                conversation_id: fixture.conversation.id.clone(),
                status: ConversationItemStatus::Completed,
                agent_run_id: None,
                provider_step_id: None,
                tool_invocation_id: None,
                provider_item_id: None,
                payload: ConversationItemPayload::ToolResult(ToolResultItem {
                    tool_invocation_id: None,
                    call_id: "call_previous".to_string(),
                    content: vec![ContentPart::Text {
                        text: "hi".to_string(),
                    }],
                    is_error: false,
                    structured_output: None,
                    raw_output: None,
                }),
            })
            .unwrap();
        let next_user_item = fixture
            .repo
            .append_conversation_item(NewConversationItem {
                conversation_id: fixture.conversation.id.clone(),
                status: ConversationItemStatus::Completed,
                agent_run_id: None,
                provider_step_id: None,
                tool_invocation_id: None,
                provider_item_id: None,
                payload: ConversationItemPayload::Message {
                    role: TranscriptRole::User,
                    content: vec![ContentPart::Text {
                        text: "continue".to_string(),
                    }],
                },
            })
            .unwrap();
        let runtime = AgentRuntime::new(fixture.repo.clone());
        let mut request = fixture.request();
        request.user_item_id = next_user_item.id;
        let model = MockCompletionModel::text("ok");

        runtime
            .run_with_model(request, model.clone())
            .await
            .unwrap();

        let requests = model.requests();
        assert_eq!(requests.len(), 1);
        let messages = requests[0].chat_history.iter().collect::<Vec<_>>();
        let tool_call = messages
            .iter()
            .find_map(|message| match message {
                RigMessage::Assistant { content, .. } => {
                    content.iter().find_map(|content| match content {
                        AssistantContent::ToolCall(call) => Some(call),
                        _ => None,
                    })
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(tool_call.id, "call_previous");
        assert_eq!(tool_call.call_id.as_deref(), Some("call_previous"));

        let tool_result = messages
            .iter()
            .find_map(|message| match message {
                RigMessage::User { content } => content.iter().find_map(|content| match content {
                    UserContent::ToolResult(result) => Some(result),
                    _ => None,
                }),
                _ => None,
            })
            .unwrap();
        assert_eq!(tool_result.id, "call_previous");
        assert_eq!(tool_result.call_id.as_deref(), Some("call_previous"));
    }

    fn insert_waiting_approval(
        fixture: &Fixture,
    ) -> (AgentRunRecord, ToolInvocationRecord, ApprovalDecisionRecord) {
        let agent_run = insert_agent_run_with_status(fixture, AgentRunStatus::WaitingForApproval);
        let invocation = insert_tool_invocation(
            fixture,
            &agent_run.id,
            None,
            ToolInvocationStatus::AwaitingApproval,
        );
        let approval = fixture
            .repo
            .insert_approval_decision(NewApprovalDecision {
                tool_invocation_id: invocation.id.clone(),
                request: ApprovalRequestPayload {
                    reason: "approve echo".to_string(),
                    tool_source: ToolSource::Local,
                    tool_name: "echo".to_string(),
                    arguments_preview: "{\"text\":\"hi\"}".to_string(),
                },
                outcome: NewApprovalDecisionOutcome::Pending { expires_at: None },
            })
            .unwrap();
        (agent_run, invocation, approval)
    }

    fn insert_agent_run_with_status(fixture: &Fixture, status: AgentRunStatus) -> AgentRunRecord {
        let agent_run = fixture
            .repo
            .insert_agent_run(new_agent_run_input(&fixture.request()))
            .unwrap();
        fixture
            .repo
            .update_agent_run_status(
                &agent_run.id,
                UpdateAgentRunStatus {
                    status,
                    output: None,
                    error: None,
                },
            )
            .unwrap()
    }

    fn insert_provider_step(
        fixture: &Fixture,
        agent_run_id: &str,
        status: ProviderStepStatus,
    ) -> ProviderStepRecord {
        fixture
            .repo
            .insert_provider_step(NewProviderStep {
                agent_run_id: agent_run_id.to_string(),
                seq: fixture.repo.next_provider_step_seq(agent_run_id).unwrap(),
                status,
                request_snapshot: ProviderStepRequestSnapshot {
                    provider_id: fixture.provider.id.clone(),
                    model_id: fixture.model.model_id.clone(),
                    input_item_ids: vec![fixture.user_item.id.clone()],
                    snapshot_kind: ProviderStepSnapshotKind::RigCompletionRequest,
                    request_body: ProviderRawPayload {
                        provider_kind: "test".to_string(),
                        value: json!({"messages": ["hello"]}),
                    },
                },
                response_snapshot: None,
                state_snapshot: None,
                settings_snapshot: run_settings(&fixture.provider.id, &fixture.model.model_id),
                error: None,
            })
            .unwrap()
    }

    fn insert_tool_invocation(
        fixture: &Fixture,
        agent_run_id: &str,
        provider_step_id: Option<ProviderStepId>,
        status: ToolInvocationStatus,
    ) -> ToolInvocationRecord {
        fixture
            .repo
            .insert_tool_invocation(NewToolInvocation {
                agent_run_id: agent_run_id.to_string(),
                provider_step_id,
                status,
                input: ToolInvocationInput {
                    source: ToolSource::Local,
                    namespace: None,
                    tool_name: "echo".to_string(),
                    runtime_tool_name: "echo".to_string(),
                    call_id: "call_approval".to_string(),
                    arguments: ToolArguments {
                        value: json!({"text": "hi"}),
                    },
                    approval_policy: ToolApprovalPolicy::OnRequest,
                    execution_policy: ToolExecutionPolicy::Foreground,
                },
                output: None,
                error: None,
            })
            .unwrap()
    }

    fn tool_result_texts(fixture: &Fixture) -> Vec<String> {
        fixture
            .repo
            .conversation_items(&fixture.conversation.id)
            .unwrap()
            .into_iter()
            .filter_map(|item| match item.payload {
                ConversationItemPayload::ToolResult(result) => {
                    Some(result.content.into_iter().filter_map(|part| match part {
                        ContentPart::Text { text } => Some(text),
                        _ => None,
                    }))
                }
                _ => None,
            })
            .flatten()
            .collect()
    }

    struct Fixture {
        dir: TempDir,
        repo: FreshRepository,
        conversation: ConversationRecord,
        provider: ProviderRecord,
        model: ProviderModelRecord,
        user_item: ConversationItemRecord,
    }

    impl Fixture {
        fn new(name: &str) -> Self {
            let dir = tempfile::tempdir().unwrap();
            let store = FreshStore::open_in_dir(dir.path()).unwrap();
            let repo = store.repository();
            let project = repo
                .insert_project(NewProject {
                    path: dir.path().to_string_lossy().to_string(),
                    display_name: name.to_string(),
                    kind: ProjectKind::Normal,
                    pinned: false,
                    removed: false,
                    metadata: ProjectMetadata {
                        scratch_reason: None,
                        git_root: Some(dir.path().to_string_lossy().to_string()),
                        last_active_conversation_id: None,
                    },
                })
                .unwrap();
            let provider = repo
                .insert_provider(NewProvider {
                    kind: "openai".to_string(),
                    display_name: "OpenAI".to_string(),
                    enabled: true,
                    settings: provider_settings(),
                    secret_refs: ProviderSecretRefs { refs: Vec::new() },
                })
                .unwrap();
            let model = repo
                .upsert_provider_model(NewProviderModel {
                    provider_id: provider.id.clone(),
                    model_id: "gpt-5.2".to_string(),
                    display_name: Some("GPT-5.2".to_string()),
                    enabled: true,
                    capabilities: model_capabilities(),
                    metadata: ProviderModelMetadata {
                        display_name: Some("GPT-5.2".to_string()),
                        family: Some("gpt".to_string()),
                        raw: None,
                    },
                })
                .unwrap();
            let conversation = repo
                .insert_conversation(NewConversation {
                    project_id: project.id,
                    title: name.to_string(),
                    pinned: false,
                    prompt_id: None,
                    default_provider_id: Some(provider.id.clone()),
                    default_model_id: Some(model.model_id.clone()),
                    metadata: ConversationMetadata {
                        summary: None,
                        tags: Vec::new(),
                    },
                    settings_snapshot: ConversationSettingsSnapshot {
                        prompt: None,
                        provider_id: Some(provider.id.clone()),
                        model_id: Some(model.model_id.clone()),
                        model_capabilities: Some(model_capabilities()),
                        tool_policy: ToolPolicySnapshot {
                            approval_policy: ToolApprovalPolicy::Never,
                            enabled_sources: vec![ToolSource::Local],
                            max_steps: 8,
                        },
                    },
                })
                .unwrap();
            let user_item = repo
                .append_conversation_item(NewConversationItem {
                    conversation_id: conversation.id.clone(),
                    status: ConversationItemStatus::Completed,
                    agent_run_id: None,
                    provider_step_id: None,
                    tool_invocation_id: None,
                    provider_item_id: None,
                    payload: ConversationItemPayload::Message {
                        role: TranscriptRole::User,
                        content: vec![ContentPart::Text {
                            text: "hello".to_string(),
                        }],
                    },
                })
                .unwrap();
            Self {
                dir,
                repo,
                conversation,
                provider,
                model,
                user_item,
            }
        }

        fn request(&self) -> AgentRunRequest {
            AgentRunRequest::new(
                self.conversation.id.clone(),
                self.user_item.id.clone(),
                self.provider.id.clone(),
                self.model.model_id.clone(),
                run_settings(&self.provider.id, &self.model.model_id),
                AgentRuntimeSnapshot {
                    engine: AgentEngineKind::Rig,
                    engine_version: "0.37.0".to_string(),
                    skill_catalog_hash: None,
                    mcp_config_hash: None,
                    tool_name_strategy: ToolNameStrategy::Namespaced,
                },
            )
        }
    }

    #[derive(Clone)]
    struct EchoTool {
        approval_policy: ToolApprovalPolicy,
    }

    impl EchoTool {
        fn new(approval_policy: ToolApprovalPolicy) -> Self {
            Self { approval_policy }
        }
    }

    #[async_trait]
    impl ToolExecutor for EchoTool {
        async fn execute(&self, arguments: serde_json::Value) -> Result<ToolInvocationOutput> {
            Ok(ToolInvocationOutput {
                content: vec![ContentPart::Text {
                    text: arguments
                        .get("text")
                        .and_then(|value| value.as_str())
                        .unwrap_or("")
                        .to_string(),
                }],
                structured_output: Some(StructuredOutput { value: arguments }),
                raw_output: None,
                is_error: false,
            })
        }
    }

    #[async_trait]
    impl LocalTool for EchoTool {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition {
                source: ToolSource::Local,
                namespace: None,
                name: "echo".to_string(),
                description: "Echo text".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "text": { "type": "string" }
                    }
                }),
                policy: ToolRunPolicy {
                    approval_policy: self.approval_policy,
                    execution_policy: ToolExecutionPolicy::Foreground,
                    timeout_ms: None,
                },
            }
        }
    }

    #[derive(Clone)]
    struct ErrorTool;

    #[async_trait]
    impl ToolExecutor for ErrorTool {
        async fn execute(&self, arguments: serde_json::Value) -> Result<ToolInvocationOutput> {
            Ok(ToolInvocationOutput {
                content: vec![ContentPart::Text {
                    text: "human readable error".to_string(),
                }],
                structured_output: Some(StructuredOutput { value: arguments }),
                raw_output: Some(ProviderRawPayload {
                    provider_kind: "test".to_string(),
                    value: json!({"raw": "details"}),
                }),
                is_error: true,
            })
        }
    }

    #[async_trait]
    impl LocalTool for ErrorTool {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition {
                source: ToolSource::Local,
                namespace: None,
                name: "error_tool".to_string(),
                description: "Return an error output".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "code": { "type": "string" }
                    }
                }),
                policy: ToolRunPolicy {
                    approval_policy: ToolApprovalPolicy::Never,
                    execution_policy: ToolExecutionPolicy::Foreground,
                    timeout_ms: None,
                },
            }
        }
    }

    fn run_settings(provider_id: &str, model_id: &str) -> RunSettingsSnapshot {
        RunSettingsSnapshot {
            prompt: Some(PromptContent {
                text: "You are useful.".to_string(),
            }),
            provider_id: provider_id.to_string(),
            model_id: model_id.to_string(),
            model_capabilities: model_capabilities(),
            provider_settings: provider_settings(),
            reasoning_selection: None,
            tool_policy: ToolPolicySnapshot {
                approval_policy: ToolApprovalPolicy::Never,
                enabled_sources: vec![ToolSource::Local],
                max_steps: 8,
            },
        }
    }

    fn provider_settings() -> ProviderSettingsPayload {
        ProviderSettingsPayload {
            provider_kind: "openai".to_string(),
            fields: Vec::new(),
        }
    }

    fn model_capabilities() -> ModelCapabilitiesSnapshot {
        ModelCapabilitiesSnapshot {
            text_input: true,
            text_output: true,
            streaming: true,
            image_input: None,
            file_input: None,
            audio_input: false,
            image_generation: false,
            tool_calling: Some(ToolCallingCapabilitySnapshot {
                parallel_tool_calls: true,
            }),
            hosted_web_search: true,
            remote_mcp: false,
            reasoning: None,
            structured_output: true,
            stateful_response_continuation: true,
            extension: ProviderCapabilityExtensionSnapshot::OpenAi {
                responses_api: true,
                raw: None,
            },
        }
    }

    fn rig_message_text(message: &RigMessage) -> String {
        match message {
            RigMessage::System { content } => content.clone(),
            RigMessage::User { content } => content
                .iter()
                .filter_map(|content| match content {
                    UserContent::Text(text) => Some(text.text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n"),
            RigMessage::Assistant { .. } => String::new(),
        }
    }

    #[derive(Clone)]
    struct DynamicMcpServer {
        tools: Arc<RwLock<Vec<Tool>>>,
    }

    impl DynamicMcpServer {
        fn new(tools: Vec<Tool>) -> Self {
            Self {
                tools: Arc::new(RwLock::new(tools)),
            }
        }
    }

    impl ServerHandler for DynamicMcpServer {
        fn get_info(&self) -> ServerInfo {
            ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
                .with_protocol_version(ProtocolVersion::LATEST)
                .with_server_info(Implementation::new("ai-chat-agent-test", "0.1.0"))
        }

        async fn list_tools(
            &self,
            _request: Option<PaginatedRequestParams>,
            _context: RequestContext<RoleServer>,
        ) -> std::result::Result<ListToolsResult, ErrorData> {
            Ok(ListToolsResult::with_all_items(
                self.tools.read().await.clone(),
            ))
        }

        async fn call_tool(
            &self,
            request: CallToolRequestParams,
            _context: RequestContext<RoleServer>,
        ) -> std::result::Result<CallToolResult, ErrorData> {
            Ok(CallToolResult::success(vec![Content::text(format!(
                "called {}",
                request.name
            ))]))
        }
    }

    fn make_mcp_tool(name: &str, description: &str) -> Tool {
        Tool::new(
            name.to_string(),
            description.to_string(),
            Arc::new(serde_json::Map::new()),
        )
    }

    async fn start_mcp_server(
        tools: Vec<Tool>,
    ) -> rmcp::service::RunningService<rmcp::service::RoleClient, ()> {
        let server = DynamicMcpServer::new(tools);
        let (client_to_server, server_from_client) = tokio::io::duplex(8192);
        let (server_to_client, client_from_server) = tokio::io::duplex(8192);

        tokio::spawn(async move {
            let service = server
                .serve((server_from_client, server_to_client))
                .await
                .expect("server failed to start");
            service.waiting().await.expect("server error");
        });

        ().serve((client_from_server, client_to_server))
            .await
            .expect("client failed to connect")
    }
}

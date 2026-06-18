use super::{PersistenceContext, error_tool_output, lock, mutex_clone, mutex_replace, run_error};
use crate::{
    AgentRuntimeEvent, AgentStep, RegisteredToolDefinition, Result,
    tool_registry::{RegisteredRuntimeTool, tool_output_to_model_text},
};
use ai_chat_core::*;
use ai_chat_db::{
    NewApprovalDecision, NewApprovalDecisionOutcome, NewConversationItem, NewToolInvocation,
    ToolInvocationRecord, UpdateAgentRunStatus, UpdateToolInvocationStatus,
};
use rig_core::{
    agent::{
        HookAction, InvalidToolCallContext, InvalidToolCallHookAction, PromptHook,
        ToolCallHookAction,
    },
    completion::{AssistantContent, CompletionModel, CompletionResponse},
};
use tokio::time::timeout;

impl PersistenceContext {
    pub(super) async fn execute_tool_invocation(
        &self,
        tool: RegisteredRuntimeTool,
        invocation: ToolInvocationRecord,
        arguments: serde_json::Value,
    ) -> Result<String> {
        let execution = timeout(tool.timeout, tool.executor.execute(arguments)).await;
        let (output, status, error) = match execution {
            Ok(Ok(output)) => {
                let error = output.is_error.then(|| {
                    run_error("tool_error", tool_output_to_model_text(&output), true, None)
                });
                let status = if output.is_error {
                    ToolInvocationStatus::Failed
                } else {
                    ToolInvocationStatus::Succeeded
                };
                (output, status, error)
            }
            Ok(Err(error)) => {
                let payload = run_error("tool_error", error.to_string(), true, None);
                (
                    error_tool_output(payload.message.clone()),
                    ToolInvocationStatus::Failed,
                    Some(payload),
                )
            }
            Err(_) => {
                let payload = run_error("tool_timeout", "tool execution timed out", true, None);
                (
                    error_tool_output(payload.message.clone()),
                    ToolInvocationStatus::Failed,
                    Some(payload),
                )
            }
        };
        let model_text = tool_output_to_model_text(&output);
        let payload = ConversationItemPayload::ToolResult(ToolResultItem {
            tool_invocation_id: Some(invocation.id.clone()),
            call_id: invocation.call_id.clone(),
            content: output.content.clone(),
            is_error: output.is_error,
            structured_output: output.structured_output.clone(),
            raw_output: output.raw_output.clone(),
        });
        let (item, _) = self
            .repo
            .append_conversation_item_and_update_tool_invocation(
                NewConversationItem {
                    conversation_id: self.conversation_id.clone(),
                    status: ConversationItemStatus::Completed,
                    agent_run_id: Some(self.agent_run_id.clone()),
                    provider_step_id: invocation.provider_step_id.clone(),
                    tool_invocation_id: Some(invocation.id.clone()),
                    provider_item_id: None,
                    payload,
                },
                &invocation.id,
                UpdateToolInvocationStatus {
                    status,
                    output: Some(output),
                    error,
                },
            )?;
        let item_id = item.id.clone();
        self.add_input_item_id(item_id.clone());
        self.push_step(AgentStep::ConversationItem(item_id.clone()));
        self.emit_runtime(AgentRuntimeEvent::ConversationItemAppended {
            conversation_id: self.conversation_id.clone(),
            item_id,
        });
        self.emit_runtime(AgentRuntimeEvent::ToolInvocationChanged {
            agent_run_id: invocation.agent_run_id.clone(),
            tool_invocation_id: invocation.id.clone(),
        });
        self.push_event(AgentRunEvent::ToolInvocationFinished {
            tool_invocation_id: invocation.id,
        });
        Ok(model_text)
    }

    pub(super) fn append_error_tool_result_and_update_tool_invocation(
        &self,
        invocation: &ToolInvocationRecord,
        status: ToolInvocationStatus,
        error: RunErrorPayload,
    ) -> Result<String> {
        let output = error_tool_output(error.message.clone());
        let model_text = tool_output_to_model_text(&output);
        let payload = ConversationItemPayload::ToolResult(ToolResultItem {
            tool_invocation_id: Some(invocation.id.clone()),
            call_id: invocation.call_id.clone(),
            content: output.content.clone(),
            is_error: true,
            structured_output: output.structured_output.clone(),
            raw_output: output.raw_output.clone(),
        });
        let (item, _) = self
            .repo
            .append_conversation_item_and_update_tool_invocation(
                NewConversationItem {
                    conversation_id: self.conversation_id.clone(),
                    status: ConversationItemStatus::Completed,
                    agent_run_id: Some(self.agent_run_id.clone()),
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
        self.add_input_item_id(item.id.clone());
        self.push_step(AgentStep::ConversationItem(item.id.clone()));
        self.emit_runtime(AgentRuntimeEvent::ConversationItemAppended {
            conversation_id: self.conversation_id.clone(),
            item_id: item.id,
        });
        self.emit_runtime(AgentRuntimeEvent::ToolInvocationChanged {
            agent_run_id: invocation.agent_run_id.clone(),
            tool_invocation_id: invocation.id.clone(),
        });
        self.push_event(AgentRunEvent::ToolInvocationFinished {
            tool_invocation_id: invocation.id.clone(),
        });
        Ok(model_text)
    }

    pub(super) fn append_recoverable_tool_error(
        &self,
        invocation: &ToolInvocationRecord,
        status: ToolInvocationStatus,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> ToolCallHookAction {
        let error = run_error(code, message, true, None);
        match self.append_error_tool_result_and_update_tool_invocation(invocation, status, error) {
            Ok(model_text) => ToolCallHookAction::skip(model_text),
            Err(error) => ToolCallHookAction::terminate(error.to_string()),
        }
    }

    pub(super) fn append_recoverable_invalid_tool_error(
        &self,
        invocation: &ToolInvocationRecord,
        status: ToolInvocationStatus,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> InvalidToolCallHookAction {
        let error = run_error(code, message, true, None);
        match self.append_error_tool_result_and_update_tool_invocation(invocation, status, error) {
            Ok(model_text) => InvalidToolCallHookAction::skip(model_text),
            Err(_) => InvalidToolCallHookAction::fail(),
        }
    }

    pub(super) fn insert_tool_invocation_and_append_call(
        &self,
        internal_call_id: &str,
        status: ToolInvocationStatus,
        input: ToolInvocationInput,
    ) -> Result<ToolInvocationRecord> {
        let invocation = self.repo.insert_tool_invocation(NewToolInvocation {
            agent_run_id: self.agent_run_id.clone(),
            provider_step_id: mutex_clone(&self.last_provider_step_id),
            status,
            input,
            output: None,
            error: None,
        })?;

        lock(&self.tool_calls).insert(internal_call_id.to_string(), invocation.id.clone());
        self.push_event(AgentRunEvent::ToolInvocationRequested {
            tool_invocation_id: invocation.id.clone(),
        });
        self.emit_runtime(AgentRuntimeEvent::ToolInvocationChanged {
            agent_run_id: self.agent_run_id.clone(),
            tool_invocation_id: invocation.id.clone(),
        });
        self.push_step(AgentStep::ToolInvocation(invocation.id.clone()));

        let payload = ConversationItemPayload::ToolCall(ToolCallItem {
            tool_invocation_id: Some(invocation.id.clone()),
            call_id: invocation.call_id.clone(),
            source: invocation.source.clone(),
            name: invocation.tool_name.clone(),
            runtime_tool_name: invocation.runtime_tool_name.clone(),
            arguments: invocation.input.arguments.clone(),
        });
        self.append_tool_item(invocation.id.clone(), payload)?;
        Ok(invocation)
    }

    pub(super) fn request_tool_approval(
        &self,
        invocation: &ToolInvocationRecord,
        definition: &RegisteredToolDefinition,
        reason: String,
        arguments_preview: String,
        access_requests: Vec<ToolAccessRequestPayload>,
    ) -> Result<()> {
        self.repo.update_tool_invocation_status(
            &invocation.id,
            UpdateToolInvocationStatus {
                status: ToolInvocationStatus::AwaitingApproval,
                output: None,
                error: None,
            },
        )?;
        self.emit_runtime(AgentRuntimeEvent::ToolInvocationChanged {
            agent_run_id: self.agent_run_id.clone(),
            tool_invocation_id: invocation.id.clone(),
        });
        let request = ApprovalRequestPayload {
            reason,
            tool_source: definition.source.clone(),
            tool_name: definition.tool_name.clone(),
            arguments_preview,
            access_requests,
        };
        let approval = self.repo.insert_approval_decision(NewApprovalDecision {
            tool_invocation_id: invocation.id.clone(),
            request: request.clone(),
            outcome: NewApprovalDecisionOutcome::Pending { expires_at: None },
        })?;
        self.push_step(AgentStep::Approval(approval.id.clone()));
        self.push_event(AgentRunEvent::ApprovalRequested {
            approval_decision_id: approval.id.clone(),
        });
        let payload = ConversationItemPayload::ApprovalRequest(ApprovalRequestItem {
            approval_decision_id: approval.id,
            tool_invocation_id: invocation.id.clone(),
            request,
        });
        self.append_tool_item(invocation.id.clone(), payload)?;
        self.repo.update_agent_run_status(
            &self.agent_run_id,
            UpdateAgentRunStatus {
                status: AgentRunStatus::WaitingForApproval,
                output: None,
                error: None,
            },
        )?;
        self.emit_runtime(AgentRuntimeEvent::AgentRunStatusChanged {
            agent_run_id: self.agent_run_id.clone(),
            status: AgentRunStatus::WaitingForApproval,
        });
        Ok(())
    }

    pub(super) fn record_auto_approval(
        &self,
        invocation: &ToolInvocationRecord,
        definition: &RegisteredToolDefinition,
        arguments_preview: String,
        access_requests: Vec<ToolAccessRequestPayload>,
    ) -> Result<()> {
        if access_requests.is_empty() {
            return Ok(());
        }
        let approval = self.repo.insert_approval_decision(NewApprovalDecision {
            tool_invocation_id: invocation.id.clone(),
            request: ApprovalRequestPayload {
                reason: "Auto-approved by current approval mode".to_string(),
                tool_source: definition.source.clone(),
                tool_name: definition.tool_name.clone(),
                arguments_preview,
                access_requests,
            },
            outcome: NewApprovalDecisionOutcome::Approved {
                decided_by: "auto".to_string(),
                reason: Some("Auto-approved by current approval mode".to_string()),
            },
        })?;
        if let Some(decision) = approval.decision {
            let payload = ConversationItemPayload::ApprovalDecision(ApprovalDecisionItem {
                approval_decision_id: approval.id,
                decision,
            });
            self.append_tool_item(invocation.id.clone(), payload)?;
        }
        Ok(())
    }

    pub(super) fn check_tool_guard(
        &self,
        runtime_tool_name: &str,
        args: &str,
    ) -> ToolCallHookAction {
        let calls = lock(&self.tool_calls);
        if calls.len() as u32 >= self.max_tool_calls {
            return ToolCallHookAction::terminate("max tool call guard reached");
        }
        drop(calls);

        let key = format!("{runtime_tool_name}\0{args}");
        let mut repeated = lock(&self.repeated_tool_calls);
        let count = repeated.entry(key).or_insert(0);
        *count += 1;
        if *count > self.repeated_tool_call_limit {
            ToolCallHookAction::terminate(format!(
                "repeated tool call guard reached for {runtime_tool_name}"
            ))
        } else {
            ToolCallHookAction::cont()
        }
    }
}

#[derive(Clone)]
pub(crate) struct PersistingPromptHook {
    pub(super) context: PersistenceContext,
}

impl<M> PromptHook<M> for PersistingPromptHook
where
    M: CompletionModel,
{
    async fn on_completion_call(
        &self,
        _prompt: &rig_core::completion::Message,
        _history: &[rig_core::completion::Message],
    ) -> HookAction {
        if self.context.cancellation_token.is_cancelled() {
            HookAction::terminate("runtime canceled")
        } else {
            HookAction::cont()
        }
    }

    async fn on_completion_response(
        &self,
        _prompt: &rig_core::completion::Message,
        response: &CompletionResponse<M::Response>,
    ) -> HookAction {
        let provider_step_id = mutex_clone(&self.context.last_provider_step_id);
        for content in response.choice.iter() {
            let payload = match content {
                AssistantContent::Text(text) if !text.text.is_empty() => {
                    Some(ConversationItemPayload::Message {
                        role: TranscriptRole::Assistant,
                        content: vec![ContentPart::Text {
                            text: text.text.clone(),
                        }],
                    })
                }
                AssistantContent::Reasoning(reasoning) => {
                    Some(ConversationItemPayload::Reasoning {
                        text: reasoning.display_text(),
                        summary: None,
                    })
                }
                _ => None,
            };

            if let Some(payload) = payload {
                match self.context.append_item(payload.clone()) {
                    Ok(item) => {
                        if matches!(payload, ConversationItemPayload::Message { .. }) {
                            mutex_replace(&self.context.final_item_id, Some(item.id.clone()));
                        }
                        if let Some(provider_step_id) = provider_step_id.as_ref() {
                            self.context.push_event(AgentRunEvent::ProviderStepEvent {
                                provider_step_id: provider_step_id.clone(),
                                event: ProviderStepEvent::OutputItemCompleted {
                                    provider_item_id: item.provider_item_id.clone(),
                                    item: payload,
                                },
                            });
                        }
                    }
                    Err(error) => return HookAction::terminate(error.to_string()),
                }
            }
        }
        HookAction::cont()
    }

    async fn on_invalid_tool_call(
        &self,
        context: &InvalidToolCallContext,
    ) -> InvalidToolCallHookAction {
        if self.context.cancellation_token.is_cancelled() {
            return InvalidToolCallHookAction::fail();
        }

        let args = context.args.as_deref().unwrap_or("");
        let guard_action = self.context.check_tool_guard(&context.tool_name, args);
        if !matches!(guard_action, ToolCallHookAction::Continue) {
            return InvalidToolCallHookAction::fail();
        }

        let internal_call_id = context
            .internal_call_id
            .as_deref()
            .or(context.tool_call_id.as_deref())
            .unwrap_or(&context.tool_name);
        let call_id = context
            .tool_call_id
            .clone()
            .unwrap_or_else(|| internal_call_id.to_string());
        let arguments = context
            .args
            .as_deref()
            .map(|args| {
                serde_json::from_str::<serde_json::Value>(args)
                    .unwrap_or_else(|_| serde_json::Value::String(args.to_string()))
            })
            .unwrap_or(serde_json::Value::Null);
        let invocation = match self.context.insert_tool_invocation_and_append_call(
            internal_call_id,
            ToolInvocationStatus::Running,
            ToolInvocationInput {
                source: ToolSource::Local,
                namespace: None,
                tool_name: context.tool_name.clone(),
                runtime_tool_name: context.tool_name.clone(),
                call_id,
                arguments: ToolArguments { value: arguments },
                approval_policy: ToolApprovalPolicy::Never,
                execution_policy: ToolExecutionPolicy::Foreground,
            },
        ) {
            Ok(invocation) => invocation,
            Err(_) => return InvalidToolCallHookAction::fail(),
        };
        self.context.append_recoverable_invalid_tool_error(
            &invocation,
            ToolInvocationStatus::Failed,
            "tool_not_found",
            format!("No tool named {} exists", context.tool_name),
        )
    }

    async fn on_tool_call(
        &self,
        tool_name: &str,
        tool_call_id: Option<String>,
        internal_call_id: &str,
        args: &str,
    ) -> ToolCallHookAction {
        if self.context.cancellation_token.is_cancelled() {
            return ToolCallHookAction::terminate("runtime canceled");
        }
        let guard_action = self.context.check_tool_guard(tool_name, args);
        if !matches!(guard_action, ToolCallHookAction::Continue) {
            return guard_action;
        }

        let call_id = tool_call_id.unwrap_or_else(|| internal_call_id.to_string());
        let arguments = serde_json::from_str::<serde_json::Value>(args)
            .unwrap_or_else(|_| serde_json::Value::String(args.to_string()));
        let Some(definition) = self.context.tool_definitions.get(tool_name).cloned() else {
            let invocation = match self.context.insert_tool_invocation_and_append_call(
                internal_call_id,
                ToolInvocationStatus::Running,
                ToolInvocationInput {
                    source: ToolSource::Local,
                    namespace: None,
                    tool_name: tool_name.to_string(),
                    runtime_tool_name: tool_name.to_string(),
                    call_id,
                    arguments: ToolArguments { value: arguments },
                    approval_policy: ToolApprovalPolicy::Never,
                    execution_policy: ToolExecutionPolicy::Foreground,
                },
            ) {
                Ok(invocation) => invocation,
                Err(error) => return ToolCallHookAction::terminate(error.to_string()),
            };
            return self.context.append_recoverable_tool_error(
                &invocation,
                ToolInvocationStatus::Failed,
                "tool_not_found",
                format!("No tool named {tool_name} exists"),
            );
        };
        let status = if definition.policy.approval_policy == ToolApprovalPolicy::Never {
            ToolInvocationStatus::Running
        } else {
            ToolInvocationStatus::AwaitingApproval
        };
        let invocation = match self.context.insert_tool_invocation_and_append_call(
            internal_call_id,
            status,
            ToolInvocationInput {
                source: definition.source.clone(),
                namespace: definition.namespace.clone(),
                tool_name: definition.tool_name.clone(),
                runtime_tool_name: definition.runtime_tool_name.clone(),
                call_id: call_id.clone(),
                arguments: ToolArguments {
                    value: arguments.clone(),
                },
                approval_policy: definition.policy.approval_policy,
                execution_policy: definition.policy.execution_policy,
            },
        ) {
            Ok(invocation) => invocation,
            Err(error) => return ToolCallHookAction::terminate(error.to_string()),
        };

        let Some(runtime_tool) = self.context.runtime_tools.get(tool_name).cloned() else {
            return self.context.append_recoverable_tool_error(
                &invocation,
                ToolInvocationStatus::Failed,
                "tool_runtime_unavailable",
                format!("Tool {tool_name} has no runtime executor"),
            );
        };

        let builtin_access_requests = if matches!(definition.source, ToolSource::Local) {
            match crate::builtin_tools::registry::access_requests_for_builtin_tool(
                &definition.tool_name,
                &arguments,
                &self.context.settings_snapshot.tool_policy,
            ) {
                Ok(access_requests) => access_requests,
                Err(error) => {
                    return self.context.append_recoverable_tool_error(
                        &invocation,
                        ToolInvocationStatus::Failed,
                        "tool_invalid_arguments",
                        format!(
                            "Invalid arguments for tool {}: {error}",
                            definition.runtime_tool_name
                        ),
                    );
                }
            }
        } else {
            None
        };
        if let Some(access_requests) = builtin_access_requests {
            let evaluator =
                match crate::builtin_tools::approval::ToolPermissionEvaluator::from_policy(
                    &self.context.settings_snapshot.tool_policy,
                    None,
                ) {
                    Ok(evaluator) => evaluator,
                    Err(error) => return ToolCallHookAction::terminate(error.to_string()),
                };
            match evaluator.evaluate(&access_requests) {
                crate::builtin_tools::approval::ToolPermissionDecision::Allow { auto_approved } => {
                    if let Err(error) = self.context.record_auto_approval(
                        &invocation,
                        &definition,
                        args.to_string(),
                        auto_approved,
                    ) {
                        return ToolCallHookAction::terminate(error.to_string());
                    }
                }
                crate::builtin_tools::approval::ToolPermissionDecision::Ask {
                    reason,
                    access_requests,
                } => {
                    if let Err(error) = self.context.request_tool_approval(
                        &invocation,
                        &definition,
                        reason,
                        args.to_string(),
                        access_requests,
                    ) {
                        return ToolCallHookAction::terminate(error.to_string());
                    }
                    return ToolCallHookAction::terminate("tool approval required");
                }
                crate::builtin_tools::approval::ToolPermissionDecision::Deny { reason } => {
                    let error = run_error("tool_permission_denied", reason, false, None);
                    return match self
                        .context
                        .append_error_tool_result_and_update_tool_invocation(
                            &invocation,
                            ToolInvocationStatus::Denied,
                            error,
                        ) {
                        Ok(model_text) => ToolCallHookAction::skip(model_text),
                        Err(error) => ToolCallHookAction::terminate(error.to_string()),
                    };
                }
            }
        }

        if definition.policy.approval_policy != ToolApprovalPolicy::Never {
            if let Err(error) = self.context.request_tool_approval(
                &invocation,
                &definition,
                format!("Tool `{}` requires approval", definition.tool_name),
                args.to_string(),
                Vec::new(),
            ) {
                return ToolCallHookAction::terminate(error.to_string());
            }
            return ToolCallHookAction::terminate("tool approval required");
        }

        match self
            .context
            .execute_tool_invocation(runtime_tool, invocation, arguments)
            .await
        {
            Ok(model_text) => ToolCallHookAction::skip(model_text),
            Err(error) => ToolCallHookAction::terminate(error.to_string()),
        }
    }

    async fn on_tool_result(
        &self,
        _tool_name: &str,
        _tool_call_id: Option<String>,
        internal_call_id: &str,
        _args: &str,
        result: &str,
    ) -> HookAction {
        let Some(tool_invocation_id) = lock(&self.context.tool_calls)
            .get(internal_call_id)
            .cloned()
        else {
            return HookAction::terminate(format!(
                "tool result {internal_call_id} has no invocation"
            ));
        };
        let output = ToolInvocationOutput {
            content: vec![ContentPart::Text {
                text: result.to_string(),
            }],
            structured_output: serde_json::from_str::<serde_json::Value>(result)
                .ok()
                .map(|value| StructuredOutput { value }),
            raw_output: None,
            is_error: false,
        };
        let invocation = match self.context.repo.update_tool_invocation_status(
            &tool_invocation_id,
            UpdateToolInvocationStatus {
                status: ToolInvocationStatus::Succeeded,
                output: Some(output.clone()),
                error: None,
            },
        ) {
            Ok(invocation) => invocation,
            Err(error) => return HookAction::terminate(error.to_string()),
        };
        let payload = ConversationItemPayload::ToolResult(ToolResultItem {
            tool_invocation_id: Some(tool_invocation_id.clone()),
            call_id: invocation.call_id,
            content: output.content.clone(),
            is_error: output.is_error,
            structured_output: output.structured_output.clone(),
            raw_output: output.raw_output.clone(),
        });
        if let Err(error) = self
            .context
            .append_tool_item(tool_invocation_id.clone(), payload)
        {
            return HookAction::terminate(error.to_string());
        }
        self.context
            .push_event(AgentRunEvent::ToolInvocationFinished { tool_invocation_id });
        self.context
            .emit_runtime(AgentRuntimeEvent::ToolInvocationChanged {
                agent_run_id: self.context.agent_run_id.clone(),
                tool_invocation_id: invocation.id,
            });
        HookAction::cont()
    }
}

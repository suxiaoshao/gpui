use super::{PersistenceContext, error_tool_output, lock, mutex_clone, mutex_replace, run_error};
use crate::{
    AgentRuntimeError, AgentRuntimeEvent, AgentStep, RegisteredToolDefinition, Result,
    ToolApprovalDecision, ToolApprovalRequest,
    tool_registry::{RegisteredRuntimeTool, tool_output_to_model_text},
};
use jaco_core::*;
use jaco_db::{
    NewConversationEntry, NewToolInvocation, ToolInvocationApproval, ToolInvocationApprovalOutcome,
    ToolInvocationRecord, UpdateToolInvocationStatus,
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
        let execution = tokio::select! {
            biased;
            _ = self.cancellation_token.cancelled() => {
                return Err(AgentRuntimeError::Canceled);
            }
            execution = timeout(tool.timeout, tool.executor.execute(arguments)) => execution,
        };
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
        if self.cancellation_token.is_cancelled() {
            return Err(AgentRuntimeError::Canceled);
        }
        let model_text = tool_output_to_model_text(&output);
        let payload = ConversationEntryPayload::ToolResult(ToolResultEntry {
            tool_invocation_id: Some(invocation.id.clone()),
            call_id: invocation.call_id.clone(),
            content: output.content.clone(),
            is_error: output.is_error,
            structured_output: output.structured_output.clone(),
            raw_output: output.raw_output.clone(),
        });
        let (item, _) = self
            .repo
            .append_conversation_entry_and_update_tool_invocation(
                NewConversationEntry {
                    conversation_id: self.conversation_id.clone(),
                    status: ConversationEntryStatus::Completed,
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
        self.push_step(AgentStep::ConversationEntry(item_id.clone()));
        self.emit_runtime(AgentRuntimeEvent::ConversationEntryAppended {
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
        let payload = ConversationEntryPayload::ToolResult(ToolResultEntry {
            tool_invocation_id: Some(invocation.id.clone()),
            call_id: invocation.call_id.clone(),
            content: output.content.clone(),
            is_error: true,
            structured_output: output.structured_output.clone(),
            raw_output: output.raw_output.clone(),
        });
        let (item, _) = self
            .repo
            .append_conversation_entry_and_update_tool_invocation(
                NewConversationEntry {
                    conversation_id: self.conversation_id.clone(),
                    status: ConversationEntryStatus::Completed,
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
        self.push_step(AgentStep::ConversationEntry(item.id.clone()));
        self.emit_runtime(AgentRuntimeEvent::ConversationEntryAppended {
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

        let payload = ConversationEntryPayload::ToolCall(ToolCallEntry {
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

    pub(super) fn record_tool_approval_request(
        &self,
        invocation: &ToolInvocationRecord,
        definition: &RegisteredToolDefinition,
        reason: String,
        arguments_preview: String,
        access_requests: Vec<ToolAccessRequestPayload>,
    ) -> Result<(ToolInvocationRecord, ToolApprovalRequest)> {
        let request = ApprovalRequestPayload {
            reason,
            tool_source: definition.source.clone(),
            tool_name: definition.tool_name.clone(),
            arguments_preview,
            access_requests,
        };
        let entry = NewConversationEntry {
            conversation_id: self.conversation_id.clone(),
            status: ConversationEntryStatus::WaitingForApproval,
            agent_run_id: Some(self.agent_run_id.clone()),
            provider_step_id: invocation.provider_step_id.clone(),
            tool_invocation_id: Some(invocation.id.clone()),
            provider_item_id: None,
            payload: ConversationEntryPayload::ApprovalRequest(ApprovalRequestEntry {
                tool_invocation_id: invocation.id.clone(),
                request: request.clone(),
            }),
        };
        let (entry, invocation) = self.repo.request_tool_invocation_approval_with_entry(
            &invocation.id,
            jaco_db::NewToolInvocationApproval {
                request: request.clone(),
                expires_at: None,
            },
            entry,
        )?;
        self.record_persisted_entries(std::slice::from_ref(&entry));
        self.emit_runtime(AgentRuntimeEvent::ToolInvocationChanged {
            agent_run_id: self.agent_run_id.clone(),
            tool_invocation_id: invocation.id.clone(),
        });
        self.push_event(AgentRunEvent::ApprovalRequested {
            tool_invocation_id: invocation.id.clone(),
        });
        let tool_invocation_id = invocation.id.clone();
        Ok((
            invocation,
            ToolApprovalRequest {
                conversation_id: self.conversation_id.clone(),
                agent_run_id: self.agent_run_id.clone(),
                tool_invocation_id,
                request,
            },
        ))
    }

    pub(super) async fn await_tool_approval(
        &self,
        request: ToolApprovalRequest,
    ) -> ToolApprovalDecision {
        let Some(broker) = self.approval_broker.clone() else {
            return ToolApprovalDecision::Denied {
                decided_by: "system".to_string(),
                reason: Some("approval broker unavailable".to_string()),
            };
        };
        let agent_run_id = request.agent_run_id.clone();
        let tool_invocation_id = request.tool_invocation_id.clone();
        let decision = broker.request_tool_approval(request);
        self.emit_runtime(AgentRuntimeEvent::ToolApprovalRequested {
            agent_run_id,
            tool_invocation_id,
        });
        tokio::select! {
            biased;
            _ = self.cancellation_token.cancelled() => ToolApprovalDecision::Canceled,
            decision = decision => decision,
        }
    }

    pub(super) fn approve_tool_invocation(
        &self,
        invocation: &ToolInvocationRecord,
        decided_by: String,
        reason: Option<String>,
    ) -> Result<ToolInvocationRecord> {
        let decision = ApprovalDecisionPayload {
            approved: true,
            decided_by: decided_by.clone(),
            reason: reason.clone(),
        };
        let entry = NewConversationEntry {
            conversation_id: self.conversation_id.clone(),
            status: ConversationEntryStatus::Completed,
            agent_run_id: Some(self.agent_run_id.clone()),
            provider_step_id: invocation.provider_step_id.clone(),
            tool_invocation_id: Some(invocation.id.clone()),
            provider_item_id: None,
            payload: ConversationEntryPayload::ApprovalDecision(ApprovalDecisionEntry {
                tool_invocation_id: invocation.id.clone(),
                decision,
            }),
        };
        let (entry, invocation) = self.repo.decide_tool_invocation_approval_with_entry(
            &invocation.id,
            ToolInvocationApprovalOutcome::Approved { decided_by, reason },
            ToolInvocationStatus::Running,
            entry,
        )?;
        self.record_persisted_entries(std::slice::from_ref(&entry));
        self.emit_runtime(AgentRuntimeEvent::ToolInvocationChanged {
            agent_run_id: invocation.agent_run_id.clone(),
            tool_invocation_id: invocation.id.clone(),
        });
        Ok(invocation)
    }

    pub(super) fn append_denied_tool_approval_result(
        &self,
        invocation: &ToolInvocationRecord,
        decided_by: String,
        reason: Option<String>,
    ) -> Result<String> {
        let message = reason
            .clone()
            .unwrap_or_else(|| "tool approval denied".to_string());
        let error = run_error("tool_approval_denied", message, false, None);
        let approval = approval_after_outcome(
            invocation,
            ApprovalStatus::Denied,
            Some(ApprovalDecisionPayload {
                approved: false,
                decided_by: decided_by.clone(),
                reason: reason.clone(),
            }),
        )?;
        let decision = ApprovalDecisionPayload {
            approved: false,
            decided_by,
            reason,
        };
        self.append_denied_or_canceled_tool_approval_result(
            invocation,
            ToolInvocationStatus::Denied,
            error,
            approval,
            Some(decision),
        )
    }

    pub(super) fn append_canceled_tool_approval_result(
        &self,
        invocation: &ToolInvocationRecord,
    ) -> Result<String> {
        let error = run_error(
            "tool_approval_canceled",
            "tool approval was canceled",
            false,
            None,
        );
        let approval = approval_after_outcome(invocation, ApprovalStatus::Canceled, None)?;
        self.append_denied_or_canceled_tool_approval_result(
            invocation,
            ToolInvocationStatus::Canceled,
            error,
            approval,
            None,
        )
    }

    fn append_denied_or_canceled_tool_approval_result(
        &self,
        invocation: &ToolInvocationRecord,
        status: ToolInvocationStatus,
        error: RunErrorPayload,
        approval: ToolInvocationApproval,
        decision: Option<ApprovalDecisionPayload>,
    ) -> Result<String> {
        let output = error_tool_output(error.message.clone());
        let model_text = tool_output_to_model_text(&output);
        let mut entries = Vec::new();
        if let Some(decision) = decision {
            entries.push(NewConversationEntry {
                conversation_id: self.conversation_id.clone(),
                status: ConversationEntryStatus::Completed,
                agent_run_id: Some(self.agent_run_id.clone()),
                provider_step_id: invocation.provider_step_id.clone(),
                tool_invocation_id: Some(invocation.id.clone()),
                provider_item_id: None,
                payload: ConversationEntryPayload::ApprovalDecision(ApprovalDecisionEntry {
                    tool_invocation_id: invocation.id.clone(),
                    decision,
                }),
            });
        }
        entries.push(NewConversationEntry {
            conversation_id: self.conversation_id.clone(),
            status: ConversationEntryStatus::Completed,
            agent_run_id: Some(self.agent_run_id.clone()),
            provider_step_id: invocation.provider_step_id.clone(),
            tool_invocation_id: Some(invocation.id.clone()),
            provider_item_id: None,
            payload: ConversationEntryPayload::ToolResult(ToolResultEntry {
                tool_invocation_id: Some(invocation.id.clone()),
                call_id: invocation.call_id.clone(),
                content: output.content.clone(),
                is_error: true,
                structured_output: output.structured_output.clone(),
                raw_output: output.raw_output.clone(),
            }),
        });
        let (entries, _) = self.append_entries_and_update_tool_invocation_full(
            entries,
            invocation,
            UpdateToolInvocationStatus {
                status,
                output: Some(output),
                error: Some(error),
            },
            Some(approval),
        )?;
        self.push_event(AgentRunEvent::ToolInvocationFinished {
            tool_invocation_id: invocation.id.clone(),
        });
        debug_assert!(!entries.is_empty());
        Ok(model_text)
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
        let now = time::OffsetDateTime::now_utc();
        let approval = ToolInvocationApproval {
            status: ApprovalStatus::Approved,
            request: ApprovalRequestPayload {
                reason: "Auto-approved by current approval mode".to_string(),
                tool_source: definition.source.clone(),
                tool_name: definition.tool_name.clone(),
                arguments_preview,
                access_requests,
            },
            decision: Some(ApprovalDecisionPayload {
                approved: true,
                decided_by: "auto".to_string(),
                reason: Some("Auto-approved by current approval mode".to_string()),
            }),
            requested_at: now,
            decided_at: Some(now),
            expires_at: None,
        };
        let request = approval.request.clone();
        let decision = approval.decision.clone().expect("auto approval decision");
        let entries = vec![
            NewConversationEntry {
                conversation_id: self.conversation_id.clone(),
                status: ConversationEntryStatus::Completed,
                agent_run_id: Some(self.agent_run_id.clone()),
                provider_step_id: invocation.provider_step_id.clone(),
                tool_invocation_id: Some(invocation.id.clone()),
                provider_item_id: None,
                payload: ConversationEntryPayload::ApprovalRequest(ApprovalRequestEntry {
                    tool_invocation_id: invocation.id.clone(),
                    request,
                }),
            },
            NewConversationEntry {
                conversation_id: self.conversation_id.clone(),
                status: ConversationEntryStatus::Completed,
                agent_run_id: Some(self.agent_run_id.clone()),
                provider_step_id: invocation.provider_step_id.clone(),
                tool_invocation_id: Some(invocation.id.clone()),
                provider_item_id: None,
                payload: ConversationEntryPayload::ApprovalDecision(ApprovalDecisionEntry {
                    tool_invocation_id: invocation.id.clone(),
                    decision,
                }),
            },
        ];
        let entries = self
            .repo
            .record_auto_tool_invocation_approval_with_entries(
                entries,
                &invocation.id,
                invocation.status,
                approval,
            )?;
        self.record_persisted_entries(&entries.0);
        self.emit_runtime(AgentRuntimeEvent::ToolInvocationChanged {
            agent_run_id: invocation.agent_run_id.clone(),
            tool_invocation_id: invocation.id.clone(),
        });
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

impl PersistingPromptHook {
    async fn await_approval_and_execute_tool(
        &self,
        runtime_tool: RegisteredRuntimeTool,
        invocation: ToolInvocationRecord,
        arguments: serde_json::Value,
        request: ToolApprovalRequest,
    ) -> ToolCallHookAction {
        match self.context.await_tool_approval(request).await {
            ToolApprovalDecision::Approved { decided_by, reason } => {
                let invocation =
                    match self
                        .context
                        .approve_tool_invocation(&invocation, decided_by, reason)
                    {
                        Ok(invocation) => invocation,
                        Err(error) => return ToolCallHookAction::terminate(error.to_string()),
                    };
                match self
                    .context
                    .execute_tool_invocation(runtime_tool, invocation, arguments)
                    .await
                {
                    Ok(model_text) => ToolCallHookAction::skip(model_text),
                    Err(error) => ToolCallHookAction::terminate(error.to_string()),
                }
            }
            ToolApprovalDecision::Denied { decided_by, reason } => match self
                .context
                .append_denied_tool_approval_result(&invocation, decided_by, reason)
            {
                Ok(model_text) => ToolCallHookAction::skip(model_text),
                Err(error) => ToolCallHookAction::terminate(error.to_string()),
            },
            ToolApprovalDecision::Canceled => {
                if let Err(error) = self
                    .context
                    .append_canceled_tool_approval_result(&invocation)
                {
                    return ToolCallHookAction::terminate(error.to_string());
                }
                ToolCallHookAction::terminate("runtime canceled")
            }
        }
    }
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
        if self.context.cancellation_token.is_cancelled() {
            return HookAction::terminate("runtime canceled");
        }

        let provider_step_id = mutex_clone(&self.context.last_provider_step_id);
        for content in response.choice.iter() {
            let payload = match content {
                AssistantContent::Text(text) if !text.text.is_empty() => {
                    Some(ConversationEntryPayload::Message {
                        role: TranscriptRole::Assistant,
                        content: vec![ContentPart::Text {
                            text: text.text.clone(),
                        }],
                    })
                }
                AssistantContent::Reasoning(reasoning) => {
                    Some(ConversationEntryPayload::Reasoning {
                        text: reasoning.display_text(),
                        summary: None,
                    })
                }
                _ => None,
            };

            if let Some(payload) = payload {
                match self.context.append_item(payload.clone()) {
                    Ok(item) => {
                        if matches!(payload, ConversationEntryPayload::Message { .. }) {
                            mutex_replace(&self.context.final_entry_id, Some(item.id.clone()));
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
                    let (invocation, request) = match self.context.record_tool_approval_request(
                        &invocation,
                        &definition,
                        reason,
                        args.to_string(),
                        access_requests,
                    ) {
                        Ok(result) => result,
                        Err(error) => return ToolCallHookAction::terminate(error.to_string()),
                    };
                    return self
                        .await_approval_and_execute_tool(
                            runtime_tool,
                            invocation,
                            arguments,
                            request,
                        )
                        .await;
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
            let (invocation, request) = match self.context.record_tool_approval_request(
                &invocation,
                &definition,
                format!("Tool `{}` requires approval", definition.tool_name),
                args.to_string(),
                Vec::new(),
            ) {
                Ok(result) => result,
                Err(error) => return ToolCallHookAction::terminate(error.to_string()),
            };
            return self
                .await_approval_and_execute_tool(runtime_tool, invocation, arguments, request)
                .await;
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
        let payload = ConversationEntryPayload::ToolResult(ToolResultEntry {
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

fn approval_after_outcome(
    invocation: &ToolInvocationRecord,
    status: ApprovalStatus,
    decision: Option<ApprovalDecisionPayload>,
) -> Result<ToolInvocationApproval> {
    let mut approval = invocation.approval.clone().ok_or_else(|| {
        AgentRuntimeError::Invariant(format!("tool invocation {} has no approval", invocation.id))
    })?;
    approval.status = status;
    approval.decision = decision;
    approval.decided_at = Some(time::OffsetDateTime::now_utc());
    approval.expires_at = None;
    Ok(approval)
}

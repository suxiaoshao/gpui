use super::{PersistenceContext, error_tool_output, lock, mutex_clone, mutex_replace, run_error};
use crate::{
    AgentRuntimeError, AgentStep, RegisteredToolDefinition, Result, ToolApprovalDecision,
    ToolApprovalRequest, tools::tool_output_to_model_text,
};
use jaco_core::*;
use jaco_db::{
    NewConversationEntry, NewToolInvocation, ToolInvocationApprovalOutcome, ToolInvocationRecord,
    UpdateToolInvocationStatus,
};
use rig::{
    agent::{
        AgentHook, CompletionCallAction, CompletionCallEvent, CompletionResponseEvent, HookContext,
        InvalidToolCallAction, InvalidToolCallContext, ObservationAction, StepEventKind,
        StreamResponseFinish, ToolCall, ToolCallAction, ToolResultAction, ToolResultEvent,
    },
    completion::AssistantContent,
    message::ToolResultContent as RigToolResultContent,
    tool::ToolOutput,
};
use std::collections::{BTreeMap, BTreeSet};

impl PersistenceContext {
    pub(super) async fn append_error_tool_result_and_update_tool_invocation(
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
        let commit = self
            .persistence
            .append_entries_and_update_tool_invocation(
                vec![NewConversationEntry {
                    conversation_id: self.conversation_id.clone(),
                    status: ConversationEntryStatus::Completed,
                    agent_run_id: Some(self.agent_run_id.clone()),
                    provider_step_id: invocation.provider_step_id.clone(),
                    tool_invocation_id: Some(invocation.id.clone()),
                    provider_item_id: None,
                    payload,
                }],
                invocation.id.clone(),
                UpdateToolInvocationStatus {
                    status,
                    output: Some(output),
                    error: Some(error),
                },
                None,
            )
            .await?;
        self.emit_tool_entries_commit(&commit);
        let (items, _) = commit.value;
        let item = items.into_iter().next().ok_or_else(|| {
            AgentRuntimeError::Invariant(format!(
                "tool invocation {} failure created no entry",
                invocation.id
            ))
        })?;
        self.add_input_item_id(item.id.clone());
        self.push_step(AgentStep::ConversationEntry(item.id.clone()));
        self.push_event(AgentRunEvent::ToolInvocationFinished {
            tool_invocation_id: invocation.id.clone(),
        });
        Ok(model_text)
    }

    pub(super) async fn append_recoverable_tool_error(
        &self,
        invocation: &ToolInvocationRecord,
        status: ToolInvocationStatus,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> ToolCallAction {
        let error = run_error(code, message, true, None);
        match self
            .append_error_tool_result_and_update_tool_invocation(invocation, status, error)
            .await
        {
            Ok(model_text) => ToolCallAction::skip(model_text),
            Err(error) => ToolCallAction::stop(error.to_string()),
        }
    }

    pub(super) async fn append_recoverable_invalid_tool_error(
        &self,
        invocation: &ToolInvocationRecord,
        status: ToolInvocationStatus,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> InvalidToolCallAction {
        let error = run_error(code, message, true, None);
        match self
            .append_error_tool_result_and_update_tool_invocation(invocation, status, error)
            .await
        {
            Ok(model_text) => InvalidToolCallAction::skip(model_text),
            Err(_) => InvalidToolCallAction::fail(),
        }
    }

    pub(super) async fn insert_tool_invocation_and_append_call(
        &self,
        internal_call_id: &str,
        status: ToolInvocationStatus,
        input: ToolInvocationInput,
    ) -> Result<ToolInvocationRecord> {
        let invocation = self
            .persistence
            .insert_tool_invocation(NewToolInvocation {
                agent_run_id: self.agent_run_id.clone(),
                provider_step_id: mutex_clone(&self.last_provider_step_id),
                status,
                input,
                output: None,
                error: None,
            })
            .await?;

        lock(&self.tool_calls).insert(internal_call_id.to_string(), invocation.id.clone());
        self.push_event(AgentRunEvent::ToolInvocationRequested {
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
        self.append_tool_item(invocation.id.clone(), payload)
            .await?;
        Ok(invocation)
    }

    pub(super) async fn record_tool_approval_request(
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
        let commit = self
            .persistence
            .request_tool_invocation_approval_with_entry(
                invocation.id.clone(),
                jaco_db::NewToolInvocationApproval {
                    request: request.clone(),
                    expires_at: None,
                },
                entry,
            )
            .await?;
        self.emit_tool_entry_commit(&commit);
        let (entry, invocation) = commit.value;
        self.record_persisted_entries(std::slice::from_ref(&entry));
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
        let decision = broker.request_tool_approval(request);
        tokio::select! {
            biased;
            _ = self.cancellation_token.cancelled() => ToolApprovalDecision::Canceled,
            decision = decision => decision,
        }
    }

    pub(super) async fn approve_tool_invocation(
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
        let commit = self
            .persistence
            .decide_tool_invocation_approval_with_entry(
                invocation.id.clone(),
                ToolInvocationApprovalOutcome::Approved { decided_by, reason },
                ToolInvocationStatus::Running,
                entry,
            )
            .await?;
        self.emit_tool_entry_commit(&commit);
        let (entry, invocation) = commit.value;
        self.record_persisted_entries(std::slice::from_ref(&entry));
        Ok(invocation)
    }

    pub(super) async fn append_denied_tool_approval_result(
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
        .await
    }

    pub(super) async fn append_canceled_tool_approval_result(
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
        .await
    }

    async fn append_denied_or_canceled_tool_approval_result(
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
        let (entries, _) = self
            .append_entries_and_update_tool_invocation_full(
                entries,
                invocation,
                UpdateToolInvocationStatus {
                    status,
                    output: Some(output),
                    error: Some(error),
                },
                Some(approval),
            )
            .await?;
        self.push_event(AgentRunEvent::ToolInvocationFinished {
            tool_invocation_id: invocation.id.clone(),
        });
        debug_assert!(!entries.is_empty());
        Ok(model_text)
    }

    pub(super) async fn record_auto_approval(
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
        let commit = self
            .persistence
            .append_entries_and_update_tool_invocation(
                entries,
                invocation.id.clone(),
                UpdateToolInvocationStatus {
                    status: invocation.status,
                    output: invocation.output.clone(),
                    error: invocation.error.clone(),
                },
                Some(approval),
            )
            .await?;
        self.emit_tool_entries_commit(&commit);
        let (entries, _) = commit.value;
        self.record_persisted_entries(&entries);
        Ok(())
    }

    pub(super) fn check_tool_guard(&self, runtime_tool_name: &str, args: &str) -> ToolCallAction {
        let calls = lock(&self.tool_calls);
        if calls.len() as u32 >= self.max_tool_calls {
            return ToolCallAction::stop("max tool call guard reached");
        }
        drop(calls);

        let key = format!("{runtime_tool_name}\0{args}");
        let mut repeated = lock(&self.repeated_tool_calls);
        let count = repeated.entry(key).or_insert(0);
        *count += 1;
        if *count > self.repeated_tool_call_limit {
            ToolCallAction::stop(format!(
                "repeated tool call guard reached for {runtime_tool_name}"
            ))
        } else {
            ToolCallAction::run()
        }
    }
}

#[derive(Clone)]
pub(crate) struct PersistingAgentHook {
    pub(super) context: PersistenceContext,
}

impl PersistingAgentHook {
    async fn await_approval(
        &self,
        hook_context: &HookContext,
        internal_call_id: &str,
        invocation: ToolInvocationRecord,
        request: ToolApprovalRequest,
    ) -> ToolCallAction {
        match self.context.await_tool_approval(request).await {
            ToolApprovalDecision::Approved { decided_by, reason } => {
                let invocation = match self
                    .context
                    .approve_tool_invocation(&invocation, decided_by, reason)
                    .await
                {
                    Ok(invocation) => invocation,
                    Err(error) => return ToolCallAction::stop(error.to_string()),
                };
                drop(invocation);
                ToolCallAction::run()
            }
            ToolApprovalDecision::Denied { decided_by, reason } => match self
                .context
                .append_denied_tool_approval_result(&invocation, decided_by, reason)
                .await
            {
                Ok(model_text) => {
                    mark_result_persisted(hook_context, internal_call_id);
                    ToolCallAction::skip(model_text)
                }
                Err(error) => ToolCallAction::stop(error.to_string()),
            },
            ToolApprovalDecision::Canceled => {
                if let Err(error) = self
                    .context
                    .append_canceled_tool_approval_result(&invocation)
                    .await
                {
                    return ToolCallAction::stop(error.to_string());
                }
                mark_result_persisted(hook_context, internal_call_id);
                ToolCallAction::stop("runtime canceled")
            }
        }
    }

    async fn persist_assistant_content(
        &self,
        content: impl IntoIterator<Item = AssistantContent>,
    ) -> Result<()> {
        let provider_step_id = mutex_clone(&self.context.last_provider_step_id);
        for content in content {
            let payload = match content {
                AssistantContent::Text(text) if !text.text.is_empty() => {
                    Some(ConversationEntryPayload::Message {
                        role: TranscriptRole::Assistant,
                        content: vec![ContentPart::Text { text: text.text }],
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
                let item = self.context.append_item(payload.clone()).await?;
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
        }
        Ok(())
    }
}

impl AgentHook for PersistingAgentHook {
    async fn on_completion_call(
        &self,
        _context: &HookContext,
        _event: CompletionCallEvent<'_>,
    ) -> CompletionCallAction {
        if self.context.cancellation_token.is_cancelled() {
            CompletionCallAction::stop("runtime canceled")
        } else {
            CompletionCallAction::continue_run()
        }
    }

    async fn on_completion_response(
        &self,
        _context: &HookContext,
        event: CompletionResponseEvent<'_>,
    ) -> ObservationAction {
        if self.context.cancellation_token.is_cancelled() {
            return ObservationAction::stop("runtime canceled");
        }
        match self
            .persist_assistant_content(event.content.iter().cloned())
            .await
        {
            Ok(()) => ObservationAction::continue_run(),
            Err(error) => ObservationAction::stop(error.to_string()),
        }
    }

    async fn on_invalid_tool_call(
        &self,
        _hook_context: &HookContext,
        context: &InvalidToolCallContext,
    ) -> Option<InvalidToolCallAction> {
        if self.context.cancellation_token.is_cancelled() {
            return Some(InvalidToolCallAction::stop("runtime canceled"));
        }

        let args = context.args.as_deref().unwrap_or("");
        let guard_action = self.context.check_tool_guard(&context.tool_name, args);
        if !matches!(guard_action, ToolCallAction::Run) {
            return Some(InvalidToolCallAction::fail());
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
        let invocation = match self
            .context
            .insert_tool_invocation_and_append_call(
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
            )
            .await
        {
            Ok(invocation) => invocation,
            Err(_) => return Some(InvalidToolCallAction::fail()),
        };
        Some(
            self.context
                .append_recoverable_invalid_tool_error(
                    &invocation,
                    ToolInvocationStatus::Failed,
                    "tool_not_found",
                    format!("No tool named {} exists", context.tool_name),
                )
                .await,
        )
    }

    async fn on_tool_call(
        &self,
        hook_context: &HookContext,
        event: ToolCall<'_>,
    ) -> ToolCallAction {
        let ToolCall {
            tool_name,
            tool_call_id,
            internal_call_id,
            args,
        } = event;
        if self.context.cancellation_token.is_cancelled() {
            return ToolCallAction::stop("runtime canceled");
        }
        let guard_action = self.context.check_tool_guard(tool_name, args);
        if !matches!(guard_action, ToolCallAction::Run) {
            return guard_action;
        }

        let call_id = tool_call_id
            .map(str::to_string)
            .unwrap_or_else(|| internal_call_id.to_string());
        let arguments = serde_json::from_str::<serde_json::Value>(args)
            .unwrap_or_else(|_| serde_json::Value::String(args.to_string()));
        let Some(definition) = self.context.tool_definitions.get(tool_name).cloned() else {
            let invocation = match self
                .context
                .insert_tool_invocation_and_append_call(
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
                )
                .await
            {
                Ok(invocation) => invocation,
                Err(error) => return ToolCallAction::stop(error.to_string()),
            };
            return self
                .context
                .append_recoverable_tool_error(
                    &invocation,
                    ToolInvocationStatus::Failed,
                    "tool_not_found",
                    format!("No tool named {tool_name} exists"),
                )
                .await;
        };
        let status = if definition.policy.approval_policy == ToolApprovalPolicy::Never {
            ToolInvocationStatus::Running
        } else {
            ToolInvocationStatus::AwaitingApproval
        };
        let invocation = match self
            .context
            .insert_tool_invocation_and_append_call(
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
            )
            .await
        {
            Ok(invocation) => invocation,
            Err(error) => return ToolCallAction::stop(error.to_string()),
        };
        remember_invocation(hook_context, internal_call_id, invocation.clone());

        let builtin_access_requests = if matches!(definition.source, ToolSource::Local) {
            match crate::tools::builtin::registry::access_requests_for_builtin_tool(
                &definition.tool_name,
                &arguments,
                &self.context.settings_snapshot.tool_policy,
            ) {
                Ok(access_requests) => access_requests,
                Err(error) => {
                    let action = self
                        .context
                        .append_recoverable_tool_error(
                            &invocation,
                            ToolInvocationStatus::Failed,
                            "tool_invalid_arguments",
                            format!(
                                "Invalid arguments for tool {}: {error}",
                                definition.runtime_tool_name
                            ),
                        )
                        .await;
                    mark_result_persisted(hook_context, internal_call_id);
                    return action;
                }
            }
        } else {
            None
        };
        if let Some(access_requests) = builtin_access_requests {
            let evaluator =
                match crate::tools::builtin::approval::ToolPermissionEvaluator::from_policy(
                    &self.context.settings_snapshot.tool_policy,
                    None,
                ) {
                    Ok(evaluator) => evaluator,
                    Err(error) => return ToolCallAction::stop(error.to_string()),
                };
            match evaluator.evaluate(&access_requests) {
                crate::tools::builtin::approval::ToolPermissionDecision::Allow {
                    auto_approved,
                } => {
                    if let Err(error) = self
                        .context
                        .record_auto_approval(
                            &invocation,
                            &definition,
                            args.to_string(),
                            auto_approved,
                        )
                        .await
                    {
                        return ToolCallAction::stop(error.to_string());
                    }
                }
                crate::tools::builtin::approval::ToolPermissionDecision::Ask {
                    reason,
                    access_requests,
                } => {
                    let (invocation, request) = match self
                        .context
                        .record_tool_approval_request(
                            &invocation,
                            &definition,
                            reason,
                            args.to_string(),
                            access_requests,
                        )
                        .await
                    {
                        Ok(result) => result,
                        Err(error) => return ToolCallAction::stop(error.to_string()),
                    };
                    return self
                        .await_approval(hook_context, internal_call_id, invocation, request)
                        .await;
                }
                crate::tools::builtin::approval::ToolPermissionDecision::Deny { reason } => {
                    let error = run_error("tool_permission_denied", reason, false, None);
                    return match self
                        .context
                        .append_error_tool_result_and_update_tool_invocation(
                            &invocation,
                            ToolInvocationStatus::Denied,
                            error,
                        )
                        .await
                    {
                        Ok(model_text) => {
                            mark_result_persisted(hook_context, internal_call_id);
                            ToolCallAction::skip(model_text)
                        }
                        Err(error) => ToolCallAction::stop(error.to_string()),
                    };
                }
            }
        }

        if definition.policy.approval_policy != ToolApprovalPolicy::Never {
            let (invocation, request) = match self
                .context
                .record_tool_approval_request(
                    &invocation,
                    &definition,
                    format!("Tool `{}` requires approval", definition.tool_name),
                    args.to_string(),
                    Vec::new(),
                )
                .await
            {
                Ok(result) => result,
                Err(error) => return ToolCallAction::stop(error.to_string()),
            };
            return self
                .await_approval(hook_context, internal_call_id, invocation, request)
                .await;
        }

        drop(invocation);
        ToolCallAction::run()
    }

    async fn on_tool_result(
        &self,
        hook_context: &HookContext,
        event: ToolResultEvent<'_>,
    ) -> ToolResultAction {
        if take_result_persisted(hook_context, event.internal_call_id) {
            take_invocation(hook_context, event.internal_call_id);
            return ToolResultAction::keep();
        }
        let Some(invocation) = take_invocation(hook_context, event.internal_call_id) else {
            return ToolResultAction::stop(format!(
                "tool result {} has no invocation",
                event.internal_call_id
            ));
        };

        let mut output = if let Some(output) =
            event.tool_context.result::<ToolInvocationOutput>().cloned()
        {
            output
        } else if let Some(result) = event.tool_context.result::<rmcp::model::CallToolResult>() {
            mcp_result_to_jaco_output(result)
        } else {
            rig_output_to_jaco_output(event.presentation)
        };
        output.is_error =
            output.is_error || event.raw_result.is_error() || event.raw_result.is_refused();
        let status = if event.raw_result.is_refused() {
            ToolInvocationStatus::Denied
        } else if output.is_error || event.raw_result.is_skipped() {
            ToolInvocationStatus::Failed
        } else {
            ToolInvocationStatus::Succeeded
        };
        let error = (status != ToolInvocationStatus::Succeeded).then(|| {
            let message = event
                .raw_result
                .error()
                .or_else(|| event.raw_result.refusal())
                .map(|error| error.message().to_string())
                .unwrap_or_else(|| tool_output_to_model_text(&output));
            run_error("tool_error", message, true, None)
        });
        let entry = NewConversationEntry {
            conversation_id: self.context.conversation_id.clone(),
            status: ConversationEntryStatus::Completed,
            agent_run_id: Some(self.context.agent_run_id.clone()),
            provider_step_id: invocation.provider_step_id.clone(),
            tool_invocation_id: Some(invocation.id.clone()),
            provider_item_id: None,
            payload: ConversationEntryPayload::ToolResult(ToolResultEntry {
                tool_invocation_id: Some(invocation.id.clone()),
                call_id: invocation.call_id.clone(),
                content: output.content.clone(),
                is_error: output.is_error,
                structured_output: output.structured_output.clone(),
                raw_output: output.raw_output.clone(),
            }),
        };
        let commit = match self
            .context
            .persistence
            .append_entries_and_update_tool_invocation(
                vec![entry],
                invocation.id.clone(),
                UpdateToolInvocationStatus {
                    status,
                    output: Some(output),
                    error,
                },
                None,
            )
            .await
        {
            Ok(commit) => commit,
            Err(error) => return ToolResultAction::stop(error.to_string()),
        };
        self.context.emit_tool_entries_commit(&commit);
        let (entries, invocation) = commit.value;
        self.context.record_persisted_entries(&entries);
        self.context
            .push_event(AgentRunEvent::ToolInvocationFinished {
                tool_invocation_id: invocation.id,
            });
        ToolResultAction::keep()
    }

    async fn on_stream_response_finish(
        &self,
        _context: &HookContext,
        event: StreamResponseFinish<'_>,
    ) -> ObservationAction {
        if self.context.cancellation_token.is_cancelled() {
            return ObservationAction::stop("runtime canceled");
        }
        // Streaming deltas are persisted incrementally by StreamAccumulator. A
        // tool-bearing model turn must finish its provider step before Rig starts
        // the next model call; the final tool-free turn is completed by runtime
        // with the terminal provider payload.
        if event
            .content
            .iter()
            .any(|content| matches!(content, AssistantContent::ToolCall(_)))
            && let Err(error) = self
                .context
                .finish_current_streaming_provider_step::<serde_json::Value>(None, event.usage)
                .await
        {
            return ObservationAction::stop(error.to_string());
        }
        ObservationAction::continue_run()
    }

    fn observes(&self, kind: StepEventKind) -> bool {
        matches!(
            kind,
            StepEventKind::CompletionCall
                | StepEventKind::CompletionResponse
                | StepEventKind::InvalidToolCall
                | StepEventKind::ToolCall
                | StepEventKind::ToolResult
                | StepEventKind::StreamResponseFinish
        )
    }
}

#[derive(Clone, Default)]
struct PendingToolInvocations(BTreeMap<String, ToolInvocationRecord>);

#[derive(Clone, Default)]
struct PersistedToolResults(BTreeSet<String>);

fn remember_invocation(
    context: &HookContext,
    internal_call_id: &str,
    invocation: ToolInvocationRecord,
) {
    context
        .scratchpad()
        .update::<PendingToolInvocations, _>(|pending| {
            pending.0.insert(internal_call_id.to_string(), invocation);
        });
}

fn take_invocation(context: &HookContext, internal_call_id: &str) -> Option<ToolInvocationRecord> {
    context
        .scratchpad()
        .update::<PendingToolInvocations, _>(|pending| pending.0.remove(internal_call_id))
}

fn mark_result_persisted(context: &HookContext, internal_call_id: &str) {
    context
        .scratchpad()
        .update::<PersistedToolResults, _>(|persisted| {
            persisted.0.insert(internal_call_id.to_string());
        });
}

fn take_result_persisted(context: &HookContext, internal_call_id: &str) -> bool {
    context
        .scratchpad()
        .update::<PersistedToolResults, _>(|persisted| persisted.0.remove(internal_call_id))
}

fn rig_output_to_jaco_output(output: &ToolOutput) -> ToolInvocationOutput {
    let mut content = Vec::new();
    let mut structured_output = None;
    for part in output.as_content().iter() {
        match part {
            RigToolResultContent::Text(text) => {
                content.push(ContentPart::Text {
                    text: text.text.clone(),
                });
            }
            RigToolResultContent::Json { value } => {
                structured_output = Some(StructuredOutput {
                    value: value.clone(),
                });
            }
            RigToolResultContent::Image(_) => {}
        }
    }
    ToolInvocationOutput {
        content,
        structured_output,
        raw_output: serde_json::to_value(output.as_content()).ok().map(|value| {
            ProviderRawPayload {
                provider_kind: "rig".to_string(),
                value,
            }
        }),
        is_error: false,
    }
}

fn mcp_result_to_jaco_output(result: &rmcp::model::CallToolResult) -> ToolInvocationOutput {
    let content = result
        .content
        .iter()
        .filter_map(|part| {
            part.as_text().map(|text| ContentPart::Text {
                text: text.text.clone(),
            })
        })
        .collect();
    ToolInvocationOutput {
        content,
        structured_output: result
            .structured_content
            .clone()
            .map(|value| StructuredOutput { value }),
        raw_output: serde_json::to_value(result)
            .ok()
            .map(|value| ProviderRawPayload {
                provider_kind: "mcp".to_string(),
                value,
            }),
        is_error: result.is_error.unwrap_or(false),
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

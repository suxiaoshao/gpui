use crate::{AgentRuntimeError, AgentStep, RegisteredToolDefinition, Result};
use ai_chat_core::*;
use ai_chat_db::{
    ConversationItemRecord, FreshRepository, NewAgentRun, NewApprovalDecision,
    NewApprovalDecisionOutcome, NewConversationItem, NewProviderStep, NewToolInvocation,
    NewUsageEvent, ProviderStepRecord, UpdateAgentRunStatus, UpdateProviderStepStatus,
    UpdateToolInvocationStatus,
};
use rig_core::{
    agent::{HookAction, PromptHook, ToolCallHookAction},
    completion::{AssistantContent, CompletionModel, CompletionRequest, CompletionResponse, Usage},
    streaming::StreamingCompletionResponse,
};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct PersistingCompletionModel<M>
where
    M: CompletionModel,
{
    inner: M,
    context: Option<PersistenceContext>,
}

impl<M> PersistingCompletionModel<M>
where
    M: CompletionModel,
{
    pub(crate) fn new(inner: M, context: PersistenceContext) -> Self {
        Self {
            inner,
            context: Some(context),
        }
    }
}

impl<M> CompletionModel for PersistingCompletionModel<M>
where
    M: CompletionModel,
    M::Response: Serialize + DeserializeOwned,
    M::StreamingResponse: Clone
        + Unpin
        + Send
        + Sync
        + Serialize
        + DeserializeOwned
        + rig_core::completion::GetTokenUsage,
{
    type Response = M::Response;
    type StreamingResponse = M::StreamingResponse;
    type Client = M::Client;

    fn make(client: &Self::Client, model: impl Into<String>) -> Self {
        Self {
            inner: M::make(client, model),
            context: None,
        }
    }

    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> std::result::Result<
        CompletionResponse<Self::Response>,
        rig_core::completion::CompletionError,
    > {
        let Some(context) = self.context.clone() else {
            return self.inner.completion(request).await;
        };
        let provider_step = context
            .insert_provider_step(&request)
            .map_err(completion_request_error)?;

        let response = self.inner.completion(request).await;
        match response {
            Ok(response) => {
                context
                    .finish_provider_step(&provider_step.id, &response)
                    .map_err(completion_request_error)?;
                Ok(response)
            }
            Err(error) => {
                let payload = run_error("provider_error", error.to_string(), true, None);
                let _ = context.fail_provider_step(&provider_step.id, payload);
                Err(error)
            }
        }
    }

    async fn stream(
        &self,
        request: CompletionRequest,
    ) -> std::result::Result<
        StreamingCompletionResponse<Self::StreamingResponse>,
        rig_core::completion::CompletionError,
    > {
        let Some(context) = self.context.clone() else {
            return self.inner.stream(request).await;
        };
        let provider_step = context
            .insert_provider_step(&request)
            .map_err(completion_request_error)?;
        let response = self.inner.stream(request).await;
        match response {
            Ok(response) => {
                let _ = context.finish_streaming_provider_step(&provider_step.id);
                Ok(response)
            }
            Err(error) => {
                let payload = run_error("provider_error", error.to_string(), true, None);
                let _ = context.fail_provider_step(&provider_step.id, payload);
                Err(error)
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct PersistenceContext {
    repo: FreshRepository,
    agent_run_id: AgentRunId,
    conversation_id: ConversationId,
    provider_id: ProviderId,
    model_id: ProviderModelId,
    settings_snapshot: RunSettingsSnapshot,
    input_item_ids: Arc<Mutex<Vec<ConversationItemId>>>,
    last_provider_step_id: Arc<Mutex<Option<ProviderStepId>>>,
    final_item_id: Arc<Mutex<Option<ConversationItemId>>>,
    events: Arc<Mutex<Vec<AgentRunEvent>>>,
    steps: Arc<Mutex<Vec<AgentStep>>>,
    tool_definitions: Arc<HashMap<String, RegisteredToolDefinition>>,
    tool_calls: Arc<Mutex<HashMap<String, ToolInvocationId>>>,
    repeated_tool_calls: Arc<Mutex<HashMap<String, u32>>>,
    max_tool_calls: u32,
    repeated_tool_call_limit: u32,
    cancellation_token: CancellationToken,
}

impl PersistenceContext {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        repo: FreshRepository,
        agent_run_id: AgentRunId,
        conversation_id: ConversationId,
        provider_id: ProviderId,
        model_id: ProviderModelId,
        settings_snapshot: RunSettingsSnapshot,
        input_item_ids: Vec<ConversationItemId>,
        tool_definitions: Vec<RegisteredToolDefinition>,
        max_tool_calls: u32,
        repeated_tool_call_limit: u32,
        cancellation_token: CancellationToken,
    ) -> Self {
        Self {
            repo,
            agent_run_id,
            conversation_id,
            provider_id,
            model_id,
            settings_snapshot,
            input_item_ids: Arc::new(Mutex::new(input_item_ids)),
            last_provider_step_id: Arc::new(Mutex::new(None)),
            final_item_id: Arc::new(Mutex::new(None)),
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
        }
    }

    pub(crate) fn hook(&self) -> PersistingPromptHook {
        PersistingPromptHook {
            context: self.clone(),
        }
    }

    pub(crate) fn events(&self) -> Vec<AgentRunEvent> {
        mutex_clone(&self.events)
    }

    pub(crate) fn steps(&self) -> Vec<AgentStep> {
        mutex_clone(&self.steps)
    }

    pub(crate) fn final_item_id(&self) -> Option<ConversationItemId> {
        mutex_clone(&self.final_item_id)
    }

    fn insert_provider_step(&self, request: &CompletionRequest) -> Result<ProviderStepRecord> {
        let seq = self.repo.next_provider_step_seq(&self.agent_run_id)?;
        let input_item_ids = mutex_clone(&self.input_item_ids);
        let step = self.repo.insert_provider_step(NewProviderStep {
            agent_run_id: self.agent_run_id.clone(),
            seq,
            status: ProviderStepStatus::Running,
            request_snapshot: ProviderStepRequestSnapshot {
                provider_id: self.provider_id.clone(),
                model_id: self.model_id.clone(),
                input_item_ids,
                snapshot_kind: ProviderStepSnapshotKind::RigCompletionRequest,
                request_body: ProviderRawPayload {
                    provider_kind: "rig".to_string(),
                    value: serde_json::to_value(request)?,
                },
            },
            response_snapshot: None,
            state_snapshot: None,
            settings_snapshot: self.settings_snapshot.clone(),
            error: None,
        })?;
        mutex_replace(&self.last_provider_step_id, Some(step.id.clone()));
        self.push_event(AgentRunEvent::ProviderStepStarted {
            provider_step_id: step.id.clone(),
        });
        self.push_step(AgentStep::ProviderStep(step.id.clone()));
        Ok(step)
    }

    fn finish_provider_step<M>(
        &self,
        provider_step_id: &str,
        response: &CompletionResponse<M>,
    ) -> Result<()>
    where
        M: Serialize,
    {
        let output_item_ids = response
            .choice
            .iter()
            .filter_map(|content| match content {
                AssistantContent::Reasoning(reasoning) => reasoning.id.clone(),
                _ => None,
            })
            .collect::<Vec<_>>();
        let response_snapshot = ProviderStepResponseSnapshot {
            provider_run_id: response.message_id.clone(),
            output_item_ids: output_item_ids.clone(),
            response_body: Some(ProviderRawPayload {
                provider_kind: "rig".to_string(),
                value: serde_json::to_value(&response.raw_response)?,
            }),
        };
        let state_snapshot = ProviderRunStateSnapshot {
            provider_id: self.provider_id.clone(),
            provider_run_id: response.message_id.clone(),
            output_item_ids,
            continuation: response
                .message_id
                .as_ref()
                .map(|message_id| ProviderRawPayload {
                    provider_kind: "rig".to_string(),
                    value: serde_json::json!({ "messageId": message_id }),
                }),
        };
        self.repo.update_provider_step_status(
            provider_step_id,
            UpdateProviderStepStatus {
                status: ProviderStepStatus::Completed,
                response_snapshot: Some(response_snapshot),
                state_snapshot: Some(state_snapshot.clone()),
                error: None,
            },
        )?;
        let usage = provider_usage(response.usage);
        self.repo.insert_usage_event(NewUsageEvent {
            provider_step_id: provider_step_id.to_string(),
            date_key: time::OffsetDateTime::now_utc().date().to_string(),
            usage: usage.clone(),
        })?;
        self.push_event(AgentRunEvent::ProviderStepEvent {
            provider_step_id: provider_step_id.to_string(),
            event: ProviderStepEvent::UsageUpdated { usage },
        });
        self.push_event(AgentRunEvent::ProviderStepEvent {
            provider_step_id: provider_step_id.to_string(),
            event: ProviderStepEvent::Completed {
                state: Some(state_snapshot),
            },
        });
        Ok(())
    }

    fn finish_streaming_provider_step(&self, provider_step_id: &str) -> Result<()> {
        self.repo.update_provider_step_status(
            provider_step_id,
            UpdateProviderStepStatus {
                status: ProviderStepStatus::Completed,
                response_snapshot: Some(ProviderStepResponseSnapshot {
                    provider_run_id: None,
                    output_item_ids: Vec::new(),
                    response_body: None,
                }),
                state_snapshot: None,
                error: None,
            },
        )?;
        Ok(())
    }

    fn fail_provider_step(&self, provider_step_id: &str, error: RunErrorPayload) -> Result<()> {
        self.repo.update_provider_step_status(
            provider_step_id,
            UpdateProviderStepStatus {
                status: ProviderStepStatus::Failed,
                response_snapshot: None,
                state_snapshot: None,
                error: Some(error.clone()),
            },
        )?;
        self.push_event(AgentRunEvent::ProviderStepEvent {
            provider_step_id: provider_step_id.to_string(),
            event: ProviderStepEvent::Failed { error },
        });
        Ok(())
    }

    fn append_item(&self, payload: ConversationItemPayload) -> Result<ConversationItemRecord> {
        let item = self.repo.append_conversation_item(NewConversationItem {
            conversation_id: self.conversation_id.clone(),
            status: ConversationItemStatus::Completed,
            agent_run_id: Some(self.agent_run_id.clone()),
            provider_step_id: mutex_clone(&self.last_provider_step_id),
            tool_invocation_id: None,
            provider_item_id: None,
            payload,
        })?;
        self.add_input_item_id(item.id.clone());
        self.push_step(AgentStep::ConversationItem(item.id.clone()));
        Ok(item)
    }

    fn append_tool_item(
        &self,
        tool_invocation_id: ToolInvocationId,
        payload: ConversationItemPayload,
    ) -> Result<ConversationItemRecord> {
        let item = self.repo.append_conversation_item(NewConversationItem {
            conversation_id: self.conversation_id.clone(),
            status: ConversationItemStatus::Completed,
            agent_run_id: Some(self.agent_run_id.clone()),
            provider_step_id: mutex_clone(&self.last_provider_step_id),
            tool_invocation_id: Some(tool_invocation_id),
            provider_item_id: None,
            payload,
        })?;
        self.add_input_item_id(item.id.clone());
        self.push_step(AgentStep::ConversationItem(item.id.clone()));
        Ok(item)
    }

    fn add_input_item_id(&self, item_id: ConversationItemId) {
        let mut guard = lock(&self.input_item_ids);
        guard.push(item_id);
    }

    fn push_event(&self, event: AgentRunEvent) {
        lock(&self.events).push(event);
    }

    fn push_step(&self, step: AgentStep) {
        lock(&self.steps).push(step);
    }

    fn check_tool_guard(&self, runtime_tool_name: &str, args: &str) -> ToolCallHookAction {
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
    context: PersistenceContext,
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

        let Some(definition) = self.context.tool_definitions.get(tool_name).cloned() else {
            return ToolCallHookAction::terminate(format!("tool {tool_name} is not registered"));
        };
        let call_id = tool_call_id.unwrap_or_else(|| internal_call_id.to_string());
        let arguments = serde_json::from_str::<serde_json::Value>(args)
            .unwrap_or_else(|_| serde_json::Value::String(args.to_string()));
        let status = if definition.policy.approval_policy == ToolApprovalPolicy::Never {
            ToolInvocationStatus::Running
        } else {
            ToolInvocationStatus::AwaitingApproval
        };
        let invocation = match self.context.repo.insert_tool_invocation(NewToolInvocation {
            agent_run_id: self.context.agent_run_id.clone(),
            provider_step_id: mutex_clone(&self.context.last_provider_step_id),
            status,
            input: ToolInvocationInput {
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
            output: None,
            error: None,
        }) {
            Ok(invocation) => invocation,
            Err(error) => return ToolCallHookAction::terminate(error.to_string()),
        };

        lock(&self.context.tool_calls).insert(internal_call_id.to_string(), invocation.id.clone());
        self.context
            .push_event(AgentRunEvent::ToolInvocationRequested {
                tool_invocation_id: invocation.id.clone(),
            });
        self.context
            .push_step(AgentStep::ToolInvocation(invocation.id.clone()));

        let payload = ConversationItemPayload::ToolCall(ToolCallItem {
            tool_invocation_id: Some(invocation.id.clone()),
            call_id: call_id.clone(),
            source: definition.source.clone(),
            name: definition.tool_name.clone(),
            runtime_tool_name: definition.runtime_tool_name.clone(),
            arguments: ToolArguments { value: arguments },
        });
        if let Err(error) = self
            .context
            .append_tool_item(invocation.id.clone(), payload)
        {
            return ToolCallHookAction::terminate(error.to_string());
        }

        if definition.policy.approval_policy != ToolApprovalPolicy::Never {
            let request = ApprovalRequestPayload {
                reason: format!("Tool `{}` requires approval", definition.tool_name),
                tool_source: definition.source.clone(),
                tool_name: definition.tool_name.clone(),
                arguments_preview: args.to_string(),
            };
            let approval = match self
                .context
                .repo
                .insert_approval_decision(NewApprovalDecision {
                    tool_invocation_id: invocation.id.clone(),
                    request: request.clone(),
                    outcome: NewApprovalDecisionOutcome::Pending { expires_at: None },
                }) {
                Ok(approval) => approval,
                Err(error) => return ToolCallHookAction::terminate(error.to_string()),
            };
            self.context
                .push_step(AgentStep::Approval(approval.id.clone()));
            self.context.push_event(AgentRunEvent::ApprovalRequested {
                approval_decision_id: approval.id.clone(),
            });
            let payload = ConversationItemPayload::ApprovalRequest(ApprovalRequestItem {
                approval_decision_id: approval.id,
                tool_invocation_id: invocation.id.clone(),
                request,
            });
            if let Err(error) = self
                .context
                .append_tool_item(invocation.id.clone(), payload)
            {
                return ToolCallHookAction::terminate(error.to_string());
            }
            if let Err(error) = self.context.repo.update_agent_run_status(
                &self.context.agent_run_id,
                UpdateAgentRunStatus {
                    status: AgentRunStatus::WaitingForApproval,
                    output: None,
                    error: None,
                },
            ) {
                return ToolCallHookAction::terminate(error.to_string());
            }
            return ToolCallHookAction::terminate("tool approval required");
        }

        ToolCallHookAction::cont()
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
        });
        if let Err(error) = self
            .context
            .append_tool_item(tool_invocation_id.clone(), payload)
        {
            return HookAction::terminate(error.to_string());
        }
        self.context
            .push_event(AgentRunEvent::ToolInvocationFinished { tool_invocation_id });
        HookAction::cont()
    }
}

pub(crate) fn new_agent_run_input(request: &crate::AgentRunRequest) -> NewAgentRun {
    NewAgentRun {
        trigger_kind: request.trigger_kind,
        status: AgentRunStatus::Queued,
        input: AgentRunInput {
            user_item_id: request.user_item_id.clone(),
            parent_agent_run_id: request.parent_agent_run_id.clone(),
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

fn completion_request_error(error: AgentRuntimeError) -> rig_core::completion::CompletionError {
    rig_core::completion::CompletionError::RequestError(Box::new(error))
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

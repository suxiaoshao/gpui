use crate::{
    AgentRuntimeError, AgentRuntimeObserver, AgentStep, RegisteredToolDefinition,
    ToolApprovalBroker, tool_registry::RegisteredRuntimeTool,
};
use jaco_core::*;
use jaco_db::{FreshRepository, NewAgentRun};
mod conversation_items;
mod model;
mod provider_step;
mod tool_hook;

pub use model::PersistingCompletionModel;

use self::tool_hook::PersistingPromptHook;
use rig_core::completion::Usage;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio_util::sync::CancellationToken;

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
    runtime_tools: Arc<HashMap<String, RegisteredRuntimeTool>>,
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
        repo: FreshRepository,
        agent_run_id: AgentRunId,
        conversation_id: ConversationId,
        provider_id: ProviderId,
        model_id: ProviderModelId,
        settings_snapshot: RunSettingsSnapshot,
        input_item_ids: Vec<ConversationItemId>,
        tool_definitions: Vec<RegisteredToolDefinition>,
        runtime_tools: Vec<RegisteredRuntimeTool>,
        max_tool_calls: u32,
        repeated_tool_call_limit: u32,
        cancellation_token: CancellationToken,
        observer: Option<AgentRuntimeObserver>,
        approval_broker: Option<Arc<dyn ToolApprovalBroker>>,
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
            runtime_tools: Arc::new(
                runtime_tools
                    .into_iter()
                    .map(|tool| (tool.definition.runtime_tool_name.clone(), tool))
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
}

pub(crate) fn new_agent_run_input(request: &crate::AgentRunRequest) -> NewAgentRun {
    NewAgentRun {
        trigger_kind: request.trigger_kind,
        status: AgentRunStatus::Queued,
        input: AgentRunInput {
            user_item_id: request.user_item_id.clone(),
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

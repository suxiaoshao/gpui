use crate::{Result, ToolRegistry};
use ai_chat_core::*;
use ai_chat_db::AgentRunRecord;
use async_trait::async_trait;
use rig_core::completion::CompletionModel;
use std::{path::PathBuf, sync::Arc};
use tokio_util::sync::CancellationToken;

pub type AgentCancellationToken = CancellationToken;

#[derive(Debug, Clone)]
pub struct RuntimeGuards {
    pub max_steps: u32,
    pub max_tool_calls: u32,
    pub repeated_tool_call_limit: u32,
    pub tool_timeout: std::time::Duration,
    pub tool_concurrency: usize,
}

impl Default for RuntimeGuards {
    fn default() -> Self {
        Self {
            max_steps: 32,
            max_tool_calls: 128,
            repeated_tool_call_limit: 3,
            tool_timeout: std::time::Duration::from_secs(120),
            tool_concurrency: 1,
        }
    }
}

#[derive(Clone)]
pub struct AgentRunRequest {
    pub conversation_id: ConversationId,
    pub user_item_id: ConversationItemId,
    pub trigger_kind: AgentRunTriggerKind,
    pub prompt_snapshot: Option<PromptContent>,
    pub provider_id: ProviderId,
    pub model_id: ProviderModelId,
    pub settings_snapshot: RunSettingsSnapshot,
    pub runtime_snapshot: AgentRuntimeSnapshot,
    pub tool_registry: ToolRegistry,
    pub skill_requests: Vec<SkillActivationRequest>,
    pub provider_tools: Vec<rig_core::completion::ProviderToolDefinition>,
    pub project_root: Option<PathBuf>,
    pub guards: RuntimeGuards,
    pub cancellation_token: AgentCancellationToken,
}

impl AgentRunRequest {
    pub fn new(
        conversation_id: ConversationId,
        user_item_id: ConversationItemId,
        provider_id: ProviderId,
        model_id: ProviderModelId,
        settings_snapshot: RunSettingsSnapshot,
        runtime_snapshot: AgentRuntimeSnapshot,
    ) -> Self {
        let max_steps = settings_snapshot.tool_policy.max_steps.max(1);
        Self {
            conversation_id,
            user_item_id,
            trigger_kind: AgentRunTriggerKind::User,
            prompt_snapshot: settings_snapshot.prompt.clone(),
            provider_id,
            model_id,
            settings_snapshot,
            runtime_snapshot,
            tool_registry: ToolRegistry::default(),
            skill_requests: Vec::new(),
            provider_tools: Vec::new(),
            project_root: None,
            guards: RuntimeGuards {
                max_steps,
                ..RuntimeGuards::default()
            },
            cancellation_token: CancellationToken::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AgentStep {
    ProviderStep(ProviderStepId),
    ToolInvocation(ToolInvocationId),
    ConversationItem(ConversationItemId),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AgentRunHandleStatus {
    Finished,
}

#[derive(Debug, Clone)]
pub struct AgentRunHandle {
    pub agent_run: AgentRunRecord,
    pub output: Option<AgentRunOutput>,
    pub status: AgentRunHandleStatus,
    pub events: Vec<AgentRunEvent>,
    pub steps: Vec<AgentStep>,
}

#[derive(Clone)]
pub struct AgentRuntimeObserver {
    sender: Arc<dyn Fn(AgentRuntimeEvent) + Send + Sync>,
}

impl AgentRuntimeObserver {
    pub fn new(sender: impl Fn(AgentRuntimeEvent) + Send + Sync + 'static) -> Self {
        Self {
            sender: Arc::new(sender),
        }
    }

    pub fn emit(&self, event: AgentRuntimeEvent) {
        (self.sender)(event);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentRuntimeEvent {
    AgentRunStarted {
        agent_run_id: AgentRunId,
        conversation_id: ConversationId,
    },
    AgentRunStatusChanged {
        agent_run_id: AgentRunId,
        status: AgentRunStatus,
    },
    ConversationItemAppended {
        conversation_id: ConversationId,
        item_id: ConversationItemId,
    },
    ConversationItemUpdated {
        conversation_id: ConversationId,
        item_id: ConversationItemId,
    },
    ProviderStepChanged {
        agent_run_id: AgentRunId,
        provider_step_id: ProviderStepId,
    },
    ToolInvocationChanged {
        agent_run_id: AgentRunId,
        tool_invocation_id: ToolInvocationId,
    },
    ToolApprovalRequested {
        agent_run_id: AgentRunId,
        tool_invocation_id: ToolInvocationId,
    },
}

#[async_trait]
pub trait CompletionModelFactory<M>: Send + Sync
where
    M: CompletionModel,
{
    async fn create_model(&self, request: &AgentRunRequest) -> Result<M>;
}

pub use crate::skills::SkillActivationRequest;

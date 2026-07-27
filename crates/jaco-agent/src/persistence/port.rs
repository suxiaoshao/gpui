use async_trait::async_trait;
use jaco_core::*;
use jaco_db::{
    AgentRunRecord, ConversationCommit, ConversationEntryRecord, ConversationTimelineRecords,
    FinishAgentRun, FinishedAgentRun, NewAgentRun, NewConversationEntry, NewProviderStep,
    NewToolInvocation, NewToolInvocationApproval, NewUsageEvent, ProviderStepRecord,
    ToolInvocationApprovalOutcome, ToolInvocationRecord, UpdateAgentRunStatus,
    UpdateProviderStepStatus, UpdateToolInvocationStatus, UsageEventRecord,
};

#[async_trait]
pub trait AgentPersistence: Send + Sync {
    async fn conversation_timeline(
        &self,
        conversation_id: ConversationId,
    ) -> jaco_db::Result<Option<ConversationTimelineRecords>>;

    async fn conversation_entries(
        &self,
        conversation_id: ConversationId,
    ) -> jaco_db::Result<Vec<ConversationEntryRecord>>;

    async fn append_conversation_entry(
        &self,
        input: NewConversationEntry,
    ) -> jaco_db::Result<ConversationCommit<ConversationEntryRecord>>;

    async fn update_conversation_entry_payload(
        &self,
        item_id: ConversationEntryId,
        status: ConversationEntryStatus,
        payload: ConversationEntryPayload,
    ) -> jaco_db::Result<ConversationCommit<ConversationEntryRecord>>;

    async fn insert_agent_run(&self, input: NewAgentRun) -> jaco_db::Result<AgentRunRecord>;

    async fn get_agent_run(&self, id: AgentRunId) -> jaco_db::Result<Option<AgentRunRecord>>;

    async fn agent_runs_for_conversation(
        &self,
        conversation_id: ConversationId,
    ) -> jaco_db::Result<Vec<AgentRunRecord>>;

    async fn agent_runs_by_status(
        &self,
        status: AgentRunStatus,
    ) -> jaco_db::Result<Vec<AgentRunRecord>>;

    async fn update_agent_run_status(
        &self,
        id: AgentRunId,
        update: UpdateAgentRunStatus,
    ) -> jaco_db::Result<AgentRunRecord>;

    async fn finish_agent_run(
        &self,
        id: AgentRunId,
        finish: FinishAgentRun,
    ) -> jaco_db::Result<ConversationCommit<FinishedAgentRun>>;

    async fn next_provider_step_seq(&self, agent_run_id: AgentRunId) -> jaco_db::Result<i32>;

    async fn insert_provider_step(
        &self,
        input: NewProviderStep,
    ) -> jaco_db::Result<ProviderStepRecord>;

    async fn provider_steps_for_run(
        &self,
        agent_run_id: AgentRunId,
    ) -> jaco_db::Result<Vec<ProviderStepRecord>>;

    async fn update_provider_step_status(
        &self,
        id: ProviderStepId,
        update: UpdateProviderStepStatus,
    ) -> jaco_db::Result<ProviderStepRecord>;

    async fn insert_usage_event(&self, input: NewUsageEvent) -> jaco_db::Result<UsageEventRecord>;

    async fn insert_tool_invocation(
        &self,
        input: NewToolInvocation,
    ) -> jaco_db::Result<ToolInvocationRecord>;

    async fn tool_invocations_for_run(
        &self,
        agent_run_id: AgentRunId,
    ) -> jaco_db::Result<Vec<ToolInvocationRecord>>;

    async fn update_tool_invocation_status(
        &self,
        id: ToolInvocationId,
        update: UpdateToolInvocationStatus,
    ) -> jaco_db::Result<ToolInvocationRecord>;

    async fn append_entries_and_update_tool_invocation(
        &self,
        entries: Vec<NewConversationEntry>,
        tool_invocation_id: ToolInvocationId,
        update: UpdateToolInvocationStatus,
        approval: Option<ToolInvocationApproval>,
    ) -> jaco_db::Result<ConversationCommit<(Vec<ConversationEntryRecord>, ToolInvocationRecord)>>;

    async fn request_tool_invocation_approval_with_entry(
        &self,
        id: ToolInvocationId,
        approval: NewToolInvocationApproval,
        entry: NewConversationEntry,
    ) -> jaco_db::Result<ConversationCommit<(ConversationEntryRecord, ToolInvocationRecord)>>;

    async fn decide_tool_invocation_approval_with_entry(
        &self,
        id: ToolInvocationId,
        outcome: ToolInvocationApprovalOutcome,
        next_status: ToolInvocationStatus,
        entry: NewConversationEntry,
    ) -> jaco_db::Result<ConversationCommit<(ConversationEntryRecord, ToolInvocationRecord)>>;
}

#[cfg(test)]
pub(crate) struct DirectAgentPersistence {
    repository: jaco_db::FreshRepository,
}

#[cfg(test)]
impl DirectAgentPersistence {
    pub(crate) fn new(repository: jaco_db::FreshRepository) -> Self {
        Self { repository }
    }
}

#[cfg(test)]
macro_rules! direct {
    ($self:ident, $method:ident ( $($argument:expr),* $(,)? )) => {{
        let repository = $self.repository.clone();
        tokio::task::spawn_blocking(move || repository.$method($($argument),*))
            .await
            .map_err(|error| jaco_db::DbError::Invariant(format!("persistence worker failed: {error}")))?
    }};
}

#[cfg(test)]
#[async_trait]
impl AgentPersistence for DirectAgentPersistence {
    async fn conversation_timeline(
        &self,
        id: ConversationId,
    ) -> jaco_db::Result<Option<ConversationTimelineRecords>> {
        direct!(self, conversation_timeline_records(&id))
    }
    async fn conversation_entries(
        &self,
        id: ConversationId,
    ) -> jaco_db::Result<Vec<ConversationEntryRecord>> {
        direct!(self, conversation_entries(&id))
    }
    async fn append_conversation_entry(
        &self,
        input: NewConversationEntry,
    ) -> jaco_db::Result<ConversationCommit<ConversationEntryRecord>> {
        direct!(self, append_conversation_entry(input))
    }
    async fn update_conversation_entry_payload(
        &self,
        id: ConversationEntryId,
        status: ConversationEntryStatus,
        payload: ConversationEntryPayload,
    ) -> jaco_db::Result<ConversationCommit<ConversationEntryRecord>> {
        direct!(
            self,
            update_conversation_entry_payload(&id, status, payload)
        )
    }
    async fn insert_agent_run(&self, input: NewAgentRun) -> jaco_db::Result<AgentRunRecord> {
        direct!(self, insert_agent_run(input))
    }
    async fn get_agent_run(&self, id: AgentRunId) -> jaco_db::Result<Option<AgentRunRecord>> {
        direct!(self, get_agent_run(&id))
    }
    async fn agent_runs_for_conversation(
        &self,
        id: ConversationId,
    ) -> jaco_db::Result<Vec<AgentRunRecord>> {
        direct!(self, agent_runs_for_conversation(&id))
    }
    async fn agent_runs_by_status(
        &self,
        status: AgentRunStatus,
    ) -> jaco_db::Result<Vec<AgentRunRecord>> {
        direct!(self, agent_runs_by_status(status))
    }
    async fn update_agent_run_status(
        &self,
        id: AgentRunId,
        update: UpdateAgentRunStatus,
    ) -> jaco_db::Result<AgentRunRecord> {
        direct!(self, update_agent_run_status(&id, update))
    }
    async fn finish_agent_run(
        &self,
        id: AgentRunId,
        finish: FinishAgentRun,
    ) -> jaco_db::Result<ConversationCommit<FinishedAgentRun>> {
        direct!(self, finish_agent_run(&id, finish))
    }
    async fn next_provider_step_seq(&self, id: AgentRunId) -> jaco_db::Result<i32> {
        direct!(self, next_provider_step_seq(&id))
    }
    async fn insert_provider_step(
        &self,
        input: NewProviderStep,
    ) -> jaco_db::Result<ProviderStepRecord> {
        direct!(self, insert_provider_step(input))
    }
    async fn provider_steps_for_run(
        &self,
        id: AgentRunId,
    ) -> jaco_db::Result<Vec<ProviderStepRecord>> {
        direct!(self, provider_steps_for_run(&id))
    }
    async fn update_provider_step_status(
        &self,
        id: ProviderStepId,
        update: UpdateProviderStepStatus,
    ) -> jaco_db::Result<ProviderStepRecord> {
        direct!(self, update_provider_step_status(&id, update))
    }
    async fn insert_usage_event(&self, input: NewUsageEvent) -> jaco_db::Result<UsageEventRecord> {
        direct!(self, insert_usage_event(input))
    }
    async fn insert_tool_invocation(
        &self,
        input: NewToolInvocation,
    ) -> jaco_db::Result<ToolInvocationRecord> {
        direct!(self, insert_tool_invocation(input))
    }
    async fn tool_invocations_for_run(
        &self,
        id: AgentRunId,
    ) -> jaco_db::Result<Vec<ToolInvocationRecord>> {
        direct!(self, tool_invocations_for_run(&id))
    }
    async fn update_tool_invocation_status(
        &self,
        id: ToolInvocationId,
        update: UpdateToolInvocationStatus,
    ) -> jaco_db::Result<ToolInvocationRecord> {
        direct!(self, update_tool_invocation_status(&id, update))
    }
    async fn append_entries_and_update_tool_invocation(
        &self,
        entries: Vec<NewConversationEntry>,
        id: ToolInvocationId,
        update: UpdateToolInvocationStatus,
        approval: Option<ToolInvocationApproval>,
    ) -> jaco_db::Result<ConversationCommit<(Vec<ConversationEntryRecord>, ToolInvocationRecord)>>
    {
        direct!(
            self,
            append_conversation_entries_and_update_tool_invocation_full(
                entries, &id, update, approval
            )
        )
    }
    async fn request_tool_invocation_approval_with_entry(
        &self,
        id: ToolInvocationId,
        approval: NewToolInvocationApproval,
        entry: NewConversationEntry,
    ) -> jaco_db::Result<ConversationCommit<(ConversationEntryRecord, ToolInvocationRecord)>> {
        direct!(
            self,
            request_tool_invocation_approval_with_entry(&id, approval, entry)
        )
    }
    async fn decide_tool_invocation_approval_with_entry(
        &self,
        id: ToolInvocationId,
        outcome: ToolInvocationApprovalOutcome,
        next_status: ToolInvocationStatus,
        entry: NewConversationEntry,
    ) -> jaco_db::Result<ConversationCommit<(ConversationEntryRecord, ToolInvocationRecord)>> {
        direct!(
            self,
            decide_tool_invocation_approval_with_entry(&id, outcome, next_status, entry)
        )
    }
}

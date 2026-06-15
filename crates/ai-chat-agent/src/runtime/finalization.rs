use super::{AgentRuntime, emit_runtime};
use crate::{
    AgentRuntimeError, AgentRuntimeEvent, AgentRuntimeObserver, Result, persistence::run_error,
};
use ai_chat_core::*;
use ai_chat_db::{
    AgentRunRecord, NewConversationItem, ToolInvocationRecord, UpdateAgentRunStatus,
    UpdateProviderStepStatus, UpdateToolInvocationStatus,
};

impl AgentRuntime {
    pub(super) fn finalize_active_tool_invocations(
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

    pub(super) fn append_error_tool_result_and_update_tool_invocation(
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

    pub(super) fn finalize_active_provider_steps(
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

    pub(super) fn fail_active_provider_steps(
        &self,
        agent_run_id: &str,
        error: RunErrorPayload,
    ) -> Result<()> {
        self.finalize_active_provider_steps(agent_run_id, ProviderStepStatus::Failed, error)
    }

    pub(super) fn latest_assistant_item_id_for_run(
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

    pub(super) fn mark_setup_failed(
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
}

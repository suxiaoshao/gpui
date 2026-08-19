use super::{AgentRuntime, emit_runtime};
use crate::{AgentRuntimeError, AgentRuntimeEvent, AgentRuntimeObserver, Result};
use jaco_core::*;
use jaco_db::{
    AgentRunRecord, NewConversationEntry, ToolInvocationRecord, UpdateProviderStepStatus,
    UpdateToolInvocationStatus,
};

impl AgentRuntime {
    pub(super) async fn finalize_active_tool_invocations(
        &self,
        agent_run_id: &str,
        conversation_id: &str,
        status: ToolInvocationStatus,
        error: RunErrorPayload,
        observer: Option<&AgentRuntimeObserver>,
    ) -> Result<()> {
        for invocation in self
            .persistence
            .tool_invocations_for_run(agent_run_id.to_string())
            .await?
        {
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
                observer,
            )
            .await?;
        }
        Ok(())
    }

    pub(super) async fn append_error_tool_result_and_update_tool_invocation(
        &self,
        conversation_id: &str,
        invocation: &ToolInvocationRecord,
        status: ToolInvocationStatus,
        error: RunErrorPayload,
        observer: Option<&AgentRuntimeObserver>,
    ) -> Result<ConversationEntryId> {
        let output = ToolInvocationOutput {
            content: vec![ContentPart::Text {
                text: error.message.clone(),
            }],
            structured_output: None,
            raw_output: None,
            is_error: true,
        };
        let payload = ConversationEntryPayload::ToolResult(ToolResultEntry {
            tool_invocation_id: Some(invocation.id.clone()),
            call_id: invocation.call_id.clone(),
            content: output.content.clone(),
            is_error: true,
            structured_output: None,
            raw_output: None,
        });
        let approval = terminalized_pending_approval(invocation);
        let mut entries = Vec::new();
        if approval
            .as_ref()
            .and_then(|approval| approval.decision.as_ref())
            .is_some()
        {
            let decision = approval
                .as_ref()
                .and_then(|approval| approval.decision.clone())
                .expect("approval decision checked above");
            entries.push(NewConversationEntry {
                conversation_id: conversation_id.to_string(),
                status: ConversationEntryStatus::Completed,
                agent_run_id: Some(invocation.agent_run_id.clone()),
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
            conversation_id: conversation_id.to_string(),
            status: ConversationEntryStatus::Completed,
            agent_run_id: Some(invocation.agent_run_id.clone()),
            provider_step_id: invocation.provider_step_id.clone(),
            tool_invocation_id: Some(invocation.id.clone()),
            provider_item_id: None,
            payload,
        });
        let commit = self
            .persistence
            .append_entries_and_update_tool_invocation(
                entries,
                invocation.id.clone(),
                UpdateToolInvocationStatus {
                    status,
                    output: Some(output),
                    error: Some(error),
                },
                approval,
            )
            .await?;
        emit_runtime(
            observer,
            AgentRuntimeEvent::ConversationCommitted {
                conversation: Box::new(commit.conversation.clone()),
                changes: commit
                    .value
                    .0
                    .iter()
                    .cloned()
                    .map(|entry| ConversationChange::EntryAppended {
                        entry: Box::new(entry),
                    })
                    .chain(std::iter::once(ConversationChange::ToolInvocationChanged {
                        invocation: Box::new(commit.value.1.clone()),
                    }))
                    .collect(),
            },
        );
        let items = commit.value.0;
        items.last().map(|item| item.id.clone()).ok_or_else(|| {
            AgentRuntimeError::Invariant(format!(
                "tool invocation {} finalization created no entries",
                invocation.id
            ))
        })
    }

    pub(super) async fn finalize_active_provider_steps(
        &self,
        agent_run_id: &str,
        status: ProviderStepStatus,
        error: RunErrorPayload,
    ) -> Result<()> {
        for step in self
            .persistence
            .provider_steps_for_run(agent_run_id.to_string())
            .await?
        {
            if !matches!(
                step.status,
                ProviderStepStatus::Queued | ProviderStepStatus::Running
            ) {
                continue;
            }

            self.persistence
                .update_provider_step_status(
                    step.id,
                    UpdateProviderStepStatus {
                        status,
                        response_snapshot: None,
                        state_snapshot: None,
                        error: Some(error.clone()),
                    },
                )
                .await?;
        }
        Ok(())
    }

    pub(super) async fn fail_active_provider_steps(
        &self,
        agent_run_id: &str,
        error: RunErrorPayload,
    ) -> Result<()> {
        self.finalize_active_provider_steps(agent_run_id, ProviderStepStatus::Failed, error)
            .await
    }

    pub(super) async fn latest_assistant_entry_id_for_run(
        &self,
        run: &AgentRunRecord,
    ) -> Result<Option<ConversationEntryId>> {
        Ok(self
            .persistence
            .conversation_entries(run.conversation_id.clone())
            .await?
            .into_iter()
            .rev()
            .find(|item| {
                item.agent_run_id.as_deref() == Some(run.id.as_str())
                    && matches!(
                        item.payload,
                        ConversationEntryPayload::Message {
                            role: TranscriptRole::Assistant,
                            ..
                        }
                    )
            })
            .map(|item| item.id))
    }
}

fn terminalized_pending_approval(
    invocation: &ToolInvocationRecord,
) -> Option<ToolInvocationApproval> {
    let mut approval = invocation.approval.clone()?;
    if approval.status == ApprovalStatus::Pending {
        approval.status = ApprovalStatus::Canceled;
        approval.decision = None;
        approval.decided_at = Some(time::OffsetDateTime::now_utc());
        approval.expires_at = None;
    }
    Some(approval)
}

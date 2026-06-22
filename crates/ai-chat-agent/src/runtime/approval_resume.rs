use super::{AgentRuntime, emit_runtime, error_tool_output, is_terminal_agent_run_status};
use crate::{
    AgentCancellationToken, AgentRuntimeError, AgentRuntimeEvent, AgentRuntimeObserver, AgentStep,
    ApprovalResumeOutcome, Result, persistence::run_error,
    tool_registry::tool_output_to_model_text,
};
use ai_chat_core::*;
use ai_chat_db::{
    AgentRunRecord, NewConversationItem, ToolInvocationApproval, ToolInvocationApprovalOutcome,
    ToolInvocationRecord, UpdateAgentRunStatus, UpdateToolInvocationStatus,
};
use std::{future::Future, time::Duration};
use tokio::time::timeout;

impl AgentRuntime {
    pub async fn approve_and_resume_tool(
        &self,
        tool_invocation_id: &str,
        decided_by: String,
        reason: Option<String>,
        tool_timeout: Duration,
        cancellation_token: AgentCancellationToken,
        observer: Option<&AgentRuntimeObserver>,
    ) -> Result<ApprovalResumeOutcome> {
        let invocation = self.pending_approval_invocation(tool_invocation_id)?;
        if !matches!(invocation.source, ToolSource::Local) {
            return Err(AgentRuntimeError::Unsupported(format!(
                "approved resume only supports local built-in tools, got {:?}",
                invocation.source
            )));
        }

        let agent_run = self.agent_run_for_approval(&invocation)?;
        let policy = &agent_run.input.settings_snapshot.tool_policy;
        let access_requests = crate::builtin_tools::registry::access_requests_for_builtin_tool(
            &invocation.tool_name,
            &invocation.input.arguments.value,
            policy,
        )?
        .ok_or_else(|| {
            AgentRuntimeError::Unsupported(format!(
                "approved resume only supports built-in local tools, got {}",
                invocation.tool_name
            ))
        })?;
        let evaluator =
            crate::builtin_tools::approval::ToolPermissionEvaluator::from_policy(policy, None)?;
        match evaluator.evaluate(&access_requests) {
            crate::builtin_tools::approval::ToolPermissionDecision::Deny { reason } => {
                return Err(AgentRuntimeError::Invariant(format!(
                    "approved tool call is denied by policy: {reason}"
                )));
            }
            crate::builtin_tools::approval::ToolPermissionDecision::Allow { .. }
            | crate::builtin_tools::approval::ToolPermissionDecision::Ask { .. } => {}
        }

        let running_invocation = self.repo.update_tool_invocation_approval(
            tool_invocation_id,
            ToolInvocationApprovalOutcome::Approved { decided_by, reason },
            ToolInvocationStatus::Running,
        )?;
        emit_runtime(
            observer,
            AgentRuntimeEvent::ToolInvocationChanged {
                agent_run_id: running_invocation.agent_run_id.clone(),
                tool_invocation_id: running_invocation.id.clone(),
            },
        );
        let mut steps = vec![AgentStep::ToolInvocation(running_invocation.id.clone())];
        if cancellation_token.is_cancelled() {
            return self.cancel_approved_tool_resume(
                &agent_run,
                &running_invocation,
                steps,
                observer,
            );
        }

        let (tool_output, tool_status, tool_error) = tokio::select! {
            biased;
            _ = cancellation_token.cancelled() => {
                return self.cancel_approved_tool_resume(
                    &agent_run,
                    &running_invocation,
                    steps,
                    observer,
                );
            }
            result = approved_builtin_tool_result_from_execution(
                &running_invocation.tool_name,
                crate::builtin_tools::registry::execute_builtin_tool(
                    &running_invocation.tool_name,
                    running_invocation.input.arguments.value.clone(),
                    policy,
                ),
                tool_timeout,
            ) => result,
        };
        if cancellation_token.is_cancelled() {
            return self.cancel_approved_tool_resume(
                &agent_run,
                &running_invocation,
                steps,
                observer,
            );
        }

        let (tool_result_item_id, finished_invocation) = self
            .append_tool_result_and_update_tool_invocation(
                &agent_run.conversation_id,
                &running_invocation,
                tool_status,
                tool_output,
                tool_error.clone(),
                observer,
            )?;
        steps.push(AgentStep::ConversationItem(tool_result_item_id.clone()));
        let output = AgentRunOutput {
            final_item_id: Some(tool_result_item_id),
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
            observer,
            AgentRuntimeEvent::AgentRunStatusChanged {
                agent_run_id: agent_run.id.clone(),
                status: AgentRunStatus::Completed,
            },
        );
        let events = vec![
            AgentRunEvent::ToolInvocationFinished {
                tool_invocation_id: finished_invocation.id.clone(),
            },
            AgentRunEvent::Completed {
                output: output.clone(),
            },
        ];

        Ok(ApprovalResumeOutcome {
            tool_invocation: finished_invocation,
            agent_run,
            output,
            events,
            steps,
        })
    }

    fn cancel_approved_tool_resume(
        &self,
        agent_run: &AgentRunRecord,
        invocation: &ToolInvocationRecord,
        mut steps: Vec<AgentStep>,
        observer: Option<&AgentRuntimeObserver>,
    ) -> Result<ApprovalResumeOutcome> {
        let error = run_error("canceled", "runtime canceled", false, None);
        let tool_result_item_id = self.append_error_tool_result_and_update_tool_invocation(
            &agent_run.conversation_id,
            invocation,
            ToolInvocationStatus::Canceled,
            error,
        )?;
        let tool_invocation = self
            .repo
            .get_tool_invocation(&invocation.id)?
            .ok_or_else(|| {
                AgentRuntimeError::Invariant(format!(
                    "tool invocation {} is missing",
                    invocation.id
                ))
            })?;
        steps.push(AgentStep::ConversationItem(tool_result_item_id.clone()));
        let output = AgentRunOutput {
            final_item_id: Some(tool_result_item_id),
            stopped_reason: AgentStoppedReason::Canceled,
        };
        let agent_run = self.repo.update_agent_run_status(
            &agent_run.id,
            UpdateAgentRunStatus {
                status: AgentRunStatus::Canceled,
                output: Some(output.clone()),
                error: None,
            },
        )?;
        emit_runtime(
            observer,
            AgentRuntimeEvent::AgentRunStatusChanged {
                agent_run_id: agent_run.id.clone(),
                status: AgentRunStatus::Canceled,
            },
        );
        Ok(ApprovalResumeOutcome {
            tool_invocation,
            agent_run,
            output,
            events: vec![
                AgentRunEvent::ToolInvocationFinished {
                    tool_invocation_id: invocation.id.clone(),
                },
                AgentRunEvent::Canceled,
            ],
            steps,
        })
    }

    fn append_tool_result_and_update_tool_invocation(
        &self,
        conversation_id: &str,
        invocation: &ToolInvocationRecord,
        status: ToolInvocationStatus,
        output: ToolInvocationOutput,
        error: Option<RunErrorPayload>,
        observer: Option<&AgentRuntimeObserver>,
    ) -> Result<(ConversationItemId, ToolInvocationRecord)> {
        let payload = ConversationItemPayload::ToolResult(ToolResultItem {
            tool_invocation_id: Some(invocation.id.clone()),
            call_id: invocation.call_id.clone(),
            content: output.content.clone(),
            is_error: output.is_error,
            structured_output: output.structured_output.clone(),
            raw_output: output.raw_output.clone(),
        });
        let (item, invocation) = self
            .repo
            .append_conversation_item_and_update_tool_invocation_full(
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
                    error,
                },
                invocation.approval.clone(),
            )?;
        emit_runtime(
            observer,
            AgentRuntimeEvent::ConversationItemAppended {
                conversation_id: conversation_id.to_string(),
                item_id: item.id.clone(),
            },
        );
        emit_runtime(
            observer,
            AgentRuntimeEvent::ToolInvocationChanged {
                agent_run_id: invocation.agent_run_id.clone(),
                tool_invocation_id: invocation.id.clone(),
            },
        );
        Ok((item.id, invocation))
    }

    pub fn decide_approval(
        &self,
        tool_invocation_id: &str,
        outcome: ToolInvocationApprovalOutcome,
    ) -> Result<ToolInvocationRecord> {
        if matches!(outcome, ToolInvocationApprovalOutcome::Approved { .. }) {
            return Err(AgentRuntimeError::Unsupported(
                "approved tool resume must use approve_and_resume_tool".to_string(),
            ));
        }

        let invocation = self.pending_approval_invocation(tool_invocation_id)?;
        let agent_run = self.agent_run_for_approval(&invocation)?;
        let (status, error, stopped_reason, run_status, run_error_payload) =
            terminal_approval_result(&outcome);
        let approval = approval_after_outcome(&invocation, outcome)?;
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
        let (item, invocation) = self
            .repo
            .append_conversation_item_and_update_tool_invocation_full(
                NewConversationItem {
                    conversation_id: agent_run.conversation_id.clone(),
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
                    error: Some(error.clone()),
                },
                Some(approval),
            )?;
        let output = AgentRunOutput {
            final_item_id: Some(item.id),
            stopped_reason,
        };
        self.repo.update_agent_run_status(
            &agent_run.id,
            UpdateAgentRunStatus {
                status: run_status,
                output: Some(output),
                error: run_error_payload.then_some(error),
            },
        )?;
        Ok(invocation)
    }

    fn pending_approval_invocation(
        &self,
        tool_invocation_id: &str,
    ) -> Result<ToolInvocationRecord> {
        let invocation = self
            .repo
            .get_tool_invocation(tool_invocation_id)?
            .ok_or_else(|| {
                AgentRuntimeError::Invariant(format!(
                    "tool invocation {tool_invocation_id} is missing"
                ))
            })?;
        if invocation.status != ToolInvocationStatus::AwaitingApproval {
            return Err(AgentRuntimeError::Invariant(format!(
                "tool invocation {} is {:?}, not awaiting approval",
                invocation.id, invocation.status
            )));
        }
        let approval = invocation.approval.as_ref().ok_or_else(|| {
            AgentRuntimeError::Invariant(format!(
                "tool invocation {} has no approval",
                invocation.id
            ))
        })?;
        if approval.status != ApprovalStatus::Pending {
            return Err(AgentRuntimeError::Invariant(format!(
                "tool invocation {} approval is {:?}, not pending",
                invocation.id, approval.status
            )));
        }
        Ok(invocation)
    }

    fn agent_run_for_approval(&self, invocation: &ToolInvocationRecord) -> Result<AgentRunRecord> {
        let agent_run = self
            .repo
            .get_agent_run(&invocation.agent_run_id)?
            .ok_or_else(|| {
                AgentRuntimeError::Invariant(format!(
                    "agent run {} is missing",
                    invocation.agent_run_id
                ))
            })?;
        if is_terminal_agent_run_status(agent_run.status) {
            return Err(AgentRuntimeError::Invariant(format!(
                "agent run {} is {:?}, cannot decide tool invocation {}",
                agent_run.id, agent_run.status, invocation.id
            )));
        }
        Ok(agent_run)
    }
}

fn approval_after_outcome(
    invocation: &ToolInvocationRecord,
    outcome: ToolInvocationApprovalOutcome,
) -> Result<ToolInvocationApproval> {
    let mut approval = invocation.approval.clone().ok_or_else(|| {
        AgentRuntimeError::Invariant(format!("tool invocation {} has no approval", invocation.id))
    })?;
    if approval.status != ApprovalStatus::Pending {
        return Err(AgentRuntimeError::Invariant(format!(
            "tool invocation {} approval is {:?}, not pending",
            invocation.id, approval.status
        )));
    }
    let now = time::OffsetDateTime::now_utc();
    match outcome {
        ToolInvocationApprovalOutcome::Approved { decided_by, reason } => {
            approval.status = ApprovalStatus::Approved;
            approval.decision = Some(ApprovalDecisionPayload {
                approved: true,
                decided_by,
                reason,
            });
        }
        ToolInvocationApprovalOutcome::Denied { decided_by, reason } => {
            approval.status = ApprovalStatus::Denied;
            approval.decision = Some(ApprovalDecisionPayload {
                approved: false,
                decided_by,
                reason,
            });
        }
        ToolInvocationApprovalOutcome::Expired => {
            approval.status = ApprovalStatus::Expired;
            approval.decision = None;
        }
        ToolInvocationApprovalOutcome::Canceled => {
            approval.status = ApprovalStatus::Canceled;
            approval.decision = None;
        }
    }
    approval.decided_at = Some(now);
    approval.expires_at = None;
    Ok(approval)
}

fn terminal_approval_result(
    outcome: &ToolInvocationApprovalOutcome,
) -> (
    ToolInvocationStatus,
    RunErrorPayload,
    AgentStoppedReason,
    AgentRunStatus,
    bool,
) {
    match outcome {
        ToolInvocationApprovalOutcome::Denied { reason, .. } => (
            ToolInvocationStatus::Denied,
            run_error(
                "approval_denied",
                reason
                    .clone()
                    .unwrap_or_else(|| "Tool call denied by user".to_string()),
                false,
                None,
            ),
            AgentStoppedReason::Failed,
            AgentRunStatus::Failed,
            true,
        ),
        ToolInvocationApprovalOutcome::Canceled => (
            ToolInvocationStatus::Canceled,
            run_error("approval_canceled", "Tool approval canceled", false, None),
            AgentStoppedReason::Canceled,
            AgentRunStatus::Canceled,
            false,
        ),
        ToolInvocationApprovalOutcome::Expired => (
            ToolInvocationStatus::Failed,
            run_error("approval_expired", "Tool approval expired", true, None),
            AgentStoppedReason::Failed,
            AgentRunStatus::Failed,
            true,
        ),
        ToolInvocationApprovalOutcome::Approved { .. } => unreachable!(),
    }
}

pub(super) async fn approved_builtin_tool_result_from_execution(
    tool_name: &str,
    execution: impl Future<Output = Result<Option<ToolInvocationOutput>>>,
    tool_timeout: Duration,
) -> (
    ToolInvocationOutput,
    ToolInvocationStatus,
    Option<RunErrorPayload>,
) {
    match timeout(tool_timeout, execution).await {
        Ok(Ok(Some(output))) => {
            let error = output
                .is_error
                .then(|| run_error("tool_error", tool_output_to_model_text(&output), true, None));
            let status = if output.is_error {
                ToolInvocationStatus::Failed
            } else {
                ToolInvocationStatus::Succeeded
            };
            (output, status, error)
        }
        Ok(Ok(None)) => {
            let error = run_error(
                "tool_error",
                format!("built-in tool {tool_name} is not registered"),
                true,
                None,
            );
            (
                error_tool_output(error.message.clone()),
                ToolInvocationStatus::Failed,
                Some(error),
            )
        }
        Ok(Err(error)) => {
            let error = run_error("tool_error", error.to_string(), true, None);
            (
                error_tool_output(error.message.clone()),
                ToolInvocationStatus::Failed,
                Some(error),
            )
        }
        Err(_) => {
            let error = run_error("tool_timeout", "tool execution timed out", true, None);
            (
                error_tool_output(error.message.clone()),
                ToolInvocationStatus::Failed,
                Some(error),
            )
        }
    }
}

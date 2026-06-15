use super::{AgentRuntime, emit_runtime, error_tool_output};
use crate::{
    AgentRuntimeError, AgentRuntimeEvent, AgentRuntimeObserver, AgentStep, ApprovalResumeOutcome,
    Result, persistence::run_error, tool_registry::tool_output_to_model_text,
};
use ai_chat_core::*;
use ai_chat_db::{
    ApprovalDecisionRecord, NewApprovalDecisionOutcome, NewConversationItem, ToolInvocationRecord,
    UpdateAgentRunStatus, UpdateToolInvocationStatus,
};

impl AgentRuntime {
    pub async fn approve_and_resume_tool(
        &self,
        approval_decision_id: &str,
        decided_by: String,
        reason: Option<String>,
        observer: Option<&AgentRuntimeObserver>,
    ) -> Result<ApprovalResumeOutcome> {
        let approval = self
            .repo
            .get_approval_decision(approval_decision_id)?
            .ok_or_else(|| {
                AgentRuntimeError::Invariant(format!(
                    "approval decision {approval_decision_id} is missing"
                ))
            })?;
        if approval.status != ApprovalStatus::Pending {
            return Err(AgentRuntimeError::Invariant(format!(
                "approval decision {approval_decision_id} is {:?}, not pending",
                approval.status
            )));
        }

        let invocation = self
            .repo
            .get_tool_invocation(&approval.tool_invocation_id)?
            .ok_or_else(|| {
                AgentRuntimeError::Invariant(format!(
                    "tool invocation {} is missing",
                    approval.tool_invocation_id
                ))
            })?;
        if invocation.status != ToolInvocationStatus::AwaitingApproval {
            return Err(AgentRuntimeError::Invariant(format!(
                "tool invocation {} is {:?}, not awaiting approval",
                invocation.id, invocation.status
            )));
        }
        if !matches!(invocation.source, ToolSource::Local) {
            return Err(AgentRuntimeError::Unsupported(format!(
                "approved resume only supports local built-in tools, got {:?}",
                invocation.source
            )));
        }

        let agent_run = self
            .repo
            .get_agent_run(&invocation.agent_run_id)?
            .ok_or_else(|| {
                AgentRuntimeError::Invariant(format!(
                    "agent run {} is missing",
                    invocation.agent_run_id
                ))
            })?;
        if agent_run.status != AgentRunStatus::WaitingForApproval {
            return Err(AgentRuntimeError::Invariant(format!(
                "agent run {} is {:?}, not waiting for approval",
                agent_run.id, agent_run.status
            )));
        }

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

        let updated_approval = self.repo.update_approval_decision(
            approval_decision_id,
            NewApprovalDecisionOutcome::Approved { decided_by, reason },
        )?;
        let decision_item_id = self.append_approval_decision_item(
            &agent_run.conversation_id,
            &invocation,
            &updated_approval,
            observer,
        )?;
        let mut steps = vec![AgentStep::Approval(updated_approval.id.clone())];
        if let Some(item_id) = decision_item_id {
            steps.push(AgentStep::ConversationItem(item_id));
        }

        let running_run = self.repo.update_agent_run_status(
            &agent_run.id,
            UpdateAgentRunStatus {
                status: AgentRunStatus::Running,
                output: None,
                error: None,
            },
        )?;
        emit_runtime(
            observer,
            AgentRuntimeEvent::AgentRunStatusChanged {
                agent_run_id: running_run.id.clone(),
                status: AgentRunStatus::Running,
            },
        );

        let running_invocation = self.repo.update_tool_invocation_status(
            &invocation.id,
            UpdateToolInvocationStatus {
                status: ToolInvocationStatus::Running,
                output: None,
                error: None,
            },
        )?;
        emit_runtime(
            observer,
            AgentRuntimeEvent::ToolInvocationChanged {
                agent_run_id: running_invocation.agent_run_id.clone(),
                tool_invocation_id: running_invocation.id.clone(),
            },
        );
        steps.push(AgentStep::ToolInvocation(running_invocation.id.clone()));

        let (tool_output, tool_status, tool_error) =
            match crate::builtin_tools::registry::execute_builtin_tool(
                &running_invocation.tool_name,
                running_invocation.input.arguments.value.clone(),
                policy,
            )
            .await
            {
                Ok(Some(output)) => {
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
                Ok(None) => {
                    let error = run_error(
                        "tool_error",
                        format!(
                            "built-in tool {} is not registered",
                            running_invocation.tool_name
                        ),
                        true,
                        None,
                    );
                    (
                        error_tool_output(error.message.clone()),
                        ToolInvocationStatus::Failed,
                        Some(error),
                    )
                }
                Err(error) => {
                    let error = run_error("tool_error", error.to_string(), true, None);
                    (
                        error_tool_output(error.message.clone()),
                        ToolInvocationStatus::Failed,
                        Some(error),
                    )
                }
            };

        let tool_result_item_id = self.append_tool_result_and_update_tool_invocation(
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
            stopped_reason: if tool_error.is_some() {
                AgentStoppedReason::Failed
            } else {
                AgentStoppedReason::Completed
            },
        };
        let final_status = if tool_error.is_some() {
            AgentRunStatus::Failed
        } else {
            AgentRunStatus::Completed
        };
        let agent_run = self.repo.update_agent_run_status(
            &agent_run.id,
            UpdateAgentRunStatus {
                status: final_status,
                output: Some(output.clone()),
                error: tool_error.clone(),
            },
        )?;
        emit_runtime(
            observer,
            AgentRuntimeEvent::AgentRunStatusChanged {
                agent_run_id: agent_run.id.clone(),
                status: final_status,
            },
        );
        let mut events = vec![AgentRunEvent::ToolInvocationFinished {
            tool_invocation_id: running_invocation.id,
        }];
        if let Some(error) = tool_error {
            events.push(AgentRunEvent::Failed { error });
        } else {
            events.push(AgentRunEvent::Completed {
                output: output.clone(),
            });
        }

        Ok(ApprovalResumeOutcome {
            approval: updated_approval,
            agent_run,
            output,
            events,
            steps,
        })
    }

    fn append_approval_decision_item(
        &self,
        conversation_id: &str,
        invocation: &ToolInvocationRecord,
        approval: &ApprovalDecisionRecord,
        observer: Option<&AgentRuntimeObserver>,
    ) -> Result<Option<ConversationItemId>> {
        let Some(decision) = approval.decision.as_ref() else {
            return Ok(None);
        };
        let payload = ConversationItemPayload::ApprovalDecision(ApprovalDecisionItem {
            approval_decision_id: approval.id.clone(),
            decision: decision.clone(),
        });
        let item = self.repo.append_conversation_item(NewConversationItem {
            conversation_id: conversation_id.to_string(),
            status: ConversationItemStatus::Completed,
            agent_run_id: Some(invocation.agent_run_id.clone()),
            provider_step_id: invocation.provider_step_id.clone(),
            tool_invocation_id: Some(invocation.id.clone()),
            provider_item_id: None,
            payload,
        })?;
        emit_runtime(
            observer,
            AgentRuntimeEvent::ConversationItemAppended {
                conversation_id: conversation_id.to_string(),
                item_id: item.id.clone(),
            },
        );
        Ok(Some(item.id))
    }

    fn append_tool_result_and_update_tool_invocation(
        &self,
        conversation_id: &str,
        invocation: &ToolInvocationRecord,
        status: ToolInvocationStatus,
        output: ToolInvocationOutput,
        error: Option<RunErrorPayload>,
        observer: Option<&AgentRuntimeObserver>,
    ) -> Result<ConversationItemId> {
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
        Ok(item.id)
    }

    pub fn decide_approval(
        &self,
        approval_decision_id: &str,
        outcome: NewApprovalDecisionOutcome,
    ) -> Result<ApprovalDecisionRecord> {
        enum TerminalApproval {
            Denied { message: String },
            Canceled,
            Expired,
            Pending,
        }

        let terminal = match &outcome {
            NewApprovalDecisionOutcome::Approved { .. } => {
                return Err(AgentRuntimeError::Unsupported(
                    "approved tool resume is not implemented in v1".to_string(),
                ));
            }
            NewApprovalDecisionOutcome::Denied { reason, .. } => TerminalApproval::Denied {
                message: reason
                    .clone()
                    .unwrap_or_else(|| "Tool call denied by user".to_string()),
            },
            NewApprovalDecisionOutcome::Canceled => TerminalApproval::Canceled,
            NewApprovalDecisionOutcome::Expired => TerminalApproval::Expired,
            NewApprovalDecisionOutcome::Pending { .. } => TerminalApproval::Pending,
        };
        let approval = self
            .repo
            .get_approval_decision(approval_decision_id)?
            .ok_or_else(|| {
                AgentRuntimeError::Invariant(format!(
                    "approval decision {approval_decision_id} is missing"
                ))
            })?;
        let invocation = self
            .repo
            .get_tool_invocation(&approval.tool_invocation_id)?
            .ok_or_else(|| {
                AgentRuntimeError::Invariant(format!(
                    "tool invocation {} is missing",
                    approval.tool_invocation_id
                ))
            })?;
        let agent_run = self
            .repo
            .get_agent_run(&invocation.agent_run_id)?
            .ok_or_else(|| {
                AgentRuntimeError::Invariant(format!(
                    "agent run {} is missing",
                    invocation.agent_run_id
                ))
            })?;
        let updated = self
            .repo
            .update_approval_decision(approval_decision_id, outcome)?;
        if let Some(decision) = updated.decision.as_ref() {
            let payload = ConversationItemPayload::ApprovalDecision(ApprovalDecisionItem {
                approval_decision_id: updated.id.clone(),
                decision: decision.clone(),
            });
            self.repo.append_conversation_item(NewConversationItem {
                conversation_id: agent_run.conversation_id.clone(),
                status: ConversationItemStatus::Completed,
                agent_run_id: Some(invocation.agent_run_id.clone()),
                provider_step_id: invocation.provider_step_id.clone(),
                tool_invocation_id: Some(invocation.id.clone()),
                provider_item_id: None,
                payload,
            })?;
        }

        match terminal {
            TerminalApproval::Denied { message } => {
                let error = run_error("approval_denied", message, false, None);
                let item_id = self.append_error_tool_result_and_update_tool_invocation(
                    &agent_run.conversation_id,
                    &invocation,
                    ToolInvocationStatus::Denied,
                    error.clone(),
                )?;
                let output = AgentRunOutput {
                    final_item_id: Some(item_id),
                    stopped_reason: AgentStoppedReason::Failed,
                };
                self.repo.update_agent_run_status(
                    &agent_run.id,
                    UpdateAgentRunStatus {
                        status: AgentRunStatus::Failed,
                        output: Some(output),
                        error: Some(error),
                    },
                )?;
            }
            TerminalApproval::Canceled => {
                let error = run_error("approval_canceled", "Tool approval canceled", false, None);
                let item_id = self.append_error_tool_result_and_update_tool_invocation(
                    &agent_run.conversation_id,
                    &invocation,
                    ToolInvocationStatus::Canceled,
                    error,
                )?;
                let output = AgentRunOutput {
                    final_item_id: Some(item_id),
                    stopped_reason: AgentStoppedReason::Canceled,
                };
                self.repo.update_agent_run_status(
                    &agent_run.id,
                    UpdateAgentRunStatus {
                        status: AgentRunStatus::Canceled,
                        output: Some(output),
                        error: None,
                    },
                )?;
            }
            TerminalApproval::Expired => {
                let error = run_error("approval_expired", "Tool approval expired", true, None);
                let item_id = self.append_error_tool_result_and_update_tool_invocation(
                    &agent_run.conversation_id,
                    &invocation,
                    ToolInvocationStatus::Failed,
                    error.clone(),
                )?;
                let output = AgentRunOutput {
                    final_item_id: Some(item_id),
                    stopped_reason: AgentStoppedReason::Failed,
                };
                self.repo.update_agent_run_status(
                    &agent_run.id,
                    UpdateAgentRunStatus {
                        status: AgentRunStatus::Failed,
                        output: Some(output),
                        error: Some(error),
                    },
                )?;
            }
            TerminalApproval::Pending => {}
        }
        Ok(updated)
    }
}

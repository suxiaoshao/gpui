use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Mutex, MutexGuard},
};

use jaco_agent::{ToolApprovalBroker, ToolApprovalDecision, ToolApprovalRequest};
use jaco_core::{AgentRunId, ConversationId, ToolInvocationId};
use tokio::sync::oneshot;

pub(super) struct ConversationApprovalBroker {
    pending: Mutex<HashMap<ToolInvocationId, PendingApproval>>,
}

struct PendingApproval {
    conversation_id: ConversationId,
    agent_run_id: AgentRunId,
    sender: oneshot::Sender<ToolApprovalDecision>,
}

pub(super) struct ApprovalResolveOutcome {
    pub(super) conversation_id: ConversationId,
    pub(super) agent_run_id: AgentRunId,
    pub(super) remaining_for_run: usize,
}

impl ConversationApprovalBroker {
    pub(super) fn new() -> Self {
        Self {
            pending: Mutex::new(HashMap::new()),
        }
    }

    pub(super) fn resolve(
        &self,
        tool_invocation_id: &ToolInvocationId,
        decision: ToolApprovalDecision,
    ) -> Option<ApprovalResolveOutcome> {
        let mut pending = self.pending();
        let approval = pending.remove(tool_invocation_id)?;
        let remaining_for_run = pending
            .values()
            .filter(|pending| pending.agent_run_id == approval.agent_run_id)
            .count();
        drop(pending);

        let outcome = ApprovalResolveOutcome {
            conversation_id: approval.conversation_id,
            agent_run_id: approval.agent_run_id,
            remaining_for_run,
        };
        let _ = approval.sender.send(decision);
        Some(outcome)
    }

    pub(super) fn is_pending_for_run(
        &self,
        agent_run_id: &AgentRunId,
        tool_invocation_id: &ToolInvocationId,
    ) -> bool {
        self.pending()
            .get(tool_invocation_id)
            .is_some_and(|pending| &pending.agent_run_id == agent_run_id)
    }

    pub(super) fn pending_count_for_run(&self, agent_run_id: &AgentRunId) -> usize {
        self.pending()
            .values()
            .filter(|pending| &pending.agent_run_id == agent_run_id)
            .count()
    }

    pub(super) fn cancel_all_for_run(&self, agent_run_id: &AgentRunId) -> usize {
        let mut pending = self.pending();
        let tool_invocation_ids = pending
            .iter()
            .filter(|(_, pending)| &pending.agent_run_id == agent_run_id)
            .map(|(tool_invocation_id, _)| tool_invocation_id.clone())
            .collect::<Vec<_>>();
        let approvals = tool_invocation_ids
            .iter()
            .filter_map(|tool_invocation_id| pending.remove(tool_invocation_id))
            .collect::<Vec<_>>();
        drop(pending);

        let canceled = approvals.len();
        for approval in approvals {
            let _ = approval.sender.send(ToolApprovalDecision::Canceled);
        }
        canceled
    }

    pub(super) fn cancel_all(&self) -> usize {
        let approvals = {
            let mut pending = self.pending();
            pending
                .drain()
                .map(|(_, pending)| pending)
                .collect::<Vec<_>>()
        };
        let canceled = approvals.len();
        for approval in approvals {
            let _ = approval.sender.send(ToolApprovalDecision::Canceled);
        }
        canceled
    }

    #[cfg(test)]
    pub(super) fn register_pending_for_test(
        &self,
        conversation_id: ConversationId,
        agent_run_id: AgentRunId,
        tool_invocation_id: ToolInvocationId,
    ) -> oneshot::Receiver<ToolApprovalDecision> {
        let (sender, receiver) = oneshot::channel();
        self.pending().insert(
            tool_invocation_id,
            PendingApproval {
                conversation_id,
                agent_run_id,
                sender,
            },
        );
        receiver
    }

    fn pending(&self) -> MutexGuard<'_, HashMap<ToolInvocationId, PendingApproval>> {
        match self.pending.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

impl ToolApprovalBroker for ConversationApprovalBroker {
    fn request_tool_approval<'a>(
        &'a self,
        request: ToolApprovalRequest,
    ) -> Pin<Box<dyn Future<Output = ToolApprovalDecision> + Send + 'a>> {
        let (sender, receiver) = oneshot::channel();
        {
            let mut pending = self.pending();
            if pending.contains_key(&request.tool_invocation_id) {
                return Box::pin(async { ToolApprovalDecision::Canceled });
            }
            pending.insert(
                request.tool_invocation_id,
                PendingApproval {
                    conversation_id: request.conversation_id,
                    agent_run_id: request.agent_run_id,
                    sender,
                },
            );
        }

        Box::pin(async move { receiver.await.unwrap_or(ToolApprovalDecision::Canceled) })
    }
}

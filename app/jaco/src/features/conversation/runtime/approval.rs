use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Mutex, MutexGuard},
};

use jaco_agent::{ToolApprovalBroker, ToolApprovalDecision, ToolApprovalRequest};
use jaco_core::{AgentRunId, ConversationId, ToolInvocationId};
use smol::channel::Sender;
use tokio::sync::oneshot;

use super::RuntimePublication;

pub(super) struct ConversationApprovalBroker {
    pending: Mutex<HashMap<ToolInvocationId, PendingApproval>>,
    publications: Sender<RuntimePublication>,
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
    pub(super) fn new(publications: Sender<RuntimePublication>) -> Self {
        Self {
            pending: Mutex::new(HashMap::new()),
            publications,
        }
    }

    pub(super) fn resolve_for(
        &self,
        conversation_id: &ConversationId,
        agent_run_id: &AgentRunId,
        tool_invocation_id: &ToolInvocationId,
        decision: ToolApprovalDecision,
    ) -> Option<ApprovalResolveOutcome> {
        let mut pending = self.pending();
        let matches = pending.get(tool_invocation_id).is_some_and(|pending| {
            &pending.conversation_id == conversation_id && &pending.agent_run_id == agent_run_id
        });
        if !matches {
            return None;
        }
        let approval = pending.remove(tool_invocation_id)?;
        let remaining_for_run = pending
            .values()
            .filter(|pending| {
                pending.conversation_id == approval.conversation_id
                    && pending.agent_run_id == approval.agent_run_id
            })
            .count();
        drop(pending);

        self.publish_availability(
            approval.conversation_id.clone(),
            approval.agent_run_id.clone(),
            tool_invocation_id.clone(),
        );
        let outcome = ApprovalResolveOutcome {
            conversation_id: approval.conversation_id,
            agent_run_id: approval.agent_run_id,
            remaining_for_run,
        };
        let _ = approval.sender.send(decision);
        Some(outcome)
    }

    pub(super) fn is_pending_for(
        &self,
        conversation_id: &ConversationId,
        agent_run_id: &AgentRunId,
        tool_invocation_id: &ToolInvocationId,
    ) -> bool {
        self.pending()
            .get(tool_invocation_id)
            .is_some_and(|pending| {
                &pending.conversation_id == conversation_id && &pending.agent_run_id == agent_run_id
            })
    }

    pub(super) fn has_pending_for_run(
        &self,
        conversation_id: &ConversationId,
        agent_run_id: &AgentRunId,
    ) -> bool {
        self.pending().values().any(|pending| {
            &pending.conversation_id == conversation_id && &pending.agent_run_id == agent_run_id
        })
    }

    pub(super) fn cancel_all_for_run(
        &self,
        conversation_id: &ConversationId,
        agent_run_id: &AgentRunId,
    ) -> usize {
        let mut pending = self.pending();
        let tool_invocation_ids = pending
            .iter()
            .filter(|(_, pending)| {
                &pending.conversation_id == conversation_id && &pending.agent_run_id == agent_run_id
            })
            .map(|(tool_invocation_id, _)| tool_invocation_id.clone())
            .collect::<Vec<_>>();
        let approvals = tool_invocation_ids
            .into_iter()
            .filter_map(|tool_invocation_id| {
                pending
                    .remove(&tool_invocation_id)
                    .map(|pending| (tool_invocation_id, pending))
            })
            .collect::<Vec<_>>();
        drop(pending);

        let canceled = approvals.len();
        for (tool_invocation_id, approval) in &approvals {
            self.publish_availability(
                approval.conversation_id.clone(),
                approval.agent_run_id.clone(),
                tool_invocation_id.clone(),
            );
        }
        for (_, approval) in approvals {
            let _ = approval.sender.send(ToolApprovalDecision::Canceled);
        }
        canceled
    }

    pub(super) fn cancel_all(&self) -> usize {
        let approvals = {
            let mut pending = self.pending();
            pending.drain().collect::<Vec<_>>()
        };
        let canceled = approvals.len();
        for (tool_invocation_id, approval) in &approvals {
            self.publish_availability(
                approval.conversation_id.clone(),
                approval.agent_run_id.clone(),
                tool_invocation_id.clone(),
            );
        }
        for (_, approval) in approvals {
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
            tool_invocation_id.clone(),
            PendingApproval {
                conversation_id: conversation_id.clone(),
                agent_run_id: agent_run_id.clone(),
                sender,
            },
        );
        self.publish_availability(conversation_id, agent_run_id, tool_invocation_id);
        receiver
    }

    fn publish_availability(
        &self,
        conversation_id: ConversationId,
        agent_run_id: AgentRunId,
        tool_invocation_id: ToolInvocationId,
    ) {
        let _ = self
            .publications
            .try_send(RuntimePublication::ToolApprovalAvailabilityChanged {
                conversation_id,
                agent_run_id,
                tool_invocation_id,
            });
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
                request.tool_invocation_id.clone(),
                PendingApproval {
                    conversation_id: request.conversation_id.clone(),
                    agent_run_id: request.agent_run_id.clone(),
                    sender,
                },
            );
        }

        self.publish_availability(
            request.conversation_id,
            request.agent_run_id,
            request.tool_invocation_id,
        );

        Box::pin(async move { receiver.await.unwrap_or(ToolApprovalDecision::Canceled) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jaco_core::{ApprovalRequestPayload, ToolSource};
    use smol::channel::Receiver;
    use std::collections::HashSet;

    fn broker() -> (ConversationApprovalBroker, Receiver<RuntimePublication>) {
        let (publications, receiver) = smol::channel::unbounded();
        (ConversationApprovalBroker::new(publications), receiver)
    }

    fn request(tool_invocation_id: &str) -> ToolApprovalRequest {
        ToolApprovalRequest {
            conversation_id: "conversation-1".to_string(),
            agent_run_id: "run-1".to_string(),
            tool_invocation_id: tool_invocation_id.to_string(),
            request: ApprovalRequestPayload {
                reason: "test approval".to_string(),
                tool_source: ToolSource::Local,
                tool_name: "echo".to_string(),
                arguments_preview: "{}".to_string(),
                access_requests: Vec::new(),
            },
        }
    }

    fn availability_key(
        publication: RuntimePublication,
    ) -> (ConversationId, AgentRunId, ToolInvocationId) {
        let RuntimePublication::ToolApprovalAvailabilityChanged {
            conversation_id,
            agent_run_id,
            tool_invocation_id,
        } = publication
        else {
            panic!("expected tool approval availability publication");
        };
        (conversation_id, agent_run_id, tool_invocation_id)
    }

    #[test]
    fn pending_authority_requires_the_exact_conversation_run_and_invocation() {
        let (broker, publications) = broker();
        let decision = broker.request_tool_approval(request("invocation-1"));
        assert_eq!(
            availability_key(publications.recv_blocking().unwrap()),
            (
                "conversation-1".to_string(),
                "run-1".to_string(),
                "invocation-1".to_string()
            )
        );

        assert!(broker.is_pending_for(
            &"conversation-1".to_string(),
            &"run-1".to_string(),
            &"invocation-1".to_string()
        ));
        assert!(broker.has_pending_for_run(&"conversation-1".to_string(), &"run-1".to_string()));
        assert!(!broker.has_pending_for_run(&"conversation-1".to_string(), &"run-2".to_string()));
        assert!(!broker.is_pending_for(
            &"conversation-2".to_string(),
            &"run-1".to_string(),
            &"invocation-1".to_string()
        ));
        assert!(!broker.is_pending_for(
            &"conversation-1".to_string(),
            &"run-2".to_string(),
            &"invocation-1".to_string()
        ));
        assert!(
            broker
                .resolve_for(
                    &"conversation-2".to_string(),
                    &"run-1".to_string(),
                    &"invocation-1".to_string(),
                    ToolApprovalDecision::Canceled,
                )
                .is_none()
        );
        assert!(publications.try_recv().is_err());

        assert!(
            broker
                .resolve_for(
                    &"conversation-1".to_string(),
                    &"run-1".to_string(),
                    &"invocation-1".to_string(),
                    ToolApprovalDecision::Canceled,
                )
                .is_some()
        );
        assert_eq!(
            availability_key(publications.recv_blocking().unwrap()).2,
            "invocation-1"
        );
        assert!(!broker.has_pending_for_run(&"conversation-1".to_string(), &"run-1".to_string()));
        assert_eq!(smol::block_on(decision), ToolApprovalDecision::Canceled);
    }

    #[test]
    fn duplicate_request_does_not_publish_or_replace_pending_authority() {
        let (broker, publications) = broker();
        let first = broker.request_tool_approval(request("invocation-1"));
        let _ = publications.recv_blocking().unwrap();
        let duplicate = broker.request_tool_approval(request("invocation-1"));

        assert_eq!(smol::block_on(duplicate), ToolApprovalDecision::Canceled);
        assert!(publications.try_recv().is_err());
        assert!(
            broker
                .resolve_for(
                    &"conversation-1".to_string(),
                    &"run-1".to_string(),
                    &"invocation-1".to_string(),
                    ToolApprovalDecision::Approved {
                        decided_by: "user".to_string(),
                        reason: None,
                    },
                )
                .is_some()
        );
        let _ = publications.recv_blocking().unwrap();
        assert!(matches!(
            smol::block_on(first),
            ToolApprovalDecision::Approved { .. }
        ));
    }

    #[test]
    fn resolve_publishes_availability_before_waking_the_waiter() {
        let (broker, publications) = broker();
        let decision = broker.request_tool_approval(request("invocation-1"));
        let _ = publications.recv_blocking().unwrap();
        let publication_sender = broker.publications.clone();

        std::thread::scope(|scope| {
            let waiter = scope.spawn(move || {
                let decision = smol::block_on(decision);
                let (acknowledgement, _receiver) = smol::channel::bounded(1);
                publication_sender
                    .send_blocking(RuntimePublication::Drain(acknowledgement))
                    .unwrap();
                decision
            });
            assert!(
                broker
                    .resolve_for(
                        &"conversation-1".to_string(),
                        &"run-1".to_string(),
                        &"invocation-1".to_string(),
                        ToolApprovalDecision::Canceled,
                    )
                    .is_some()
            );

            assert_eq!(
                availability_key(publications.recv_blocking().unwrap()).2,
                "invocation-1"
            );
            assert!(matches!(
                publications.recv_blocking().unwrap(),
                RuntimePublication::Drain(_)
            ));
            assert_eq!(waiter.join().unwrap(), ToolApprovalDecision::Canceled);
        });
    }

    #[test]
    fn batch_cancel_publishes_every_removal_before_waking_any_waiter() {
        let (broker, publications) = broker();
        let first = broker.request_tool_approval(request("invocation-1"));
        let second = broker.request_tool_approval(request("invocation-2"));
        let _ = publications.recv_blocking().unwrap();
        let _ = publications.recv_blocking().unwrap();

        std::thread::scope(|scope| {
            let waiters = [first, second]
                .into_iter()
                .map(|decision| {
                    let publication_sender = broker.publications.clone();
                    scope.spawn(move || {
                        let decision = smol::block_on(decision);
                        let (acknowledgement, _receiver) = smol::channel::bounded(1);
                        publication_sender
                            .send_blocking(RuntimePublication::Drain(acknowledgement))
                            .unwrap();
                        decision
                    })
                })
                .collect::<Vec<_>>();

            assert_eq!(
                broker.cancel_all_for_run(&"conversation-1".into(), &"run-1".into()),
                2
            );
            let removed = [
                availability_key(publications.recv_blocking().unwrap()).2,
                availability_key(publications.recv_blocking().unwrap()).2,
            ]
            .into_iter()
            .collect::<HashSet<_>>();
            assert_eq!(
                removed,
                HashSet::from(["invocation-1".to_string(), "invocation-2".to_string()])
            );
            for waiter in waiters {
                assert_eq!(waiter.join().unwrap(), ToolApprovalDecision::Canceled);
            }
        });
    }

    #[test]
    fn disconnected_publication_channel_does_not_block_mutation_or_decision() {
        let (broker, publications) = broker();
        drop(publications);
        let decision = broker.request_tool_approval(request("invocation-1"));
        assert!(broker.is_pending_for(
            &"conversation-1".into(),
            &"run-1".into(),
            &"invocation-1".into()
        ));
        assert!(
            broker
                .resolve_for(
                    &"conversation-1".into(),
                    &"run-1".into(),
                    &"invocation-1".into(),
                    ToolApprovalDecision::Canceled,
                )
                .is_some()
        );
        assert_eq!(smol::block_on(decision), ToolApprovalDecision::Canceled);
    }
}

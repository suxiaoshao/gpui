use jaco_core::{AgentRunId, ApprovalRequestPayload, ConversationId, ToolInvocationId};
use std::{future::Future, pin::Pin};

pub trait ToolApprovalBroker: Send + Sync {
    fn request_tool_approval<'a>(
        &'a self,
        request: ToolApprovalRequest,
    ) -> Pin<Box<dyn Future<Output = ToolApprovalDecision> + Send + 'a>>;
}

#[derive(Debug, Clone)]
pub struct ToolApprovalRequest {
    pub conversation_id: ConversationId,
    pub agent_run_id: AgentRunId,
    pub tool_invocation_id: ToolInvocationId,
    pub request: ApprovalRequestPayload,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ToolApprovalDecision {
    Approved {
        decided_by: String,
        reason: Option<String>,
    },
    Denied {
        decided_by: String,
        reason: Option<String>,
    },
    Canceled,
}

mod payloads;

pub use payloads::*;

pub type ProjectId = String;
pub type ConversationId = String;
pub type ConversationItemId = String;
pub type AttachmentId = String;
pub type AgentRunId = String;
pub type ProviderStepId = String;
pub type ToolInvocationId = String;
pub type ApprovalDecisionId = String;
pub type ProviderId = String;
pub type PromptId = String;
pub type ProviderModelId = String;
pub type ShortcutId = String;
pub type UsageEventId = String;

pub fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

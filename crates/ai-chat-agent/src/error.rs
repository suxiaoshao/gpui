use ai_chat_core::ToolInvocationId;

pub type Result<T> = std::result::Result<T, AgentRuntimeError>;

#[derive(Debug, thiserror::Error)]
pub enum AgentRuntimeError {
    #[error("database error: {0}")]
    Db(#[from] ai_chat_db::DbError),
    #[error("filesystem error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Rig completion error: {0}")]
    RigCompletion(#[from] rig_core::completion::CompletionError),
    #[error("Rig prompt error: {0}")]
    RigPrompt(#[from] Box<rig_core::completion::PromptError>),
    #[error("Rig tool server error: {0}")]
    RigToolServer(#[from] rig_core::tool::server::ToolServerError),
    #[error("MCP error: {0}")]
    Mcp(String),
    #[error("tool {tool_invocation_id} is waiting for approval")]
    WaitingForApproval {
        tool_invocation_id: ToolInvocationId,
    },
    #[error("runtime canceled")]
    Canceled,
    #[error("unsupported runtime operation: {0}")]
    Unsupported(String),
    #[error("runtime invariant failed: {0}")]
    Invariant(String),
}

impl From<rig_core::completion::PromptError> for AgentRuntimeError {
    fn from(value: rig_core::completion::PromptError) -> Self {
        Self::RigPrompt(Box::new(value))
    }
}

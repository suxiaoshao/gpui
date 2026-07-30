pub type Result<T> = std::result::Result<T, AgentRuntimeError>;

#[derive(Debug, thiserror::Error)]
pub enum AgentRuntimeError {
    #[error("database error: {0}")]
    Db(#[from] jaco_db::DbError),
    #[error("filesystem error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Rig completion error: {0}")]
    RigCompletion(#[from] rig::completion::CompletionError),
    #[error("Rig prompt error: {0}")]
    RigPrompt(#[from] Box<rig::completion::PromptError>),
    #[error("Rig tool server error: {0}")]
    RigToolServer(#[from] rig::tool::server::ToolServerError),
    #[error("MCP error: {0}")]
    Mcp(String),
    #[error("runtime canceled")]
    Canceled,
    #[error("unsupported runtime operation: {0}")]
    Unsupported(String),
    #[error("runtime invariant failed: {0}")]
    Invariant(String),
}

impl From<rig::completion::PromptError> for AgentRuntimeError {
    fn from(value: rig::completion::PromptError) -> Self {
        Self::RigPrompt(Box::new(value))
    }
}

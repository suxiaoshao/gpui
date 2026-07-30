mod error;
mod mcp;
mod persistence;
mod providers;
mod runtime;
mod skills;
mod tools;

pub use error::{AgentRuntimeError, Result};
pub use mcp::{
    McpConfigLayer, McpConnector, McpOAuthCredentialsSnapshot, McpOAuthStatusSnapshot,
    McpPreparedTools, McpRuntimeEvent, McpServerConfig, McpServerConnectionState,
    McpServerInfoSnapshot, McpServerRuntimeConfig, McpServerStatusSnapshot, McpServerTransport,
    McpServerTransportKindSnapshot, McpSessionManager, McpSessionPruneMode, McpStdioTransport,
    McpStreamableHttpTransport, McpToolRegistrationOptions, McpToolSnapshot,
};
pub use persistence::AgentPersistence;
pub use persistence::PersistingCompletionModel;
pub use providers::openai::OpenAiResponsesSessionPool;
pub use providers::{
    ProviderModelFetchError, ProviderModelFetchRequest, ProviderSecretValues,
    fetch_provider_models, provider_model_from_rig_model,
};
pub use runtime::{
    AgentRuntime, PreparingAgentRun,
    types::{
        AgentCancellationToken, AgentRunHandle, AgentRunHandleStatus, AgentRunRequest,
        AgentRuntimeEvent, AgentRuntimeObserver, AgentStep, CompletionModelFactory, RuntimeGuards,
    },
};
pub use skills::{
    SkillActivationRequest, SkillCatalog, SkillCatalogEntry, SkillCatalogWarning, SkillLoader,
};
pub use tools::{
    LocalTool, RegisteredToolDefinition, ToolDefinition, ToolExecutor, ToolRegistry, ToolRunPolicy,
    approval::{ToolApprovalBroker, ToolApprovalDecision, ToolApprovalRequest},
};

mod config_hash;

pub use config_hash::mcp_config_hash;

use crate::{AgentRuntimeError, Result, ToolDefinition, ToolRegistry, ToolRunPolicy};
use ai_chat_core::{
    McpRuntimeConfigSnapshot, McpToolApprovalModeSnapshot, ToolApprovalPolicy, ToolExecutionPolicy,
    ToolSource,
};
use http::{HeaderName, HeaderValue};
use rig_core::tool::rmcp::McpTool;
use rmcp::{
    ServiceExt,
    handler::client::ClientHandler,
    model::{ClientInfo, ServerInfo, Tool as RmcpToolDefinition},
    service::{NotificationContext, RoleClient, RunningService, ServerSink},
    transport::{
        StreamableHttpClientTransport, TokioChildProcess,
        streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::mpsc;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpConfigLayer {
    pub servers: Vec<McpServerConfig>,
}

impl McpConfigLayer {
    pub fn merge_ordered(layers: impl IntoIterator<Item = McpConfigLayer>) -> Vec<McpServerConfig> {
        let mut servers = BTreeMap::new();
        for layer in layers {
            for server in layer.servers {
                servers.insert(server.server_id.clone(), server);
            }
        }
        servers.into_values().collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpServerConfig {
    pub server_id: String,
    pub display_name: Option<String>,
    pub transport: McpServerTransport,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    pub cwd: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum McpServerTransport {
    Stdio(McpStdioTransport),
    StreamableHttp(McpStreamableHttpTransport),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpStdioTransport {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpStreamableHttpTransport {
    pub url: String,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    pub oauth: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServerRuntimeConfig {
    pub server: McpServerConfig,
    pub required: bool,
    pub startup_timeout: Duration,
    pub tool_timeout: Duration,
    pub enabled_tools: Option<BTreeSet<String>>,
    pub disabled_tools: BTreeSet<String>,
    pub default_approval_mode: McpToolApprovalModeSnapshot,
    pub default_approval_policy: ToolApprovalPolicy,
    pub execution_policy: ToolExecutionPolicy,
    pub tool_approval_overrides: BTreeMap<String, McpToolApprovalModeSnapshot>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct McpPreparedTools {
    pub config_hash: String,
    pub config_snapshot: McpRuntimeConfigSnapshot,
    pub statuses: Vec<McpServerStatusSnapshot>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum McpRuntimeEvent {
    ServerStatusChanged(Box<McpServerStatusSnapshot>),
    ToolsChanged {
        server_id: String,
        tools: Vec<McpToolSnapshot>,
    },
    OAuthChanged {
        server_id: String,
        status: McpOAuthStatusSnapshot,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct McpSessionKey {
    pub server_id: String,
    pub config_hash: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct McpToolSnapshot {
    pub name: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct McpServerInfoSnapshot {
    pub protocol_version: String,
    pub name: String,
    pub title: Option<String>,
    pub version: String,
    pub instructions: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpServerTransportKindSnapshot {
    Stdio,
    StreamableHttp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpServerConnectionState {
    Disabled,
    NotConnected,
    Connecting,
    Connected,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpOAuthStatusSnapshot {
    NotConfigured,
    SignedOut,
    SigningIn,
    Authorized {
        scopes: Vec<String>,
        expires_at_unix_ms: Option<u64>,
    },
    AuthorizationRequired,
    ScopeUpgradeRequired {
        required_scope: String,
        authorization_url: String,
    },
    Failed {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct McpServerStatusSnapshot {
    pub server_id: String,
    pub display_name: Option<String>,
    pub transport: McpServerTransportKindSnapshot,
    pub state: McpServerConnectionState,
    pub auth: McpOAuthStatusSnapshot,
    pub server_info: Option<McpServerInfoSnapshot>,
    pub tools: Vec<McpToolSnapshot>,
    pub last_error: Option<String>,
    pub updated_at_unix_ms: u64,
}

pub struct McpServerSession {
    pub sink: ServerSink,
    pub service: RunningService<RoleClient, McpClientHandler>,
    pub tools: Vec<RmcpToolDefinition>,
    pub status: McpServerStatusSnapshot,
}

#[derive(Default)]
pub struct McpSessionManager {
    sessions: BTreeMap<McpSessionKey, McpServerSession>,
    connector: McpConnector,
    event_tx: Option<mpsc::UnboundedSender<McpRuntimeEvent>>,
}

impl McpSessionManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_event_sender(mut self, sender: mpsc::UnboundedSender<McpRuntimeEvent>) -> Self {
        self.event_tx = Some(sender);
        self
    }

    pub fn status_snapshots(&self) -> Vec<McpServerStatusSnapshot> {
        self.sessions
            .values()
            .map(|session| session.status.clone())
            .collect()
    }

    pub async fn prepare_tool_registry(
        &mut self,
        registry: &mut ToolRegistry,
        config_snapshot: McpRuntimeConfigSnapshot,
        configs: Vec<McpServerRuntimeConfig>,
    ) -> Result<McpPreparedTools> {
        let config_hash = mcp_config_hash(&config_snapshot)?;
        let active_server_ids = configs
            .iter()
            .map(|config| config.server.server_id.clone())
            .collect::<BTreeSet<_>>();
        self.close_stale_sessions(&active_server_ids, &config_hash)
            .await;

        let mut statuses = Vec::new();
        for config in configs {
            match self
                .register_tools_for_server(registry, config, &config_hash)
                .await
            {
                Ok(status) => statuses.push(status),
                Err(err) => {
                    let status = failed_status(&config_hash, &err);
                    if status.server_id.is_empty() {
                        return Err(err);
                    }
                    statuses.push(status);
                }
            }
        }

        Ok(McpPreparedTools {
            config_hash,
            config_snapshot,
            statuses,
        })
    }

    async fn register_tools_for_server(
        &mut self,
        registry: &mut ToolRegistry,
        config: McpServerRuntimeConfig,
        config_hash: &str,
    ) -> Result<McpServerStatusSnapshot> {
        let required = config.required;
        let server_id = config.server.server_id.clone();
        let display_name = config.server.display_name.clone();
        let transport = transport_kind(&config.server.transport);
        let result = self.ensure_session(config.clone(), config_hash).await;
        let session = match result {
            Ok(session) => session,
            Err(err) if required => return Err(err),
            Err(err) => {
                return Ok(failed_server_status(
                    server_id,
                    display_name,
                    transport,
                    failed_auth_status(&config.server.transport),
                    err.to_string(),
                ));
            }
        };
        let sink = session.sink.clone();
        let tools = session.tools.clone();
        let status = session.status.clone();
        self.register_filtered_tools(registry, &config, tools, sink)?;
        Ok(status)
    }

    async fn ensure_session(
        &mut self,
        config: McpServerRuntimeConfig,
        config_hash: &str,
    ) -> Result<&mut McpServerSession> {
        let key = McpSessionKey {
            server_id: config.server.server_id.clone(),
            config_hash: config_hash.to_string(),
        };
        if self.sessions.contains_key(&key) {
            return Ok(self.sessions.get_mut(&key).expect("session key exists"));
        }

        let session = connect_mcp_server(config, self.event_tx.clone()).await?;
        self.emit(McpRuntimeEvent::ServerStatusChanged(Box::new(
            session.status.clone(),
        )));
        self.sessions.insert(key.clone(), session);
        Ok(self
            .sessions
            .get_mut(&key)
            .expect("inserted session exists"))
    }

    fn register_filtered_tools(
        &self,
        registry: &mut ToolRegistry,
        config: &McpServerRuntimeConfig,
        tools: Vec<RmcpToolDefinition>,
        sink: ServerSink,
    ) -> Result<()> {
        for tool in tools {
            let tool_name = tool.name.to_string();
            if !tool_allowed(&tool_name, config) {
                continue;
            }
            let approval_policy = approval_policy_for_tool(&tool_name, config);
            self.connector.register_rmcp_tool(
                registry,
                config.server.server_id.clone(),
                tool,
                sink.clone(),
                McpToolRegistrationOptions {
                    approval_policy,
                    execution_policy: config.execution_policy,
                    timeout_ms: Some(
                        config.tool_timeout.as_millis().min(u128::from(u64::MAX)) as u64
                    ),
                },
            )?;
        }
        Ok(())
    }

    async fn close_stale_sessions(
        &mut self,
        active_server_ids: &BTreeSet<String>,
        config_hash: &str,
    ) {
        let stale_keys = self
            .sessions
            .keys()
            .filter(|key| {
                key.config_hash != config_hash || !active_server_ids.contains(&key.server_id)
            })
            .cloned()
            .collect::<Vec<_>>();
        for key in stale_keys {
            if let Some(mut session) = self.sessions.remove(&key) {
                let _ = session
                    .service
                    .close_with_timeout(Duration::from_secs(5))
                    .await;
            }
        }
    }

    pub async fn disconnect_server(&mut self, server_id: &str) {
        let keys = self
            .sessions
            .keys()
            .filter(|key| key.server_id == server_id)
            .cloned()
            .collect::<Vec<_>>();
        for key in keys {
            if let Some(mut session) = self.sessions.remove(&key) {
                let _ = session
                    .service
                    .close_with_timeout(Duration::from_secs(5))
                    .await;
            }
        }
    }

    fn emit(&self, event: McpRuntimeEvent) {
        if let Some(sender) = &self.event_tx {
            let _ = sender.send(event);
        }
    }
}

#[derive(Default)]
pub struct McpConnector;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct McpToolRegistrationOptions {
    pub approval_policy: ToolApprovalPolicy,
    pub execution_policy: ToolExecutionPolicy,
    pub timeout_ms: Option<u64>,
}

impl McpConnector {
    pub fn new() -> Self {
        Self
    }

    pub fn register_rmcp_tools(
        &self,
        registry: &mut ToolRegistry,
        server_id: impl Into<String>,
        tools: impl IntoIterator<Item = RmcpToolDefinition>,
        client: ServerSink,
        approval_policy: ToolApprovalPolicy,
        execution_policy: ToolExecutionPolicy,
    ) -> Result<()> {
        let server_id = server_id.into();
        for tool in tools {
            self.register_rmcp_tool(
                registry,
                server_id.clone(),
                tool,
                client.clone(),
                McpToolRegistrationOptions {
                    approval_policy,
                    execution_policy,
                    timeout_ms: None,
                },
            )?;
        }
        Ok(())
    }

    pub fn register_rmcp_tool(
        &self,
        registry: &mut ToolRegistry,
        server_id: impl Into<String>,
        tool: RmcpToolDefinition,
        client: ServerSink,
        options: McpToolRegistrationOptions,
    ) -> Result<()> {
        let server_id = server_id.into();
        let original_name = tool.name.to_string();
        let description = tool
            .description
            .clone()
            .map(|description| description.to_string());
        let parameters = tool.schema_as_json_value();
        let mcp_tool = McpTool::from_mcp_server(tool, client);
        registry.register_mcp_tool(
            ToolDefinition {
                source: ToolSource::Mcp {
                    server_id: server_id.clone(),
                },
                namespace: Some(server_id),
                name: original_name,
                description: description.unwrap_or_default(),
                parameters,
                policy: ToolRunPolicy {
                    approval_policy: options.approval_policy,
                    execution_policy: options.execution_policy,
                    timeout_ms: options.timeout_ms,
                },
            },
            mcp_tool,
        )?;
        Ok(())
    }
}

pub struct McpClientHandler {
    server_id: String,
    event_tx: Option<mpsc::UnboundedSender<McpRuntimeEvent>>,
}

impl McpClientHandler {
    pub fn new(
        server_id: impl Into<String>,
        event_tx: Option<mpsc::UnboundedSender<McpRuntimeEvent>>,
    ) -> Self {
        Self {
            server_id: server_id.into(),
            event_tx,
        }
    }
}

impl ClientHandler for McpClientHandler {
    fn get_info(&self) -> ClientInfo {
        ClientInfo::default()
    }

    async fn on_tool_list_changed(&self, context: NotificationContext<RoleClient>) {
        let Ok(tools) = context.peer.list_all_tools().await else {
            return;
        };
        if let Some(sender) = &self.event_tx {
            let _ = sender.send(McpRuntimeEvent::ToolsChanged {
                server_id: self.server_id.clone(),
                tools: tools.iter().map(tool_snapshot).collect(),
            });
        }
    }
}

async fn connect_mcp_server(
    config: McpServerRuntimeConfig,
    event_tx: Option<mpsc::UnboundedSender<McpRuntimeEvent>>,
) -> Result<McpServerSession> {
    let server_id = config.server.server_id.clone();
    let display_name = config.server.display_name.clone();
    let transport_kind = transport_kind(&config.server.transport);
    let startup_timeout = config.startup_timeout;
    let handler = McpClientHandler::new(server_id.clone(), event_tx.clone());
    let service = match &config.server.transport {
        McpServerTransport::Stdio(stdio) => {
            let mut command = tokio::process::Command::new(&stdio.command);
            command.args(&stdio.args);
            command.envs(&config.server.env);
            if let Some(cwd) = &config.server.cwd {
                command.current_dir(cwd);
            }
            let transport = TokioChildProcess::new(command)?;
            tokio::time::timeout(startup_timeout, handler.serve(transport))
                .await
                .map_err(|_| {
                    AgentRuntimeError::Mcp(format!("mcp server `{server_id}` startup timed out"))
                })?
                .map_err(|err| AgentRuntimeError::Mcp(err.to_string()))?
        }
        McpServerTransport::StreamableHttp(http) => {
            if http.oauth.is_some() {
                return Err(AgentRuntimeError::Mcp(format!(
                    "mcp server `{server_id}` requires OAuth; app runtime must attach an AuthorizationManager before connecting"
                )));
            }
            let transport =
                StreamableHttpClientTransport::from_config(http_transport_config(http)?);
            tokio::time::timeout(startup_timeout, handler.serve(transport))
                .await
                .map_err(|_| {
                    AgentRuntimeError::Mcp(format!("mcp server `{server_id}` startup timed out"))
                })?
                .map_err(|err| AgentRuntimeError::Mcp(err.to_string()))?
        }
    };
    let sink = service.peer().clone();
    let tools = tokio::time::timeout(startup_timeout, service.peer().list_all_tools())
        .await
        .map_err(|_| {
            AgentRuntimeError::Mcp(format!("mcp server `{server_id}` tools/list timed out"))
        })?
        .map_err(|err| AgentRuntimeError::Mcp(err.to_string()))?;
    let status = connected_status(
        server_id.clone(),
        display_name,
        transport_kind,
        service.peer().peer_info().as_deref(),
        &tools,
        http_oauth_status(&config.server.transport),
    );
    Ok(McpServerSession {
        sink,
        service,
        tools,
        status,
    })
}

fn http_transport_config(
    transport: &McpStreamableHttpTransport,
) -> Result<StreamableHttpClientTransportConfig> {
    let mut headers = HashMap::new();
    for (name, value) in &transport.headers {
        headers.insert(
            HeaderName::from_bytes(name.as_bytes())
                .map_err(|err| AgentRuntimeError::Mcp(err.to_string()))?,
            HeaderValue::from_str(value).map_err(|err| AgentRuntimeError::Mcp(err.to_string()))?,
        );
    }
    Ok(
        StreamableHttpClientTransportConfig::with_uri(transport.url.clone())
            .custom_headers(headers),
    )
}

fn tool_allowed(tool_name: &str, config: &McpServerRuntimeConfig) -> bool {
    if config.disabled_tools.contains(tool_name) {
        return false;
    }
    if config
        .tool_approval_overrides
        .get(tool_name)
        .is_some_and(|mode| *mode == McpToolApprovalModeSnapshot::Deny)
    {
        return false;
    }
    if config.default_approval_mode == McpToolApprovalModeSnapshot::Deny
        && !matches!(
            config.tool_approval_overrides.get(tool_name),
            Some(McpToolApprovalModeSnapshot::Auto | McpToolApprovalModeSnapshot::Prompt)
        )
    {
        return false;
    }
    config
        .enabled_tools
        .as_ref()
        .is_none_or(|enabled| enabled.contains(tool_name))
}

fn approval_policy_for_tool(
    tool_name: &str,
    config: &McpServerRuntimeConfig,
) -> ToolApprovalPolicy {
    match config.tool_approval_overrides.get(tool_name) {
        Some(McpToolApprovalModeSnapshot::Auto) => ToolApprovalPolicy::Never,
        Some(McpToolApprovalModeSnapshot::Prompt) => ToolApprovalPolicy::OnRequest,
        Some(McpToolApprovalModeSnapshot::Deny) => config.default_approval_policy,
        None => config.default_approval_policy,
    }
}

fn tool_snapshot(tool: &RmcpToolDefinition) -> McpToolSnapshot {
    McpToolSnapshot {
        name: tool.name.to_string(),
        title: tool.title.clone(),
        description: tool
            .description
            .clone()
            .map(|description| description.to_string()),
        input_schema: tool.schema_as_json_value(),
    }
}

fn server_info_snapshot(info: &ServerInfo) -> McpServerInfoSnapshot {
    McpServerInfoSnapshot {
        protocol_version: info.protocol_version.to_string(),
        name: info.server_info.name.clone(),
        title: info.server_info.title.clone(),
        version: info.server_info.version.clone(),
        instructions: info.instructions.clone(),
    }
}

fn connected_status(
    server_id: String,
    display_name: Option<String>,
    transport: McpServerTransportKindSnapshot,
    server_info: Option<&ServerInfo>,
    tools: &[RmcpToolDefinition],
    auth: McpOAuthStatusSnapshot,
) -> McpServerStatusSnapshot {
    McpServerStatusSnapshot {
        server_id,
        display_name,
        transport,
        state: McpServerConnectionState::Connected,
        auth,
        server_info: server_info.map(server_info_snapshot),
        tools: tools.iter().map(tool_snapshot).collect(),
        last_error: None,
        updated_at_unix_ms: now_unix_ms(),
    }
}

fn failed_server_status(
    server_id: String,
    display_name: Option<String>,
    transport: McpServerTransportKindSnapshot,
    auth: McpOAuthStatusSnapshot,
    message: String,
) -> McpServerStatusSnapshot {
    McpServerStatusSnapshot {
        server_id,
        display_name,
        transport,
        state: McpServerConnectionState::Failed,
        auth,
        server_info: None,
        tools: Vec::new(),
        last_error: Some(message),
        updated_at_unix_ms: now_unix_ms(),
    }
}

fn failed_status(config_hash: &str, err: &AgentRuntimeError) -> McpServerStatusSnapshot {
    failed_server_status(
        String::new(),
        Some(config_hash.to_string()),
        McpServerTransportKindSnapshot::Stdio,
        McpOAuthStatusSnapshot::NotConfigured,
        err.to_string(),
    )
}

fn transport_kind(transport: &McpServerTransport) -> McpServerTransportKindSnapshot {
    match transport {
        McpServerTransport::Stdio(_) => McpServerTransportKindSnapshot::Stdio,
        McpServerTransport::StreamableHttp(_) => McpServerTransportKindSnapshot::StreamableHttp,
    }
}

fn http_oauth_status(transport: &McpServerTransport) -> McpOAuthStatusSnapshot {
    match transport {
        McpServerTransport::StreamableHttp(http) if http.oauth.is_some() => {
            McpOAuthStatusSnapshot::SignedOut
        }
        _ => McpOAuthStatusSnapshot::NotConfigured,
    }
}

fn failed_auth_status(transport: &McpServerTransport) -> McpOAuthStatusSnapshot {
    match transport {
        McpServerTransport::StreamableHttp(http) if http.oauth.is_some() => {
            McpOAuthStatusSnapshot::AuthorizationRequired
        }
        _ => McpOAuthStatusSnapshot::NotConfigured,
    }
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

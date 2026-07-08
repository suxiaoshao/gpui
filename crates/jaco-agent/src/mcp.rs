mod config_hash;

use crate::{AgentRuntimeError, Result, ToolDefinition, ToolRegistry, ToolRunPolicy};
use async_trait::async_trait;
use http::{HeaderName, HeaderValue};
use jaco_core::{McpToolApprovalModeSnapshot, ToolApprovalPolicy, ToolExecutionPolicy, ToolSource};
use rig_core::tool::rmcp::McpTool;
use rmcp::{
    ServiceExt,
    handler::client::ClientHandler,
    model::{ClientInfo, ServerInfo, Tool as RmcpToolDefinition},
    service::{NotificationContext, RoleClient, RunningService, ServerSink},
    transport::{
        AuthClient, AuthError, AuthorizationManager, CredentialStore, InMemoryCredentialStore,
        StoredCredentials, StreamableHttpClientTransport, TokioChildProcess,
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

use config_hash::mcp_server_fingerprint;

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

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpStreamableHttpTransport {
    pub url: String,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    pub oauth: Option<serde_json::Value>,
    #[serde(skip)]
    pub oauth_credentials: Option<serde_json::Value>,
}

impl std::fmt::Debug for McpStreamableHttpTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpStreamableHttpTransport")
            .field("url", &self.url)
            .field("headers", &self.headers)
            .field("oauth", &self.oauth)
            .field(
                "oauth_credentials",
                &self.oauth_credentials.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
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
    pub statuses: Vec<McpServerStatusSnapshot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpSessionPruneMode {
    PruneStale,
    KeepExistingSessions,
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
    OAuthCredentialsChanged(Box<McpOAuthCredentialsSnapshot>),
}

#[derive(Clone, PartialEq)]
pub struct McpOAuthCredentialsSnapshot {
    pub server_id: String,
    pub server_url: String,
    pub credentials: serde_json::Value,
    pub status: McpOAuthStatusSnapshot,
}

impl std::fmt::Debug for McpOAuthCredentialsSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpOAuthCredentialsSnapshot")
            .field("server_id", &self.server_id)
            .field("server_url", &self.server_url)
            .field("credentials", &"[REDACTED]")
            .field("status", &self.status)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct McpSessionKey {
    pub server_id: String,
    pub fingerprint: String,
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
        configs: Vec<McpServerRuntimeConfig>,
        prune_mode: McpSessionPruneMode,
    ) -> Result<McpPreparedTools> {
        let active_fingerprints = configs
            .iter()
            .map(|config| {
                (
                    config.server.server_id.clone(),
                    mcp_server_fingerprint(config),
                )
            })
            .collect::<BTreeMap<_, _>>();
        if prune_mode == McpSessionPruneMode::PruneStale {
            self.close_stale_sessions(&active_fingerprints).await;
        }

        let mut statuses = Vec::new();
        for config in configs {
            let fingerprint = mcp_server_fingerprint(&config);
            match self
                .register_tools_for_server(registry, config, fingerprint)
                .await
            {
                Ok(status) => statuses.push(status),
                Err(err) => {
                    return Err(err);
                }
            }
        }

        Ok(McpPreparedTools { statuses })
    }

    async fn register_tools_for_server(
        &mut self,
        registry: &mut ToolRegistry,
        config: McpServerRuntimeConfig,
        fingerprint: String,
    ) -> Result<McpServerStatusSnapshot> {
        let required = config.required;
        let server_id = config.server.server_id.clone();
        let display_name = config.server.display_name.clone();
        let transport = transport_kind(&config.server.transport);
        let result = self.ensure_session(config.clone(), fingerprint).await;
        let session = match result {
            Ok(session) => session,
            Err(err) if required => return Err(err),
            Err(err) => {
                let message = err.to_string();
                return Ok(failed_server_status(
                    server_id,
                    display_name,
                    transport,
                    failed_auth_status(&config.server.transport, &message),
                    message,
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
        fingerprint: String,
    ) -> Result<&mut McpServerSession> {
        let key = McpSessionKey {
            server_id: config.server.server_id.clone(),
            fingerprint,
        };
        if self.sessions.contains_key(&key) {
            let refresh_result = {
                let session = self.sessions.get_mut(&key).expect("session key exists");
                refresh_session_tools(session, config.startup_timeout).await
            };
            match refresh_result {
                Ok(refreshed_status) => {
                    self.emit(McpRuntimeEvent::ServerStatusChanged(Box::new(
                        refreshed_status,
                    )));
                    return Ok(self.sessions.get_mut(&key).expect("session key exists"));
                }
                Err(_) => {
                    if let Some(mut session) = self.sessions.remove(&key) {
                        let _ = session
                            .service
                            .close_with_timeout(Duration::from_secs(5))
                            .await;
                    }
                }
            }
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

    async fn close_stale_sessions(&mut self, active_fingerprints: &BTreeMap<String, String>) {
        let stale_keys = stale_session_keys(self.sessions.keys(), active_fingerprints);
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

fn stale_session_keys<'a>(
    keys: impl Iterator<Item = &'a McpSessionKey>,
    active_fingerprints: &BTreeMap<String, String>,
) -> Vec<McpSessionKey> {
    keys.filter(|key| {
        active_fingerprints
            .get(&key.server_id)
            .is_none_or(|fingerprint| fingerprint != &key.fingerprint)
    })
    .cloned()
    .collect()
}

async fn refresh_session_tools(
    session: &mut McpServerSession,
    timeout: Duration,
) -> Result<McpServerStatusSnapshot> {
    let server_id = session.status.server_id.clone();
    let tools = tokio::time::timeout(timeout, session.sink.list_all_tools())
        .await
        .map_err(|_| {
            AgentRuntimeError::Mcp(format!("mcp server `{server_id}` tools/list timed out"))
        })?
        .map_err(|err| AgentRuntimeError::Mcp(err.to_string()))?;
    session.tools = tools;
    session.status.tools = session.tools.iter().map(tool_snapshot).collect();
    session.status.updated_at_unix_ms = now_unix_ms();
    Ok(session.status.clone())
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
                if http.oauth_credentials.is_none() {
                    return Err(AgentRuntimeError::Mcp(format!(
                        "mcp server `{server_id}` requires OAuth authorization"
                    )));
                }
                let auth_manager =
                    authorization_manager_for_http(&server_id, http, event_tx.clone())
                        .await
                        .map_err(|err| {
                            AgentRuntimeError::Mcp(format!(
                                "mcp server `{server_id}` OAuth authorization failed: {err}"
                            ))
                        })?;
                let transport = StreamableHttpClientTransport::with_client(
                    AuthClient::new(reqwest::Client::default(), auth_manager),
                    http_transport_config(http)?,
                );
                tokio::time::timeout(startup_timeout, handler.serve(transport))
                    .await
                    .map_err(|_| {
                        AgentRuntimeError::Mcp(format!(
                            "mcp server `{server_id}` startup timed out"
                        ))
                    })?
                    .map_err(|err| AgentRuntimeError::Mcp(err.to_string()))?
            } else {
                let transport =
                    StreamableHttpClientTransport::from_config(http_transport_config(http)?);
                tokio::time::timeout(startup_timeout, handler.serve(transport))
                    .await
                    .map_err(|_| {
                        AgentRuntimeError::Mcp(format!(
                            "mcp server `{server_id}` startup timed out"
                        ))
                    })?
                    .map_err(|err| AgentRuntimeError::Mcp(err.to_string()))?
            }
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

async fn authorization_manager_for_http(
    server_id: &str,
    transport: &McpStreamableHttpTransport,
    event_tx: Option<mpsc::UnboundedSender<McpRuntimeEvent>>,
) -> Result<AuthorizationManager> {
    let credentials_value = transport
        .oauth_credentials
        .as_ref()
        .ok_or_else(|| AgentRuntimeError::Mcp("OAuth credentials are missing".to_string()))?;
    let credentials = serde_json::from_value::<StoredCredentials>(credentials_value.clone())
        .map_err(|err| AgentRuntimeError::Mcp(err.to_string()))?;
    let credential_store =
        MirroringCredentialStore::new(server_id.to_string(), transport.url.clone(), event_tx);
    credential_store
        .seed(credentials)
        .await
        .map_err(|err| AgentRuntimeError::Mcp(err.to_string()))?;

    let mut manager = AuthorizationManager::new(transport.url.clone())
        .await
        .map_err(|err| AgentRuntimeError::Mcp(err.to_string()))?;
    manager.set_credential_store(credential_store);
    let initialized = manager
        .initialize_from_store()
        .await
        .map_err(|err| AgentRuntimeError::Mcp(err.to_string()))?;
    if !initialized {
        return Err(AgentRuntimeError::Mcp(
            "OAuth credentials are incomplete".to_string(),
        ));
    }
    Ok(manager)
}

#[derive(Clone)]
struct MirroringCredentialStore {
    inner: InMemoryCredentialStore,
    server_id: String,
    server_url: String,
    event_tx: Option<mpsc::UnboundedSender<McpRuntimeEvent>>,
}

impl MirroringCredentialStore {
    fn new(
        server_id: String,
        server_url: String,
        event_tx: Option<mpsc::UnboundedSender<McpRuntimeEvent>>,
    ) -> Self {
        Self {
            inner: InMemoryCredentialStore::new(),
            server_id,
            server_url,
            event_tx,
        }
    }

    async fn seed(&self, credentials: StoredCredentials) -> std::result::Result<(), AuthError> {
        self.inner.save(credentials).await
    }

    fn emit_credentials_changed(
        &self,
        credentials: &StoredCredentials,
    ) -> std::result::Result<(), AuthError> {
        let Some(sender) = &self.event_tx else {
            return Ok(());
        };
        let credentials_value = serde_json::to_value(credentials)
            .map_err(|err| AuthError::InternalError(err.to_string()))?;
        let _ = sender.send(McpRuntimeEvent::OAuthCredentialsChanged(Box::new(
            McpOAuthCredentialsSnapshot {
                server_id: self.server_id.clone(),
                server_url: self.server_url.clone(),
                credentials: credentials_value,
                status: oauth_status_from_credentials(credentials),
            },
        )));
        Ok(())
    }
}

#[async_trait]
impl CredentialStore for MirroringCredentialStore {
    async fn load(&self) -> std::result::Result<Option<StoredCredentials>, AuthError> {
        self.inner.load().await
    }

    async fn save(&self, credentials: StoredCredentials) -> std::result::Result<(), AuthError> {
        self.inner.save(credentials.clone()).await?;
        self.emit_credentials_changed(&credentials)?;
        Ok(())
    }

    async fn clear(&self) -> std::result::Result<(), AuthError> {
        self.inner.clear().await
    }
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

fn transport_kind(transport: &McpServerTransport) -> McpServerTransportKindSnapshot {
    match transport {
        McpServerTransport::Stdio(_) => McpServerTransportKindSnapshot::Stdio,
        McpServerTransport::StreamableHttp(_) => McpServerTransportKindSnapshot::StreamableHttp,
    }
}

fn http_oauth_status(transport: &McpServerTransport) -> McpOAuthStatusSnapshot {
    match transport {
        McpServerTransport::StreamableHttp(http)
            if http.oauth.is_some() && http.oauth_credentials.is_some() =>
        {
            http.oauth_credentials
                .as_ref()
                .and_then(|credentials| {
                    serde_json::from_value::<StoredCredentials>(credentials.clone()).ok()
                })
                .map(|credentials| oauth_status_from_credentials(&credentials))
                .unwrap_or(McpOAuthStatusSnapshot::AuthorizationRequired)
        }
        McpServerTransport::StreamableHttp(http) if http.oauth.is_some() => {
            McpOAuthStatusSnapshot::SignedOut
        }
        _ => McpOAuthStatusSnapshot::NotConfigured,
    }
}

fn failed_auth_status(transport: &McpServerTransport, message: &str) -> McpOAuthStatusSnapshot {
    match transport {
        McpServerTransport::StreamableHttp(http) if http.oauth.is_some() => {
            oauth_error_status(message)
        }
        _ => McpOAuthStatusSnapshot::NotConfigured,
    }
}

fn oauth_status_from_credentials(credentials: &StoredCredentials) -> McpOAuthStatusSnapshot {
    if credentials.token_response.is_some() {
        McpOAuthStatusSnapshot::Authorized {
            scopes: credentials.granted_scopes.clone(),
            expires_at_unix_ms: None,
        }
    } else {
        McpOAuthStatusSnapshot::AuthorizationRequired
    }
}

fn oauth_error_status(message: &str) -> McpOAuthStatusSnapshot {
    let lower = message.to_ascii_lowercase();
    if lower.contains("insufficient scope") {
        return McpOAuthStatusSnapshot::ScopeUpgradeRequired {
            required_scope: "unknown".to_string(),
            authorization_url: String::new(),
        };
    }
    if lower.contains("authorization required") || lower.contains("requires oauth authorization") {
        return McpOAuthStatusSnapshot::AuthorizationRequired;
    }
    McpOAuthStatusSnapshot::Failed {
        message: message.to_string(),
    }
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    fn oauth_http_transport() -> McpServerTransport {
        McpServerTransport::StreamableHttp(McpStreamableHttpTransport {
            url: "https://example.com/mcp".to_string(),
            headers: BTreeMap::new(),
            oauth: Some(serde_json::json!({ "type": "authorizationCodePkce" })),
            oauth_credentials: None,
        })
    }

    fn stdio_runtime_config(server_id: &str, command: &str) -> McpServerRuntimeConfig {
        McpServerRuntimeConfig {
            server: McpServerConfig {
                server_id: server_id.to_string(),
                display_name: None,
                transport: McpServerTransport::Stdio(McpStdioTransport {
                    command: command.to_string(),
                    args: Vec::new(),
                }),
                env: BTreeMap::new(),
                cwd: None,
            },
            required: false,
            startup_timeout: Duration::from_secs(30),
            tool_timeout: Duration::from_secs(300),
            enabled_tools: None,
            disabled_tools: BTreeSet::new(),
            default_approval_mode: McpToolApprovalModeSnapshot::Auto,
            default_approval_policy: ToolApprovalPolicy::Never,
            execution_policy: ToolExecutionPolicy::Foreground,
            tool_approval_overrides: BTreeMap::new(),
        }
    }

    #[test]
    fn stale_session_keys_keep_matching_servers_only() {
        let keys = [
            McpSessionKey {
                server_id: "alpha".to_string(),
                fingerprint: "same".to_string(),
            },
            McpSessionKey {
                server_id: "beta".to_string(),
                fingerprint: "old".to_string(),
            },
            McpSessionKey {
                server_id: "removed".to_string(),
                fingerprint: "gone".to_string(),
            },
        ];
        let active = BTreeMap::from([
            ("alpha".to_string(), "same".to_string()),
            ("beta".to_string(), "new".to_string()),
        ]);

        let stale = stale_session_keys(keys.iter(), &active);

        assert_eq!(
            stale,
            vec![
                McpSessionKey {
                    server_id: "beta".to_string(),
                    fingerprint: "old".to_string(),
                },
                McpSessionKey {
                    server_id: "removed".to_string(),
                    fingerprint: "gone".to_string(),
                },
            ]
        );
    }

    #[test]
    fn server_fingerprint_changes_with_runtime_config() {
        let first = stdio_runtime_config("server", "echo");
        let mut second = first.clone();
        second
            .server
            .env
            .insert("TOKEN".to_string(), "secret".to_string());

        assert_ne!(
            mcp_server_fingerprint(&first),
            mcp_server_fingerprint(&second)
        );
    }

    #[test]
    fn oauth_error_status_maps_authorization_required() {
        assert_eq!(
            failed_auth_status(&oauth_http_transport(), "OAuth authorization required"),
            McpOAuthStatusSnapshot::AuthorizationRequired
        );
    }

    #[test]
    fn oauth_error_status_maps_insufficient_scope() {
        assert!(matches!(
            failed_auth_status(&oauth_http_transport(), "Insufficient scope"),
            McpOAuthStatusSnapshot::ScopeUpgradeRequired { .. }
        ));
    }

    #[tokio::test]
    async fn mirroring_credential_store_emits_credentials_changed_on_save() {
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let store = MirroringCredentialStore::new(
            "server".to_string(),
            "https://example.com/mcp".to_string(),
            Some(event_tx),
        );

        store
            .save(StoredCredentials::new(
                "client".to_string(),
                None,
                Vec::new(),
                None,
            ))
            .await
            .unwrap();

        match event_rx.recv().await.unwrap() {
            McpRuntimeEvent::OAuthCredentialsChanged(snapshot) => {
                assert_eq!(snapshot.server_id, "server");
                assert_eq!(snapshot.server_url, "https://example.com/mcp");
                assert_eq!(
                    snapshot.status,
                    McpOAuthStatusSnapshot::AuthorizationRequired
                );
                assert!(snapshot.credentials.get("client_id").is_some());
            }
            event => panic!("unexpected event: {event:?}"),
        }
    }
}

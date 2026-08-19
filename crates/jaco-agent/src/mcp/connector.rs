use super::*;

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
            tool,
            client,
        )?;
        Ok(())
    }
}

pub struct McpClientHandler {
    identity: McpSessionIdentity,
    event_tx: Option<mpsc::UnboundedSender<McpRuntimeEvent>>,
}

impl McpClientHandler {
    pub fn new(
        identity: McpSessionIdentity,
        event_tx: Option<mpsc::UnboundedSender<McpRuntimeEvent>>,
    ) -> Self {
        Self { identity, event_tx }
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
                identity: self.identity.clone(),
                tools: tools.iter().map(tool_snapshot).collect(),
            });
        }
    }
}

pub(super) async fn connect_mcp_server(
    config: McpServerRuntimeConfig,
    identity: McpSessionIdentity,
    event_tx: Option<mpsc::UnboundedSender<McpRuntimeEvent>>,
) -> Result<McpServerSession> {
    let server_id = config.server.server_id.clone();
    let display_name = config.server.display_name.clone();
    let transport_kind = transport_kind(&config.server.transport);
    let startup_timeout = config.startup_timeout;
    let handler = McpClientHandler::new(identity.clone(), event_tx.clone());
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
                    authorization_manager_for_http(&identity, http, event_tx.clone())
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
    identity: &McpSessionIdentity,
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
        MirroringCredentialStore::new(identity.clone(), transport.url.clone(), event_tx);
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
pub(super) struct MirroringCredentialStore {
    inner: InMemoryCredentialStore,
    identity: McpSessionIdentity,
    server_url: String,
    event_tx: Option<mpsc::UnboundedSender<McpRuntimeEvent>>,
}

impl MirroringCredentialStore {
    pub(super) fn new(
        identity: McpSessionIdentity,
        server_url: String,
        event_tx: Option<mpsc::UnboundedSender<McpRuntimeEvent>>,
    ) -> Self {
        Self {
            inner: InMemoryCredentialStore::new(),
            identity,
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
                identity: self.identity.clone(),
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

pub(super) fn tool_allowed(tool_name: &str, config: &McpServerRuntimeConfig) -> bool {
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

pub(super) fn approval_policy_for_tool(
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

pub(super) fn tool_snapshot(tool: &RmcpToolDefinition) -> McpToolSnapshot {
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

pub(super) fn failed_server_status(
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

pub(super) fn transport_kind(transport: &McpServerTransport) -> McpServerTransportKindSnapshot {
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

pub(super) fn failed_auth_status(
    transport: &McpServerTransport,
    message: &str,
) -> McpOAuthStatusSnapshot {
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

pub(super) fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

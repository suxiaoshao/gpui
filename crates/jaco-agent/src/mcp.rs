mod config_hash;
mod connector;

#[cfg(test)]
use connector::MirroringCredentialStore;
pub use connector::{McpClientHandler, McpConnector, McpToolRegistrationOptions};
use connector::{
    approval_policy_for_tool, connect_mcp_server, failed_auth_status, failed_server_status,
    now_unix_ms, tool_allowed, tool_snapshot, transport_kind,
};

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

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

pub use config_hash::mcp_server_fingerprint;

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
    pub generation: u64,
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
    ServerStatusChanged {
        identity: McpSessionIdentity,
        status: Box<McpServerStatusSnapshot>,
    },
    ToolsChanged {
        identity: McpSessionIdentity,
        tools: Vec<McpToolSnapshot>,
    },
    OAuthChanged {
        identity: McpSessionIdentity,
        status: McpOAuthStatusSnapshot,
    },
    OAuthCredentialsChanged(Box<McpOAuthCredentialsSnapshot>),
}

#[derive(Clone, PartialEq)]
pub struct McpOAuthCredentialsSnapshot {
    pub identity: McpSessionIdentity,
    pub server_url: String,
    pub credentials: serde_json::Value,
    pub status: McpOAuthStatusSnapshot,
}

impl std::fmt::Debug for McpOAuthCredentialsSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpOAuthCredentialsSnapshot")
            .field("identity", &self.identity)
            .field("server_url", &self.server_url)
            .field("credentials", &"[REDACTED]")
            .field("status", &self.status)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct McpSessionIdentity {
    pub server_id: String,
    pub fingerprint: String,
    pub generation: u64,
}

type McpSessionKey = McpSessionIdentity;

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
    latest_generations: BTreeMap<String, u64>,
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
        authority_generations: BTreeMap<String, u64>,
        prune_mode: McpSessionPruneMode,
    ) -> Result<McpPreparedTools> {
        self.accept_config_generations(&configs, &authority_generations)?;
        self.close_superseded_generations().await;

        let active_sessions = configs
            .iter()
            .map(|config| {
                (
                    config.server.server_id.clone(),
                    McpSessionIdentity {
                        server_id: config.server.server_id.clone(),
                        fingerprint: mcp_server_fingerprint(config),
                        generation: config.generation,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let replaced_keys = self
            .sessions
            .keys()
            .filter(|key| {
                active_sessions
                    .get(&key.server_id)
                    .is_some_and(|active| active != *key)
            })
            .cloned()
            .collect();
        self.close_session_keys(replaced_keys).await;
        if prune_mode == McpSessionPruneMode::PruneStale {
            self.close_stale_sessions(&active_sessions, &authority_generations)
                .await;
        }

        let mut statuses = Vec::new();
        for config in configs {
            let identity = McpSessionIdentity {
                server_id: config.server.server_id.clone(),
                fingerprint: mcp_server_fingerprint(&config),
                generation: config.generation,
            };
            match self
                .register_tools_for_server(registry, config, identity)
                .await
            {
                Ok(status) => statuses.push(status),
                Err(err) => return Err(err),
            }
        }

        Ok(McpPreparedTools { statuses })
    }

    fn accept_config_generations(
        &mut self,
        configs: &[McpServerRuntimeConfig],
        authority_generations: &BTreeMap<String, u64>,
    ) -> Result<()> {
        for (server_id, generation) in authority_generations {
            if self
                .latest_generations
                .get(server_id)
                .is_some_and(|latest| generation < latest)
            {
                let latest = self.latest_generations[server_id];
                return Err(AgentRuntimeError::Mcp(format!(
                    "MCP server `{server_id}` runtime generation {} was superseded by generation {}",
                    generation, latest
                )));
            }
        }
        for config in configs {
            let server_id = &config.server.server_id;
            if authority_generations.get(server_id) != Some(&config.generation) {
                return Err(AgentRuntimeError::Mcp(format!(
                    "MCP server `{server_id}` runtime generation {} is outside the accepted configuration snapshot",
                    config.generation
                )));
            }
        }
        for (server_id, generation) in authority_generations {
            self.latest_generations
                .entry(server_id.clone())
                .and_modify(|latest| *latest = (*latest).max(*generation))
                .or_insert(*generation);
        }
        Ok(())
    }

    async fn register_tools_for_server(
        &mut self,
        registry: &mut ToolRegistry,
        config: McpServerRuntimeConfig,
        identity: McpSessionIdentity,
    ) -> Result<McpServerStatusSnapshot> {
        let required = config.required;
        let server_id = config.server.server_id.clone();
        let display_name = config.server.display_name.clone();
        let transport = transport_kind(&config.server.transport);
        let result = self.ensure_session(config.clone(), identity).await;
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
        identity: McpSessionIdentity,
    ) -> Result<&mut McpServerSession> {
        let key = identity;
        if self.sessions.contains_key(&key) {
            let refresh_result = {
                let session = self.sessions.get_mut(&key).expect("session key exists");
                refresh_session_tools(session, config.startup_timeout).await
            };
            match refresh_result {
                Ok(refreshed_status) => {
                    self.emit(McpRuntimeEvent::ServerStatusChanged {
                        identity: key.clone(),
                        status: Box::new(refreshed_status),
                    });
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

        let session = connect_mcp_server(config, key.clone(), self.event_tx.clone()).await?;
        self.emit(McpRuntimeEvent::ServerStatusChanged {
            identity: key.clone(),
            status: Box::new(session.status.clone()),
        });
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
        active_sessions: &BTreeMap<String, McpSessionIdentity>,
        authority_generations: &BTreeMap<String, u64>,
    ) {
        let stale_keys =
            stale_session_keys(self.sessions.keys(), active_sessions, authority_generations);
        self.close_session_keys(stale_keys).await;
    }

    async fn close_superseded_generations(&mut self) {
        let stale_keys = self
            .sessions
            .keys()
            .filter(|key| {
                self.latest_generations
                    .get(&key.server_id)
                    .is_some_and(|generation| key.generation < *generation)
            })
            .cloned()
            .collect::<Vec<_>>();
        self.close_session_keys(stale_keys).await;
    }

    async fn close_session_keys(&mut self, keys: Vec<McpSessionKey>) {
        for key in keys {
            if let Some(mut session) = self.sessions.remove(&key) {
                let _ = session
                    .service
                    .close_with_timeout(Duration::from_secs(5))
                    .await;
            }
        }
    }

    pub async fn advance_server_generation(&mut self, server_id: &str, generation: u64) {
        let latest = self
            .latest_generations
            .entry(server_id.to_string())
            .or_insert(generation);
        *latest = (*latest).max(generation);
        self.close_superseded_generations().await;
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
    active_sessions: &BTreeMap<String, McpSessionIdentity>,
    authority_generations: &BTreeMap<String, u64>,
) -> Vec<McpSessionKey> {
    keys.filter(|key| match active_sessions.get(&key.server_id) {
        Some(active) => active != *key,
        None => authority_generations
            .get(&key.server_id)
            .is_some_and(|generation| key.generation <= *generation),
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
            generation: 0,
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
                generation: 1,
            },
            McpSessionKey {
                server_id: "beta".to_string(),
                fingerprint: "old".to_string(),
                generation: 1,
            },
            McpSessionKey {
                server_id: "removed".to_string(),
                fingerprint: "gone".to_string(),
                generation: 1,
            },
        ];
        let active = BTreeMap::from([
            (
                "alpha".to_string(),
                McpSessionIdentity {
                    server_id: "alpha".to_string(),
                    fingerprint: "same".to_string(),
                    generation: 1,
                },
            ),
            (
                "beta".to_string(),
                McpSessionIdentity {
                    server_id: "beta".to_string(),
                    fingerprint: "new".to_string(),
                    generation: 1,
                },
            ),
        ]);
        let authority_generations = BTreeMap::from([
            ("alpha".to_string(), 1),
            ("beta".to_string(), 1),
            ("removed".to_string(), 1),
        ]);

        let stale = stale_session_keys(keys.iter(), &active, &authority_generations);

        assert_eq!(
            stale,
            vec![
                McpSessionKey {
                    server_id: "beta".to_string(),
                    fingerprint: "old".to_string(),
                    generation: 1,
                },
                McpSessionKey {
                    server_id: "removed".to_string(),
                    fingerprint: "gone".to_string(),
                    generation: 1,
                },
            ]
        );
    }

    #[test]
    fn stale_prune_snapshot_does_not_close_unrepresented_new_server() {
        let new_session = McpSessionKey {
            server_id: "new".to_string(),
            fingerprint: "new-fingerprint".to_string(),
            generation: 0,
        };

        assert!(
            stale_session_keys(
                std::iter::once(&new_session),
                &BTreeMap::new(),
                &BTreeMap::from([("old".to_string(), 1)]),
            )
            .is_empty()
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
    fn server_generation_is_not_part_of_config_fingerprint() {
        let first = stdio_runtime_config("server", "echo");
        let mut after_aba = first.clone();
        after_aba.generation = 2;

        assert_eq!(
            mcp_server_fingerprint(&first),
            mcp_server_fingerprint(&after_aba)
        );
        assert_ne!(first.generation, after_aba.generation);
    }

    #[tokio::test]
    async fn prepare_then_advance_keeps_same_generation_and_rejects_older_work() {
        let mut manager = McpSessionManager::new();
        let mut current = stdio_runtime_config("server", "echo");
        current.generation = 4;

        manager
            .accept_config_generations(
                std::slice::from_ref(&current),
                &BTreeMap::from([("server".to_string(), 4)]),
            )
            .expect("accept current prepare");
        manager.advance_server_generation("server", 4).await;
        manager
            .accept_config_generations(
                std::slice::from_ref(&current),
                &BTreeMap::from([("server".to_string(), 4)]),
            )
            .expect("same generation remains current");

        let mut stale = current;
        stale.generation = 3;
        assert!(
            manager
                .accept_config_generations(
                    std::slice::from_ref(&stale),
                    &BTreeMap::from([("server".to_string(), 3)]),
                )
                .unwrap_err()
                .to_string()
                .contains("superseded")
        );
    }

    #[tokio::test]
    async fn advance_then_prepare_accepts_the_advanced_generation() {
        let mut manager = McpSessionManager::new();
        manager.advance_server_generation("server", 9).await;
        let mut current = stdio_runtime_config("server", "echo");
        current.generation = 9;

        manager
            .accept_config_generations(
                std::slice::from_ref(&current),
                &BTreeMap::from([("server".to_string(), 9)]),
            )
            .expect("prepare at advanced generation");
        assert_eq!(manager.latest_generations.get("server"), Some(&9));
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
        let identity = McpSessionIdentity {
            server_id: "server".to_string(),
            fingerprint: "fingerprint".to_string(),
            generation: 7,
        };
        let store = MirroringCredentialStore::new(
            identity.clone(),
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
                assert_eq!(snapshot.identity, identity);
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

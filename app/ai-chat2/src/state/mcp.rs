use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use ai_chat_agent::{
    AgentRunRequest, McpOAuthStatusSnapshot, McpPreparedTools, McpRuntimeEvent,
    McpServerConnectionState, McpServerInfoSnapshot, McpServerRuntimeConfig,
    McpServerStatusSnapshot, McpServerTransportKindSnapshot, McpSessionManager, McpToolSnapshot,
    ToolRegistry, mcp_config_hash,
};
use ai_chat_core::{
    McpRuntimeConfigSnapshot, McpToolApprovalModeSnapshot, ToolApprovalMode, ToolSource,
};
use gpui::{App, AppContext, AsyncWindowContext, Context, Entity, EventEmitter, Global, Task};
use tokio::sync::{Mutex, mpsc};
use tracing::{Level, event};

use crate::{
    errors::AiChat2Result,
    state::{
        config::{self, AiChat2Config, McpOAuthTomlConfig, McpTransportKind},
        mcp_oauth,
    },
};

#[derive(Clone)]
pub(crate) struct McpRuntimeGlobal(Entity<McpRuntimeStore>);

impl McpRuntimeGlobal {
    pub(crate) fn entity(&self) -> Entity<McpRuntimeStore> {
        self.0.clone()
    }
}

impl Global for McpRuntimeGlobal {}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct McpServerStatusRow {
    pub(crate) server_id: String,
    pub(crate) display_name: Option<String>,
    pub(crate) enabled: bool,
    pub(crate) required: bool,
    pub(crate) transport: McpTransportKind,
    pub(crate) connection: McpServerConnectionState,
    pub(crate) auth: ai_chat_agent::McpOAuthStatusSnapshot,
    pub(crate) tool_count: usize,
    pub(crate) tools: Vec<McpToolSnapshot>,
    pub(crate) server_info: Option<McpServerInfoSnapshot>,
    pub(crate) last_error: Option<String>,
    pub(crate) updated_at_unix_ms: Option<u64>,
}

pub(crate) struct McpPreparedRun {
    pub(crate) request: AgentRunRequest,
}

pub(crate) struct McpPrepareRunError {
    pub(crate) request: AgentRunRequest,
    pub(crate) message: String,
}

pub(crate) struct McpRuntimeStore {
    manager: Arc<Mutex<McpSessionManager>>,
    statuses: BTreeMap<String, McpServerStatusSnapshot>,
    server_tasks: BTreeMap<String, Task<()>>,
    oauth_tasks: BTreeMap<String, Task<()>>,
    oauth_task_targets: BTreeMap<String, McpOAuthTaskTarget>,
    disconnect_tasks: BTreeMap<String, Task<()>>,
    last_error: Option<String>,
    _event_task: Task<()>,
}

#[derive(Clone, Debug, PartialEq)]
struct McpOAuthTaskTarget {
    status_server_id: String,
    server: config::McpServerTomlConfig,
}

#[derive(Clone, Debug, PartialEq)]
struct McpRuntimeSetup {
    snapshot: McpRuntimeConfigSnapshot,
    configs: Vec<McpServerRuntimeConfig>,
    preflight_statuses: Vec<McpServerStatusSnapshot>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum McpRuntimeStoreEvent {
    StatusChanged,
}

impl EventEmitter<McpRuntimeStoreEvent> for McpRuntimeStore {}

impl McpRuntimeStore {
    fn new(cx: &mut Context<Self>) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let manager = McpSessionManager::new().with_event_sender(event_tx);
        let event_task = Self::spawn_event_listener(event_rx, cx);
        Self {
            manager: Arc::new(Mutex::new(manager)),
            statuses: BTreeMap::new(),
            server_tasks: BTreeMap::new(),
            oauth_tasks: BTreeMap::new(),
            oauth_task_targets: BTreeMap::new(),
            disconnect_tasks: BTreeMap::new(),
            last_error: None,
            _event_task: event_task,
        }
    }

    pub(crate) fn rows(&self, cx: &App) -> Vec<McpServerStatusRow> {
        let servers = config::read(cx, |config| config.mcp_servers.clone());
        servers
            .into_iter()
            .map(|(server_id, server)| {
                let status = self.statuses.get(&server_id);
                let connecting = self.server_tasks.contains_key(&server_id);
                let connection = if !server.enabled {
                    McpServerConnectionState::Disabled
                } else if connecting {
                    McpServerConnectionState::Connecting
                } else {
                    status
                        .map(|status| status.state)
                        .unwrap_or(McpServerConnectionState::NotConnected)
                };
                let auth = row_auth(&server, status);
                let tools = status
                    .map(|status| status.tools.clone())
                    .unwrap_or_default();
                McpServerStatusRow {
                    server_id,
                    display_name: server.display_name,
                    enabled: server.enabled,
                    required: server.required,
                    transport: server.transport,
                    connection,
                    auth,
                    tool_count: tools.len(),
                    tools,
                    server_info: status.and_then(|status| status.server_info.clone()),
                    last_error: status.and_then(|status| status.last_error.clone()),
                    updated_at_unix_ms: status.map(|status| status.updated_at_unix_ms),
                }
            })
            .collect()
    }

    pub(crate) fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub(crate) fn test_server(
        &mut self,
        server_id: String,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let setup = match build_mcp_runtime_setup_for_server(cx, &server_id) {
            Ok(setup) => setup,
            Err(err) => {
                self.last_error = Some(err.to_string());
                cx.emit(McpRuntimeStoreEvent::StatusChanged);
                cx.notify();
                return;
            }
        };
        let manager = self.manager.clone();
        let store = cx.entity().downgrade();
        self.server_tasks.remove(&server_id);
        self.last_error = None;
        let task_server_id = server_id.clone();
        let task = window.spawn(cx, async move |cx| {
            let result = match attach_oauth_credentials(setup, cx).await {
                Ok(setup) => gpui_tokio::Tokio::spawn(cx, async move {
                    let mut registry = ToolRegistry::default();
                    let mut manager = manager.lock().await;
                    let preflight_statuses = setup.preflight_statuses;
                    manager
                        .prepare_tool_registry(&mut registry, setup.snapshot, setup.configs)
                        .await
                        .map(|mut prepared| {
                            prepared.statuses.extend(preflight_statuses);
                            prepared
                        })
                })
                .await
                .map_err(|err| err.to_string())
                .and_then(|result| result.map_err(|err| err.to_string())),
                Err(err) => Err(err),
            };

            if let Err(err) = store.update_in(cx, |store, _window, cx| {
                store.finish_server_test(task_server_id, result, cx);
            }) {
                event!(Level::ERROR, error = ?err, "finish MCP server test failed");
            }
        });
        self.server_tasks.insert(server_id, task);
        cx.emit(McpRuntimeStoreEvent::StatusChanged);
        cx.notify();
    }

    pub(crate) fn authenticate_server_config(
        &mut self,
        status_key: String,
        server_id: String,
        server: config::McpServerTomlConfig,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let Some((server_url, oauth_config)) = oauth_authorization_config(&server_id, &server)
            .map_or_else(
                |err| {
                    self.last_error = Some(err);
                    None
                },
                Some,
            )
        else {
            cx.emit(McpRuntimeStoreEvent::StatusChanged);
            cx.notify();
            return;
        };

        self.oauth_tasks.remove(&status_key);
        self.oauth_task_targets.insert(
            status_key.clone(),
            McpOAuthTaskTarget {
                status_server_id: status_key.clone(),
                server: server.clone(),
            },
        );
        self.set_server_auth_status(
            status_key.clone(),
            &server,
            McpOAuthStatusSnapshot::SigningIn,
            None,
        );
        self.last_error = None;
        let store = cx.entity().downgrade();
        let task_status_key = status_key.clone();
        let task = window.spawn(cx, async move |cx| {
            let result =
                mcp_oauth::authorize_with_browser(server_url.clone(), oauth_config, cx).await;
            let result = match result {
                Ok(authorized) => {
                    match mcp_oauth::write_credentials(&server_url, &authorized.credentials, cx)
                        .await
                    {
                        Ok(()) => Ok(authorized.status),
                        Err(err) => Err(err),
                    }
                }
                Err(err) => Err(err),
            };
            if let Err(err) = store.update_in(cx, |store, _window, cx| {
                store.finish_oauth_authorization(task_status_key, result, cx);
            }) {
                event!(
                    Level::ERROR,
                    error = ?err,
                    "finish MCP OAuth authorization failed"
                );
            }
        });
        self.oauth_tasks.insert(status_key, task);
        cx.emit(McpRuntimeStoreEvent::StatusChanged);
        cx.notify();
    }

    pub(crate) fn discard_draft_oauth_authorization(
        &mut self,
        server_id: &str,
        cx: &mut Context<Self>,
    ) {
        self.oauth_tasks.remove(server_id);
        self.oauth_task_targets.remove(server_id);
        self.statuses.remove(server_id);
        cx.emit(McpRuntimeStoreEvent::StatusChanged);
        cx.notify();
    }

    pub(crate) fn promote_draft_oauth_authorization(
        &mut self,
        draft_key: &str,
        server_id: String,
        server: config::McpServerTomlConfig,
        cx: &mut Context<Self>,
    ) {
        if let Some(target) = self.oauth_task_targets.get_mut(draft_key) {
            target.status_server_id = server_id.clone();
            target.server = server.clone();
        }
        if let Some(status) = self.statuses.remove(draft_key) {
            self.replace_server_auth_status(server_id, &server, status.auth, status.last_error);
        }
        cx.emit(McpRuntimeStoreEvent::StatusChanged);
        cx.notify();
    }

    pub(crate) fn replace_saved_server_status(
        &mut self,
        server_id: String,
        server: &config::McpServerTomlConfig,
        auth: McpOAuthStatusSnapshot,
        cx: &mut Context<Self>,
    ) {
        self.replace_server_auth_status(server_id, server, auth, None);
        cx.emit(McpRuntimeStoreEvent::StatusChanged);
        cx.notify();
    }

    pub(crate) fn auth_status(&self, server_id: &str) -> Option<McpOAuthStatusSnapshot> {
        self.statuses
            .get(server_id)
            .map(|status| status.auth.clone())
    }

    pub(crate) fn sign_out_server(
        &mut self,
        server_id: String,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let server = config::read(cx, |config| config.mcp_servers.get(&server_id).cloned());
        let Some(server) = server else {
            self.last_error = Some(format!("MCP server `{server_id}` not found"));
            cx.emit(McpRuntimeStoreEvent::StatusChanged);
            cx.notify();
            return;
        };
        if !oauth_configured(&server) {
            self.last_error = Some(format!("mcp server `{server_id}` does not enable OAuth"));
            cx.emit(McpRuntimeStoreEvent::StatusChanged);
            cx.notify();
            return;
        }
        if let Some(url) = server.url.as_deref()
            && let Err(err) = mcp_oauth::delete_credentials(url, cx)
        {
            self.last_error = Some(err);
            cx.emit(McpRuntimeStoreEvent::StatusChanged);
            cx.notify();
            return;
        }

        self.disconnect_server(server_id.clone(), window, cx);
        self.set_server_auth_status(server_id, &server, McpOAuthStatusSnapshot::SignedOut, None);
        self.last_error = None;
        cx.emit(McpRuntimeStoreEvent::StatusChanged);
        cx.notify();
    }

    pub(crate) fn disconnect_server(
        &mut self,
        server_id: String,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        self.statuses.remove(&server_id);
        self.server_tasks.remove(&server_id);
        self.oauth_tasks.remove(&server_id);
        self.oauth_task_targets.remove(&server_id);
        self.disconnect_tasks.remove(&server_id);
        let manager = self.manager.clone();
        let store = cx.entity().downgrade();
        let task_server_id = server_id.clone();
        let finish_server_id = server_id.clone();
        let task = window.spawn(cx, async move |cx| {
            let result = gpui_tokio::Tokio::spawn(cx, async move {
                let mut manager = manager.lock().await;
                manager.disconnect_server(&task_server_id).await;
            })
            .await
            .map_err(|err| err.to_string());
            if let Err(err) = store.update_in(cx, |store, _window, cx| {
                store.finish_disconnect_server(finish_server_id, result, cx);
            }) {
                event!(Level::ERROR, error = ?err, "finish MCP server disconnect failed");
            }
        });
        self.disconnect_tasks.insert(server_id, task);
        cx.emit(McpRuntimeStoreEvent::StatusChanged);
        cx.notify();
    }

    fn finish_oauth_authorization(
        &mut self,
        server_id: String,
        result: Result<McpOAuthStatusSnapshot, String>,
        cx: &mut Context<Self>,
    ) {
        self.oauth_tasks.remove(&server_id);
        let target = self.oauth_task_targets.remove(&server_id);
        let (status_server_id, server) = match target {
            Some(target) => {
                let server = config::read(cx, |config| {
                    config.mcp_servers.get(&target.status_server_id).cloned()
                })
                .unwrap_or(target.server);
                (target.status_server_id, Some(server))
            }
            None => {
                let server = config::read(cx, |config| config.mcp_servers.get(&server_id).cloned());
                (server_id, server)
            }
        };
        match result {
            Ok(status) => {
                if let Some(server) = server {
                    self.set_server_auth_status(status_server_id, &server, status, None);
                }
                self.last_error = None;
            }
            Err(err) => {
                if let Some(server) = server {
                    self.set_server_auth_status(
                        status_server_id,
                        &server,
                        McpOAuthStatusSnapshot::Failed {
                            message: err.clone(),
                        },
                        Some(err.clone()),
                    );
                }
                self.last_error = Some(err);
            }
        }
        cx.emit(McpRuntimeStoreEvent::StatusChanged);
        cx.notify();
    }

    fn replace_server_auth_status(
        &mut self,
        server_id: String,
        server: &config::McpServerTomlConfig,
        auth: McpOAuthStatusSnapshot,
        last_error: Option<String>,
    ) {
        self.statuses.insert(
            server_id.clone(),
            server_status_snapshot(server_id, server, auth, last_error),
        );
    }

    fn set_server_auth_status(
        &mut self,
        server_id: String,
        server: &config::McpServerTomlConfig,
        auth: McpOAuthStatusSnapshot,
        last_error: Option<String>,
    ) {
        let updated_at_unix_ms = now_unix_ms();
        if let Some(status) = self.statuses.get_mut(&server_id) {
            status.auth = auth;
            status.last_error = last_error;
            status.updated_at_unix_ms = updated_at_unix_ms;
            return;
        }

        self.statuses.insert(
            server_id.clone(),
            server_status_snapshot(server_id, server, auth, last_error),
        );
    }

    fn finish_disconnect_server(
        &mut self,
        server_id: String,
        result: Result<(), String>,
        cx: &mut Context<Self>,
    ) {
        self.disconnect_tasks.remove(&server_id);
        if let Err(err) = result {
            self.last_error = Some(err);
        }
        cx.emit(McpRuntimeStoreEvent::StatusChanged);
        cx.notify();
    }

    fn finish_server_test(
        &mut self,
        server_id: String,
        result: Result<McpPreparedTools, String>,
        cx: &mut Context<Self>,
    ) {
        self.server_tasks.remove(&server_id);
        match result {
            Ok(prepared) => {
                self.last_error = None;
                self.apply_statuses(prepared.statuses);
            }
            Err(err) => {
                self.set_server_failed_status(server_id, err.clone(), cx);
                self.last_error = Some(err);
            }
        }
        cx.emit(McpRuntimeStoreEvent::StatusChanged);
        cx.notify();
    }

    fn apply_statuses(&mut self, statuses: Vec<McpServerStatusSnapshot>) {
        for status in statuses {
            self.statuses.insert(status.server_id.clone(), status);
        }
    }

    fn set_server_failed_status(
        &mut self,
        server_id: String,
        message: String,
        cx: &mut Context<Self>,
    ) {
        let server = config::read(cx, |config| config.mcp_servers.get(&server_id).cloned());
        let Some(server) = server else {
            return;
        };
        self.statuses.insert(
            server_id.clone(),
            McpServerStatusSnapshot {
                server_id,
                display_name: server.display_name.clone(),
                transport: transport_kind_snapshot(server.transport),
                state: McpServerConnectionState::Failed,
                auth: oauth_error_status(&server, &message),
                server_info: None,
                tools: Vec::new(),
                last_error: Some(message),
                updated_at_unix_ms: now_unix_ms(),
            },
        );
    }

    fn spawn_event_listener(
        mut event_rx: mpsc::UnboundedReceiver<McpRuntimeEvent>,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        cx.spawn(async move |store, cx| {
            while let Some(event) = event_rx.recv().await {
                let Some(store) = store.upgrade() else {
                    break;
                };
                store.update(cx, |store, cx| {
                    store.handle_runtime_event(event, cx);
                });
            }
        })
    }

    fn handle_runtime_event(&mut self, event: McpRuntimeEvent, cx: &mut Context<Self>) {
        match event {
            McpRuntimeEvent::ServerStatusChanged(status) => {
                let status = *status;
                self.statuses.insert(status.server_id.clone(), status);
            }
            McpRuntimeEvent::ToolsChanged { server_id, tools } => {
                if let Some(status) = self.statuses.get_mut(&server_id) {
                    status.tools = tools;
                    status.updated_at_unix_ms = now_unix_ms();
                }
            }
            McpRuntimeEvent::OAuthChanged { server_id, status } => {
                if let Some(server_status) = self.statuses.get_mut(&server_id) {
                    server_status.auth = status;
                    server_status.updated_at_unix_ms = now_unix_ms();
                }
            }
            McpRuntimeEvent::OAuthCredentialsChanged(snapshot) => {
                let snapshot = *snapshot;
                match mcp_oauth::write_credentials_detached(
                    &snapshot.server_url,
                    snapshot.credentials,
                    cx,
                ) {
                    Ok(()) => {
                        let server = config::read(cx, |config| {
                            config.mcp_servers.get(&snapshot.server_id).cloned()
                        });
                        if let Some(server) = server {
                            self.set_server_auth_status(
                                snapshot.server_id,
                                &server,
                                snapshot.status,
                                None,
                            );
                        }
                        self.last_error = None;
                    }
                    Err(err) => {
                        let server = config::read(cx, |config| {
                            config.mcp_servers.get(&snapshot.server_id).cloned()
                        });
                        if let Some(server) = server {
                            self.set_server_auth_status(
                                snapshot.server_id,
                                &server,
                                McpOAuthStatusSnapshot::Failed {
                                    message: err.clone(),
                                },
                                Some(err.clone()),
                            );
                        }
                        self.last_error = Some(err);
                    }
                }
            }
        }
        cx.emit(McpRuntimeStoreEvent::StatusChanged);
        cx.notify();
    }
}

pub(crate) fn init(cx: &mut App) -> AiChat2Result<()> {
    let store = cx.new(McpRuntimeStore::new);
    cx.set_global(McpRuntimeGlobal(store));
    Ok(())
}

pub(crate) fn runtime(cx: &App) -> Entity<McpRuntimeStore> {
    cx.global::<McpRuntimeGlobal>().entity()
}

pub(crate) fn session_manager_handle(cx: &App) -> Arc<Mutex<McpSessionManager>> {
    runtime(cx).read(cx).manager.clone()
}

pub(crate) async fn prepare_run_request(
    mut request: AgentRunRequest,
    cx: &mut AsyncWindowContext,
) -> Result<McpPreparedRun, McpPrepareRunError> {
    let inherited_approval_mode =
        mcp_default_approval_from_chat_form(request.settings_snapshot.tool_policy.approval_mode);
    let setup = match cx
        .update(|_, cx| build_mcp_runtime_setup(cx, inherited_approval_mode))
        .map_err(|err| err.to_string())
        .and_then(|result| result.map_err(|err| err.to_string()))
    {
        Ok(setup) => setup,
        Err(message) => {
            return Err(McpPrepareRunError { request, message });
        }
    };
    if setup.configs.is_empty() {
        let preflight_statuses = setup.preflight_statuses.clone();
        if let Err(message) = close_all_sessions(cx, setup).await {
            return Err(McpPrepareRunError { request, message });
        }
        request.runtime_snapshot.mcp_config_hash = None;
        request.runtime_snapshot.mcp_config_snapshot = None;
        apply_preflight_statuses(cx, preflight_statuses).await;
        return Ok(McpPreparedRun { request });
    }

    match mcp_config_hash(&setup.snapshot) {
        Ok(config_hash) => {
            request.runtime_snapshot.mcp_config_hash = Some(config_hash);
            request.runtime_snapshot.mcp_config_snapshot = Some(setup.snapshot.clone());
        }
        Err(err) => {
            return Err(McpPrepareRunError {
                request,
                message: err.to_string(),
            });
        }
    }

    let setup = match attach_oauth_credentials(setup, cx).await {
        Ok(setup) => setup,
        Err(message) => {
            return Err(McpPrepareRunError { request, message });
        }
    };

    let manager = match cx.update(|_, cx| runtime(cx).read(cx).manager.clone()) {
        Ok(manager) => manager,
        Err(err) => {
            return Err(McpPrepareRunError {
                request,
                message: err.to_string(),
            });
        }
    };
    let mut tool_registry = std::mem::take(&mut request.tool_registry);
    let preflight_statuses = setup.preflight_statuses.clone();
    let prepared_result = gpui_tokio::Tokio::spawn(cx, async move {
        let mut manager = manager.lock().await;
        let result = manager
            .prepare_tool_registry(&mut tool_registry, setup.snapshot, setup.configs)
            .await;
        (tool_registry, result)
    })
    .await;

    let (tool_registry, prepared) = match prepared_result {
        Ok(result) => result,
        Err(err) => {
            return Err(McpPrepareRunError {
                request,
                message: err.to_string(),
            });
        }
    };
    request.tool_registry = tool_registry;
    match prepared {
        Ok(mut prepared) => {
            prepared.statuses.extend(preflight_statuses);
            let connected_servers = connected_mcp_server_sources(&prepared.statuses);
            add_mcp_enabled_sources(&mut request, connected_servers);
            if let Err(err) = cx.update(move |_, cx| {
                runtime(cx).update(cx, |store, cx| {
                    store.last_error = None;
                    store.apply_statuses(prepared.statuses);
                    cx.emit(McpRuntimeStoreEvent::StatusChanged);
                    cx.notify();
                });
            }) {
                event!(Level::ERROR, error = ?err, "update MCP run setup statuses failed");
            }
            Ok(McpPreparedRun { request })
        }
        Err(err) => {
            let message = err.to_string();
            if let Err(update_err) = cx.update({
                let message = message.clone();
                move |_, cx| {
                    runtime(cx).update(cx, |store, cx| {
                        store.last_error = Some(message);
                        cx.emit(McpRuntimeStoreEvent::StatusChanged);
                        cx.notify();
                    });
                }
            }) {
                event!(Level::ERROR, error = ?update_err, "update MCP run setup error failed");
            }
            Err(McpPrepareRunError { request, message })
        }
    }
}

async fn close_all_sessions(
    cx: &mut AsyncWindowContext,
    setup: McpRuntimeSetup,
) -> Result<(), String> {
    let manager = cx
        .update(|_, cx| runtime(cx).read(cx).manager.clone())
        .map_err(|err| err.to_string())?;
    gpui_tokio::Tokio::spawn(cx, async move {
        let mut manager = manager.lock().await;
        let mut registry = ToolRegistry::default();
        manager
            .prepare_tool_registry(&mut registry, setup.snapshot, setup.configs)
            .await
    })
    .await
    .map_err(|err| err.to_string())?
    .map(|_| ())
    .map_err(|err| err.to_string())
}

async fn apply_preflight_statuses(
    cx: &mut AsyncWindowContext,
    statuses: Vec<McpServerStatusSnapshot>,
) {
    if statuses.is_empty() {
        return;
    }
    if let Err(err) = cx.update(move |_, cx| {
        runtime(cx).update(cx, |store, cx| {
            store.apply_statuses(statuses);
            cx.emit(McpRuntimeStoreEvent::StatusChanged);
            cx.notify();
        });
    }) {
        event!(Level::ERROR, error = ?err, "update MCP preflight statuses failed");
    }
}

async fn attach_oauth_credentials(
    mut setup: McpRuntimeSetup,
    cx: &mut AsyncWindowContext,
) -> Result<McpRuntimeSetup, String> {
    for config in &mut setup.configs {
        let ai_chat_agent::McpServerTransport::StreamableHttp(http) = &mut config.server.transport
        else {
            continue;
        };
        if http.oauth.is_none() {
            continue;
        }
        if let Some(credentials) = mcp_oauth::read_credentials(&http.url, cx).await? {
            http.oauth_credentials =
                Some(serde_json::to_value(credentials).map_err(|err| err.to_string())?);
        }
    }
    Ok(setup)
}

fn build_mcp_runtime_setup(
    cx: &App,
    inherited_approval_mode: McpToolApprovalModeSnapshot,
) -> AiChat2Result<McpRuntimeSetup> {
    config::read(cx, |config| {
        setup_from_config_with_approval(config, inherited_approval_mode.clone())
    })
}

fn build_mcp_runtime_setup_for_server(cx: &App, server_id: &str) -> AiChat2Result<McpRuntimeSetup> {
    config::read(cx, |config| setup_from_config_for_server(config, server_id))
}

#[cfg(test)]
fn setup_from_config(config: &AiChat2Config) -> AiChat2Result<McpRuntimeSetup> {
    setup_from_config_with_approval(
        config,
        mcp_default_approval_from_chat_form(config.chat_form.approval_mode),
    )
}

fn setup_from_config_with_approval(
    config: &AiChat2Config,
    inherited_approval_mode: McpToolApprovalModeSnapshot,
) -> AiChat2Result<McpRuntimeSetup> {
    setup_from_config_filtered(config, None, true, inherited_approval_mode)
}

fn setup_from_config_for_server(
    config: &AiChat2Config,
    server_id: &str,
) -> AiChat2Result<McpRuntimeSetup> {
    setup_from_config_filtered(
        config,
        Some(server_id),
        false,
        mcp_default_approval_from_chat_form(config.chat_form.approval_mode),
    )
}

fn setup_from_config_filtered(
    config: &AiChat2Config,
    only_server_id: Option<&str>,
    fail_required: bool,
    inherited_approval_mode: McpToolApprovalModeSnapshot,
) -> AiChat2Result<McpRuntimeSetup> {
    let mut snapshot = McpRuntimeConfigSnapshot {
        servers: Vec::new(),
    };
    let mut configs = Vec::new();
    let mut preflight_statuses = Vec::new();
    for (server_id, server) in &config.mcp_servers {
        if !server.enabled || only_server_id.is_some_and(|only| only != server_id) {
            continue;
        }
        match server_runtime_parts(server_id, server, inherited_approval_mode.clone()) {
            Ok((server_snapshot, runtime_config)) => {
                snapshot.servers.push(server_snapshot);
                configs.push(runtime_config);
            }
            Err(err) if fail_required && server.required => return Err(err),
            Err(err) => {
                preflight_statuses.push(preflight_failed_status(
                    server_id,
                    server,
                    err.to_string(),
                ));
            }
        }
    }
    Ok(McpRuntimeSetup {
        snapshot,
        configs,
        preflight_statuses,
    })
}

fn server_runtime_parts(
    server_id: &str,
    server: &config::McpServerTomlConfig,
    inherited_approval_mode: McpToolApprovalModeSnapshot,
) -> AiChat2Result<(
    ai_chat_core::McpServerRuntimeConfigSnapshot,
    McpServerRuntimeConfig,
)> {
    let snapshot = server.to_runtime_config_snapshot(server_id)?;
    let runtime_config = server.to_server_runtime_config(server_id, inherited_approval_mode)?;
    Ok((snapshot, runtime_config))
}

fn mcp_default_approval_from_chat_form(
    approval_mode: ToolApprovalMode,
) -> McpToolApprovalModeSnapshot {
    match approval_mode {
        ToolApprovalMode::RequestApproval => McpToolApprovalModeSnapshot::Prompt,
        ToolApprovalMode::AutoApprove | ToolApprovalMode::FullAccess => {
            McpToolApprovalModeSnapshot::Auto
        }
    }
}

fn preflight_failed_status(
    server_id: &str,
    server: &config::McpServerTomlConfig,
    message: String,
) -> McpServerStatusSnapshot {
    McpServerStatusSnapshot {
        server_id: server_id.to_string(),
        display_name: server.display_name.clone(),
        transport: transport_kind_snapshot(server.transport),
        state: McpServerConnectionState::Failed,
        auth: configured_auth_status(server),
        server_info: None,
        tools: Vec::new(),
        last_error: Some(message),
        updated_at_unix_ms: now_unix_ms(),
    }
}

fn server_status_snapshot(
    server_id: String,
    server: &config::McpServerTomlConfig,
    auth: McpOAuthStatusSnapshot,
    last_error: Option<String>,
) -> McpServerStatusSnapshot {
    McpServerStatusSnapshot {
        server_id,
        display_name: server.display_name.clone(),
        transport: transport_kind_snapshot(server.transport),
        state: if server.enabled {
            McpServerConnectionState::NotConnected
        } else {
            McpServerConnectionState::Disabled
        },
        auth,
        server_info: None,
        tools: Vec::new(),
        last_error,
        updated_at_unix_ms: now_unix_ms(),
    }
}

fn connected_mcp_server_sources(statuses: &[McpServerStatusSnapshot]) -> BTreeSet<String> {
    statuses
        .iter()
        .filter(|status| status.state == McpServerConnectionState::Connected)
        .map(|status| status.server_id.clone())
        .collect()
}

fn add_mcp_enabled_sources(request: &mut AgentRunRequest, server_ids: BTreeSet<String>) {
    for server_id in server_ids {
        let source = ToolSource::Mcp { server_id };
        if !request
            .settings_snapshot
            .tool_policy
            .enabled_sources
            .contains(&source)
        {
            request
                .settings_snapshot
                .tool_policy
                .enabled_sources
                .push(source);
        }
    }
}

pub(crate) fn transport_icon_kind(row: &McpServerStatusRow) -> McpServerTransportKindSnapshot {
    transport_kind_snapshot(row.transport)
}

fn transport_kind_snapshot(transport: McpTransportKind) -> McpServerTransportKindSnapshot {
    match transport {
        McpTransportKind::Stdio => McpServerTransportKindSnapshot::Stdio,
        McpTransportKind::StreamableHttp => McpServerTransportKindSnapshot::StreamableHttp,
    }
}

fn row_auth(
    server: &config::McpServerTomlConfig,
    status: Option<&McpServerStatusSnapshot>,
) -> McpOAuthStatusSnapshot {
    match status.map(|status| status.auth.clone()) {
        Some(McpOAuthStatusSnapshot::NotConfigured) if oauth_configured(server) => {
            configured_auth_status(server)
        }
        Some(auth) => auth,
        None => configured_auth_status(server),
    }
}

fn configured_auth_status(server: &config::McpServerTomlConfig) -> McpOAuthStatusSnapshot {
    if oauth_configured(server) {
        McpOAuthStatusSnapshot::SignedOut
    } else {
        McpOAuthStatusSnapshot::NotConfigured
    }
}

fn oauth_error_status(
    server: &config::McpServerTomlConfig,
    message: &str,
) -> McpOAuthStatusSnapshot {
    if !oauth_configured(server) {
        return McpOAuthStatusSnapshot::NotConfigured;
    }
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

fn oauth_configured(server: &config::McpServerTomlConfig) -> bool {
    server.transport == McpTransportKind::StreamableHttp && server.oauth.is_some()
}

fn oauth_authorization_config(
    server_id: &str,
    server: &config::McpServerTomlConfig,
) -> Result<(String, mcp_oauth::AuthorizationCodePkceConfig), String> {
    if server.transport != McpTransportKind::StreamableHttp {
        return Err(format!(
            "mcp server `{server_id}` OAuth authorization is only supported for HTTP transport"
        ));
    }
    let server_url = server
        .url
        .as_ref()
        .filter(|url| !url.trim().is_empty())
        .cloned()
        .ok_or_else(|| format!("mcp server `{server_id}` URL is required for OAuth"))?;
    let oauth = server
        .oauth
        .as_ref()
        .ok_or_else(|| format!("mcp server `{server_id}` does not enable OAuth"))?;

    match oauth {
        McpOAuthTomlConfig::AuthorizationCodePkce {
            scopes,
            client_id,
            client_metadata_url,
            callback_port,
            callback_url,
            ..
        } => Ok((
            server_url,
            mcp_oauth::AuthorizationCodePkceConfig {
                scopes: scopes.clone(),
                client_id: client_id.clone(),
                client_metadata_url: client_metadata_url.clone(),
                callback_port: *callback_port,
                callback_url: callback_url.clone(),
            },
        )),
        McpOAuthTomlConfig::ClientCredentials { .. } => Err(format!(
            "mcp server `{server_id}` uses OAuth client_credentials; browser authorization is not applicable"
        )),
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
    use crate::state::config::{
        McpOAuthTomlConfig, McpServerTomlConfig, McpToolApprovalMode, McpTransportKind,
    };
    use std::collections::BTreeMap;

    #[test]
    fn runtime_setup_uses_enabled_servers_only() {
        let mut config = AiChat2Config::default();
        config.mcp_servers.insert(
            "enabled".to_string(),
            McpServerTomlConfig {
                command: Some("echo".to_string()),
                ..Default::default()
            },
        );
        config.mcp_servers.insert(
            "disabled".to_string(),
            McpServerTomlConfig {
                enabled: false,
                command: Some("echo".to_string()),
                ..Default::default()
            },
        );

        let setup = setup_from_config(&config).unwrap();

        assert_eq!(setup.snapshot.servers.len(), 1);
        assert_eq!(setup.configs.len(), 1);
        assert!(setup.preflight_statuses.is_empty());
        assert_eq!(setup.configs[0].server.server_id, "enabled");
    }

    #[test]
    fn runtime_setup_skips_non_required_preflight_errors() {
        let mut config = AiChat2Config::default();
        config.mcp_servers.insert(
            "valid".to_string(),
            McpServerTomlConfig {
                command: Some("echo".to_string()),
                ..Default::default()
            },
        );
        config.mcp_servers.insert(
            "optional_bad".to_string(),
            McpServerTomlConfig {
                transport: McpTransportKind::StreamableHttp,
                url: Some("https://example.com/mcp".to_string()),
                headers: BTreeMap::from([("Mcp-Session-Id".to_string(), "bad".to_string())]),
                ..Default::default()
            },
        );

        let setup = setup_from_config(&config).unwrap();

        assert_eq!(setup.snapshot.servers.len(), 1);
        assert_eq!(setup.configs.len(), 1);
        assert_eq!(setup.configs[0].server.server_id, "valid");
        assert_eq!(setup.preflight_statuses.len(), 1);
        assert_eq!(setup.preflight_statuses[0].server_id, "optional_bad");
        assert_eq!(
            setup.preflight_statuses[0].state,
            McpServerConnectionState::Failed
        );
        assert!(
            setup.preflight_statuses[0]
                .last_error
                .as_deref()
                .is_some_and(|message| message.contains("reserved"))
        );
    }

    #[test]
    fn runtime_setup_fails_required_preflight_errors() {
        let mut config = AiChat2Config::default();
        config.mcp_servers.insert(
            "required_bad".to_string(),
            McpServerTomlConfig {
                required: true,
                transport: McpTransportKind::StreamableHttp,
                url: Some("https://example.com/mcp".to_string()),
                headers: BTreeMap::from([("Mcp-Session-Id".to_string(), "bad".to_string())]),
                ..Default::default()
            },
        );

        let err = setup_from_config(&config).unwrap_err().to_string();

        assert!(err.contains("reserved"));
    }

    #[test]
    fn runtime_setup_preserves_deny_default_for_agent_filtering() {
        let mut config = AiChat2Config::default();
        config.mcp_servers.insert(
            "server".to_string(),
            McpServerTomlConfig {
                command: Some("echo".to_string()),
                default_tools_approval_mode: Some(McpToolApprovalMode::Deny),
                ..Default::default()
            },
        );

        let setup = setup_from_config(&config).unwrap();

        assert_eq!(
            setup.configs[0].default_approval_mode,
            ai_chat_core::McpToolApprovalModeSnapshot::Deny
        );
    }

    #[test]
    fn runtime_setup_inherits_chat_form_approval_default() {
        for (chat_form_mode, expected_mcp_mode, expected_policy) in [
            (
                ToolApprovalMode::RequestApproval,
                McpToolApprovalModeSnapshot::Prompt,
                ai_chat_core::ToolApprovalPolicy::OnRequest,
            ),
            (
                ToolApprovalMode::AutoApprove,
                McpToolApprovalModeSnapshot::Auto,
                ai_chat_core::ToolApprovalPolicy::Never,
            ),
            (
                ToolApprovalMode::FullAccess,
                McpToolApprovalModeSnapshot::Auto,
                ai_chat_core::ToolApprovalPolicy::Never,
            ),
        ] {
            let mut config = AiChat2Config::default();
            config.chat_form.approval_mode = chat_form_mode;
            config.mcp_servers.insert(
                "server".to_string(),
                McpServerTomlConfig {
                    command: Some("echo".to_string()),
                    ..Default::default()
                },
            );

            let setup = setup_from_config(&config).unwrap();

            assert_eq!(setup.configs[0].default_approval_mode, expected_mcp_mode);
            assert_eq!(setup.configs[0].default_approval_policy, expected_policy);
        }
    }

    #[test]
    fn oauth_error_status_maps_authorization_required() {
        let server = oauth_http_server();

        assert_eq!(
            oauth_error_status(&server, "OAuth authorization required"),
            McpOAuthStatusSnapshot::AuthorizationRequired
        );
    }

    #[test]
    fn oauth_error_status_maps_scope_upgrade_required() {
        let server = oauth_http_server();

        assert!(matches!(
            oauth_error_status(&server, "Insufficient scope"),
            McpOAuthStatusSnapshot::ScopeUpgradeRequired { .. }
        ));
    }

    #[gpui::test]
    fn finish_oauth_authorization_uses_pending_draft_server(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("config.toml");
        let config = AiChat2Config::load_from_path_for_test(&path).expect("load test config");

        cx.update(|cx| {
            config::install_for_test(cx, config).expect("install config store");
            let store = cx.new(McpRuntimeStore::new);
            store.update(cx, |store, cx| {
                let server_id = "draft-oauth".to_string();
                let server = oauth_http_server();
                let authorized = McpOAuthStatusSnapshot::Authorized {
                    scopes: vec!["tools".to_string()],
                    expires_at_unix_ms: Some(123),
                };

                store.oauth_task_targets.insert(
                    server_id.clone(),
                    McpOAuthTaskTarget {
                        status_server_id: server_id.clone(),
                        server: server.clone(),
                    },
                );
                store.set_server_auth_status(
                    server_id.clone(),
                    &server,
                    McpOAuthStatusSnapshot::SigningIn,
                    None,
                );

                store.finish_oauth_authorization(server_id.clone(), Ok(authorized.clone()), cx);

                assert_eq!(store.auth_status(&server_id), Some(authorized));
                assert!(!store.oauth_task_targets.contains_key(&server_id));
            });
        });
    }

    #[gpui::test]
    fn promoted_draft_oauth_finish_updates_saved_server(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("config.toml");
        let config = AiChat2Config::load_from_path_for_test(&path).expect("load test config");

        cx.update(|cx| {
            config::install_for_test(cx, config).expect("install config store");
            let store = cx.new(McpRuntimeStore::new);
            store.update(cx, |store, cx| {
                let draft_key = "__draft".to_string();
                let saved_server_id = "renamed".to_string();
                let server = oauth_http_server();
                let authorized = McpOAuthStatusSnapshot::Authorized {
                    scopes: vec!["tools".to_string()],
                    expires_at_unix_ms: Some(123),
                };

                store.oauth_task_targets.insert(
                    draft_key.clone(),
                    McpOAuthTaskTarget {
                        status_server_id: draft_key.clone(),
                        server: server.clone(),
                    },
                );
                store.set_server_auth_status(
                    draft_key.clone(),
                    &server,
                    McpOAuthStatusSnapshot::SigningIn,
                    None,
                );
                store.promote_draft_oauth_authorization(
                    &draft_key,
                    saved_server_id.clone(),
                    server,
                    cx,
                );

                store.finish_oauth_authorization(draft_key.clone(), Ok(authorized.clone()), cx);

                assert_eq!(store.auth_status(&saved_server_id), Some(authorized));
                assert_eq!(store.auth_status(&draft_key), None);
                assert!(!store.oauth_task_targets.contains_key(&draft_key));
            });
        });
    }

    fn oauth_http_server() -> McpServerTomlConfig {
        McpServerTomlConfig {
            transport: McpTransportKind::StreamableHttp,
            url: Some("https://example.com/mcp".to_string()),
            oauth: Some(McpOAuthTomlConfig::AuthorizationCodePkce {
                scopes: Vec::new(),
                client_id: None,
                client_metadata_url: None,
                resource: None,
                callback_port: None,
                callback_url: None,
            }),
            ..Default::default()
        }
    }
}

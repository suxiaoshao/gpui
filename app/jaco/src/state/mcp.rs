pub(crate) mod oauth;

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use gpui::{App, AppContext, AsyncApp, Context, Entity, EventEmitter, Global, Subscription, Task};
use jaco_agent::{
    AgentRunRequest, McpOAuthCredentialsSnapshot, McpOAuthStatusSnapshot, McpPreparedTools,
    McpRuntimeEvent, McpServerConnectionState, McpServerInfoSnapshot, McpServerRuntimeConfig,
    McpServerStatusSnapshot, McpServerTransport, McpServerTransportKindSnapshot,
    McpSessionIdentity, McpSessionManager, McpSessionPruneMode, McpToolSnapshot, ToolRegistry,
    mcp_server_fingerprint,
};
use jaco_core::{McpToolApprovalModeSnapshot, ToolApprovalMode, ToolSource};
use tokio::sync::{Mutex, mpsc};
use tracing::{Level, event};

use crate::{
    errors::JacoResult,
    state::config::{self, JacoConfig, McpOAuthTomlConfig, McpTransportKind},
};

use self::oauth as mcp_oauth;

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
    pub(crate) auth: jaco_agent::McpOAuthStatusSnapshot,
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
    next_oauth_attempt_id: u64,
    oauth_credential_write_tasks: BTreeMap<String, Task<()>>,
    disconnect_tasks: BTreeMap<String, Task<()>>,
    disconnect_generations: BTreeMap<String, u64>,
    server_generations: BTreeMap<String, u64>,
    accepted_sessions: BTreeMap<String, McpSessionIdentity>,
    published_mcp_servers: BTreeMap<String, config::McpServerTomlConfig>,
    last_error: Option<String>,
    _config_subscription: Subscription,
    _event_task: Task<()>,
}

#[derive(Clone, Debug, PartialEq)]
struct McpOAuthTaskTarget {
    attempt_id: u64,
    status_server_id: String,
    server: config::McpServerTomlConfig,
}

#[derive(Clone, Debug, PartialEq)]
struct McpRuntimeSetup {
    configs: Vec<McpServerRuntimeConfig>,
    preflight_statuses: Vec<McpServerStatusSnapshot>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum McpRuntimeStoreEvent {
    StatusChanged,
}

impl EventEmitter<McpRuntimeStoreEvent> for McpRuntimeStore {}

fn ready_mcp_servers(
    operation: &config::ConfigOperation,
) -> Option<BTreeMap<String, config::McpServerTomlConfig>> {
    match operation {
        config::ConfigOperation::Ready(ready) => Some(ready.data().mcp_servers.clone()),
        _ => None,
    }
}

fn changed_existing_mcp_servers(
    previous: &BTreeMap<String, config::McpServerTomlConfig>,
    next: &BTreeMap<String, config::McpServerTomlConfig>,
) -> BTreeSet<String> {
    previous
        .iter()
        .filter_map(|(server_id, server)| {
            (next.get(server_id) != Some(server)).then_some(server_id.clone())
        })
        .collect()
}

fn session_identities(configs: &[McpServerRuntimeConfig]) -> BTreeMap<String, McpSessionIdentity> {
    configs
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
        .collect()
}

impl McpRuntimeStore {
    fn new(cx: &mut Context<Self>) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let manager = McpSessionManager::new().with_event_sender(event_tx);
        let event_task = Self::spawn_event_listener(event_rx, cx);
        let config_store = config::store(cx);
        let published_mcp_servers = config_store.read(cx, ready_mcp_servers).unwrap_or_default();
        let server_generations = published_mcp_servers
            .keys()
            .map(|server_id| (server_id.clone(), 0))
            .collect();
        let config_subscription = config_store.observe_select(
            cx,
            |operation: &config::ConfigOperation| ready_mcp_servers(operation),
            |store, servers, cx| store.on_ready_mcp_servers_changed(servers, cx),
        );
        Self {
            manager: Arc::new(Mutex::new(manager)),
            statuses: BTreeMap::new(),
            server_tasks: BTreeMap::new(),
            oauth_tasks: BTreeMap::new(),
            oauth_task_targets: BTreeMap::new(),
            next_oauth_attempt_id: 0,
            oauth_credential_write_tasks: BTreeMap::new(),
            disconnect_tasks: BTreeMap::new(),
            disconnect_generations: BTreeMap::new(),
            server_generations,
            accepted_sessions: BTreeMap::new(),
            published_mcp_servers,
            last_error: None,
            _config_subscription: config_subscription,
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
        if let Some(servers) = config::store(cx).read(cx, ready_mcp_servers) {
            self.reconcile_mcp_servers(servers, cx);
        }
        let setup = match build_mcp_runtime_setup_for_server(cx, &server_id) {
            Ok(mut setup) => {
                self.assign_setup_generations(&mut setup);
                setup
            }
            Err(err) => {
                self.last_error = Some(err.to_string());
                cx.emit(McpRuntimeStoreEvent::StatusChanged);
                cx.notify();
                return;
            }
        };
        let attempt_generation = setup
            .configs
            .first()
            .map(|config| config.generation)
            .unwrap_or_else(|| self.current_server_generation(&server_id));
        let attempt_generations = BTreeMap::from([(server_id.clone(), attempt_generation)]);
        let manager = self.manager.clone();
        let store = cx.entity().downgrade();
        self.server_tasks.remove(&server_id);
        self.last_error = None;
        let task_server_id = server_id.clone();
        let task = window.spawn(cx, async move |cx| {
            let result = match attach_oauth_credentials(setup, cx).await {
                Ok(setup) => {
                    let identities = session_identities(&setup.configs);
                    let authority_generations = attempt_generations.clone();
                    match store.update_in(cx, |store, _window, cx| {
                        store.reconcile_current_ready_config(cx);
                        store.accept_sessions_if_current(identities, &authority_generations)
                    }) {
                        Ok(true) => gpui_tokio::Tokio::spawn(cx, async move {
                            let mut registry = ToolRegistry::default();
                            let mut manager = manager.lock().await;
                            let preflight_statuses = setup.preflight_statuses;
                            manager
                                .prepare_tool_registry(
                                    &mut registry,
                                    setup.configs,
                                    authority_generations,
                                    McpSessionPruneMode::KeepExistingSessions,
                                )
                                .await
                                .map(|mut prepared| {
                                    prepared.statuses.extend(preflight_statuses);
                                    prepared
                                })
                        })
                        .await
                        .map_err(|err| err.to_string())
                        .and_then(|result| result.map_err(|err| err.to_string())),
                        Ok(false) => Err("MCP server config changed while testing".to_string()),
                        Err(err) => Err(err.to_string()),
                    }
                }
                Err(err) => Err(err),
            };

            if let Err(err) = store.update_in(cx, |store, _window, cx| {
                store.finish_server_test(task_server_id, attempt_generation, result, cx);
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
        let Some(credentials_key) = mcp_oauth::credentials_key_for_server(&server_id, &server)
            .unwrap_or_else(|err| {
                self.last_error = Some(err);
                None
            })
        else {
            cx.emit(McpRuntimeStoreEvent::StatusChanged);
            cx.notify();
            return;
        };

        let attempt_id = self.next_oauth_attempt_id();
        self.oauth_tasks.remove(&status_key);
        self.oauth_task_targets.insert(
            status_key.clone(),
            McpOAuthTaskTarget {
                attempt_id,
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
                    match mcp_oauth::write_credentials(
                        &credentials_key,
                        &authorized.credentials,
                        cx,
                    )
                    .await
                    {
                        Ok(()) => Ok(authorized.status),
                        Err(err) => Err(err),
                    }
                }
                Err(err) => Err(err),
            };
            if let Err(err) = store.update_in(cx, |store, _window, cx| {
                store.finish_oauth_authorization(task_status_key, attempt_id, result, cx);
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

    pub(crate) fn finish_server_sign_out(
        &mut self,
        server_id: String,
        server: config::McpServerTomlConfig,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        if !oauth_configured(&server) {
            self.last_error = Some(format!("mcp server `{server_id}` does not enable OAuth"));
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
        _window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let reconciled = config::store(cx)
            .read(cx, ready_mcp_servers)
            .map(|servers| self.reconcile_mcp_servers(servers, cx))
            .unwrap_or_default();
        if !reconciled.contains(&server_id) {
            self.invalidate_server_runtime(server_id, cx);
            cx.emit(McpRuntimeStoreEvent::StatusChanged);
            cx.notify();
        }
    }

    fn on_ready_mcp_servers_changed(
        &mut self,
        servers: &Option<BTreeMap<String, config::McpServerTomlConfig>>,
        cx: &mut Context<Self>,
    ) {
        if crate::app::is_shutting_down() {
            return;
        }
        let Some(servers) = servers.clone() else {
            return;
        };
        self.reconcile_mcp_servers(servers, cx);
    }

    fn reconcile_mcp_servers(
        &mut self,
        next: BTreeMap<String, config::McpServerTomlConfig>,
        cx: &mut Context<Self>,
    ) -> BTreeSet<String> {
        let affected = changed_existing_mcp_servers(&self.published_mcp_servers, &next);
        self.published_mcp_servers = next;
        for server_id in self.published_mcp_servers.keys() {
            self.server_generations
                .entry(server_id.clone())
                .or_insert(0);
        }
        for server_id in &affected {
            self.invalidate_server_runtime(server_id.clone(), cx);
        }
        if !affected.is_empty() {
            cx.emit(McpRuntimeStoreEvent::StatusChanged);
            cx.notify();
        }
        affected
    }

    fn invalidate_server_runtime(&mut self, server_id: String, cx: &mut Context<Self>) {
        let generation = self
            .server_generations
            .entry(server_id.clone())
            .and_modify(|generation| {
                *generation = generation
                    .checked_add(1)
                    .expect("MCP server generation overflow")
            })
            .or_insert(1)
            .to_owned();
        self.accepted_sessions.remove(&server_id);
        self.statuses.remove(&server_id);
        self.server_tasks.remove(&server_id);
        let oauth_task_keys = self
            .oauth_task_targets
            .iter()
            .filter_map(|(task_key, target)| {
                (task_key == &server_id || target.status_server_id == server_id)
                    .then_some(task_key.clone())
            })
            .chain(
                self.oauth_tasks
                    .contains_key(&server_id)
                    .then_some(server_id.clone()),
            )
            .collect::<BTreeSet<_>>();
        for task_key in oauth_task_keys {
            self.oauth_tasks.remove(&task_key);
            self.oauth_task_targets.remove(&task_key);
            self.statuses.remove(&task_key);
        }
        self.oauth_credential_write_tasks.remove(&server_id);
        self.disconnect_tasks.remove(&server_id);
        self.disconnect_generations
            .insert(server_id.clone(), generation);
        let manager = self.manager.clone();
        let task_server_id = server_id.clone();
        let finish_server_id = server_id.clone();
        let task = cx.spawn(async move |store, cx| {
            let result = gpui_tokio::Tokio::spawn(cx, async move {
                let mut manager = manager.lock().await;
                manager
                    .advance_server_generation(&task_server_id, generation)
                    .await;
            })
            .await
            .map_err(|err| err.to_string());
            let Some(store) = store.upgrade() else {
                return;
            };
            store.update(cx, |store, cx| {
                store.finish_disconnect_server(finish_server_id, generation, result, cx);
            });
        });
        self.disconnect_tasks.insert(server_id, task);
    }

    fn current_server_generation(&self, server_id: &str) -> u64 {
        self.server_generations.get(server_id).copied().unwrap_or(0)
    }

    fn assign_setup_generations(&self, setup: &mut McpRuntimeSetup) {
        for config in &mut setup.configs {
            config.generation = self.current_server_generation(&config.server.server_id);
        }
    }

    fn accept_sessions(&mut self, identities: BTreeMap<String, McpSessionIdentity>) {
        self.accepted_sessions.extend(identities);
    }

    fn accept_sessions_if_current(
        &mut self,
        identities: BTreeMap<String, McpSessionIdentity>,
        authority_generations: &BTreeMap<String, u64>,
    ) -> bool {
        if !self.generations_are_current(authority_generations)
            || identities.iter().any(|(server_id, identity)| {
                authority_generations.get(server_id) != Some(&identity.generation)
            })
        {
            return false;
        }
        for server_id in authority_generations.keys() {
            self.accepted_sessions.remove(server_id);
        }
        self.accept_sessions(identities);
        true
    }

    fn reconcile_current_ready_config(&mut self, cx: &mut Context<Self>) {
        if crate::app::is_shutting_down() {
            return;
        }
        if let Some(servers) = config::store(cx).read(cx, ready_mcp_servers) {
            self.reconcile_mcp_servers(servers, cx);
        }
    }

    fn next_oauth_attempt_id(&mut self) -> u64 {
        self.next_oauth_attempt_id = self
            .next_oauth_attempt_id
            .checked_add(1)
            .expect("MCP OAuth attempt ID overflow");
        self.next_oauth_attempt_id
    }

    fn sessions_are_current(&self, identities: &BTreeMap<String, McpSessionIdentity>) -> bool {
        identities
            .iter()
            .all(|(server_id, identity)| self.accepted_sessions.get(server_id) == Some(identity))
    }

    fn generations_are_current(&self, generations: &BTreeMap<String, u64>) -> bool {
        generations
            .iter()
            .all(|(server_id, generation)| self.current_server_generation(server_id) == *generation)
    }

    fn generation_snapshot(&self) -> BTreeMap<String, u64> {
        self.server_generations.clone()
    }

    fn generation_snapshot_is_current(&self, generations: &BTreeMap<String, u64>) -> bool {
        &self.server_generations == generations
    }

    fn apply_preflight_statuses_if_current(
        &mut self,
        statuses: Vec<McpServerStatusSnapshot>,
        attempt_generations: &BTreeMap<String, u64>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.generation_snapshot_is_current(attempt_generations) {
            return false;
        }
        self.apply_statuses(statuses);
        cx.emit(McpRuntimeStoreEvent::StatusChanged);
        cx.notify();
        true
    }

    fn finish_oauth_authorization(
        &mut self,
        server_id: String,
        attempt_id: u64,
        result: Result<McpOAuthStatusSnapshot, String>,
        cx: &mut Context<Self>,
    ) {
        let Some(current_target) = self.oauth_task_targets.get(&server_id) else {
            return;
        };
        if current_target.attempt_id != attempt_id {
            return;
        }
        self.oauth_tasks.remove(&server_id);
        let target = self
            .oauth_task_targets
            .remove(&server_id)
            .expect("MCP OAuth task target disappeared after validation");
        let server = config::read(cx, |config| {
            config.mcp_servers.get(&target.status_server_id).cloned()
        })
        .unwrap_or(target.server);
        let status_server_id = target.status_server_id;
        match result {
            Ok(status) => {
                self.set_server_auth_status(status_server_id, &server, status, None);
                self.last_error = None;
            }
            Err(err) => {
                self.set_server_auth_status(
                    status_server_id,
                    &server,
                    McpOAuthStatusSnapshot::Failed {
                        message: err.clone(),
                    },
                    Some(err.clone()),
                );
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
        generation: u64,
        result: Result<(), String>,
        cx: &mut Context<Self>,
    ) {
        if self.disconnect_generations.get(&server_id) != Some(&generation) {
            return;
        }
        self.disconnect_tasks.remove(&server_id);
        self.disconnect_generations.remove(&server_id);
        if let Err(err) = result {
            self.last_error = Some(err);
        }
        cx.emit(McpRuntimeStoreEvent::StatusChanged);
        cx.notify();
    }

    fn finish_server_test(
        &mut self,
        server_id: String,
        generation: u64,
        result: Result<McpPreparedTools, String>,
        cx: &mut Context<Self>,
    ) {
        self.reconcile_current_ready_config(cx);
        if self.current_server_generation(&server_id) != generation {
            return;
        }
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
        self.reconcile_current_ready_config(cx);
        let identity = match &event {
            McpRuntimeEvent::ServerStatusChanged { identity, status } => {
                if status.server_id != identity.server_id {
                    return;
                }
                identity
            }
            McpRuntimeEvent::ToolsChanged { identity, .. }
            | McpRuntimeEvent::OAuthChanged { identity, .. } => identity,
            McpRuntimeEvent::OAuthCredentialsChanged(snapshot) => &snapshot.identity,
        };
        if self
            .accepted_sessions
            .get(&identity.server_id)
            .is_none_or(|accepted| accepted != identity)
        {
            return;
        }
        match event {
            McpRuntimeEvent::ServerStatusChanged { status, .. } => {
                let status = *status;
                self.statuses.insert(status.server_id.clone(), status);
            }
            McpRuntimeEvent::ToolsChanged { identity, tools } => {
                let server_id = identity.server_id;
                if let Some(status) = self.statuses.get_mut(&server_id) {
                    status.tools = tools;
                    status.updated_at_unix_ms = now_unix_ms();
                }
            }
            McpRuntimeEvent::OAuthChanged { identity, status } => {
                let server_id = identity.server_id;
                if let Some(server_status) = self.statuses.get_mut(&server_id) {
                    server_status.auth = status;
                    server_status.updated_at_unix_ms = now_unix_ms();
                }
            }
            McpRuntimeEvent::OAuthCredentialsChanged(snapshot) => {
                self.spawn_oauth_credentials_write(*snapshot, cx);
            }
        }
        cx.emit(McpRuntimeStoreEvent::StatusChanged);
        cx.notify();
    }

    fn spawn_oauth_credentials_write(
        &mut self,
        snapshot: McpOAuthCredentialsSnapshot,
        cx: &mut Context<Self>,
    ) {
        let identity = snapshot.identity.clone();
        let server_id = identity.server_id.clone();
        let key_result = config::read(cx, |config| config.mcp_servers.get(&server_id).cloned())
            .ok_or_else(|| format!("MCP server `{server_id}` not found"))
            .and_then(|server| {
                mcp_oauth::credentials_key_for_server(&server_id, &server)?.ok_or_else(|| {
                    format!("MCP server `{server_id}` does not have OAuth credentials")
                })
            });
        let credentials_key = match key_result {
            Ok(key) => key,
            Err(err) => {
                self.finish_oauth_credentials_write(identity, snapshot.status, Err(err), cx);
                return;
            }
        };

        self.oauth_credential_write_tasks.remove(&server_id);
        let status = snapshot.status;
        let credentials = snapshot.credentials;
        let task_identity = identity;
        let task = cx.spawn(async move |store, cx| {
            let result =
                mcp_oauth::write_credentials_value(&credentials_key, credentials, cx).await;
            let Some(store) = store.upgrade() else {
                return;
            };
            store.update(cx, |store, cx| {
                store.finish_oauth_credentials_write(task_identity, status, result, cx);
            });
        });
        self.oauth_credential_write_tasks.insert(server_id, task);
    }

    fn finish_oauth_credentials_write(
        &mut self,
        identity: McpSessionIdentity,
        status: McpOAuthStatusSnapshot,
        result: Result<(), String>,
        cx: &mut Context<Self>,
    ) {
        self.reconcile_current_ready_config(cx);
        if self.accepted_sessions.get(&identity.server_id) != Some(&identity) {
            return;
        }
        let server_id = identity.server_id;
        self.oauth_credential_write_tasks.remove(&server_id);
        let server = config::read(cx, |config| config.mcp_servers.get(&server_id).cloned());
        match result {
            Ok(()) => {
                if let Some(server) = server {
                    self.set_server_auth_status(server_id, &server, status, None);
                }
                self.last_error = None;
            }
            Err(err) => {
                if let Some(server) = server {
                    self.set_server_auth_status(
                        server_id,
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
}

pub(crate) fn init(cx: &mut App) -> JacoResult<()> {
    let store = cx.new(McpRuntimeStore::new);
    cx.set_global(McpRuntimeGlobal(store));
    oauth::init_credential_cleanup(cx);
    Ok(())
}

pub(crate) fn runtime(cx: &App) -> Entity<McpRuntimeStore> {
    cx.global::<McpRuntimeGlobal>().entity()
}

fn reconcile_runtime_with_ready_config(cx: &mut AsyncApp) {
    cx.update(|cx| {
        let Some(servers) = config::store(cx).read(cx, ready_mcp_servers) else {
            return;
        };
        runtime(cx).update(cx, |runtime, cx| {
            runtime.reconcile_mcp_servers(servers, cx);
        });
    });
}

pub(crate) async fn prepare_run_request(
    mut request: AgentRunRequest,
    cx: &mut AsyncApp,
) -> Result<McpPreparedRun, McpPrepareRunError> {
    reconcile_runtime_with_ready_config(cx);
    let inherited_approval_mode =
        mcp_default_approval_from_chat_form(request.settings_snapshot.tool_policy.approval_mode);
    let setup = match cx.update(|cx| build_mcp_runtime_setup(cx, inherited_approval_mode)) {
        Ok(setup) => setup,
        Err(error) => {
            return Err(McpPrepareRunError {
                request,
                message: error.to_string(),
            });
        }
    };
    let mut setup = setup;
    cx.update(|cx| {
        runtime(cx).update(cx, |runtime, _cx| {
            runtime.assign_setup_generations(&mut setup);
        });
    });
    let attempt_generations =
        cx.update(|cx| runtime(cx).read_with(cx, |runtime, _| runtime.generation_snapshot()));
    if setup.configs.is_empty() {
        let accepted = cx.update(|cx| {
            runtime(cx).update(cx, |runtime, cx| {
                runtime.reconcile_current_ready_config(cx);
                runtime.generation_snapshot_is_current(&attempt_generations)
                    && runtime.accept_sessions_if_current(BTreeMap::new(), &attempt_generations)
            })
        });
        if !accepted {
            return Err(McpPrepareRunError {
                request,
                message: "MCP server config changed while preparing the run".to_string(),
            });
        }
        let preflight_statuses = setup.preflight_statuses.clone();
        if let Err(message) = close_all_sessions(cx, setup, attempt_generations.clone()).await {
            return Err(McpPrepareRunError { request, message });
        }
        reconcile_runtime_with_ready_config(cx);
        let attempt_is_current = cx.update(|cx| {
            runtime(cx).read_with(cx, |runtime, _| {
                runtime.generation_snapshot_is_current(&attempt_generations)
            })
        });
        if !attempt_is_current {
            return Err(McpPrepareRunError {
                request,
                message: "MCP server config changed while preparing the run".to_string(),
            });
        }
        apply_preflight_statuses(cx, preflight_statuses, attempt_generations).await;
        return Ok(McpPreparedRun { request });
    }

    let setup = match attach_oauth_credentials(setup, cx).await {
        Ok(setup) => setup,
        Err(message) => {
            return Err(McpPrepareRunError { request, message });
        }
    };

    let accepted_sessions = session_identities(&setup.configs);
    let accepted = cx.update(|cx| {
        runtime(cx).update(cx, |runtime, cx| {
            runtime.reconcile_current_ready_config(cx);
            runtime.generation_snapshot_is_current(&attempt_generations)
                && runtime
                    .accept_sessions_if_current(accepted_sessions.clone(), &attempt_generations)
        })
    });
    if !accepted {
        return Err(McpPrepareRunError {
            request,
            message: "MCP server config changed while preparing the run".to_string(),
        });
    }

    let manager = cx.update(|cx| runtime(cx).read(cx).manager.clone());
    let mut tool_registry = std::mem::take(&mut request.tool_registry);
    let preflight_statuses = setup.preflight_statuses.clone();
    let attempt_sessions = accepted_sessions.clone();
    let manager_generations = attempt_generations.clone();
    let prepared_result = gpui_tokio::Tokio::spawn(cx, async move {
        let mut manager = manager.lock().await;
        let result = manager
            .prepare_tool_registry(
                &mut tool_registry,
                setup.configs,
                manager_generations,
                McpSessionPruneMode::PruneStale,
            )
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
    reconcile_runtime_with_ready_config(cx);
    let attempt_is_current = cx.update(|cx| {
        runtime(cx).read_with(cx, |runtime, _| {
            runtime.generation_snapshot_is_current(&attempt_generations)
                && runtime.sessions_are_current(&attempt_sessions)
        })
    });
    if !attempt_is_current {
        return Err(McpPrepareRunError {
            request,
            message: "MCP server config changed while preparing the run".to_string(),
        });
    }
    request.tool_registry = tool_registry;
    match prepared {
        Ok(mut prepared) => {
            prepared.statuses.extend(preflight_statuses);
            let connected_servers = connected_mcp_server_sources(&prepared.statuses);
            add_mcp_enabled_sources(&mut request, connected_servers);
            cx.update(move |cx| {
                runtime(cx).update(cx, |store, cx| {
                    if !store.generation_snapshot_is_current(&attempt_generations)
                        || !store.sessions_are_current(&attempt_sessions)
                    {
                        return;
                    }
                    store.last_error = None;
                    store.apply_statuses(prepared.statuses);
                    cx.emit(McpRuntimeStoreEvent::StatusChanged);
                    cx.notify();
                });
            });
            Ok(McpPreparedRun { request })
        }
        Err(err) => {
            let message = err.to_string();
            cx.update({
                let message = message.clone();
                let attempt_sessions = attempt_sessions.clone();
                let attempt_generations = attempt_generations.clone();
                move |cx| {
                    runtime(cx).update(cx, |store, cx| {
                        if !store.generation_snapshot_is_current(&attempt_generations)
                            || !store.sessions_are_current(&attempt_sessions)
                        {
                            return;
                        }
                        store.last_error = Some(message);
                        cx.emit(McpRuntimeStoreEvent::StatusChanged);
                        cx.notify();
                    });
                }
            });
            Err(McpPrepareRunError { request, message })
        }
    }
}

async fn close_all_sessions(
    cx: &mut AsyncApp,
    setup: McpRuntimeSetup,
    authority_generations: BTreeMap<String, u64>,
) -> Result<(), String> {
    let manager = cx.update(|cx| runtime(cx).read(cx).manager.clone());
    gpui_tokio::Tokio::spawn(cx, async move {
        let mut manager = manager.lock().await;
        let mut registry = ToolRegistry::default();
        manager
            .prepare_tool_registry(
                &mut registry,
                setup.configs,
                authority_generations,
                McpSessionPruneMode::PruneStale,
            )
            .await
    })
    .await
    .map_err(|err| err.to_string())?
    .map(|_| ())
    .map_err(|err| err.to_string())
}

async fn apply_preflight_statuses(
    cx: &mut AsyncApp,
    statuses: Vec<McpServerStatusSnapshot>,
    attempt_generations: BTreeMap<String, u64>,
) {
    if statuses.is_empty() {
        return;
    }
    cx.update(move |cx| {
        runtime(cx).update(cx, |store, cx| {
            store.apply_preflight_statuses_if_current(statuses, &attempt_generations, cx);
        });
    });
}

async fn attach_oauth_credentials(
    setup: McpRuntimeSetup,
    cx: &mut AsyncApp,
) -> Result<McpRuntimeSetup, String> {
    let mut attached_setup = McpRuntimeSetup {
        configs: Vec::with_capacity(setup.configs.len()),
        preflight_statuses: setup.preflight_statuses,
    };
    for mut config in setup.configs {
        match attach_oauth_credentials_to_config(&mut config, cx).await {
            Ok(()) => attached_setup.configs.push(config),
            Err(err) => {
                record_oauth_credentials_attach_error(&mut attached_setup, &config, err)?;
            }
        }
    }
    Ok(attached_setup)
}

fn record_oauth_credentials_attach_error(
    setup: &mut McpRuntimeSetup,
    config: &McpServerRuntimeConfig,
    err: String,
) -> Result<(), String> {
    if config.required {
        return Err(err);
    }
    setup
        .preflight_statuses
        .push(runtime_preflight_failed_status(config, err));
    Ok(())
}

async fn attach_oauth_credentials_to_config(
    config: &mut McpServerRuntimeConfig,
    cx: &mut AsyncApp,
) -> Result<(), String> {
    {
        let jaco_agent::McpServerTransport::StreamableHttp(http) = &mut config.server.transport
        else {
            return Ok(());
        };
        if http.oauth.is_none() {
            return Ok(());
        }
        let Some(oauth) = http.oauth.as_ref() else {
            return Ok(());
        };
        let Some(credentials_key) =
            mcp_oauth::credentials_key_for_oauth_value(&config.server.server_id, &http.url, oauth)?
        else {
            return Ok(());
        };
        if let Some(credentials) = mcp_oauth::read_credentials(&credentials_key, cx).await? {
            http.oauth_credentials =
                Some(serde_json::to_value(credentials).map_err(|err| err.to_string())?);
        }
    }
    Ok(())
}

fn build_mcp_runtime_setup(
    cx: &App,
    inherited_approval_mode: McpToolApprovalModeSnapshot,
) -> JacoResult<McpRuntimeSetup> {
    config::read(cx, |config| {
        setup_from_config_with_approval(config, inherited_approval_mode.clone())
    })
}

fn build_mcp_runtime_setup_for_server(cx: &App, server_id: &str) -> JacoResult<McpRuntimeSetup> {
    config::read(cx, |config| setup_from_config_for_server(config, server_id))
}

#[cfg(test)]
fn setup_from_config(config: &JacoConfig) -> JacoResult<McpRuntimeSetup> {
    setup_from_config_with_approval(
        config,
        mcp_default_approval_from_chat_form(config.chat_form.approval_mode),
    )
}

fn setup_from_config_with_approval(
    config: &JacoConfig,
    inherited_approval_mode: McpToolApprovalModeSnapshot,
) -> JacoResult<McpRuntimeSetup> {
    setup_from_config_filtered(config, None, true, inherited_approval_mode)
}

fn setup_from_config_for_server(
    config: &JacoConfig,
    server_id: &str,
) -> JacoResult<McpRuntimeSetup> {
    setup_from_config_filtered(
        config,
        Some(server_id),
        false,
        mcp_default_approval_from_chat_form(config.chat_form.approval_mode),
    )
}

fn setup_from_config_filtered(
    config: &JacoConfig,
    only_server_id: Option<&str>,
    fail_required: bool,
    inherited_approval_mode: McpToolApprovalModeSnapshot,
) -> JacoResult<McpRuntimeSetup> {
    let mut configs = Vec::new();
    let mut preflight_statuses = Vec::new();
    for (server_id, server) in &config.mcp_servers {
        if !server.enabled || only_server_id.is_some_and(|only| only != server_id) {
            continue;
        }
        match server_runtime_parts(server_id, server, inherited_approval_mode.clone()) {
            Ok(runtime_config) => configs.push(runtime_config),
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
        configs,
        preflight_statuses,
    })
}

fn server_runtime_parts(
    server_id: &str,
    server: &config::McpServerTomlConfig,
    inherited_approval_mode: McpToolApprovalModeSnapshot,
) -> JacoResult<McpServerRuntimeConfig> {
    server.to_server_runtime_config(server_id, inherited_approval_mode)
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

fn runtime_preflight_failed_status(
    config: &McpServerRuntimeConfig,
    message: String,
) -> McpServerStatusSnapshot {
    McpServerStatusSnapshot {
        server_id: config.server.server_id.clone(),
        display_name: config.server.display_name.clone(),
        transport: runtime_transport_kind_snapshot(&config.server.transport),
        state: McpServerConnectionState::Failed,
        auth: runtime_oauth_error_status(&config.server.transport, &message),
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

fn runtime_transport_kind_snapshot(
    transport: &McpServerTransport,
) -> McpServerTransportKindSnapshot {
    match transport {
        McpServerTransport::Stdio(_) => McpServerTransportKindSnapshot::Stdio,
        McpServerTransport::StreamableHttp(_) => McpServerTransportKindSnapshot::StreamableHttp,
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
    oauth_error_status_for_configured(oauth_configured(server), message)
}

fn runtime_oauth_error_status(
    transport: &McpServerTransport,
    message: &str,
) -> McpOAuthStatusSnapshot {
    let configured = matches!(
        transport,
        McpServerTransport::StreamableHttp(http) if http.oauth.is_some()
    );
    oauth_error_status_for_configured(configured, message)
}

fn oauth_error_status_for_configured(configured: bool, message: &str) -> McpOAuthStatusSnapshot {
    if !configured {
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
            resource,
            callback_port,
            callback_url,
        } => Ok((
            server_url,
            mcp_oauth::AuthorizationCodePkceConfig {
                scopes: scopes.clone(),
                client_id: client_id.clone(),
                client_metadata_url: client_metadata_url.clone(),
                resource: resource.clone(),
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
        let mut config = JacoConfig::default();
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

        assert_eq!(setup.configs.len(), 1);
        assert!(setup.preflight_statuses.is_empty());
        assert_eq!(setup.configs[0].server.server_id, "enabled");
    }

    #[test]
    fn config_diff_only_invalidates_existing_changed_servers() {
        let unchanged = stdio_server("unchanged");
        let removed = stdio_server("removed");
        let disabled = stdio_server("disabled");
        let changed = stdio_server("old");
        let previous = BTreeMap::from([
            ("unchanged".to_string(), unchanged.clone()),
            ("removed".to_string(), removed),
            ("disabled".to_string(), disabled.clone()),
            ("changed".to_string(), changed.clone()),
        ]);
        let next = BTreeMap::from([
            ("unchanged".to_string(), unchanged),
            (
                "disabled".to_string(),
                McpServerTomlConfig {
                    enabled: false,
                    ..disabled
                },
            ),
            (
                "changed".to_string(),
                McpServerTomlConfig {
                    command: Some("new".to_string()),
                    ..changed
                },
            ),
            ("added".to_string(), stdio_server("added")),
        ]);

        assert_eq!(
            changed_existing_mcp_servers(&previous, &next),
            BTreeSet::from([
                "changed".to_string(),
                "disabled".to_string(),
                "removed".to_string(),
            ])
        );
    }

    #[gpui::test]
    fn runtime_events_reject_stale_generation_after_config_aba(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("config.toml");
        let mut config = JacoConfig::load_from_path_for_test(&path).expect("load test config");
        let server = stdio_server("echo");
        config
            .mcp_servers
            .insert("server".to_string(), server.clone());

        cx.update(|cx| {
            config::install_for_test(cx, path, config).expect("install config store");
            let runtime = cx.new(McpRuntimeStore::new);
            runtime.update(cx, |runtime, cx| {
                let current = McpSessionIdentity {
                    server_id: "server".to_string(),
                    fingerprint: "same-fingerprint".to_string(),
                    generation: 2,
                };
                let stale = McpSessionIdentity {
                    generation: 0,
                    ..current.clone()
                };
                runtime
                    .accepted_sessions
                    .insert("server".to_string(), current.clone());
                runtime.statuses.insert(
                    "server".to_string(),
                    connected_status_with_tool("server", &server),
                );

                runtime.handle_runtime_event(
                    McpRuntimeEvent::ToolsChanged {
                        identity: stale.clone(),
                        tools: vec![tool_snapshot("stale")],
                    },
                    cx,
                );
                runtime.handle_runtime_event(
                    McpRuntimeEvent::OAuthCredentialsChanged(Box::new(
                        McpOAuthCredentialsSnapshot {
                            identity: stale,
                            server_url: "https://stale.example/mcp".to_string(),
                            credentials: serde_json::json!({}),
                            status: McpOAuthStatusSnapshot::SignedOut,
                        },
                    )),
                    cx,
                );

                assert_eq!(runtime.statuses["server"].tools[0].name, "tool");
                assert!(runtime.oauth_credential_write_tasks.is_empty());

                runtime.handle_runtime_event(
                    McpRuntimeEvent::ToolsChanged {
                        identity: current,
                        tools: vec![tool_snapshot("current")],
                    },
                    cx,
                );
                assert_eq!(runtime.statuses["server"].tools[0].name, "current");
            });
        });
    }

    #[gpui::test]
    fn accepted_session_authority_replaces_partial_sessions_and_rejects_old_events(
        cx: &mut gpui::TestAppContext,
    ) {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("config.toml");
        let mut config = JacoConfig::load_from_path_for_test(&path).expect("load test config");
        let server = stdio_server("echo");
        config
            .mcp_servers
            .insert("active".to_string(), server.clone());

        cx.update(|cx| {
            config::install_for_test(cx, path, config).expect("install config store");
            let runtime = cx.new(McpRuntimeStore::new);
            runtime.update(cx, |runtime, cx| {
                let active_old = McpSessionIdentity {
                    server_id: "active".to_string(),
                    fingerprint: "old".to_string(),
                    generation: 0,
                };
                let active_new = McpSessionIdentity {
                    fingerprint: "new".to_string(),
                    ..active_old.clone()
                };
                let missing = McpSessionIdentity {
                    server_id: "missing".to_string(),
                    fingerprint: "missing".to_string(),
                    generation: 0,
                };
                let outside_authority = McpSessionIdentity {
                    server_id: "outside".to_string(),
                    fingerprint: "outside".to_string(),
                    generation: 0,
                };
                runtime
                    .accepted_sessions
                    .insert("active".to_string(), active_old.clone());
                runtime
                    .accepted_sessions
                    .insert("missing".to_string(), missing.clone());
                runtime
                    .accepted_sessions
                    .insert("outside".to_string(), outside_authority.clone());
                runtime.statuses.insert(
                    "active".to_string(),
                    connected_status_with_tool("active", &server),
                );

                let authority =
                    BTreeMap::from([("active".to_string(), 0), ("missing".to_string(), 0)]);
                assert!(runtime.accept_sessions_if_current(
                    BTreeMap::from([("active".to_string(), active_new.clone())]),
                    &authority,
                ));
                assert_eq!(runtime.accepted_sessions.get("active"), Some(&active_new));
                assert!(!runtime.accepted_sessions.contains_key("missing"));
                assert_eq!(
                    runtime.accepted_sessions.get("outside"),
                    Some(&outside_authority)
                );

                assert!(runtime.accept_sessions_if_current(BTreeMap::new(), &authority));
                assert_eq!(
                    runtime.accepted_sessions,
                    BTreeMap::from([("outside".to_string(), outside_authority)])
                );
                runtime.handle_runtime_event(
                    McpRuntimeEvent::ToolsChanged {
                        identity: active_old.clone(),
                        tools: vec![tool_snapshot("stale")],
                    },
                    cx,
                );
                runtime.handle_runtime_event(
                    McpRuntimeEvent::OAuthCredentialsChanged(Box::new(
                        McpOAuthCredentialsSnapshot {
                            identity: active_old,
                            server_url: "https://stale.example/mcp".to_string(),
                            credentials: serde_json::json!({}),
                            status: McpOAuthStatusSnapshot::SignedOut,
                        },
                    )),
                    cx,
                );
                assert_eq!(runtime.statuses["active"].tools[0].name, "tool");
                assert!(runtime.oauth_credential_write_tasks.is_empty());
            });
        });
    }

    #[gpui::test]
    fn stale_completions_do_not_remove_or_overwrite_new_generation_tasks(
        cx: &mut gpui::TestAppContext,
    ) {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("config.toml");
        let mut config = JacoConfig::load_from_path_for_test(&path).expect("load test config");
        let server = oauth_http_server();
        config
            .mcp_servers
            .insert("server".to_string(), server.clone());

        cx.update(|cx| {
            config::install_for_test(cx, path, config).expect("install config store");
            let runtime = cx.new(McpRuntimeStore::new);
            runtime.update(cx, |runtime, cx| {
                let current = McpSessionIdentity {
                    server_id: "server".to_string(),
                    fingerprint: "same-fingerprint".to_string(),
                    generation: 2,
                };
                let stale = McpSessionIdentity {
                    generation: 1,
                    ..current.clone()
                };
                runtime.server_generations.insert("server".to_string(), 2);
                runtime
                    .accepted_sessions
                    .insert("server".to_string(), current);
                runtime
                    .disconnect_generations
                    .insert("server".to_string(), 2);
                runtime
                    .disconnect_tasks
                    .insert("server".to_string(), Task::ready(()));
                runtime
                    .server_tasks
                    .insert("server".to_string(), Task::ready(()));
                runtime
                    .oauth_credential_write_tasks
                    .insert("server".to_string(), Task::ready(()));
                runtime.statuses.insert(
                    "server".to_string(),
                    connected_status_with_tool("server", &server),
                );

                runtime.finish_disconnect_server("server".to_string(), 1, Ok(()), cx);
                runtime.finish_server_test(
                    "server".to_string(),
                    1,
                    Ok(McpPreparedTools {
                        statuses: Vec::new(),
                    }),
                    cx,
                );
                runtime.finish_oauth_credentials_write(
                    stale,
                    McpOAuthStatusSnapshot::SignedOut,
                    Err("stale write".to_string()),
                    cx,
                );
                let stale_preflight =
                    preflight_failed_status("server", &server, "stale preflight".to_string());
                assert!(!runtime.apply_preflight_statuses_if_current(
                    vec![stale_preflight],
                    &BTreeMap::from([("server".to_string(), 1)]),
                    cx,
                ));

                assert!(runtime.disconnect_tasks.contains_key("server"));
                assert_eq!(runtime.disconnect_generations.get("server"), Some(&2));
                assert!(runtime.server_tasks.contains_key("server"));
                assert!(runtime.oauth_credential_write_tasks.contains_key("server"));
                assert_eq!(runtime.statuses["server"].tools[0].name, "tool");
                assert!(runtime.last_error.is_none());
            });
        });
    }

    #[gpui::test]
    fn config_publication_reconciles_removed_disabled_and_changed_servers(
        cx: &mut gpui::TestAppContext,
    ) {
        cx.executor().allow_parking();
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("config.toml");
        let mut initial = JacoConfig::load_from_path_for_test(&path).expect("load test config");
        let unchanged = stdio_server("unchanged");
        let removed = stdio_server("removed");
        let disabled = stdio_server("disabled");
        let changed = stdio_server("old");
        initial.mcp_servers = BTreeMap::from([
            ("unchanged".to_string(), unchanged.clone()),
            ("removed".to_string(), removed.clone()),
            ("disabled".to_string(), disabled.clone()),
            ("changed".to_string(), changed.clone()),
        ]);
        std::fs::write(
            &path,
            toml::to_string_pretty(&initial).expect("encode initial config"),
        )
        .expect("write initial config");
        let mut externally_reloaded = initial.clone();
        externally_reloaded.mcp_servers.remove("removed");
        externally_reloaded
            .mcp_servers
            .get_mut("disabled")
            .expect("disabled server exists")
            .enabled = false;
        externally_reloaded
            .mcp_servers
            .insert("changed".to_string(), stdio_server("new"));
        externally_reloaded
            .mcp_servers
            .insert("added".to_string(), stdio_server("added"));

        let runtime = cx.update(|cx| {
            gpui_tokio::init(cx);
            config::install_for_test(cx, path.clone(), initial).expect("install config store");
            let runtime = cx.new(McpRuntimeStore::new);
            runtime.update(cx, |runtime, _cx| {
                for (server_id, server) in [
                    ("unchanged", &unchanged),
                    ("removed", &removed),
                    ("disabled", &disabled),
                    ("changed", &changed),
                ] {
                    runtime.statuses.insert(
                        server_id.to_string(),
                        connected_status_with_tool(server_id, server),
                    );
                    runtime
                        .server_tasks
                        .insert(server_id.to_string(), Task::ready(()));
                    runtime
                        .oauth_tasks
                        .insert(server_id.to_string(), Task::ready(()));
                    runtime.oauth_task_targets.insert(
                        server_id.to_string(),
                        McpOAuthTaskTarget {
                            attempt_id: 1,
                            status_server_id: server_id.to_string(),
                            server: server.clone(),
                        },
                    );
                    runtime
                        .oauth_credential_write_tasks
                        .insert(server_id.to_string(), Task::ready(()));
                }
            });
            std::fs::write(
                &path,
                toml::to_string_pretty(&externally_reloaded).expect("encode external config"),
            )
            .expect("write external config");
            config::request_reload(cx);
            runtime
        });

        let mut reconciled = false;
        for _ in 0..100 {
            cx.run_until_parked();
            reconciled = cx.update(|cx| {
                !runtime
                    .read(cx)
                    .published_mcp_servers
                    .contains_key("removed")
            });
            if reconciled {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(reconciled, "external config reload did not settle");

        cx.update(|cx| {
            let runtime = runtime.read(cx);
            for server_id in ["removed", "disabled", "changed"] {
                assert!(
                    !runtime.statuses.contains_key(server_id),
                    "{server_id} remained; statuses={:?}; published={:?}",
                    runtime.statuses.keys().collect::<Vec<_>>(),
                    runtime.published_mcp_servers.keys().collect::<Vec<_>>()
                );
                assert!(!runtime.server_tasks.contains_key(server_id));
                assert!(!runtime.oauth_tasks.contains_key(server_id));
                assert!(!runtime.oauth_task_targets.contains_key(server_id));
                assert!(!runtime.oauth_credential_write_tasks.contains_key(server_id));
            }
            assert!(runtime.statuses.contains_key("unchanged"));
            assert!(runtime.server_tasks.contains_key("unchanged"));
            assert!(runtime.oauth_tasks.contains_key("unchanged"));
            assert!(runtime.oauth_task_targets.contains_key("unchanged"));
            assert!(
                runtime
                    .oauth_credential_write_tasks
                    .contains_key("unchanged")
            );

            let rows = runtime
                .rows(cx)
                .into_iter()
                .map(|row| (row.server_id.clone(), row))
                .collect::<BTreeMap<_, _>>();
            assert!(!rows.contains_key("removed"));
            assert_eq!(rows["disabled"].tool_count, 0);
            assert_eq!(rows["changed"].tool_count, 0);
            assert_eq!(rows["added"].tool_count, 0);
            assert_eq!(rows["unchanged"].tool_count, 1);
        });
    }

    #[test]
    fn runtime_setup_skips_non_required_preflight_errors() {
        let mut config = JacoConfig::default();
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
        let mut config = JacoConfig::default();
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
    fn oauth_credentials_attach_error_fails_only_required_servers() {
        let optional_config = oauth_http_server()
            .to_server_runtime_config("optional_oauth", McpToolApprovalModeSnapshot::Prompt)
            .unwrap();
        let mut setup = McpRuntimeSetup {
            configs: Vec::new(),
            preflight_statuses: Vec::new(),
        };

        record_oauth_credentials_attach_error(
            &mut setup,
            &optional_config,
            "failed to deserialize OAuth credentials".to_string(),
        )
        .unwrap();

        assert_eq!(setup.preflight_statuses.len(), 1);
        assert_eq!(setup.preflight_statuses[0].server_id, "optional_oauth");
        assert_eq!(
            setup.preflight_statuses[0].state,
            McpServerConnectionState::Failed
        );
        assert_eq!(
            setup.preflight_statuses[0].auth,
            McpOAuthStatusSnapshot::Failed {
                message: "failed to deserialize OAuth credentials".to_string()
            }
        );

        let mut required_server = oauth_http_server();
        required_server.required = true;
        let required_config = required_server
            .to_server_runtime_config("required_oauth", McpToolApprovalModeSnapshot::Prompt)
            .unwrap();
        let err = record_oauth_credentials_attach_error(
            &mut setup,
            &required_config,
            "failed to deserialize OAuth credentials".to_string(),
        )
        .unwrap_err();

        assert!(err.contains("failed to deserialize"));
    }

    #[test]
    fn runtime_setup_preserves_deny_default_for_agent_filtering() {
        let mut config = JacoConfig::default();
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
            jaco_core::McpToolApprovalModeSnapshot::Deny
        );
    }

    #[test]
    fn runtime_setup_inherits_chat_form_approval_default() {
        for (chat_form_mode, expected_mcp_mode, expected_policy) in [
            (
                ToolApprovalMode::RequestApproval,
                McpToolApprovalModeSnapshot::Prompt,
                jaco_core::ToolApprovalPolicy::OnRequest,
            ),
            (
                ToolApprovalMode::AutoApprove,
                McpToolApprovalModeSnapshot::Auto,
                jaco_core::ToolApprovalPolicy::Never,
            ),
            (
                ToolApprovalMode::FullAccess,
                McpToolApprovalModeSnapshot::Auto,
                jaco_core::ToolApprovalPolicy::Never,
            ),
        ] {
            let mut config = JacoConfig::default();
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

    #[test]
    fn oauth_authorization_config_preserves_resource() {
        let mut server = oauth_http_server();
        if let Some(McpOAuthTomlConfig::AuthorizationCodePkce { resource, .. }) =
            server.oauth.as_mut()
        {
            *resource = Some("https://api.example.com/mcp".to_string());
        }

        let (server_url, config) = oauth_authorization_config("server", &server).unwrap();

        assert_eq!(server_url, "https://example.com/mcp");
        assert_eq!(
            config.resource.as_deref(),
            Some("https://api.example.com/mcp")
        );
    }

    #[gpui::test]
    fn finish_oauth_authorization_uses_pending_draft_server(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("config.toml");
        let config = JacoConfig::load_from_path_for_test(&path).expect("load test config");

        cx.update(|cx| {
            config::install_for_test(cx, path.clone(), config).expect("install config store");
            let store = cx.new(McpRuntimeStore::new);
            store.update(cx, |store, cx| {
                let server_id = "draft-oauth".to_string();
                let server = oauth_http_server();
                let authorized = McpOAuthStatusSnapshot::Authorized {
                    scopes: vec!["tools".to_string()],
                    expires_at_unix_ms: Some(123),
                };
                let attempt_id = store.next_oauth_attempt_id();

                store.oauth_task_targets.insert(
                    server_id.clone(),
                    McpOAuthTaskTarget {
                        attempt_id,
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

                store.finish_oauth_authorization(
                    server_id.clone(),
                    attempt_id,
                    Ok(authorized.clone()),
                    cx,
                );

                assert_eq!(store.auth_status(&server_id), Some(authorized));
                assert!(!store.oauth_task_targets.contains_key(&server_id));
            });
        });
    }

    #[gpui::test]
    fn promoted_draft_oauth_finish_updates_saved_server(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("config.toml");
        let config = JacoConfig::load_from_path_for_test(&path).expect("load test config");

        cx.update(|cx| {
            config::install_for_test(cx, path.clone(), config).expect("install config store");
            let store = cx.new(McpRuntimeStore::new);
            store.update(cx, |store, cx| {
                let draft_key = "__draft".to_string();
                let saved_server_id = "renamed".to_string();
                let server = oauth_http_server();
                let authorized = McpOAuthStatusSnapshot::Authorized {
                    scopes: vec!["tools".to_string()],
                    expires_at_unix_ms: Some(123),
                };
                let attempt_id = store.next_oauth_attempt_id();

                store.oauth_task_targets.insert(
                    draft_key.clone(),
                    McpOAuthTaskTarget {
                        attempt_id,
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

                store.finish_oauth_authorization(
                    draft_key.clone(),
                    attempt_id,
                    Ok(authorized.clone()),
                    cx,
                );

                assert_eq!(store.auth_status(&saved_server_id), Some(authorized));
                assert_eq!(store.auth_status(&draft_key), None);
                assert!(!store.oauth_task_targets.contains_key(&draft_key));
            });
        });
    }

    #[gpui::test]
    fn stale_oauth_authorization_completion_does_not_touch_new_attempt(
        cx: &mut gpui::TestAppContext,
    ) {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("config.toml");
        let config = JacoConfig::load_from_path_for_test(&path).expect("load test config");

        cx.update(|cx| {
            config::install_for_test(cx, path.clone(), config).expect("install config store");
            let store = cx.new(McpRuntimeStore::new);
            store.update(cx, |store, cx| {
                let status_key = "server".to_string();
                let server = oauth_http_server();
                let old_attempt_id = store.next_oauth_attempt_id();
                store
                    .oauth_tasks
                    .insert(status_key.clone(), Task::ready(()));
                store.oauth_task_targets.insert(
                    status_key.clone(),
                    McpOAuthTaskTarget {
                        attempt_id: old_attempt_id,
                        status_server_id: status_key.clone(),
                        server: server.clone(),
                    },
                );
                store.set_server_auth_status(
                    status_key.clone(),
                    &server,
                    McpOAuthStatusSnapshot::SigningIn,
                    None,
                );

                let new_attempt_id = store.next_oauth_attempt_id();
                store
                    .oauth_tasks
                    .insert(status_key.clone(), Task::ready(()));
                store.oauth_task_targets.insert(
                    status_key.clone(),
                    McpOAuthTaskTarget {
                        attempt_id: new_attempt_id,
                        status_server_id: status_key.clone(),
                        server: server.clone(),
                    },
                );

                store.finish_oauth_authorization(
                    status_key.clone(),
                    old_attempt_id,
                    Ok(McpOAuthStatusSnapshot::Authorized {
                        scopes: vec!["stale".to_string()],
                        expires_at_unix_ms: Some(1),
                    }),
                    cx,
                );

                assert_eq!(
                    store
                        .oauth_task_targets
                        .get(&status_key)
                        .map(|target| target.attempt_id),
                    Some(new_attempt_id)
                );
                assert!(store.oauth_tasks.contains_key(&status_key));
                assert_eq!(
                    store.auth_status(&status_key),
                    Some(McpOAuthStatusSnapshot::SigningIn)
                );

                store.oauth_task_targets.remove(&status_key);
                store.finish_oauth_authorization(
                    status_key.clone(),
                    new_attempt_id,
                    Ok(McpOAuthStatusSnapshot::Authorized {
                        scopes: vec!["canceled".to_string()],
                        expires_at_unix_ms: Some(2),
                    }),
                    cx,
                );
                assert!(store.oauth_tasks.contains_key(&status_key));
                assert_eq!(
                    store.auth_status(&status_key),
                    Some(McpOAuthStatusSnapshot::SigningIn)
                );
            });
        });
    }

    #[gpui::test]
    fn oauth_credentials_write_result_updates_status(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("config.toml");
        let mut config = JacoConfig::load_from_path_for_test(&path).expect("load test config");
        config
            .mcp_servers
            .insert("server".to_string(), oauth_http_server());
        cx.update(|cx| {
            config::install_for_test(cx, path.clone(), config).expect("install config store");
            let store = cx.new(McpRuntimeStore::new);
            store.update(cx, |store, cx| {
                let identity = McpSessionIdentity {
                    server_id: "server".to_string(),
                    fingerprint: "fingerprint".to_string(),
                    generation: 0,
                };
                store
                    .accepted_sessions
                    .insert("server".to_string(), identity.clone());
                let status = McpOAuthStatusSnapshot::Authorized {
                    scopes: vec!["tools".to_string()],
                    expires_at_unix_ms: Some(123),
                };
                store.finish_oauth_credentials_write(identity.clone(), status.clone(), Ok(()), cx);
                assert_eq!(store.auth_status("server"), Some(status));
                assert!(!store.oauth_credential_write_tasks.contains_key("server"));

                store.finish_oauth_credentials_write(
                    identity,
                    McpOAuthStatusSnapshot::SignedOut,
                    Err("failed to persist OAuth credentials".to_string()),
                    cx,
                );
                assert_eq!(
                    store.auth_status("server"),
                    Some(McpOAuthStatusSnapshot::Failed {
                        message: "failed to persist OAuth credentials".to_string(),
                    })
                );
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

    fn stdio_server(command: &str) -> McpServerTomlConfig {
        McpServerTomlConfig {
            command: Some(command.to_string()),
            ..Default::default()
        }
    }

    fn connected_status_with_tool(
        server_id: &str,
        server: &McpServerTomlConfig,
    ) -> McpServerStatusSnapshot {
        McpServerStatusSnapshot {
            server_id: server_id.to_string(),
            display_name: server.display_name.clone(),
            transport: transport_kind_snapshot(server.transport),
            state: McpServerConnectionState::Connected,
            auth: McpOAuthStatusSnapshot::NotConfigured,
            server_info: None,
            tools: vec![tool_snapshot("tool")],
            last_error: None,
            updated_at_unix_ms: 1,
        }
    }

    fn tool_snapshot(name: &str) -> McpToolSnapshot {
        McpToolSnapshot {
            name: name.to_string(),
            title: None,
            description: None,
            input_schema: serde_json::json!({"type": "object"}),
        }
    }
}

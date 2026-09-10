//! Conversation ownership and source-bound RPC routing; views never own a Pi process.
use super::{
    history::{DisplayMessage, History},
    pi::{self, InstanceId, PiEvent},
};
use crate::foundation::{
    model_scope, paths, persistence,
    session_catalog::{self, Catalog, Discovery, SessionInfo},
};
use gpui_kit::*;
use pi_rpc::{
    Client, ConnectionState, LaunchOptions,
    protocol::{self, Event, Model, Prompt, StreamingBehavior, UiMethod, UiReply},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashSet, VecDeque},
    path::PathBuf,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub(crate) struct PendingUi {
    pub request: protocol::ExtensionRequest,
    pub text: String,
    pub deadline: Option<Instant>,
}
#[derive(Clone)]
pub(crate) struct Widget {
    pub lines: Vec<String>,
    pub below: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Activity {
    Idle,
    Loading,
    Running,
    Failed,
    Waiting,
}
#[derive(Clone)]
pub(crate) struct ToolActivity {
    pub id: String,
    pub name: String,
    pub args: Value,
    pub output: Value,
    pub done: bool,
    pub error: bool,
}
pub(crate) struct Session {
    pub info: SessionInfo,
    pub draft: String,
    pub instance: Option<InstanceId>,
    pub binding: u64,
    pub state: Option<protocol::SessionState>,
    pub history: History,
    pub live: Vec<DisplayMessage>,
    pub active_messages: HashSet<String>,
    pub tools: Vec<ToolActivity>,
    pub models: Vec<Model>,
    pub model_error: Option<String>,
    pub thinking_levels: Vec<String>,
    pub stats: Option<protocol::SessionStats>,
    pub fork_messages: Vec<protocol::ForkMessage>,
    pub pending_ui: VecDeque<PendingUi>,
    pub statuses: BTreeMap<String, String>,
    pub widgets: BTreeMap<String, Widget>,
    pub extension_title: Option<String>,
    pub error: Option<String>,
    pub recovery: Option<String>,
    pub accepted: bool,
    pub running: bool,
    awaiting_settled: bool,
    pub compacting: bool,
    pub retrying: bool,
    pub stopping: bool,
    pub interrupted: bool,
    pub draft_revision: u64,
    pub content_revision: u64,
    pub pending_count: usize,
    inflight_prompts: usize,
    submitted_revision: Option<u64>,
    pub operation: Option<&'static str>,
    pub refresh: Option<Task<()>>,
    refresh_again: bool,
    pending_send: Option<(String, StreamingBehavior, u64)>,
    fork_editor: Option<String>,
}
impl Session {
    fn new(info: SessionInfo, draft: String) -> Self {
        Self {
            info,
            draft,
            instance: None,
            binding: 0,
            state: None,
            history: History::default(),
            live: vec![],
            active_messages: HashSet::new(),
            tools: vec![],
            models: vec![],
            model_error: None,
            thinking_levels: vec![],
            stats: None,
            fork_messages: vec![],
            pending_ui: VecDeque::new(),
            statuses: BTreeMap::new(),
            widgets: BTreeMap::new(),
            extension_title: None,
            error: None,
            recovery: None,
            accepted: false,
            running: false,
            awaiting_settled: false,
            compacting: false,
            retrying: false,
            stopping: false,
            interrupted: false,
            draft_revision: 0,
            content_revision: 0,
            pending_count: 0,
            inflight_prompts: 0,
            submitted_revision: None,
            operation: None,
            refresh: None,
            refresh_again: false,
            pending_send: None,
            fork_editor: None,
        }
    }
    pub fn busy(&self) -> bool {
        self.running
            || self.compacting
            || self.retrying
            || self.stopping
            || self.pending_count > 0
            || self.inflight_prompts > 0
    }
    pub fn activity(&self) -> Activity {
        if !self.pending_ui.is_empty() {
            Activity::Waiting
        } else if self.error.is_some() {
            Activity::Failed
        } else if self.busy() {
            Activity::Running
        } else if self.operation.is_some() || self.instance.is_some() && self.state.is_none() {
            Activity::Loading
        } else {
            Activity::Idle
        }
    }
    pub fn messages(&self, preview: Option<&str>) -> Vec<DisplayMessage> {
        let leaf = preview
            .and_then(|id| self.history.preview_leaf(id))
            .or_else(|| self.history.leaf.clone());
        let mut messages = self.history.messages(leaf.as_deref());
        if preview.is_none_or(|id| self.history.on_current_path(id)) {
            for message in &self.live {
                let signature = message.signature();
                if let Some(existing) = messages.iter_mut().find(|m| m.signature() == signature) {
                    existing.value = message.value.clone();
                } else {
                    messages.push(message.clone());
                }
            }
        }
        messages
    }
}
#[derive(Default, Serialize, Deserialize)]
struct WorkspaceFile {
    #[serde(default)]
    drafts: Vec<DraftFile>,
}
#[derive(Serialize, Deserialize)]
struct DraftFile {
    key: String,
    #[serde(default)]
    session_id: Option<String>,
    cwd: PathBuf,
    path: PathBuf,
    draft: String,
}
struct Snapshot {
    state: protocol::SessionState,
    entries: protocol::Entries,
    models: Vec<Model>,
    model_error: Option<String>,
    thinking: Vec<String>,
    stats: protocol::SessionStats,
    fork_messages: Vec<protocol::ForkMessage>,
}
async fn snapshot(
    client: &Client,
    agent: PathBuf,
    cwd: PathBuf,
) -> Result<Snapshot, pi_rpc::Error> {
    client.ready().await?;
    let state = client.get_state().await?;
    let entries = client.get_entries().await?;
    let models = client.get_available_models().await?.models;
    let (models, model_error) = smol::unblock(move || match model_scope::load(&agent, &cwd) {
        Ok(patterns) => (model_scope::filter(models, patterns.as_deref()), None),
        Err(error) => (vec![], Some(error.to_string())),
    })
    .await;
    let thinking = client.get_available_thinking_levels().await?.levels;
    let stats = client.get_session_stats().await?;
    let fork_messages = client.get_fork_messages().await?.messages;
    Ok(Snapshot {
        state,
        entries,
        models,
        model_error,
        thinking,
        stats,
        fork_messages,
    })
}
pub(crate) enum ConversationEvent {
    Notify { message: String, error: bool },
}
pub(crate) struct ConversationState {
    pub sessions: BTreeMap<String, Session>,
    pub selected: Option<String>,
    pub catalog: Option<Catalog>,
    pub storage_error: Option<String>,
    command: PathBuf,
    discovery: Discovery,
    restore_task: Option<Task<()>>,
    workspace_loaded: bool,
    scan_task: Option<Task<()>>,
    save_task: Option<Task<()>>,
    revision: u64,
    serial: u64,
    draining: bool,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<ConversationEvent> for ConversationState {}
impl ConversationState {
    pub fn new(command: PathBuf, cx: &mut Context<Self>) -> Self {
        let pi = pi::global(cx);
        let subscriptions = vec![
            cx.subscribe(&pi, |this, _, event: &PiEvent, cx| this.on_event(event, cx)),
            cx.observe(&pi, |this, _, cx| this.connections_changed(cx)),
        ];
        let discovery = Discovery::environment().unwrap_or_else(|_| Discovery {
            home: PathBuf::from("."),
            agent: PathBuf::from(".pi/agent"),
            current: PathBuf::from("."),
            session_override: None,
        });
        Self {
            sessions: BTreeMap::new(),
            selected: None,
            catalog: None,
            storage_error: None,
            command,
            discovery,
            restore_task: None,
            workspace_loaded: false,
            scan_task: None,
            save_task: None,
            revision: 0,
            serial: 0,
            draining: false,
            _subscriptions: subscriptions,
        }
    }
    pub fn set_command(&mut self, command: PathBuf) {
        self.command = command;
    }
    pub fn load(&mut self, cx: &mut Context<Self>) {
        // The foreground starts blank; persisted drafts are restored in the
        // background and must never replace this selection or start Pi.
        self.insert_draft(None);
        let task = cx.spawn(async move |owner, cx| {
            let saved = smol::unblock(|| -> Result<WorkspaceFile, String> {
                let path = paths::config_dir()
                    .map_err(|e| e.to_string())?
                    .join("conversations.toml");
                let Some(bytes) = persistence::read(&path).map_err(|e| e.to_string())? else {
                    return Ok(WorkspaceFile::default());
                };
                toml::from_str(std::str::from_utf8(&bytes).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())
            })
            .await;
            let _ = owner.update(cx, |this, cx| {
                match saved {
                    Ok(saved) => {
                        this.restore_drafts(saved);
                        this.workspace_loaded = true;
                    }
                    Err(error) => this.storage_error = Some(error),
                }
                this.restore_task = None;
                this.scan(cx);
                this.changed(cx);
            });
        });
        self.restore_task = Some(task);
        cx.notify();
    }
    fn restore_drafts(&mut self, saved: WorkspaceFile) {
        for draft in saved
            .drafts
            .into_iter()
            .filter(|draft| !draft.draft.is_empty())
        {
            self.sessions.entry(draft.key.clone()).or_insert_with(|| {
                Session::new(
                    SessionInfo {
                        path: draft.path,
                        id: draft.session_id.unwrap_or(draft.key),
                        cwd: draft.cwd,
                        name: None,
                        first_message: String::new(),
                        activity: String::new(),
                        parent_session: None,
                    },
                    draft.draft,
                )
            });
        }
    }
    pub fn scanning(&self) -> bool {
        self.restore_task.is_some() || self.scan_task.is_some()
    }
    pub fn scan(&mut self, cx: &mut Context<Self>) {
        if self.scanning() || self.draining {
            return;
        }
        let options = self.discovery.clone();
        let known = self
            .sessions
            .values()
            .map(|s| s.info.cwd.clone())
            .collect::<Vec<_>>();
        self.scan_task = Some(cx.spawn(async move |owner, cx| {
            let catalog = smol::unblock(move || session_catalog::scan(&options, &known)).await;
            let _ = owner.update(cx, |this, cx| {
                this.apply_catalog(catalog, cx);
            });
        }));
        cx.notify();
    }
    fn apply_catalog(&mut self, catalog: Catalog, cx: &mut Context<Self>) {
        let by_path = catalog
            .sessions
            .iter()
            .map(|info| (info.key(), info))
            .collect::<BTreeMap<_, _>>();
        for session in self.sessions.values_mut() {
            // Locally created conversations keep their draft key after Pi gives
            // them a file path. Match metadata by that path, not by the alias.
            if session.instance.is_none()
                && let Some(info) = by_path.get(&session.info.key())
            {
                session.info = (*info).clone();
            }
        }
        self.catalog = Some(catalog);
        self.scan_task = None;
        // Discovery only updates navigation metadata. Opening a session
        // remains an explicit user action, including after a refresh.
        cx.notify();
    }
    pub fn current(&self) -> Option<&Session> {
        self.selected.as_ref().and_then(|k| self.sessions.get(k))
    }
    pub fn infos(&self) -> Vec<(String, SessionInfo)> {
        let mut infos: BTreeMap<_, _> = self
            .catalog
            .iter()
            .flat_map(|catalog| &catalog.sessions)
            .map(|i| (i.key(), i.clone()))
            .collect();
        // The local draft key stays stable even after Pi creates its session file.
        for (key, s) in &self.sessions {
            if s.info.path.as_os_str().is_empty()
                && s.draft.is_empty()
                && s.history.entries.is_empty()
                && s.instance.is_none()
            {
                continue;
            }
            if !s.info.path.as_os_str().is_empty() {
                infos.remove(&s.info.key());
            }
            infos.insert(key.clone(), s.info.clone());
        }
        let mut result = infos.into_iter().collect::<Vec<_>>();
        result.sort_by(|(ak, a), (bk, b)| b.activity.cmp(&a.activity).then(ak.cmp(bk)));
        result
    }
    pub fn new_draft(&mut self, cwd: Option<PathBuf>, cx: &mut Context<Self>) {
        if self.draining {
            return;
        }
        self.insert_draft(cwd);
        self.changed(cx);
    }
    fn insert_draft(&mut self, cwd: Option<PathBuf>) {
        let cwd = cwd
            .or_else(|| self.current().map(|s| s.info.cwd.clone()))
            .unwrap_or_else(|| self.discovery.current.clone());
        self.serial += 1;
        let key = format!(
            "draft-{}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            self.serial
        );
        self.sessions.insert(
            key.clone(),
            Session::new(
                SessionInfo {
                    path: PathBuf::new(),
                    id: key.clone(),
                    cwd,
                    name: None,
                    first_message: String::new(),
                    activity: String::new(),
                    parent_session: None,
                },
                String::new(),
            ),
        );
        self.selected = Some(key);
    }
    pub fn set_cwd(&mut self, key: &str, cwd: PathBuf, cx: &mut Context<Self>) {
        if self
            .sessions
            .get(key)
            .is_some_and(|s| s.instance.is_none() && s.history.entries.is_empty())
        {
            self.sessions.get_mut(key).unwrap().info.cwd = cwd;
            self.changed(cx);
        } else {
            self.new_draft(Some(cwd), cx);
        }
    }
    pub fn open(&mut self, key: &str, cx: &mut Context<Self>) {
        if self.draining {
            return;
        }
        let existing = self
            .sessions
            .iter()
            .find(|(_, s)| !s.info.path.as_os_str().is_empty() && s.info.key() == key)
            .map(|(k, _)| k.clone());
        let key = existing.unwrap_or_else(|| key.to_owned());
        if !self.sessions.contains_key(&key) {
            if let Some(info) = self
                .catalog
                .iter()
                .flat_map(|catalog| &catalog.sessions)
                .find(|i| i.key() == key)
            {
                self.sessions
                    .insert(key.clone(), Session::new(info.clone(), String::new()));
            } else {
                return;
            }
        }
        self.selected = Some(key.clone());
        if !self.sessions[&key].info.path.as_os_str().is_empty() {
            self.connect(&key, cx);
        }
        self.changed(cx);
    }
    pub fn connect(&mut self, key: &str, cx: &mut Context<Self>) {
        if self.draining {
            return;
        }
        let Some(s) = self.sessions.get_mut(key) else {
            return;
        };
        if s.instance.is_some() || s.operation.is_some() {
            return;
        }
        if !s.info.path.as_os_str().is_empty() {
            let path = s.info.path.clone();
            let expected_id = s.info.id.clone();
            let expected_cwd = s.info.cwd.clone();
            let key = key.to_owned();
            s.operation = Some("opening");
            s.refresh = Some(cx.spawn(async move |owner, cx| {
                let result = smol::unblock(move || session_catalog::read_metadata(&path).map_err(|e| e.to_string())).await;
                let _ = owner.update(cx, |this, cx| {
                    let Some(s) = this.sessions.get_mut(&key) else { return; };
                    s.refresh = None; s.operation = None;
                    match result {
                        Ok(info) if info.cwd == expected_cwd && (info.id == expected_id || expected_id.starts_with("draft-") || expected_id == key) => {
                            s.info = info; this.launch(&key, cx);
                        }
                        Ok(_) => s.error = Some("Session identity or working directory changed; refresh the session catalog.".into()),
                        Err(error) => s.error = Some(error),
                    }
                    cx.notify();
                });
            }));
        } else {
            self.launch(key, cx);
        }
        cx.notify();
    }
    fn launch(&mut self, key: &str, cx: &mut Context<Self>) {
        if self.draining {
            return;
        }
        let Some(s) = self.sessions.get_mut(key) else {
            return;
        };
        let mut options = LaunchOptions::new(self.command.clone(), s.info.cwd.clone());
        if !s.info.path.as_os_str().is_empty() {
            options
                .args
                .extend(["--session".into(), s.info.path.as_os_str().to_owned()]);
        }
        match pi::global(cx).update(cx, |pi, cx| pi.start(options, cx)) {
            Ok(id) => {
                s.instance = Some(id);
                s.binding += 1;
                s.error = None;
                s.state = None;
            }
            Err(error) => s.error = Some(error.to_string()),
        };
        cx.notify();
    }
    fn client(&self, key: &str, cx: &App) -> Option<Client> {
        let id = self.sessions.get(key)?.instance?;
        pi::global(cx).read(cx).client(id).ok().flatten()
    }
    fn connections_changed(&mut self, cx: &mut Context<Self>) {
        let ids = self
            .sessions
            .iter()
            .filter_map(|(k, s)| s.instance.map(|id| (k.clone(), id)))
            .collect::<Vec<_>>();
        for (key, id) in ids {
            let result = pi::global(cx).read(cx).client(id);
            match result {
                Ok(Some(client)) => match client.state() {
                    ConnectionState::Closed(report) => {
                        let s = self.sessions.get_mut(&key).unwrap();
                        s.error = Some(report.reason.map(|e| e.to_string()).unwrap_or_else(|| {
                            if report.stderr.is_empty() {
                                "Pi connection closed".into()
                            } else {
                                report.stderr.clone()
                            }
                        }));
                        s.instance = None;
                        s.running = false;
                        s.awaiting_settled = false;
                        s.compacting = false;
                        s.retrying = false;
                        s.stopping = false;
                        s.pending_ui.clear();
                        s.refresh = None;
                        s.operation = None;
                        s.pending_count = 0;
                        s.inflight_prompts = 0;
                        s.submitted_revision = None;
                        s.binding += 1;
                        pi::global(cx)
                            .update(cx, |pi, cx| pi.close(id, cx))
                            .detach();
                    }
                    _ => {
                        if self.sessions[&key].state.is_none()
                            && self.sessions[&key].refresh.is_none()
                        {
                            self.refresh(&key, cx);
                        }
                    }
                },
                Err(error) => {
                    let s = self.sessions.get_mut(&key).unwrap();
                    s.error = Some(error.to_string());
                    s.instance = None;
                    s.pending_ui.clear();
                    s.operation = None;
                    s.binding += 1;
                }
                Ok(None) => {}
            }
        }
        cx.notify();
    }
    pub fn refresh(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(client) = self.client(key, cx) else {
            return;
        };
        let s = self.sessions.get_mut(key).unwrap();
        if s.refresh.is_some() {
            s.refresh_again = true;
            return;
        }
        let binding = s.binding;
        let event_revision = s.content_revision;
        let cwd = s.info.cwd.clone();
        let agent = self.discovery.agent.clone();
        let task_key = key.to_owned();
        let task = cx.spawn(async move |owner, cx| {
            let key = task_key;
            let result = snapshot(&client, agent, cwd).await;
            let _ = owner.update(cx, |this, cx| {
                if this.sessions.get(&key).is_none_or(|s| s.binding != binding) {
                    return;
                }
                this.sessions.get_mut(&key).unwrap().refresh = None;
                let success = result.is_ok();
                match result {
                    Ok(mut snapshot) => {
                        let s = &this.sessions[&key];
                        if s.content_revision != event_revision {
                            snapshot.state.is_streaming = s.running;
                            snapshot.state.is_compacting = s.compacting;
                        }
                        this.apply_snapshot(&key, snapshot, cx);
                    }
                    Err(error) => {
                        let s = this.sessions.get_mut(&key).unwrap();
                        s.error = Some(error.to_string());
                        s.operation = None;
                    }
                }
                let send = this
                    .sessions
                    .get_mut(&key)
                    .and_then(|s| s.pending_send.take());
                if success && let Some((text, mode, revision)) = send {
                    this.submit(&key, text, mode, revision, cx);
                }
                if this.sessions[&key].refresh_again {
                    this.sessions.get_mut(&key).unwrap().refresh_again = false;
                    this.refresh(&key, cx);
                }
                cx.notify();
            });
        });
        self.sessions.get_mut(key).unwrap().refresh = Some(task);
    }
    fn apply_snapshot(&mut self, key: &str, snapshot: Snapshot, cx: &mut Context<Self>) {
        let s = self.sessions.get_mut(key).unwrap();
        s.info.id = snapshot.state.session_id.clone();
        if let Some(path) = &snapshot.state.session_file {
            s.info.path = PathBuf::from(path);
        }
        s.info.name = snapshot.state.session_name.clone();
        s.awaiting_settled |= snapshot.state.is_streaming;
        s.running = snapshot.state.is_streaming || s.awaiting_settled;
        s.compacting = snapshot.state.is_compacting;
        if !s.running && !s.compacting {
            s.stopping = false;
            s.retrying = false;
        }
        s.history.replace(snapshot.entries);
        let saved = s
            .history
            .messages(s.history.leaf.as_deref())
            .iter()
            .map(DisplayMessage::signature)
            .collect::<std::collections::HashSet<_>>();
        s.live.retain(|m| !saved.contains(&m.signature()));
        if !s.running {
            s.live.clear();
            s.tools.clear();
        }
        for e in &s.history.entries {
            if let Some(m) = e.data.get("message") {
                if m["role"] == "user" && s.info.first_message.is_empty() {
                    s.info.first_message =
                        session_catalog::summary(&session_catalog::text_content(m));
                }
                if matches!(m["role"].as_str(), Some("user" | "assistant"))
                    && e.timestamp > s.info.activity
                {
                    s.info.activity = e.timestamp.clone();
                }
            }
        }
        s.pending_count = snapshot.state.pending_message_count;
        s.state = Some(snapshot.state);
        s.models = snapshot.models;
        s.model_error = snapshot.model_error;
        s.thinking_levels = snapshot.thinking;
        s.stats = Some(snapshot.stats);
        s.fork_messages = snapshot.fork_messages;
        s.operation = None;
        s.content_revision += 1;
        self.changed(cx);
    }
    pub fn set_draft(&mut self, key: &str, text: String, cx: &mut Context<Self>) {
        if let Some(s) = self.sessions.get_mut(key)
            && s.draft != text
        {
            s.draft = text;
            s.draft_revision += 1;
            self.changed(cx);
        }
    }
    pub fn send(&mut self, key: &str, mode: StreamingBehavior, cx: &mut Context<Self>) {
        let Some(s) = self.sessions.get_mut(key) else {
            return;
        };
        if s.draft.trim().is_empty() || !s.pending_ui.is_empty() || s.operation.is_some() {
            return;
        }
        if s.submitted_revision == Some(s.draft_revision) {
            return;
        }
        let text = s.draft.clone();
        let revision = s.draft_revision;
        if self.client(key, cx).is_some() && self.sessions[key].state.is_some() {
            self.submit(key, text, mode, revision, cx);
        } else {
            self.sessions.get_mut(key).unwrap().pending_send = Some((text, mode, revision));
            self.connect(key, cx);
        }
    }
    fn submit(
        &mut self,
        key: &str,
        text: String,
        mode: StreamingBehavior,
        revision: u64,
        cx: &mut Context<Self>,
    ) {
        let Some(client) = self.client(key, cx) else {
            return;
        };
        let s = self.sessions.get_mut(key).unwrap();
        let binding = s.binding;
        s.inflight_prompts += 1;
        s.submitted_revision = Some(revision);
        s.error = None;
        s.accepted = false;
        s.interrupted = false;
        let key = key.to_owned();
        let mut prompt = Prompt::new(text.clone());
        prompt.streaming_behavior = Some(mode);
        cx.spawn(async move |owner, cx| {
            let result = client.prompt(prompt).await;
            let _ = owner.update(cx, |this, cx| {
                let Some(s) = this.sessions.get_mut(&key).filter(|s| s.binding == binding) else {
                    return;
                };
                s.inflight_prompts = s.inflight_prompts.saturating_sub(1);
                if s.submitted_revision == Some(revision) {
                    s.submitted_revision = None;
                }
                match result {
                    Ok(_) => {
                        s.accepted = true;
                        if s.draft_revision == revision {
                            s.draft.clear();
                            s.draft_revision += 1;
                        }
                    }
                    Err(error) => {
                        s.error = Some(error.to_string());
                        if s.draft_revision != revision {
                            s.recovery = Some(text);
                        }
                    }
                }
                this.changed(cx);
                this.refresh(&key, cx);
            });
        })
        .detach();
        cx.notify();
    }
    pub fn abort(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(client) = self.client(key, cx) else {
            return;
        };
        let s = self.sessions.get_mut(key).unwrap();
        let binding = s.binding;
        s.stopping = true;
        s.interrupted = true;
        let key = key.to_owned();
        cx.spawn(async move |owner, cx| {
            let result = client.abort().await;
            let _ = owner.update(cx, |this, cx| {
                if let Some(s) = this.sessions.get_mut(&key).filter(|s| s.binding == binding) {
                    if let Err(error) = result {
                        s.error = Some(error.to_string());
                        s.stopping = false;
                    }
                    this.refresh(&key, cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub fn close(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(s) = self
            .sessions
            .get_mut(key)
            .filter(|s| !s.busy() && s.pending_ui.is_empty())
        else {
            return;
        };
        if let Some(id) = s.instance.take() {
            s.binding += 1;
            s.refresh = None;
            s.state = None;
            s.operation = None;
            pi::global(cx)
                .update(cx, |pi, cx| pi.close(id, cx))
                .detach();
        }
        cx.notify();
    }
    pub fn rename(&mut self, key: &str, name: String, cx: &mut Context<Self>) {
        self.simple_command(key, protocol::Command::SetSessionName { name }, cx);
    }
    pub fn set_model(&mut self, key: &str, model: Model, cx: &mut Context<Self>) {
        if !self.sessions.get(key).is_some_and(|s| {
            s.pending_ui.is_empty()
                && s.models
                    .iter()
                    .any(|m| m.provider == model.provider && m.id == model.id)
        }) {
            return;
        }
        self.simple_command(
            key,
            protocol::Command::SetModel {
                provider: model.provider,
                model_id: model.id,
            },
            cx,
        );
    }
    pub fn set_thinking(&mut self, key: &str, level: String, cx: &mut Context<Self>) {
        if !self
            .sessions
            .get(key)
            .is_some_and(|s| s.pending_ui.is_empty() && s.thinking_levels.contains(&level))
        {
            return;
        }
        self.simple_command(key, protocol::Command::SetThinkingLevel { level }, cx);
    }
    fn simple_command(&mut self, key: &str, command: protocol::Command, cx: &mut Context<Self>) {
        let Some(client) = self.client(key, cx) else {
            self.connect(key, cx);
            return;
        };
        let s = self.sessions.get_mut(key).unwrap();
        if s.busy() || s.operation.is_some() {
            return;
        }
        let binding = s.binding;
        s.operation = Some(command.name());
        let key = key.to_owned();
        cx.spawn(async move |owner, cx| {
            let result = client.request(command).await;
            let _ = owner.update(cx, |this, cx| {
                let Some(s) = this.sessions.get_mut(&key).filter(|s| s.binding == binding) else {
                    return;
                };
                s.operation = None;
                if let Err(error) = result {
                    s.error = Some(error.to_string());
                } else {
                    s.error = None;
                }
                this.refresh(&key, cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub fn fork(&mut self, key: &str, entry: String, cx: &mut Context<Self>) {
        let Some(client) = self.client(key, cx) else {
            return;
        };
        let s = self.sessions.get_mut(key).unwrap();
        if s.busy()
            || s.operation.is_some()
            || !s.pending_ui.is_empty()
            || !s.fork_messages.iter().any(|m| m.entry_id == entry)
        {
            return;
        }
        let cwd = s.info.cwd.clone();
        let agent = self.discovery.agent.clone();
        s.operation = Some("fork");
        s.refresh = None;
        s.binding += 1;
        let binding = s.binding;
        let key = key.to_owned();
        cx.spawn(async move |owner, cx| {
            let result = async {
                let fork = client.fork(entry).await?;
                let snapshot = snapshot(&client, agent, cwd).await?;
                Ok::<_, pi_rpc::Error>((fork, snapshot))
            }
            .await;
            let _ = owner.update(cx, |this, cx| {
                let Some(source) = this.sessions.get_mut(&key).filter(|s| s.binding == binding)
                else {
                    return;
                };
                source.operation = None;
                match result {
                    Ok((fork, snapshot)) if !fork.cancelled => {
                        let instance = source.instance.take();
                        let cwd = source.info.cwd.clone();
                        let origin = source.info.path.to_string_lossy().into_owned();
                        let pending_ui = std::mem::take(&mut source.pending_ui);
                        let statuses = std::mem::take(&mut source.statuses);
                        let widgets = std::mem::take(&mut source.widgets);
                        let extension_title = source.extension_title.take();
                        let editor = source.fork_editor.take().unwrap_or(fork.text);
                        source.state = None;
                        let new_key =
                            snapshot.state.session_file.clone().unwrap_or_else(|| {
                                format!("session-{}", snapshot.state.session_id)
                            });
                        let mut session = Session::new(
                            SessionInfo {
                                path: PathBuf::new(),
                                id: snapshot.state.session_id.clone(),
                                cwd,
                                name: None,
                                first_message: String::new(),
                                activity: String::new(),
                                parent_session: Some(origin),
                            },
                            editor,
                        );
                        session.pending_ui = pending_ui;
                        session.statuses = statuses;
                        session.widgets = widgets;
                        session.extension_title = extension_title;
                        session.instance = instance;
                        session.binding = binding + 1;
                        this.sessions.insert(new_key.clone(), session);
                        this.selected = Some(new_key.clone());
                        this.apply_snapshot(&new_key, snapshot, cx);
                    }
                    Ok((_, snapshot)) => {
                        if let Some(editor) = source.fork_editor.take() {
                            source.draft = editor;
                            source.draft_revision += 1;
                        }
                        this.apply_snapshot(&key, snapshot, cx);
                    }
                    Err(error) => {
                        source.error = Some(error.to_string());
                        if let Some(id) = source.instance.take() {
                            pi::global(cx)
                                .update(cx, |pi, cx| pi.close(id, cx))
                                .detach();
                        }
                        source.state = None;
                        source.pending_ui.clear();
                    }
                }
                this.changed(cx);
            });
        })
        .detach();
        cx.notify();
    }
    pub fn reply(&mut self, key: &str, id: &str, reply: UiReply, cx: &mut Context<Self>) {
        let Some(client) = self.client(key, cx) else {
            return;
        };
        let Some(s) = self.sessions.get_mut(key) else {
            return;
        };
        if !s
            .pending_ui
            .front()
            .is_some_and(|p| p.request.id == id && p.deadline.is_none_or(|d| Instant::now() < d))
        {
            return;
        }
        match client.reply(id, reply) {
            Ok(()) => {
                s.pending_ui.pop_front();
            }
            Err(error) => s.error = Some(error.to_string()),
        };
        cx.notify();
    }
    fn on_event(&mut self, event: &PiEvent, cx: &mut Context<Self>) {
        let Some(key) = self
            .sessions
            .iter()
            .find(|(_, s)| s.instance == Some(event.instance))
            .map(|(k, _)| k.clone())
        else {
            return;
        };
        let s = self.sessions.get_mut(&key).unwrap();
        let mut refresh = false;
        match &event.event {
            Event::Agent { kind, raw } => {
                match kind.as_str() {
                    "agent_start" => {
                        s.tools.clear();
                        s.active_messages.clear();
                        s.running = true;
                        s.awaiting_settled = true;
                        s.error = None;
                        s.interrupted = false;
                        s.accepted = false;
                    }
                    "agent_end" => {
                        refresh = true;
                    }
                    "agent_settled" => {
                        s.awaiting_settled = false;
                        s.running = false;
                        s.stopping = false;
                        s.accepted = false;
                        refresh = true;
                    }
                    "auto_compaction_start" => s.compacting = true,
                    "auto_compaction_end" => {
                        s.compacting = false;
                        refresh = true;
                    }
                    "auto_retry_start" => s.retrying = true,
                    "auto_retry_end" => {
                        s.retrying = false;
                        refresh = true;
                        if raw.get("success") == Some(&Value::Bool(false)) {
                            s.error = raw
                                .get("finalError")
                                .and_then(Value::as_str)
                                .map(str::to_owned);
                        }
                    }
                    "message_start" | "message_update" | "message_end" => {
                        if let Some(message) = raw.get("message") {
                            let candidate = DisplayMessage {
                                id: format!(
                                    "live-{}-{}",
                                    message["role"].as_str().unwrap_or("message"),
                                    message
                                        .get("timestamp")
                                        .map(Value::to_string)
                                        .unwrap_or_default()
                                ),
                                entry: None,
                                value: message.clone(),
                                completed_at: (kind == "message_end").then(|| {
                                    (time::OffsetDateTime::now_utc().unix_timestamp_nanos()
                                        / 1_000_000) as i64
                                }),
                            };
                            let signature = candidate.signature();
                            if s.running {
                                s.active_messages.insert(signature.clone());
                            }
                            if let Some(existing) =
                                s.live.iter_mut().find(|m| m.signature() == signature)
                            {
                                existing.value = message.clone();
                                existing.completed_at =
                                    candidate.completed_at.or(existing.completed_at);
                            } else {
                                s.live.push(candidate);
                            }
                            if message["stopReason"] == "error" {
                                s.error = message
                                    .get("errorMessage")
                                    .and_then(Value::as_str)
                                    .map(str::to_owned);
                            }
                            if message["stopReason"] == "aborted" {
                                s.interrupted = true;
                            }
                        }
                    }
                    "tool_execution_start" | "tool_execution_update" | "tool_execution_end" => {
                        let id = raw["toolCallId"].as_str().unwrap_or_default();
                        if !s.tools.iter().any(|t| t.id == id) {
                            s.tools.push(ToolActivity {
                                id: id.into(),
                                name: raw["toolName"].as_str().unwrap_or_default().into(),
                                args: raw.get("args").cloned().unwrap_or(Value::Null),
                                output: Value::Null,
                                done: false,
                                error: false,
                            });
                        }
                        let tool = s.tools.iter_mut().find(|t| t.id == id).unwrap();
                        if let Some(result) = raw.get("partialResult").or_else(|| raw.get("result"))
                        {
                            tool.output = result.clone();
                        }
                        if kind == "tool_execution_end" {
                            tool.done = true;
                            tool.error = raw["isError"].as_bool().unwrap_or(false);
                        }
                    }
                    _ => {}
                }
                s.content_revision += 1;
            }
            Event::ExtensionUi { request, .. } => match &request.method {
                UiMethod::Select { timeout, .. }
                | UiMethod::Confirm { timeout, .. }
                | UiMethod::Input { timeout, .. } => {
                    let timeout = *timeout;
                    s.pending_ui.push_back(PendingUi {
                        request: request.clone(),
                        text: String::new(),
                        deadline: timeout.map(|ms| Instant::now() + Duration::from_millis(ms)),
                    });
                    if let Some(ms) = timeout {
                        let id = request.id.clone();
                        let instance = event.instance;
                        cx.spawn(async move |owner, cx| {
                            cx.background_executor()
                                .timer(Duration::from_millis(ms))
                                .await;
                            let _ = owner.update(cx, |this, cx| {
                                if let Some(s) = this
                                    .sessions
                                    .values_mut()
                                    .find(|s| s.instance == Some(instance))
                                {
                                    s.pending_ui.retain(|p| p.request.id != id);
                                    cx.notify();
                                }
                            });
                        })
                        .detach();
                    }
                }
                UiMethod::Editor { prefill, .. } => s.pending_ui.push_back(PendingUi {
                    request: request.clone(),
                    text: prefill.clone().unwrap_or_default(),
                    deadline: None,
                }),
                UiMethod::SetEditorText { text } => {
                    if s.operation == Some("fork") {
                        s.fork_editor = Some(text.clone());
                    } else {
                        s.draft = text.clone();
                        s.draft_revision += 1;
                    }
                }
                UiMethod::SetTitle { title } => s.extension_title = Some(title.clone()),
                UiMethod::SetStatus { key, text } => {
                    if let Some(text) = text {
                        s.statuses.insert(key.clone(), text.clone());
                    } else {
                        s.statuses.remove(key);
                    }
                }
                UiMethod::SetWidget {
                    key,
                    lines,
                    placement,
                } => {
                    if let Some(lines) = lines {
                        s.widgets.insert(
                            key.clone(),
                            Widget {
                                lines: lines.clone(),
                                below: placement.as_deref() == Some("belowEditor"),
                            },
                        );
                    } else {
                        s.widgets.remove(key);
                    }
                }
                UiMethod::Notify {
                    message,
                    notify_type,
                } => cx.emit(ConversationEvent::Notify {
                    message: message.clone(),
                    error: notify_type.as_deref() == Some("error"),
                }),
                UiMethod::Unknown => {}
            },
        }
        let save_draft = matches!(
            &event.event,
            Event::ExtensionUi {
                request: protocol::ExtensionRequest {
                    method: UiMethod::SetEditorText { .. },
                    ..
                },
                ..
            }
        );
        if refresh {
            self.refresh(&key, cx);
        }
        if save_draft {
            self.changed(cx);
        } else {
            cx.notify();
        }
    }
    fn file(&self) -> WorkspaceFile {
        WorkspaceFile {
            drafts: self
                .sessions
                .iter()
                .filter(|(_, s)| !s.draft.is_empty())
                .map(|(key, s)| DraftFile {
                    key: key.clone(),
                    session_id: Some(s.info.id.clone()),
                    cwd: s.info.cwd.clone(),
                    path: s.info.path.clone(),
                    draft: s.draft.clone(),
                })
                .collect(),
        }
    }
    fn changed(&mut self, cx: &mut Context<Self>) {
        self.revision += 1;
        cx.notify();
        if !self.workspace_loaded || self.save_task.is_some() || self.draining {
            return;
        }
        self.save_task = Some(cx.spawn(async move |owner, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(250))
                .await;
            loop {
                let Ok((file, revision)) =
                    owner.read_with(cx, |this, _| (this.file(), this.revision))
                else {
                    return;
                };
                let result = smol::unblock(move || save_file(&file)).await;
                let finished = owner
                    .update(cx, |this, cx| {
                        this.storage_error = result.err();
                        let done = this.revision == revision;
                        if done {
                            this.save_task = None;
                        }
                        cx.notify();
                        done
                    })
                    .unwrap_or(true);
                if finished {
                    break;
                }
            }
        }));
    }
    pub fn flush(&mut self, cx: &mut Context<Self>) -> Task<()> {
        self.draining = true;
        let restore = self.restore_task.take();
        let prior = self.save_task.take();
        cx.spawn(async move |owner, cx| {
            if let Some(restore) = restore {
                restore.await;
            }
            if let Some(prior) = prior {
                prior.await;
            }
            let file = owner
                .read_with(cx, |this, _| this.workspace_loaded.then(|| this.file()))
                .ok()
                .flatten();
            // A failed or unfinished read must never erase existing drafts.
            if let Some(file) = file
                && let Err(error) = smol::unblock(move || save_file(&file)).await
            {
                tracing::error!(%error, "conversation state save failed");
            }
        })
    }
}
fn save_file(file: &WorkspaceFile) -> Result<(), String> {
    let path = paths::config_dir()
        .map_err(|e| e.to_string())?
        .join("conversations.toml");
    let bytes = toml::to_string(file).map_err(|e| e.to_string())?;
    persistence::write_atomic(&path, bytes.as_bytes()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::{ConversationState, WorkspaceFile};
    use crate::{foundation::session_catalog::Catalog, state::pi};
    use gpui_kit as gpui;
    use gpui_kit::{AppContext, TestAppContext};
    use std::path::PathBuf;

    #[gpui::test]
    fn startup_ignores_legacy_selection_and_restores_only_nonempty_drafts(cx: &mut TestAppContext) {
        cx.update(pi::init);
        let state = cx.new(|cx| ConversationState::new(PathBuf::from("unused-pi"), cx));
        state.update(cx, |state, cx| {
            state.insert_draft(None);
            let foreground = state.selected.clone().unwrap();
            assert!(state.infos().is_empty());
            assert!(state.file().drafts.is_empty());
            // Typing while the old file is still being read must not overwrite it.
            state.set_draft(&foreground, "new input during loading".into(), cx);
            assert!(state.save_task.is_none());
            let old: WorkspaceFile = toml::from_str(
                r#"
selected = "old-session"
[[drafts]]
key = "old-session"
cwd = "/old-project"
path = "/old-project/session.jsonl"
draft = "keep this draft"
[[drafts]]
key = "old-empty-page"
cwd = "/old-project"
path = ""
draft = ""
"#,
            )
            .unwrap();
            state.restore_drafts(old);
            assert_eq!(state.selected.as_deref(), Some(foreground.as_str()));
            assert_eq!(state.current().unwrap().draft, "new input during loading");
            assert_eq!(state.sessions["old-session"].draft, "keep this draft");
            assert!(!state.sessions.contains_key("old-empty-page"));
            assert!(
                state
                    .sessions
                    .values()
                    .all(|session| session.instance.is_none())
            );
            let saved = toml::to_string(&state.file()).unwrap();
            assert!(!saved.contains("selected ="));
            assert_eq!(state.file().drafts.len(), 2);
        });
    }

    #[gpui::test]
    fn refreshing_catalog_never_opens_a_session_or_changes_selection(cx: &mut TestAppContext) {
        cx.update(pi::init);
        let state = cx.new(|cx| ConversationState::new(PathBuf::from("unused-pi"), cx));
        state.update(cx, |state, cx| {
            state.insert_draft(None);
            let foreground = state.selected.clone();
            state.apply_catalog(Catalog::default(), cx);
            assert_eq!(state.selected, foreground);
            assert!(state.infos().is_empty());
            assert!(state.catalog.is_some());
            state.restore_drafts(
                toml::from_str(
                    r#"
[[drafts]]
key = "history"
cwd = "/old-project"
path = "/old-project/session.jsonl"
draft = "saved input"
"#,
                )
                .unwrap(),
            );
            state.selected = Some("history".into());
            let mut info = state.sessions["history"].info.clone();
            info.name = Some("Restored title".into());
            state.apply_catalog(
                Catalog {
                    sessions: vec![info],
                    ..Default::default()
                },
                cx,
            );
            assert_eq!(state.sessions["history"].info.title(), "Restored title");
            assert_eq!(state.sessions["history"].draft, "saved input");
            assert_eq!(state.infos().len(), 1);
            assert_eq!(state.selected.as_deref(), Some("history"));
            assert!(state.sessions["history"].instance.is_none());
            assert!(state.sessions["history"].operation.is_none());
        });
    }
}

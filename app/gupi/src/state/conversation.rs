//! Conversation ownership and source-bound RPC routing; views never own a Pi process.
pub(crate) mod catalog;
mod command;
pub(crate) mod content;
mod deletion;
pub(crate) mod execution;
pub(crate) mod loading;
mod model_change;
mod reads;
mod renaming;
use super::{
    history::DisplayMessage,
    pi::{self, InstanceId, PiEvent},
};
use crate::foundation::{
    paths, persistence,
    session_catalog::{self, Discovery, SessionInfo},
};
use catalog::{CatalogState, Message as CatalogMessage, ScanWork, Update as CatalogUpdate};
use command::SessionCommand;
use content::Transcript;
use execution::{RunState, ToolExecution};
use gpui_kit::*;
use gpui_operation::Transition;
use loading::{CoreRead, ModelChange, ReadState};
use pi_rpc::{
    Client, ConnectionState, LaunchOptions,
    protocol::{self, Event, Model, Prompt, StreamingBehavior, UiMethod, UiReply},
};
use reads::ThinkingLevels;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashSet, VecDeque},
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
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
    pub execution: ToolExecution,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum ConnectionPurpose {
    Conversation,
    ModelOptions,
}
#[derive(Default)]
enum Submission {
    #[default]
    Idle,
    Connecting {
        text: String,
        mode: StreamingBehavior,
        revision: u64,
    },
    Sending {
        _task: Task<()>,
    },
}
pub(crate) struct Session {
    pub info: SessionInfo,
    pub draft: String,
    pub instance: Option<InstanceId>,
    connection_purpose: ConnectionPurpose,
    pub binding: u64,
    /// Last observed Pi snapshot, also retained after disconnect for display.
    /// Connection availability comes from the instance/client, not this cache.
    pub state: Option<protocol::SessionState>,
    transcript: Transcript,
    pub live: Vec<DisplayMessage>,
    run: RunState,
    pub tools: Vec<ToolActivity>,
    pub models: ReadState<Vec<Model>>,
    pub thinking_levels: ReadState<ThinkingLevels>,
    pub stats: ReadState<protocol::SessionStats>,
    pub fork_messages: ReadState<Vec<protocol::ForkMessage>>,
    pub model_change: ModelChange,
    pub pending_ui: VecDeque<PendingUi>,
    pub statuses: BTreeMap<String, String>,
    pub widgets: BTreeMap<String, Widget>,
    pub extension_title: Option<String>,
    pub error: Option<String>,
    pub compacting: bool,
    pub retrying: bool,
    pub stopping: bool,
    pub interrupted: bool,
    pub draft_revision: u64,
    pub content_revision: u64,
    pub pending_count: usize,
    pub command: SessionCommand,
    pub core_read: CoreRead,
    event_revision: u64,
    model_revision: u64,
    read_serial: u64,
    submission: Submission,
}
impl Session {
    fn new(info: SessionInfo, draft: String) -> Self {
        let transcript = if info.path.as_os_str().is_empty() {
            Transcript::New
        } else {
            Transcript::Unloaded
        };
        Self {
            info,
            draft,
            instance: None,
            connection_purpose: ConnectionPurpose::Conversation,
            binding: 0,
            state: None,
            transcript,
            live: vec![],
            run: RunState::Idle,
            tools: vec![],
            models: ReadState::Idle,
            thinking_levels: ReadState::Idle,
            stats: ReadState::Idle,
            fork_messages: ReadState::Idle,
            model_change: ModelChange::Idle,
            pending_ui: VecDeque::new(),
            statuses: BTreeMap::new(),
            widgets: BTreeMap::new(),
            extension_title: None,
            error: None,
            compacting: false,
            retrying: false,
            stopping: false,
            interrupted: false,
            draft_revision: 0,
            content_revision: 0,
            pending_count: 0,
            command: SessionCommand::Idle,
            core_read: CoreRead::Idle,
            event_revision: 0,
            model_revision: 0,
            read_serial: 0,
            submission: Submission::Idle,
        }
    }
    fn fail_submission(&mut self, error: String, cx: &mut Context<ConversationState>) {
        if self.submitting() {
            self.submission = Submission::Idle;
            cx.emit(ConversationEvent::Notify {
                message: error,
                error: true,
            });
        }
    }
    fn clear_extension_ui(&mut self) {
        self.pending_ui.clear();
        self.extension_title = None;
        self.statuses.clear();
        self.widgets.clear();
    }
    pub fn active_messages(&self) -> Option<&HashSet<String>> {
        self.run.active_messages()
    }
    pub fn running(&self) -> bool {
        self.active_messages().is_some()
    }
    pub fn submitting(&self) -> bool {
        !matches!(self.submission, Submission::Idle)
    }
    pub fn busy(&self) -> bool {
        self.running()
            || self.compacting
            || self.retrying
            || self.stopping
            || self.pending_count > 0
            || self.submitting()
    }
    pub fn activity(&self) -> Activity {
        if !self.pending_ui.is_empty() {
            Activity::Waiting
        } else if self.error.is_some() || self.state.is_none() && self.core_read.error().is_some() {
            Activity::Failed
        } else if self.busy() {
            Activity::Running
        } else if matches!(self.command, SessionCommand::Deleting { .. })
            || matches!(self.core_read, CoreRead::CheckingFile { .. })
            || self.instance.is_some() && self.state.is_none()
        {
            Activity::Loading
        } else {
            Activity::Idle
        }
    }
    pub fn messages(&self, preview: Option<&str>) -> Vec<DisplayMessage> {
        let leaf = preview
            .and_then(|id| self.history().preview_leaf(id))
            .or_else(|| self.history().leaf.clone());
        let mut messages = self.history().messages(leaf.as_deref());
        if preview.is_none_or(|id| self.history().on_current_path(id)) {
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
}
async fn snapshot(client: &Client) -> Result<Snapshot, pi_rpc::Error> {
    client.ready().await?;
    let state = client.get_state().await?;
    let entries = client.get_entries().await?;
    Ok(Snapshot { state, entries })
}
pub(crate) enum ConversationEvent {
    Notify { message: String, error: bool },
    Deleted,
}
pub(crate) struct ConversationState {
    pub sessions: BTreeMap<String, Session>,
    pub selected: Option<String>,
    pub catalog: CatalogState<Task<()>>,
    pub storage_error: Option<String>,
    command: PathBuf,
    discovery: Discovery,
    restore_task: Option<Task<()>>,
    workspace_loaded: bool,
    scan_serial: u64,
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
            catalog: CatalogState::Idle,
            storage_error: None,
            command,
            discovery,
            restore_task: None,
            workspace_loaded: false,
            scan_serial: 0,
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
        self.restore_task.is_some() || self.catalog.running()
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
        self.scan_serial += 1;
        let id = self.scan_serial;
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let task = cx.spawn(async move |owner, cx| {
            let (sender, receiver) = smol::channel::unbounded();
            let scan = smol::unblock(move || {
                session_catalog::scan(&options, &known, &worker_cancel, |value| {
                    let _ = sender.try_send(value);
                })
                .map_err(|error| error.to_string())
            });
            let receive = async {
                while let Ok(value) = receiver.recv().await {
                    if owner
                        .update(cx, |this, cx| {
                            if this
                                .catalog
                                .transition(CatalogMessage::Progress { id, value })
                                == CatalogUpdate::Changed
                            {
                                cx.notify();
                            }
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            };
            let (result, ()) = smol::future::zip(scan, receive).await;
            let _ = owner.update(cx, |this, cx| {
                let success = result.is_ok();
                if let CatalogUpdate::Finished { rescan } = this
                    .catalog
                    .transition(CatalogMessage::Finish { id, result })
                {
                    if success {
                        this.apply_catalog(cx);
                    }
                    if rescan {
                        this.scan(cx);
                    }
                    cx.notify();
                }
            });
        });
        self.catalog
            .transition(CatalogMessage::Start(ScanWork::new(id, task, cancel)));
        cx.notify();
    }
    fn request_scan(&mut self, cx: &mut Context<Self>) {
        if self.catalog.running() {
            self.catalog.transition(CatalogMessage::QueueRefresh);
        } else {
            self.scan(cx);
        }
    }
    fn apply_catalog(&mut self, cx: &mut Context<Self>) {
        let Some(catalog) = self.catalog.data() else {
            return;
        };
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
            .data()
            .into_iter()
            .flat_map(|catalog| &catalog.sessions)
            .map(|i| (i.key(), i.clone()))
            .collect();
        // The local draft key stays stable even after Pi creates its session file.
        for (key, s) in &self.sessions {
            if s.info.path.as_os_str().is_empty()
                && s.draft.is_empty()
                && s.history().entries.is_empty()
                && s.live.is_empty()
                && !s.busy()
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
        if self.draining {
            return;
        }
        let Some(source) = self.sessions.get(key) else {
            return;
        };
        if source.info.cwd == cwd {
            return;
        }
        let can_retarget = source.instance.is_none() && source.history().entries.is_empty();
        // Keep carrying unsent input when changing an unconnected draft's project.
        // Otherwise select the retained empty session, including its model-only connection.
        let reusable = (!can_retarget || source.draft.is_empty())
            .then(|| {
                self.sessions
                    .iter()
                    .filter(|(_, s)| {
                        s.info.cwd == cwd
                            && s.info.path.as_os_str().is_empty()
                            && s.draft.is_empty()
                            && s.history().entries.is_empty()
                            && s.live.is_empty()
                            && !s.busy()
                            && !s.command.running()
                            && !s.core_read.running()
                            && s.pending_ui.is_empty()
                    })
                    .max_by_key(|(_, s)| s.instance.is_some())
                    .map(|(key, _)| key.clone())
            })
            .flatten();
        if let Some(key) = reusable {
            self.selected = Some(key);
        } else if can_retarget {
            self.sessions.get_mut(key).unwrap().info.cwd = cwd;
        } else {
            self.insert_draft(Some(cwd));
        }
        self.changed(cx);
        self.request_scan(cx);
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
                .data()
                .into_iter()
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
        self.connect_for(key, ConnectionPurpose::Conversation, cx);
    }
    fn connect_for(&mut self, key: &str, purpose: ConnectionPurpose, cx: &mut Context<Self>) {
        if self.draining {
            return;
        }
        let Some(s) = self.sessions.get_mut(key) else {
            return;
        };
        // Conversation work can join a connection started for model options.
        // A model read never downgrades a pending conversation open/send.
        if purpose == ConnectionPurpose::Conversation
            || s.instance.is_none() && !s.core_read.running()
        {
            s.connection_purpose = if s.info.path.as_os_str().is_empty() {
                purpose
            } else {
                ConnectionPurpose::Conversation
            };
        }
        if s.instance.is_some() || s.command.running() || s.core_read.running() {
            return;
        }
        if !s.info.path.as_os_str().is_empty() {
            let path = s.info.path.clone();
            let expected_id = s.info.id.clone();
            let expected_cwd = s.info.cwd.clone();
            let key = key.to_owned();
            let binding = s.binding;
            s.error = None;
            s.core_read = CoreRead::CheckingFile { _task: cx.spawn(async move |owner, cx| {
                let result = smol::unblock(move || session_catalog::read_metadata(&path).map_err(|e| e.to_string())).await;
                let _ = owner.update(cx, |this, cx| {
                    let Some(s) = this.sessions.get_mut(&key).filter(|s| s.binding == binding && matches!(s.core_read, CoreRead::CheckingFile { .. })) else { return; };
                    s.core_read.finish(None);
                    match result {
                        Ok(info) if info.cwd == expected_cwd && (info.id == expected_id || expected_id.starts_with("draft-") || expected_id == key) => {
                            s.info = info; this.launch(&key, cx);
                        }
                        Ok(_) => { let error = "Session identity or working directory changed; refresh the session catalog.".to_owned(); s.core_read.finish(Some(error.clone())); s.fail_submission(error, cx); }
                        Err(error) => { s.core_read.finish(Some(error.clone())); s.fail_submission(error, cx); }
                    }
                    cx.notify();
                });
            }) };
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
                s.reset_reads();
                s.instance = Some(id);
                s.binding += 1;
                s.error = None;
                s.state = None;
            }
            Err(error) => {
                s.error = Some(error.to_string());
                s.fail_submission(error.to_string(), cx);
            }
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
                        s.run = RunState::Idle;
                        s.compacting = false;
                        s.retrying = false;
                        s.stopping = false;
                        s.clear_extension_ui();
                        s.reset_reads();
                        s.command.finish();
                        s.pending_count = 0;
                        s.fail_submission(s.error.clone().unwrap(), cx);
                        s.binding += 1;
                        pi::global(cx)
                            .update(cx, |pi, cx| pi.close(id, cx))
                            .detach();
                    }
                    ConnectionState::Ready(initial)
                        if self.sessions[&key].state.is_none()
                            && matches!(self.sessions[&key].core_read, CoreRead::Idle) =>
                    {
                        if self.sessions[&key].connection_purpose == ConnectionPurpose::ModelOptions
                        {
                            // The handshake already supplies the current model.
                            // No history, statistics, fork list or catalog is needed
                            // merely to configure an untouched new conversation.
                            self.sessions.get_mut(&key).unwrap().state = Some(*initial);
                            self.read_models(&key, cx);
                            self.read_thinking(&key, cx);
                        } else {
                            self.refresh(&key, cx);
                        }
                    }
                    _ => {}
                },
                Err(error) => {
                    let s = self.sessions.get_mut(&key).unwrap();
                    s.error = Some(error.to_string());
                    s.fail_submission(error.to_string(), cx);
                    s.instance = None;
                    s.reset_reads();
                    s.clear_extension_ui();
                    s.command.finish();
                    s.binding += 1;
                }
                Ok(None) => {}
            }
        }
        cx.notify();
    }
    pub fn refresh(&mut self, key: &str, cx: &mut Context<Self>) {
        if self.draining {
            return;
        }
        let Some(client) = self.client(key, cx) else {
            return;
        };
        let s = self.sessions.get_mut(key).unwrap();
        if matches!(s.command, SessionCommand::Forking { .. }) {
            return;
        }
        if s.model_change.running() {
            s.model_change.queue_core_refresh();
            return;
        }
        if s.core_read.running() {
            s.core_read.queue();
            return;
        }
        let binding = s.binding;
        let event_revision = s.event_revision;
        let model_revision = s.model_revision;
        let task_key = key.to_owned();
        let task = cx.spawn(async move |owner, cx| {
            let key = task_key;
            let result = snapshot(&client).await;
            let _ = owner.update(cx, |this, cx| {
                if this.sessions.get(&key).is_none_or(|s| s.binding != binding) {
                    return;
                }
                let s = this.sessions.get_mut(&key).unwrap();
                let stale =
                    s.event_revision != event_revision || s.model_revision != model_revision;
                let again = s
                    .core_read
                    .finish(result.as_ref().err().map(ToString::to_string));
                if stale && result.is_ok() {
                    // A later settled event supplies another read while streaming;
                    // otherwise fetch once more now. Never erase newer live data.
                    if matches!(s.submission, Submission::Connecting { .. })
                        || !s.running() && !s.compacting && !s.retrying
                    {
                        this.refresh(&key, cx);
                    }
                    cx.notify();
                    return;
                }
                match result {
                    Ok(snapshot) => {
                        this.apply_snapshot(&key, snapshot, cx);
                        this.submit_pending(&key, cx);
                        if again {
                            this.refresh(&key, cx);
                        }
                    }
                    Err(error) => {
                        let s = this.sessions.get_mut(&key).unwrap();
                        if matches!(s.submission, Submission::Connecting { .. }) {
                            s.fail_submission(error.to_string(), cx);
                        }
                    }
                }
                cx.notify();
            });
        });
        self.sessions.get_mut(key).unwrap().core_read = CoreRead::Reading {
            _task: task,
            again: false,
        };
        cx.notify();
    }
    fn apply_snapshot(&mut self, key: &str, snapshot: Snapshot, cx: &mut Context<Self>) {
        let s = self.sessions.get_mut(key).unwrap();
        let model_changed = s.model_identity()
            != snapshot
                .state
                .model
                .as_ref()
                .map(|model| (model.provider.clone(), model.id.clone()));
        s.info.id = snapshot.state.session_id.clone();
        if let Some(path) = &snapshot.state.session_file {
            s.info.path = PathBuf::from(path);
        }
        s.info.name = snapshot.state.session_name.clone();
        s.run.observe_streaming(snapshot.state.is_streaming);
        s.compacting = snapshot.state.is_compacting;
        if !s.running() && !s.compacting {
            s.stopping = false;
            s.retrying = false;
        }
        s.transcript.replace(snapshot.entries);
        let saved = s
            .history()
            .messages(s.history().leaf.as_deref())
            .iter()
            .map(DisplayMessage::signature)
            .collect::<std::collections::HashSet<_>>();
        s.live.retain(|m| !saved.contains(&m.signature()));
        if !s.running() {
            s.live.clear();
            s.tools.clear();
        }
        for e in &s.transcript.history().entries {
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
        if model_changed {
            s.model_revision += 1;
            s.thinking_levels.reset();
        }
        s.content_revision += 1;
        self.changed(cx);
        self.refresh_auxiliary(key, cx);
    }
    pub fn set_draft(&mut self, key: &str, text: String, cx: &mut Context<Self>) {
        if let Some(s) = self.sessions.get_mut(key)
            && !s.submitting()
            && s.draft != text
        {
            s.draft = text;
            s.draft_revision += 1;
            self.changed(cx);
        }
    }
    pub fn send(&mut self, key: &str, mode: StreamingBehavior, cx: &mut Context<Self>) {
        if self.draining {
            return;
        }
        let Some(s) = self.sessions.get_mut(key) else {
            return;
        };
        if s.draft.trim().is_empty()
            || s.submitting()
            || !s.pending_ui.is_empty()
            || s.command.running()
            || s.model_change.running()
            || s.model_change.unconfirmed()
        {
            return;
        }
        s.submission = Submission::Connecting {
            text: s.draft.clone(),
            mode,
            revision: s.draft_revision,
        };
        s.error = None;
        if self.client(key, cx).is_some() && self.sessions[key].state.is_some() {
            self.submit_pending(key, cx);
        } else {
            self.connect(key, cx);
            // A prior initial snapshot can fail while the process stays connected.
            // Retrying submission must retry that read, not wait for another handshake.
            if self.client(key, cx).is_some() && !self.sessions[key].core_read.running() {
                self.refresh(key, cx);
            }
        }
        cx.notify();
    }
    fn submit_pending(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(client) = self.client(key, cx) else {
            return;
        };
        let s = self.sessions.get_mut(key).unwrap();
        if !matches!(s.submission, Submission::Connecting { .. }) {
            return;
        }
        let Submission::Connecting {
            text,
            mode,
            revision,
        } = std::mem::take(&mut s.submission)
        else {
            unreachable!();
        };
        let binding = s.binding;
        s.interrupted = false;
        let key = key.to_owned();
        let mut prompt = Prompt::new(text);
        prompt.streaming_behavior = Some(mode);
        let task = cx.spawn(async move |owner, cx| {
            let result = client.prompt(prompt).await;
            let _ = owner.update(cx, |this, cx| {
                let Some(s) = this.sessions.get_mut(&key).filter(|s| {
                    s.binding == binding && matches!(s.submission, Submission::Sending { .. })
                }) else {
                    return;
                };
                match result {
                    Ok(_) => {
                        s.submission = Submission::Idle;
                        // An extension can replace the editor while the prompt is pending.
                        // Its new draft belongs to the next submission.
                        if s.draft_revision == revision {
                            s.draft.clear();
                            s.draft_revision += 1;
                        }
                    }
                    Err(error) => {
                        s.error = Some(error.to_string());
                        s.fail_submission(error.to_string(), cx);
                    }
                }
                this.changed(cx);
                this.refresh(&key, cx);
            });
        });
        s.submission = Submission::Sending { _task: task };
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
            .filter(|s| !s.busy() && s.pending_ui.is_empty() && !s.command.running())
        else {
            return;
        };
        if let Some(id) = s.instance.take() {
            s.binding += 1;
            s.reset_reads();
            s.state = None;
            s.clear_extension_ui();
            s.command.finish();
            let closing = pi::global(cx).update(cx, |pi, cx| pi.close(id, cx));
            let key = key.to_owned();
            let task = cx.spawn(async move |owner, cx| {
                closing.await;
                let _ = owner.update(cx, |this, cx| {
                    if let Some(s) = this.sessions.get_mut(&key) {
                        s.command.finish();
                    }
                    cx.notify();
                });
            });
            s.command = SessionCommand::Closing { _task: task };
        }
        cx.notify();
    }
    pub fn set_model(&mut self, key: &str, model: Model, cx: &mut Context<Self>) {
        if !self.sessions.get(key).is_some_and(|s| {
            s.pending_ui.is_empty()
                && !s.models.running()
                && s.models.error().is_none()
                && !s.model_change.unconfirmed()
                && s.model_options()
                    .iter()
                    .any(|m| m.provider == model.provider && m.id == model.id)
        }) {
            return;
        }
        self.change_model_setting(
            key,
            Some(protocol::Command::SetModel {
                provider: model.provider,
                model_id: model.id,
            }),
            cx,
        );
    }
    pub fn set_thinking(&mut self, key: &str, level: String, cx: &mut Context<Self>) {
        if !self.sessions.get(key).is_some_and(|s| {
            s.pending_ui.is_empty()
                && !s.thinking_levels.running()
                && s.thinking_levels.error().is_none()
                && !s.model_change.unconfirmed()
                && s.levels().contains(&level)
        }) {
            return;
        }
        self.change_model_setting(key, Some(protocol::Command::SetThinkingLevel { level }), cx);
    }
    pub fn fork(&mut self, key: &str, entry: String, cx: &mut Context<Self>) {
        let Some(client) = self.client(key, cx) else {
            return;
        };
        let s = self.sessions.get_mut(key).unwrap();
        if s.busy()
            || s.command.running()
            || s.model_change.running()
            || s.model_change.unconfirmed()
            || !s.pending_ui.is_empty()
            || !s.fork_options().iter().any(|m| m.entry_id == entry)
        {
            return;
        }
        s.reset_reads();
        s.binding += 1;
        let binding = s.binding;
        let target = key.to_owned();
        let task_key = target.clone();
        let task = cx.spawn(async move |owner, cx| {
            let key = task_key;
            let result = client.fork(entry).await;
            let _ = owner.update(cx, |this, cx| {
                let Some(source) = this.sessions.get_mut(&key).filter(|s| s.binding == binding)
                else {
                    return;
                };
                source.command.finish();
                match result {
                    Ok(fork) if !fork.cancelled => {
                        let instance = source.instance.take();
                        let cwd = source.info.cwd.clone();
                        let origin = source.info.path.to_string_lossy().into_owned();
                        let pending_ui = std::mem::take(&mut source.pending_ui);
                        let statuses = std::mem::take(&mut source.statuses);
                        let widgets = std::mem::take(&mut source.widgets);
                        let extension_title = source.extension_title.take();
                        source.state = None;
                        // Like Pi TUI, a successful fork replaces the editor with
                        // the selected message. Loading its history is independent.
                        this.insert_draft(Some(cwd));
                        let new_key = this.selected.clone().unwrap();
                        let session = this.sessions.get_mut(&new_key).unwrap();
                        session.info.parent_session = Some(origin);
                        session.draft = fork.text;
                        session.transcript = Transcript::Unloaded;
                        session.pending_ui = pending_ui;
                        session.statuses = statuses;
                        session.widgets = widgets;
                        session.extension_title = extension_title;
                        session.instance = instance;
                        session.binding = binding + 1;
                        this.refresh(&new_key, cx);
                        this.request_scan(cx);
                    }
                    Ok(_) => {
                        this.refresh(&key, cx);
                    }
                    Err(error) => {
                        source.error = Some(error.to_string());
                        this.refresh_auxiliary(&key, cx);
                    }
                }
                this.changed(cx);
            });
        });
        self.sessions.get_mut(&target).unwrap().command = SessionCommand::Forking { _task: task };
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
        let mut scan = false;
        match &event.event {
            Event::Agent { kind, raw } => {
                match kind.as_str() {
                    "agent_start" => {
                        s.tools.clear();
                        s.run.start();
                        s.error = None;
                        s.interrupted = false;
                    }
                    "agent_end" => {
                        refresh = true;
                    }
                    "agent_settled" => {
                        s.run = RunState::Idle;
                        s.stopping = false;
                        refresh = true;
                        scan = true;
                    }
                    "compaction_start" => s.compacting = true,
                    "compaction_end" => {
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
                            s.transcript.receive_message();
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
                            s.run.record_message(signature.clone());
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
                                execution: ToolExecution::Running(Value::Null),
                            });
                        }
                        let tool = s.tools.iter_mut().find(|t| t.id == id).unwrap();
                        let output = raw.get("partialResult").or_else(|| raw.get("result"));
                        let previous = match &tool.execution {
                            ToolExecution::Running(value)
                            | ToolExecution::Complete(value)
                            | ToolExecution::Failed(value) => value,
                        };
                        let output = output.unwrap_or(previous).clone();
                        tool.execution = match kind.as_str() {
                            "tool_execution_end" if raw["isError"].as_bool().unwrap_or(false) => {
                                ToolExecution::Failed(output)
                            }
                            "tool_execution_end" => ToolExecution::Complete(output),
                            _ => match &tool.execution {
                                ToolExecution::Running(_) => ToolExecution::Running(output),
                                ToolExecution::Complete(_) => ToolExecution::Complete(output),
                                ToolExecution::Failed(_) => ToolExecution::Failed(output),
                            },
                        };
                    }
                    _ => {}
                }
                s.content_revision += 1;
                s.event_revision += 1;
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
                    s.draft = text.clone();
                    s.draft_revision += 1;
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
        if scan {
            self.request_scan(cx);
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
        self.catalog.transition(CatalogMessage::Cancel);
        let mut deletions = Vec::new();
        for s in self.sessions.values_mut() {
            s.reset_reads();
            s.submission = Submission::Idle;
            if matches!(s.command, SessionCommand::Deleting { .. })
                && let SessionCommand::Deleting { task } = std::mem::take(&mut s.command)
            {
                deletions.push(task);
            }
        }
        let restore = self.restore_task.take();
        let prior = self.save_task.take();
        cx.spawn(async move |owner, cx| {
            for deletion in deletions {
                deletion.await;
            }
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
    mod loading;
    use super::{ConversationState, WorkspaceFile, catalog::CatalogState};
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
            state.catalog = CatalogState::Ready(Catalog::default());
            state.apply_catalog(cx);
            assert_eq!(state.selected, foreground);
            assert!(state.infos().is_empty());
            assert!(state.catalog.data().is_some());
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
            state.catalog = CatalogState::Ready(Catalog {
                sessions: vec![info],
                ..Default::default()
            });
            state.apply_catalog(cx);
            assert_eq!(state.sessions["history"].info.title(), "Restored title");
            assert_eq!(state.sessions["history"].draft, "saved input");
            assert_eq!(state.infos().len(), 1);
            assert_eq!(state.selected.as_deref(), Some("history"));
            assert!(state.sessions["history"].instance.is_none());
            assert!(!state.sessions["history"].command.running());
        });
    }
}

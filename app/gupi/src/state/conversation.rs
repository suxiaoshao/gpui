//! Conversation ownership and source-bound RPC routing; views never own a Pi process.
pub(crate) mod catalog;
mod command;
mod compaction;
pub(crate) mod content;
mod deletion;
pub(crate) mod execution;
mod export;
pub(crate) mod loading;
mod messages;
mod model_change;
mod queue;
mod reads;
mod reconnect;
mod renaming;
pub(crate) mod temporary;
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
use loading::{CoreRead, ModelChange, ReadScope, ReadState};
use pi_rpc::{
    Client, ConnectionState, LaunchOptions,
    protocol::{self, Event, Model, Prompt, StreamingBehavior, UiMethod, UiReply},
};
use reads::ThinkingLevels;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet, HashSet, VecDeque},
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub(crate) struct PendingUi {
    pub request: protocol::ExtensionRequest,
    pub text: String,
    pub deadline: Option<Instant>,
}
#[derive(Clone, PartialEq)]
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
    Sending {
        _task: Task<()>,
        result: tokio::sync::oneshot::Sender<bool>,
    },
}
pub(crate) struct Session {
    pub info: SessionInfo,
    pub draft: String,
    pub preparing: bool,
    pub attachments: Vec<crate::foundation::attachments::Attachment>,
    pub attachments_read: Option<Task<()>>,
    pub pending_template: Option<super::shortcuts::PendingTemplate>,
    pub instance: Option<InstanceId>,
    connection_purpose: ConnectionPurpose,
    pub binding: u64,
    /// Last observed Pi snapshot, also retained after disconnect for display.
    /// Connection availability comes from the instance/client, not this cache.
    pub state: Option<protocol::SessionState>,
    transcript: Transcript,
    pub live: Vec<DisplayMessage>,
    message_stream: Option<messages::MessageStream>,
    run: RunState,
    pub tools: Vec<ToolActivity>,
    pub models: ReadState<Vec<Model>>,
    pub commands: ReadState<Vec<protocol::SlashCommand>>,
    pub thinking_levels: ReadState<ThinkingLevels>,
    pub stats: ReadState<protocol::SessionStats>,
    pub fork_messages: ReadState<Vec<protocol::ForkMessage>>,
    pub model_change: ModelChange,
    pub pending_ui: VecDeque<PendingUi>,
    pub unread: bool,
    pub notices: Vec<super::notifications::NoticeContent>,
    pub statuses: BTreeMap<String, String>,
    pub widgets: BTreeMap<String, Widget>,
    pub extension_title: Option<String>,
    pub error: Option<String>,
    pub compacting: bool,
    pub retrying: bool,
    pub retry: Option<execution::RetryProgress>,
    pub summary_retry: Option<execution::RetryProgress>,
    pub history_dirty: bool,
    settings_event_revision: u64,
    usage_revision: u64,
    pub stopping: bool,
    pub interrupted: bool,
    pub draft_revision: u64,
    pub content_revision: u64,
    pub pending_count: usize,
    pub queued: Option<protocol::ClearedQueue>,
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
            preparing: false,
            attachments: Vec::new(),
            attachments_read: None,
            pending_template: None,
            instance: None,
            connection_purpose: ConnectionPurpose::Conversation,
            binding: 0,
            state: None,
            transcript,
            live: vec![],
            message_stream: None,
            run: RunState::Idle,
            tools: vec![],
            models: ReadState::Idle,
            commands: ReadState::Idle,
            thinking_levels: ReadState::Idle,
            stats: ReadState::Idle,
            fork_messages: ReadState::Idle,
            model_change: ModelChange::Idle,
            pending_ui: VecDeque::new(),
            unread: false,
            notices: Vec::new(),
            statuses: BTreeMap::new(),
            widgets: BTreeMap::new(),
            extension_title: None,
            error: None,
            compacting: false,
            retrying: false,
            retry: None,
            summary_retry: None,
            history_dirty: false,
            settings_event_revision: 0,
            usage_revision: 0,
            stopping: false,
            interrupted: false,
            draft_revision: 0,
            content_revision: 0,
            pending_count: 0,
            queued: None,
            command: SessionCommand::Idle,
            core_read: CoreRead::Idle,
            event_revision: 0,
            model_revision: 0,
            read_serial: 0,
            submission: Submission::Idle,
        }
    }
    fn finish_submission(&mut self, accepted: bool) {
        match std::mem::take(&mut self.submission) {
            Submission::Sending { result, .. } => {
                let _ = result.send(accepted);
            }
            Submission::Idle => {}
        }
    }
    fn fail_submission(&mut self, error: String, cx: &mut Context<ConversationState>) {
        if self.submitting() {
            self.finish_submission(false);
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
    pub fn run_started_at(&self) -> Option<i64> {
        self.run.started_at()
    }
    pub fn running(&self) -> bool {
        self.active_messages().is_some()
    }
    pub fn submitting(&self) -> bool {
        !matches!(self.submission, Submission::Idle)
    }
    pub fn can_edit_draft(&self) -> bool {
        !self.preparing
            && !self.submitting()
            && (!self.command.running() || self.command.clearing_queue())
            && self.state.is_some()
            && !matches!(self.core_read, CoreRead::CheckingFile { .. })
    }
    pub fn busy(&self) -> bool {
        self.preparing
            || self.running()
            || self.compacting
            || self.command.compacting()
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
    pub fn composer_empty(&self) -> bool {
        self.draft.trim().is_empty()
            && self.attachments.is_empty()
            && self.attachments_read.is_none()
    }
    /// Only a successful final response may be sent back to another application.
    pub fn completed_answer(&self) -> Option<String> {
        if self.busy() || self.interrupted || !self.pending_ui.is_empty() {
            return None;
        }
        self.final_answer_text()
    }
    // The final-state event decides completion. A prompt response may still be
    // awaiting UI processing and must not gate the event's notification.
    fn final_answer_text(&self) -> Option<String> {
        let messages = self.messages(None);
        let last = messages
            .iter()
            .rev()
            .find(|m| matches!(m.role(), "user" | "assistant"))?;
        if last.role() != "assistant" || last.value["stopReason"] != "stop" {
            return None;
        }
        let text = if let Some(start) = last.final_part() {
            last.value["content"]
                .as_array()?
                .iter()
                .skip(start)
                .filter(|part| part["type"] == "text")
                .filter_map(|part| part["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            last.text()
        };
        (!text.trim().is_empty()).then_some(text)
    }
    pub fn last_assistant_text(&self) -> Option<String> {
        self.messages(None)
            .into_iter()
            .rev()
            .find(|m| {
                m.role() == "assistant"
                    && !(m.value["stopReason"] == "aborted" && m.text().is_empty())
            })
            .map(|m| m.text())
            .filter(|text| !text.is_empty())
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
                    existing.final_answer_part =
                        message.final_answer_part.or(existing.final_answer_part);
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
    entries: Option<protocol::Entries>,
}
async fn snapshot(client: &Client, scope: ReadScope) -> Result<Snapshot, pi_rpc::Error> {
    client.ready().await?;
    let state = client.get_state().await?;
    let entries = if scope >= ReadScope::History {
        Some(client.get_entries().await?)
    } else {
        None
    };
    Ok(Snapshot { state, entries })
}
/// A batch of invalidations, delivered after the current GPUI update.
#[derive(Default)]
pub(crate) struct Changes {
    pub catalog: bool,
    pub progress: bool,
    pub selection: bool,
    pub bodies: HashSet<String>,
    /// true means navigation metadata may have changed; false is controls only.
    pub sessions: BTreeMap<String, bool>,
}
impl Changes {
    pub fn affects(&self, key: Option<&String>) -> bool {
        self.selection || key.is_some_and(|key| self.sessions.contains_key(key))
    }
}
fn publish(change: impl FnOnce(&mut Changes) + 'static, cx: &mut Context<ConversationState>) {
    let owner = cx.weak_entity();
    cx.defer(move |cx| {
        let _ = owner.update(cx, |state, cx| {
            let schedule = state.pending_changes.is_none();
            change(state.pending_changes.get_or_insert_with(Changes::default));
            if schedule {
                let owner = cx.weak_entity();
                cx.defer(move |cx| {
                    let _ = owner.update(cx, |state, cx| {
                        if let Some(changes) = state.pending_changes.take() {
                            cx.emit(ConversationEvent::Changed(changes));
                            cx.notify();
                        }
                    });
                });
            }
        });
    });
}
pub(crate) fn notify(cx: &mut Context<ConversationState>) {
    publish(|changes| changes.catalog = true, cx);
}
pub(crate) fn notify_session(key: &str, cx: &mut Context<ConversationState>) {
    let key = key.to_owned();
    publish(
        move |changes| {
            changes.sessions.insert(key, true);
        },
        cx,
    );
}
pub(crate) fn notify_controls(key: &str, cx: &mut Context<ConversationState>) {
    let key = key.to_owned();
    publish(
        move |changes| {
            changes.sessions.entry(key).or_insert(false);
        },
        cx,
    );
}
fn notify_body(key: &str, cx: &mut Context<ConversationState>) {
    let key = key.to_owned();
    publish(
        move |changes| {
            changes.bodies.insert(key);
        },
        cx,
    );
}
fn notify_selection(cx: &mut Context<ConversationState>) {
    publish(|changes| changes.selection = true, cx);
}
fn notify_progress(cx: &mut Context<ConversationState>) {
    publish(|changes| changes.progress = true, cx);
}
pub(crate) enum ConversationEvent {
    Changed(Changes),
    Attention(super::notifications::Notice),
    Notify { message: String, error: bool },
}
pub(crate) struct ConversationState {
    pub sessions: BTreeMap<String, Session>,
    pub selected: Option<String>,
    pub catalog: CatalogState<Task<()>>,
    pub storage_error: Option<String>,
    pub temporary: bool,
    command: PathBuf,
    discovery: Discovery,
    restore_task: Option<Task<()>>,
    workspace_loaded: bool,
    scan_serial: u64,
    discovered_projects: BTreeSet<PathBuf>,
    pending_projects: BTreeSet<PathBuf>,
    pending_changes: Option<Changes>,
    save_task: Option<Task<()>>,
    revision: u64,
    serial: u64,
    draining: bool,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<ConversationEvent> for ConversationState {}
impl ConversationState {
    pub fn new(command: PathBuf, cx: &mut Context<Self>) -> Self {
        let owner = cx.entity().downgrade();
        cx.defer(move |cx| {
            if let Some(owner) = owner.upgrade() {
                crate::app::notifications::attach(&owner, cx);
            }
        });
        let pi = pi::global(cx);
        let subscriptions = vec![
            cx.subscribe(&pi, |this, _, event: &PiEvent, cx| this.on_event(event, cx)),
            cx.subscribe(&pi, |this, _, event: &pi::ConnectionChanged, cx| {
                this.connection_changed(event.0, cx)
            }),
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
            temporary: false,
            command,
            discovery,
            restore_task: None,
            workspace_loaded: false,
            scan_serial: 0,
            discovered_projects: BTreeSet::new(),
            pending_projects: BTreeSet::new(),
            pending_changes: None,
            save_task: None,
            revision: 0,
            serial: 0,
            draining: false,
            _subscriptions: subscriptions,
        }
    }
    pub fn mark_read(&mut self, key: &str, cx: &mut Context<Self>) {
        if let Some(session) = self.sessions.get_mut(key)
            && session.unread
        {
            session.unread = false;
            notify_session(key, cx);
        }
    }
    pub fn clear_notices(&mut self, key: &str, cx: &mut Context<Self>) {
        if let Some(session) = self.sessions.get_mut(key)
            && !session.notices.is_empty()
        {
            session.notices.clear();
            notify_controls(key, cx);
        }
    }
    pub fn set_command(&mut self, command: PathBuf) {
        self.command = command;
    }
    pub fn load(&mut self, cx: &mut Context<Self>) {
        if self.temporary {
            self.new_draft(None, cx);
            return;
        }
        // The foreground starts blank; persisted drafts are restored in the
        // background and must never replace this selection or start background sessions.
        self.insert_draft(None);
        self.connect_selected(cx);
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
                notify(cx);
            });
        });
        self.restore_task = Some(task);
        notify(cx);
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
        if self.catalog.running() {
            self.catalog.transition(CatalogMessage::QueueRefresh);
            return;
        }
        self.scan_scope(None, cx);
    }
    fn discover_project(&mut self, cwd: PathBuf, cx: &mut Context<Self>) {
        if self.discovered_projects.contains(&cwd) {
            return;
        }
        if self.scanning() {
            self.pending_projects.insert(cwd);
        } else {
            self.scan_scope(Some(cwd), cx);
        }
    }
    fn discover_pending_projects(&mut self, cx: &mut Context<Self>) {
        if self.scanning() {
            return;
        }
        while let Some(project) = self.pending_projects.pop_first() {
            if !self.discovered_projects.contains(&project) {
                self.discover_project(project, cx);
                break;
            }
        }
    }
    fn scan_scope(&mut self, project: Option<PathBuf>, cx: &mut Context<Self>) {
        if self.temporary || self.scanning() || self.draining {
            return;
        }
        let before: BTreeMap<_, _> = self
            .sessions
            .iter()
            .map(|(key, s)| (key.clone(), s.info.clone()))
            .collect();
        let options = self.discovery.clone();
        let known = self
            .sessions
            .values()
            .map(|s| s.info.cwd.clone())
            .collect::<Vec<_>>();
        let projects: BTreeSet<_> = project
            .iter()
            .cloned()
            .chain(
                project
                    .is_none()
                    .then_some(known.clone())
                    .into_iter()
                    .flatten(),
            )
            .collect();
        let worker_project = project.clone();
        self.scan_serial += 1;
        let id = self.scan_serial;
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let task = cx.spawn(async move |owner, cx| {
            let (sender, receiver) = smol::channel::unbounded();
            let scan = smol::unblock(move || {
                let report = |value| {
                    let _ = sender.try_send(value);
                };
                match worker_project {
                    Some(cwd) => {
                        session_catalog::scan_project(&options, &cwd, &worker_cancel, report)
                    }
                    None => session_catalog::scan(&options, &known, &worker_cancel, report),
                }
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
                                notify_progress(cx);
                            }
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            };
            let (mut result, ()) = smol::future::zip(scan, receive).await;
            let _ = owner.update(cx, |this, cx| {
                if let Ok(catalog) = &mut result {
                    for (key, session) in &this.sessions {
                        if before.get(key) != Some(&session.info)
                            && !session.info.path.as_os_str().is_empty()
                            && let Some(info) = catalog
                                .sessions
                                .iter_mut()
                                .find(|info| info.path == session.info.path)
                        {
                            *info = session.info.clone();
                        }
                    }
                }
                if let Ok(catalog) = &mut result
                    && let Some(project) = &project
                    && let Some(previous) = this.catalog.data()
                {
                    catalog.sessions.extend(
                        previous
                            .sessions
                            .iter()
                            .filter(|s| &s.cwd != project)
                            .cloned(),
                    );
                    catalog
                        .directories
                        .extend(previous.directories.iter().cloned());
                }
                let success = result.is_ok();
                if let CatalogUpdate::Finished { rescan } = this
                    .catalog
                    .transition(CatalogMessage::Finish { id, result })
                {
                    if success {
                        this.discovered_projects.extend(projects);
                        if let Some(catalog) = this.catalog.data() {
                            this.discovered_projects
                                .extend(catalog.sessions.iter().map(|s| s.cwd.clone()));
                        }
                        this.apply_catalog(cx);
                    }
                    if rescan {
                        this.scan(cx);
                    } else {
                        this.discover_pending_projects(cx);
                    }
                    notify_progress(cx);
                }
            });
        });
        self.catalog
            .transition(CatalogMessage::Start(ScanWork::new(id, task, cancel)));
        notify_progress(cx);
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
        notify(cx);
    }
    pub fn draining(&self) -> bool {
        self.draining
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
            if !s.info.path.as_os_str().is_empty() {
                infos.remove(&s.info.key());
            }
            infos.insert(key.clone(), s.info.clone());
        }
        let mut result = infos.into_iter().collect::<Vec<_>>();
        if self.temporary {
            result.sort_by(|(ak, _), (bk, _)| ak.cmp(bk));
        } else {
            result.sort_by(|(ak, a), (bk, b)| b.activity.cmp(&a.activity).then(ak.cmp(bk)));
        }
        result
    }
    /// User-requested new pages reuse an unsent ordinary draft. Template tasks
    /// deliberately keep calling `new_draft` to obtain an independent instance.
    pub fn new_or_reuse(&mut self, cwd: Option<PathBuf>, cx: &mut Context<Self>) {
        if self.draining {
            return;
        }
        let cwd = cwd
            .or_else(|| self.current().map(|s| s.info.cwd.clone()))
            .unwrap_or_else(|| self.discovery.current.clone());
        let reusable = |s: &Session| {
            (self.temporary || s.info.cwd == cwd)
                && !matches!(s.transcript, Transcript::Unloaded)
                && !s.busy()
                && !s.command.running()
                // Temporary in-memory instances cannot recover after exiting.
                && (!self.temporary
                    || (s.error.is_none()
                        && s.core_read.error().is_none()
                        && (s.binding == 0 || s.instance.is_some())))
                && s.pending_template.is_none()
                && s.pending_ui.is_empty()
                && s.info.first_message.is_empty()
                && s.empty_conversation()
        };
        let key = self
            .selected
            .as_ref()
            .filter(|key| self.sessions.get(*key).is_some_and(reusable))
            .cloned()
            .or_else(|| {
                self.sessions
                    .iter()
                    .find(|(_, s)| reusable(s))
                    .map(|(key, _)| key.clone())
            });
        if let Some(key) = key {
            self.open(&key, cx);
        } else {
            self.new_draft(Some(cwd), cx);
        }
    }

    pub fn new_draft(&mut self, cwd: Option<PathBuf>, cx: &mut Context<Self>) {
        if self.draining {
            return;
        }
        self.insert_draft(cwd);
        self.connect_selected(cx);
        notify_session(self.selected.as_ref().unwrap(), cx);
        notify_selection(cx);
    }
    fn connect_selected(&mut self, cx: &mut Context<Self>) {
        if let Some(key) = self.selected.clone() {
            self.connect(&key, cx);
        }
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
        let cwd = if self.temporary {
            match paths::temporary_dir() {
                Ok(root) => root.join(&key),
                Err(error) => {
                    self.storage_error = Some(error.to_string());
                    return;
                }
            }
        } else {
            cwd
        };
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
        if self.temporary {
            return;
        }
        if self.draining {
            return;
        }
        let Some(source) = self.sessions.get(key) else {
            return;
        };
        if source.info.cwd == cwd {
            return;
        }
        let project = cwd.clone();
        let had_draft = !source.draft.is_empty();
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
                            && s.empty_conversation()
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
        self.connect_selected(cx);
        notify_session(key, cx);
        notify_session(self.selected.as_ref().unwrap(), cx);
        notify_selection(cx);
        if had_draft && can_retarget {
            self.save_changes(cx);
        }
        self.discover_project(project, cx);
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
        self.select_existing(&key, cx);
        self.connect(&key, cx);
    }
    /// Navigate to a retained session without starting or reconnecting Pi.
    pub(crate) fn select_existing(&mut self, key: &str, cx: &mut Context<Self>) {
        if self.draining || !self.sessions.contains_key(key) {
            return;
        }
        if self.selected.as_deref() != Some(key) {
            self.selected = Some(key.to_owned());
            notify_selection(cx);
        }
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
        // An exited in-memory instance cannot resume its lost model context.
        if self.temporary && s.binding > 0 && s.instance.is_none() {
            return;
        }
        // Conversation work can join a connection started for model options.
        // A model read never downgrades a pending conversation open.
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
        if self.temporary {
            let directory = s.info.cwd.clone();
            let key = key.to_owned();
            let binding = s.binding;
            s.core_read = CoreRead::CheckingFile {
                _task: cx.spawn(async move |owner, cx| {
                    let result = smol::unblock(move || std::fs::create_dir_all(directory)).await;
                    let _ = owner.update(cx, |this, cx| {
                        let Some(s) = this.sessions.get_mut(&key).filter(|s| s.binding == binding)
                        else {
                            return;
                        };
                        s.core_read
                            .finish(result.as_ref().err().map(ToString::to_string));
                        if result.is_ok() {
                            this.launch(&key, cx);
                        }
                        notify_session(&key, cx);
                    });
                }),
            };
        } else if !s.info.path.as_os_str().is_empty() {
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
                    notify_session(&key, cx);
                });
            }) };
        } else {
            self.launch(key, cx);
        }
        notify_session(key, cx);
    }
    fn launch(&mut self, key: &str, cx: &mut Context<Self>) {
        if self.draining {
            return;
        }
        let Some(s) = self.sessions.get_mut(key) else {
            return;
        };
        let mut options = LaunchOptions::new(self.command.clone(), s.info.cwd.clone());
        if self.temporary {
            options.args.push("--no-session".into());
        }
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
                s.queued = None;
                s.pending_count = 0;
            }
            Err(error) => {
                s.error = Some(error.to_string());
                s.fail_submission(error.to_string(), cx);
            }
        };
        notify_session(key, cx);
    }
    pub(crate) fn client(&self, key: &str, cx: &App) -> Option<Client> {
        let id = self.sessions.get(key)?.instance?;
        pi::global(cx).read(cx).client(id).ok().flatten()
    }
    fn connection_changed(&mut self, instance: InstanceId, cx: &mut Context<Self>) {
        let ids = self
            .sessions
            .iter()
            .filter_map(|(k, s)| {
                s.instance
                    .filter(|id| *id == instance)
                    .map(|id| (k.clone(), id))
            })
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
                        s.queued = None;
                        s.finish_submission(false);
                        s.binding += 1;
                        cx.emit(ConversationEvent::Attention(super::notifications::Notice {
                            key: key.clone(),
                            binding: s.binding,
                            kind: super::notifications::Kind::Failed,
                            message: None,
                        }));
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
                    ConnectionState::Starting | ConnectionState::Closing => {
                        notify_controls(&key, cx);
                        continue;
                    }
                    _ => continue,
                },
                Err(error) => {
                    let s = self.sessions.get_mut(&key).unwrap();
                    s.error = Some(error.to_string());
                    s.finish_submission(false);
                    s.instance = None;
                    s.reset_reads();
                    s.clear_extension_ui();
                    s.command.finish();
                    s.queued = None;
                    s.pending_count = 0;
                    s.binding += 1;
                    cx.emit(ConversationEvent::Attention(super::notifications::Notice {
                        key: key.clone(),
                        binding: s.binding,
                        kind: super::notifications::Kind::Failed,
                        message: None,
                    }));
                }
                Ok(None) => continue,
            }
            notify_session(&key, cx);
        }
    }
    pub(crate) fn read_visible_history(&mut self, key: &str, cx: &mut Context<Self>) {
        if self
            .sessions
            .get(key)
            .is_some_and(|s| s.history_dirty && matches!(s.core_read, CoreRead::Idle))
        {
            self.read_session(key, ReadScope::History, cx);
        }
    }
    pub fn refresh(&mut self, key: &str, cx: &mut Context<Self>) {
        self.read_session(key, ReadScope::Full, cx);
    }
    fn read_session(&mut self, key: &str, scope: ReadScope, cx: &mut Context<Self>) {
        if self.draining {
            return;
        }
        let Some(client) = self.client(key, cx) else {
            return;
        };
        let s = self.sessions.get_mut(key).unwrap();
        let scope = if s.state.is_none() {
            ReadScope::Full
        } else {
            scope
        };
        if matches!(s.command, SessionCommand::Forking { .. }) {
            return;
        }
        if s.model_change.running() {
            s.model_change.queue_core_refresh();
            return;
        }
        if s.core_read.running() {
            s.core_read.queue(scope);
            return;
        }
        let identity = s.state.as_ref().map(|state| state.session_id.clone());
        let binding = s.binding;
        let event_revision = s.event_revision;
        let model_revision = s.model_revision;
        let history_revision = s.history().revision;
        let task_key = key.to_owned();
        let task = cx.spawn(async move |owner, cx| {
            let key = task_key;
            let result = snapshot(&client, scope).await.and_then(|snapshot| {
                if identity
                    .as_ref()
                    .is_some_and(|id| *id != snapshot.state.session_id)
                {
                    Err(pi_rpc::Error::Protocol(
                        "session changed while reading conversation".into(),
                    ))
                } else {
                    Ok(snapshot)
                }
            });
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
                match result {
                    Ok(mut snapshot) if stale => {
                        // Deltas invalidate get_state, not the persisted entries returned
                        // by get_entries. Never replace a newer transcript or clear live
                        // output while applying this independent portion of the read.
                        let accept_entries = s.history().revision == history_revision;
                        let idle = !s.running() && !s.compacting && !s.retrying;
                        if accept_entries && let Some(entries) = snapshot.entries.take() {
                            this.apply_history(&key, entries, cx);
                        }
                        // A queued append still needs its read while streaming. Without
                        // another request, the settled event will reconcile runtime state.
                        if let Some(next) = again {
                            this.read_session(&key, if idle { scope.max(next) } else { next }, cx);
                        } else if idle {
                            this.read_session(&key, scope, cx);
                        }
                    }
                    Ok(snapshot) => {
                        this.apply_snapshot(&key, snapshot, scope, cx);
                        if let Some(next) = again {
                            this.read_session(&key, next, cx);
                        }
                    }
                    Err(_) => {
                        let s = this.sessions.get_mut(&key).unwrap();
                        if s.command.reconnecting() {
                            s.command.finish();
                        }
                    }
                }
                notify_session(&key, cx);
            });
        });
        self.sessions.get_mut(key).unwrap().core_read = CoreRead::Reading {
            _task: task,
            pending: None,
        };
        notify_session(key, cx);
    }
    /// Accept persisted entries without applying an older runtime snapshot.
    fn apply_history(&mut self, key: &str, entries: protocol::Entries, cx: &mut Context<Self>) {
        let s = self.sessions.get_mut(key).unwrap();
        let previous_revision = s.history().revision;
        s.transcript.replace(entries);
        s.history_dirty = false;
        if s.history().revision != previous_revision {
            s.content_revision += 1;
            self.refresh_stats(key, cx);
            self.refresh_fork_messages(key, cx);
        }
    }
    fn apply_snapshot(
        &mut self,
        key: &str,
        snapshot: Snapshot,
        scope: ReadScope,
        cx: &mut Context<Self>,
    ) {
        let s = self.sessions.get_mut(key).unwrap();
        let model_changed = s.model_identity()
            != snapshot
                .state
                .model
                .as_ref()
                .map(|model| (model.provider.clone(), model.id.clone()));
        let old_identity = (s.info.id.clone(), s.info.path.clone());
        s.info.id = snapshot.state.session_id.clone();
        if let Some(path) = &snapshot.state.session_file
            && (!s.info.path.as_os_str().is_empty()
                || snapshot.entries.as_ref().is_some_and(|entries| {
                    entries.entries.iter().any(|e| {
                        e.data
                            .get("message")
                            .is_some_and(|m| m["role"] == "assistant")
                    })
                }))
        {
            s.info.path = PathBuf::from(path);
        }
        if !self.temporary || snapshot.state.session_name.is_some() {
            s.info.name = snapshot.state.session_name.clone();
        }
        let previous_history = s.history().revision;
        let had_live = !s.live.is_empty() || !s.tools.is_empty();
        let was_running = s.running();
        if let Some(entries) = snapshot.entries {
            s.run.observe_streaming(snapshot.state.is_streaming);
            s.compacting = snapshot.state.is_compacting;
            if !s.running() && !s.compacting {
                s.stopping = false;
                s.retrying = false;
                s.retry = None;
                s.summary_retry = None;
            }
            s.transcript.replace(entries);
            s.history_dirty = false;
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
        }
        s.pending_count = snapshot.state.pending_message_count;
        if s.pending_count == 0
            || s.queued
                .as_ref()
                .is_some_and(|q| q.steering.len() + q.follow_up.len() != s.pending_count)
        {
            s.queued = None;
        }
        s.state = Some(snapshot.state);
        if s.command.reconnecting() {
            s.command.finish();
        }
        if model_changed {
            s.model_revision += 1;
            s.thinking_levels.reset();
        }
        let history_changed = previous_history != s.history().revision;
        if history_changed || had_live && scope >= ReadScope::History || was_running != s.running()
        {
            s.content_revision += 1;
        }
        if !s.draft.is_empty() && old_identity != (s.info.id.clone(), s.info.path.clone()) {
            self.save_changes(cx);
        }
        if scope == ReadScope::Full {
            self.refresh_auxiliary(key, cx);
        } else {
            if history_changed {
                self.refresh_stats(key, cx);
                self.refresh_fork_messages(key, cx);
            }
            if model_changed {
                self.read_thinking(key, cx);
            }
        }
    }
    pub fn set_draft(&mut self, key: &str, text: String, cx: &mut Context<Self>) {
        if let Some(s) = self.sessions.get_mut(key)
            && !s.submitting()
            && s.draft != text
        {
            s.draft = text;
            s.draft_revision += 1;
            self.save_changes(cx);
            notify_controls(key, cx);
        }
    }
    pub fn can_submit(&self, key: &str, cx: &App) -> bool {
        !self.draining
            && self
                .client(key, cx)
                .is_some_and(|client| matches!(client.state(), ConnectionState::Ready(_)))
            && self.sessions.get(key).is_some_and(|s| {
                s.state.is_some()
                    && !s.preparing
                    && s.attachments_read.is_none()
                    && !s.submitting()
                    && s.pending_ui.is_empty()
                    && !s.command.running()
                    && !s.model_change.running()
                    && !s.model_change.unconfirmed()
            })
    }
    pub fn send(&mut self, key: &str, mode: StreamingBehavior, cx: &mut Context<Self>) {
        let Some(s) = self.sessions.get(key) else {
            return;
        };
        let text = s.draft.clone();
        let revision = s.draft_revision;
        self.submit_text(key, text, Some(revision), mode, cx);
    }
    pub fn send_draft(
        &mut self,
        key: &str,
        cx: &mut Context<Self>,
    ) -> Option<tokio::sync::oneshot::Receiver<bool>> {
        let s = self.sessions.get(key)?;
        self.submit_text(
            key,
            s.draft.clone(),
            Some(s.draft_revision),
            StreamingBehavior::Steer,
            cx,
        )
    }
    /// Submit transient command input without storing it in the conversation draft.
    pub fn send_text(
        &mut self,
        key: &str,
        text: String,
        mode: StreamingBehavior,
        cx: &mut Context<Self>,
    ) -> Option<tokio::sync::oneshot::Receiver<bool>> {
        self.submit_text(key, text, None, mode, cx)
    }
    fn submit_text(
        &mut self,
        key: &str,
        text: String,
        revision: Option<u64>,
        mode: StreamingBehavior,
        cx: &mut Context<Self>,
    ) -> Option<tokio::sync::oneshot::Receiver<bool>> {
        let attachments = if revision.is_some() {
            self.sessions.get(key)?.attachments.clone()
        } else {
            Vec::new()
        };
        if (text.trim().is_empty() && attachments.is_empty()) || !self.can_submit(key, cx) {
            return None;
        }
        let client = self.client(key, cx)?;
        let (reply, receiver) = tokio::sync::oneshot::channel();
        let s = self.sessions.get_mut(key)?;
        s.error = None;
        let binding = s.binding;
        s.interrupted = false;
        let key = key.to_owned();
        let mut prompt = Prompt::new(text);
        for attachment in &attachments {
            match &attachment.content {
                crate::foundation::attachments::Content::File { path, .. } => {
                    if !prompt.message.is_empty() {
                        prompt.message.push('\n');
                    }
                    prompt.message.push('@');
                    prompt.message.push_str(&path.to_string_lossy());
                }
                crate::foundation::attachments::Content::Image { .. } => {}
            }
        }
        let used_template = revision.is_some() && s.pending_template.is_some();
        if used_template {
            let template = s.pending_template.as_ref().unwrap();
            prompt.message =
                super::shortcuts::template_message(&template.name, &template.body, &prompt.message);
        }
        notify_session(&key, cx);
        let attachment_ids: Vec<_> = attachments.iter().map(|a| a.id.clone()).collect();
        prompt.streaming_behavior = Some(mode);
        let task = cx.spawn(async move |owner, cx| {
            let prepared = smol::unblock(move || {
                for attachment in &attachments {
                    if let crate::foundation::attachments::Content::Image { image } =
                        &attachment.content
                    {
                        prompt.images.push(image.to_rpc_image()?);
                    }
                }
                Ok::<_, String>(prompt)
            })
            .await;
            // File encoding can finish after the user stops or replaces this session.
            let current = owner
                .update(cx, |this, cx| {
                    let Some(s) = this.sessions.get_mut(&key).filter(|s| {
                        s.binding == binding && matches!(s.submission, Submission::Sending { .. })
                    }) else {
                        return false;
                    };
                    if s.interrupted {
                        s.finish_submission(false);
                        notify_session(&key, cx);
                        return false;
                    }
                    true
                })
                .unwrap_or(false);
            if !current {
                return;
            }
            let result = match prepared {
                Ok(prompt) => client.prompt(prompt).await.map_err(|e| e.to_string()),
                Err(error) => Err(error),
            };
            let _ = owner.update(cx, |this, cx| {
                let Some(s) = this.sessions.get_mut(&key).filter(|s| {
                    s.binding == binding && matches!(s.submission, Submission::Sending { .. })
                }) else {
                    return;
                };
                let mut draft_changed = false;
                match result {
                    Ok(_) => {
                        s.finish_submission(true);
                        if used_template {
                            s.pending_template = None;
                        }
                        s.attachments.retain(|a| !attachment_ids.contains(&a.id));
                        // Extension editor updates belong to the next submission.
                        if revision == Some(s.draft_revision) {
                            draft_changed = !s.draft.is_empty();
                            s.draft.clear();
                            s.draft_revision += 1;
                        }
                    }
                    Err(error) => {
                        s.error = Some(error.to_string());
                        s.fail_submission(error.to_string(), cx);
                    }
                }
                if draft_changed {
                    this.save_changes(cx);
                }
                notify_session(&key, cx);
                if !this.sessions[&key].running() {
                    this.read_session(&key, ReadScope::History, cx);
                }
            });
        });
        s.submission = Submission::Sending {
            _task: task,
            result: reply,
        };
        Some(receiver)
    }
    pub fn abort(&mut self, key: &str, cx: &mut Context<Self>) {
        if let Some(session) = self.sessions.get_mut(key).filter(|s| s.preparing) {
            crate::app::shortcuts::cancel_preparation(key, cx);
            session.preparing = false;
            session.interrupted = true;
            notify_session(key, cx);
            return;
        }
        let Some(client) = self.client(key, cx) else {
            return;
        };
        let s = self.sessions.get_mut(key).unwrap();
        let binding = s.binding;
        s.stopping = true;
        s.interrupted = true;
        s.retry = None;
        s.summary_retry = None;
        let key = key.to_owned();
        notify_session(&key, cx);
        cx.spawn(async move |owner, cx| {
            let result = client.abort().await;
            let _ = owner.update(cx, |this, cx| {
                if let Some(s) = this.sessions.get_mut(&key).filter(|s| s.binding == binding) {
                    if let Err(error) = result {
                        s.error = Some(error.to_string());
                        s.stopping = false;
                    }
                    this.read_session(&key, ReadScope::History, cx);
                }
                notify_session(&key, cx);
            });
        })
        .detach();
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
                    notify_session(&key, cx);
                });
            });
            s.command = SessionCommand::Closing { _task: task };
        }
        notify_session(key, cx);
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
        self.fork_session(key, Some(entry), cx);
    }
    pub fn can_clone(&self, key: &str, cx: &App) -> bool {
        self.can_export(key, cx) && self.sessions[key].history().leaf.is_some()
    }
    pub fn clone_session(&mut self, key: &str, cx: &mut Context<Self>) {
        if self.can_clone(key, cx) {
            self.fork_session(key, None, cx);
        }
    }
    fn fork_session(&mut self, key: &str, entry: Option<String>, cx: &mut Context<Self>) {
        if self.temporary {
            return;
        }
        let Some(client) = self.client(key, cx) else {
            return;
        };
        let s = self.sessions.get_mut(key).unwrap();
        if s.busy()
            || s.command.running()
            || s.model_change.running()
            || s.model_change.unconfirmed()
            || !s.pending_ui.is_empty()
            || entry
                .as_ref()
                .is_some_and(|entry| !s.fork_options().iter().any(|m| &m.entry_id == entry))
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
            let result = match entry {
                Some(entry) => client.fork(entry).await,
                None => client.clone_session().await,
            };
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
                        // Pi TUI uses the selected message for fork and an empty
                        // editor for clone. Loading history is independent.
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
                        if !session.draft.is_empty() {
                            this.save_changes(cx);
                        }
                        this.refresh(&new_key, cx);
                        notify_session(&new_key, cx);
                        notify_selection(cx);
                    }
                    Ok(_) => {
                        this.refresh(&key, cx);
                    }
                    Err(error) => {
                        source.error = Some(error.to_string());
                        this.refresh_auxiliary(&key, cx);
                    }
                }
                notify_session(&key, cx);
            });
        });
        self.sessions.get_mut(&target).unwrap().command = SessionCommand::Forking { _task: task };
        notify_session(&target, cx);
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
        notify_session(key, cx);
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
        let previous_activity = s.activity();
        let previous_unread = s.unread;
        let mut notice = None;
        let mut refresh = None;
        let mut content_changed = false;
        let mut history_event = false;
        let mut name_changed = false;
        match &event.event {
            Event::Agent { kind, raw } => {
                match kind.as_str() {
                    "queue_update" => {
                        if let Ok(queue) =
                            serde_json::from_value::<protocol::ClearedQueue>(raw.clone())
                        {
                            s.pending_count = queue.steering.len() + queue.follow_up.len();
                            s.queued = Some(queue);
                        }
                    }
                    "agent_start" => {
                        s.message_stream = None;
                        s.tools.clear();
                        s.run.start();
                        s.error = None;
                        s.interrupted = false;
                        content_changed = true;
                    }
                    "agent_end" => return,
                    "agent_settled" => {
                        let was_running = s.running();
                        // A turn with visible assistant output can become unread. Waiting,
                        // failure state, and plugin toasts alone never increment the badge.
                        let has_output = s.active_messages().is_some_and(|ids| {
                            s.messages(None).iter().any(|m| {
                                ids.contains(&m.signature())
                                    && m.role() == "assistant"
                                    && m.value["content"].as_array().is_some_and(|parts| {
                                        parts.iter().any(|p| {
                                            p["type"] == "text"
                                                && p["text"]
                                                    .as_str()
                                                    .is_some_and(|t| !t.trim().is_empty())
                                        })
                                    })
                            })
                        });
                        s.unread |= has_output && !s.interrupted;
                        s.message_stream = None;
                        s.run = RunState::Idle;
                        s.stopping = false;
                        s.retrying = false;
                        s.retry = None;
                        content_changed = true;
                        refresh = Some(ReadScope::History);
                        if was_running && !s.interrupted {
                            if has_output && s.final_answer_text().is_some() {
                                notice = Some(super::notifications::Kind::Completed);
                            } else if s.error.is_some() {
                                notice = Some(super::notifications::Kind::Failed);
                            }
                        }
                    }
                    "extension_error" => {
                        let message = raw["error"].as_str().unwrap_or_default().to_owned();
                        let message = super::notifications::NoticeContent {
                            id: None,
                            message,
                            severity: super::notifications::Severity::Error,
                        };
                        s.notices.push(message.clone());
                        cx.emit(ConversationEvent::Attention(super::notifications::Notice {
                            key: key.clone(),
                            binding: s.binding,
                            kind: super::notifications::Kind::Plugin,
                            message: Some(message),
                        }));
                    }
                    "compaction_start" => s.compacting = true,
                    "compaction_end" => {
                        s.compacting = false;
                        s.summary_retry = None;
                        if s.command.compacting() && raw["aborted"] == true {
                            s.interrupted = true;
                        }
                        if !s.command.compacting() {
                            refresh = Some(ReadScope::History);
                            if raw["aborted"] != true
                                && raw["willRetry"] != true
                                && let Some(error) =
                                    raw["errorMessage"].as_str().filter(|s| !s.is_empty())
                            {
                                let message = super::notifications::NoticeContent {
                                    id: None,
                                    message: error.into(),
                                    severity: super::notifications::Severity::Error,
                                };
                                s.notices.push(message.clone());
                                cx.emit(ConversationEvent::Attention(
                                    super::notifications::Notice {
                                        key: key.clone(),
                                        binding: s.binding,
                                        kind: super::notifications::Kind::Failed,
                                        message: Some(message),
                                    },
                                ));
                            }
                        }
                    }
                    "auto_retry_start" => {
                        s.retrying = true;
                        s.retry = execution::RetryProgress::from_event(raw);
                    }
                    "auto_retry_end" => {
                        s.retrying = false;
                        s.retry = None;
                        if raw.get("success") == Some(&Value::Bool(false)) {
                            s.error = raw
                                .get("finalError")
                                .and_then(Value::as_str)
                                .map(str::to_owned);
                        }
                    }
                    "session_info_changed" => {
                        let name = raw.get("name").and_then(Value::as_str).map(str::to_owned);
                        s.info.name = name.clone();
                        if let Some(state) = &mut s.state {
                            state.session_name = name;
                        }
                        s.history_dirty = true;
                        name_changed = true;
                    }
                    "thinking_level_changed" => {
                        if let Some(level) = raw
                            .get("thinkingLevel")
                            .or_else(|| raw.get("level"))
                            .and_then(Value::as_str)
                        {
                            if let Some(state) = &mut s.state {
                                state.thinking_level = level.to_owned();
                            }
                            s.settings_event_revision += 1;
                            s.history_dirty = true;
                            if !s.model_change.running() {
                                refresh = Some(ReadScope::State);
                            }
                        }
                    }
                    "entry_appended" => {
                        s.usage_revision += 1;
                        s.history_dirty = true;
                        history_event = true;
                        refresh = Some(ReadScope::History);
                    }
                    "summarization_retry_scheduled" => {
                        s.summary_retry = execution::RetryProgress::from_event(raw);
                    }
                    "summarization_retry_attempt_start" => {
                        s.summary_retry = None;
                    }
                    "summarization_retry_finished" => s.summary_retry = None,
                    "message_start" | "message_update" | "message_end" => {
                        if raw["message"]["role"] == "custom" {
                            if kind == "message_end" {
                                s.history_dirty = true;
                                history_event = true;
                                refresh = Some(ReadScope::History);
                            } else {
                                return;
                            }
                        } else {
                            s.retry = None;
                            s.receive_message(kind, raw);
                            if kind == "message_end" {
                                s.usage_revision += 1;
                            }
                            content_changed = true;
                        }
                    }
                    "tool_execution_start" | "tool_execution_update" | "tool_execution_end" => {
                        content_changed = true;
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
                    _ => return,
                }
                if content_changed {
                    s.content_revision += 1;
                }
                // Retry display and unknown events cannot invalidate in-flight data.
                if content_changed
                    || history_event
                    || matches!(
                        kind.as_str(),
                        "queue_update"
                            | "session_info_changed"
                            | "thinking_level_changed"
                            | "compaction_start"
                            | "compaction_end"
                    )
                {
                    s.event_revision += 1;
                }
            }
            Event::ExtensionUi { request, .. } => match &request.method {
                UiMethod::Select { timeout, .. }
                | UiMethod::Confirm { timeout, .. }
                | UiMethod::Input { timeout, .. } => {
                    if s.pending_ui.iter().any(|p| p.request.id == request.id) {
                        return;
                    }
                    notice = Some(super::notifications::Kind::Waiting(request.id.clone()));
                    let timeout = *timeout;
                    s.pending_ui.push_back(PendingUi {
                        request: request.clone(),
                        text: String::new(),
                        deadline: timeout.map(|ms| Instant::now() + Duration::from_millis(ms)),
                    });
                    if let Some(ms) = timeout {
                        let id = request.id.clone();
                        let instance = event.instance;
                        let target = key.clone();
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
                                    let len = s.pending_ui.len();
                                    s.pending_ui.retain(|p| p.request.id != id);
                                    if len != s.pending_ui.len() {
                                        notify_session(&target, cx);
                                    }
                                }
                            });
                        })
                        .detach();
                    }
                }
                UiMethod::Editor { prefill, .. } => {
                    if s.pending_ui.iter().any(|p| p.request.id == request.id) {
                        return;
                    }
                    notice = Some(super::notifications::Kind::Waiting(request.id.clone()));
                    s.pending_ui.push_back(PendingUi {
                        request: request.clone(),
                        text: prefill.clone().unwrap_or_default(),
                        deadline: None,
                    });
                }
                UiMethod::SetEditorText { text } => {
                    if s.draft == *text {
                        return;
                    }
                    s.draft = text.clone();
                    s.draft_revision += 1;
                }
                UiMethod::SetTitle { title } => {
                    if s.extension_title.as_ref() == Some(title) {
                        return;
                    }
                    s.extension_title = Some(title.clone());
                }
                UiMethod::SetStatus { key, text } => {
                    if s.statuses.get(key) == text.as_ref() {
                        return;
                    }
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
                } => {
                    let message = super::notifications::NoticeContent {
                        id: Some(request.id.clone()),
                        message: message.clone(),
                        severity: super::notifications::Severity::from_pi(notify_type.as_deref()),
                    };
                    s.notices.push(message.clone());
                    cx.emit(ConversationEvent::Attention(super::notifications::Notice {
                        key: key.clone(),
                        binding: s.binding,
                        kind: super::notifications::Kind::Plugin,
                        message: Some(message),
                    }));
                }
                UiMethod::Unknown => return,
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
        if let Some(kind) = notice {
            cx.emit(ConversationEvent::Attention(super::notifications::Notice {
                key: key.clone(),
                binding: s.binding,
                kind,
                message: None,
            }));
        }
        let navigation_changed =
            name_changed || s.activity() != previous_activity || s.unread != previous_unread;
        if name_changed {
            let info = self.sessions[&key].info.clone();
            if !info.path.as_os_str().is_empty() {
                for (alias, other) in self
                    .sessions
                    .iter_mut()
                    .filter(|(_, s)| s.info.path == info.path)
                {
                    other.info.name = info.name.clone();
                    if let Some(state) = &mut other.state {
                        state.session_name = info.name.clone();
                    }
                    other.history_dirty = true;
                    if alias != &key {
                        notify_session(alias, cx);
                    }
                }
            }
        }
        if let Some(scope) = refresh {
            self.read_session(&key, scope, cx);
        }
        if save_draft {
            self.save_changes(cx);
        }
        if navigation_changed {
            notify_session(&key, cx);
        } else if content_changed {
            notify_body(&key, cx);
        } else {
            notify_controls(&key, cx);
        }
    }
    fn file(&self) -> WorkspaceFile {
        if self.temporary {
            return WorkspaceFile { drafts: vec![] };
        }
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
    fn save_changes(&mut self, cx: &mut Context<Self>) {
        self.revision += 1;
        if self.temporary || !self.workspace_loaded || self.save_task.is_some() || self.draining {
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
                        let error = result.err();
                        let changed = this.storage_error != error;
                        this.storage_error = error;
                        let done = this.revision == revision;
                        if done {
                            this.save_task = None;
                        }
                        if changed {
                            notify(cx);
                        }
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
            s.attachments_read = None;
            s.submission = Submission::Idle;
            if matches!(
                s.command,
                SessionCommand::Deleting { .. }
                    | SessionCommand::Reconnecting { .. }
                    | SessionCommand::Closing { .. }
            ) {
                match std::mem::take(&mut s.command) {
                    SessionCommand::Deleting { task }
                    | SessionCommand::Reconnecting { _task: task }
                    | SessionCommand::Closing { _task: task } => deletions.push(task),
                    _ => unreachable!(),
                }
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
    mod temporary_view;
    use super::{ConversationState, WorkspaceFile, catalog::CatalogState};
    use crate::{foundation::session_catalog::Catalog, state::pi};
    use gpui_kit as gpui;
    use gpui_kit::{AppContext, TestAppContext};
    use std::path::PathBuf;

    #[test]
    fn copy_last_answer_uses_execution_branch_and_skips_empty_aborted_output() {
        let mut session = super::Session::new(
            super::SessionInfo {
                path: PathBuf::new(),
                id: "copy".into(),
                cwd: PathBuf::new(),
                name: None,
                first_message: String::new(),
                activity: String::new(),
                parent_session: None,
            },
            String::new(),
        );
        assert_eq!(session.last_assistant_text(), None);
        session.transcript.replace(serde_json::from_value(serde_json::json!({
            "leafId":"aborted", "entries":[
                {"id":"user", "timestamp":"1", "type":"message", "message":{"role":"user", "content":"question"}},
                {"id":"answer", "timestamp":"2", "parentId":"user", "type":"message", "message":{"role":"assistant", "content":[{"type":"text", "text":"current answer"}]}},
                {"id":"aborted", "timestamp":"3", "parentId":"answer", "type":"message", "message":{"role":"assistant", "stopReason":"aborted", "content":[]}},
                {"id":"other", "timestamp":"4", "parentId":"user", "type":"message", "message":{"role":"assistant", "content":"other branch"}}
            ]
        })).unwrap());
        assert_eq!(
            session.last_assistant_text().as_deref(),
            Some("current answer")
        );
        // Previewing another branch must not retarget the command's source.
        assert!(
            session
                .messages(Some("other"))
                .iter()
                .any(|m| m.text() == "other branch")
        );
        assert_eq!(
            session.last_assistant_text().as_deref(),
            Some("current answer")
        );
    }

    #[gpui::test]
    fn startup_ignores_legacy_selection_and_restores_only_nonempty_drafts(cx: &mut TestAppContext) {
        cx.update(pi::init);
        let state = cx.new(|cx| ConversationState::new(PathBuf::from("unused-pi"), cx));
        state.update(cx, |state, cx| {
            state.insert_draft(None);
            let foreground = state.selected.clone().unwrap();
            assert_eq!(state.infos().len(), 1);
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
            assert_eq!(state.infos().len(), 1);
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
            assert_eq!(state.infos().len(), 2);
            assert_eq!(state.selected.as_deref(), Some("history"));
            assert!(state.sessions["history"].instance.is_none());
            assert!(!state.sessions["history"].command.running());
        });
    }
}

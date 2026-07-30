mod approval;

use std::{collections::HashMap, sync::Arc};

use gpui::{App, AppContext, AsyncApp, Context, Entity, EventEmitter, Task, WeakEntity};
use gpui_operation::{Cancel, Complete, Load, Retry, Transition, refresh};
use jaco_agent::{
    AgentCancellationToken, AgentPersistence, AgentRunHandle, AgentRunRequest, AgentRuntime,
    AgentRuntimeObserver, OpenAiResponsesSessionPool, ToolApprovalDecision,
};
use jaco_core::{AgentRunId, ConversationId, ToolInvocationId};
use jaco_db::ProviderRecord;
use smol::channel::{Receiver, Sender};
use tracing::{Level, event};

use self::approval::ConversationApprovalBroker;
use crate::{database, errors::JacoResult, state::providers::secrets::ProviderSecretStore};

enum RuntimePublication {
    Event(jaco_agent::AgentRuntimeEvent),
    Drain(Sender<()>),
}

pub(crate) struct ConversationRuntimeStore {
    active_runs: HashMap<ConversationId, ActiveRun>,
    last_errors: HashMap<ConversationId, String>,
    next_run_key: u64,
    shutting_down: bool,
    openai_sessions: OpenAiResponsesSessionPool,
    recovery: refresh::Operation<(), ConversationRuntimeProblem, Task<()>>,
}

#[derive(Debug)]
pub(crate) struct ConversationRuntimeProblem(String);

impl std::fmt::Display for ConversationRuntimeProblem {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ConversationRuntimeProblem {}

struct ActiveRun {
    key: ActiveRunKey,
    agent_run_id: Option<AgentRunId>,
    cancellation_token: AgentCancellationToken,
    approval_broker: Arc<ConversationApprovalBroker>,
    task: ActiveRunTask,
    _event_task: Task<()>,
}

enum ActiveRunTask {
    Running(Task<()>),
    Stopping(Task<()>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ConversationRunStatus {
    Idle,
    Running,
    Stopping,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ActiveRunKey(u64);

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ConversationRuntimeEvent {
    RunStarted { conversation_id: ConversationId },
    RunFinished { conversation_id: ConversationId },
}

impl EventEmitter<ConversationRuntimeEvent> for ConversationRuntimeStore {}

impl ConversationRuntimeStore {
    fn new() -> Self {
        Self {
            active_runs: HashMap::new(),
            last_errors: HashMap::new(),
            next_run_key: 0,
            shutting_down: false,
            openai_sessions: OpenAiResponsesSessionPool::new(),
            recovery: refresh::Operation::new(),
        }
    }

    #[cfg(test)]
    fn new_ready_for_test() -> Self {
        let mut runtime = Self::new();
        runtime.recovery.transition(Load(Task::ready(())));
        runtime.recovery.transition(Complete(Ok(())));
        runtime
    }

    pub(crate) fn run_status(&self, conversation_id: &ConversationId) -> ConversationRunStatus {
        match self
            .active_runs
            .get(conversation_id)
            .map(|active| &active.task)
        {
            None => ConversationRunStatus::Idle,
            Some(ActiveRunTask::Running(_)) => ConversationRunStatus::Running,
            Some(ActiveRunTask::Stopping(_)) => ConversationRunStatus::Stopping,
        }
    }

    pub(crate) fn is_running(&self, conversation_id: &ConversationId) -> bool {
        self.run_status(conversation_id) != ConversationRunStatus::Idle
    }

    pub(crate) fn close_conversation_sessions(
        &mut self,
        conversation_id: ConversationId,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        let sessions = self.openai_sessions.clone();
        cx.spawn(async move |_, _| sessions.close_conversation(&conversation_id).await)
    }

    pub(crate) fn active_agent_run_id(
        &self,
        conversation_id: &ConversationId,
    ) -> Option<AgentRunId> {
        self.active_runs
            .get(conversation_id)
            .and_then(|active| active.agent_run_id.clone())
    }

    pub(crate) fn recovery(&self) -> &refresh::Operation<(), ConversationRuntimeProblem, Task<()>> {
        &self.recovery
    }

    pub(crate) fn take_last_error(&mut self, conversation_id: &ConversationId) -> Option<String> {
        self.last_errors.remove(conversation_id)
    }

    pub(crate) fn stop_run(
        &mut self,
        conversation_id: &ConversationId,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(active) = self.active_runs.get_mut(conversation_id) else {
            return false;
        };
        if matches!(active.task, ActiveRunTask::Stopping(_)) {
            return false;
        }

        active.cancellation_token.cancel();
        if let Some(agent_run_id) = active.agent_run_id.as_ref() {
            active.approval_broker.cancel_all_for_run(agent_run_id);
        } else {
            active.approval_broker.cancel_all();
        }
        let run_key = active.key;
        let agent_run_id = active.agent_run_id.clone();
        let persistence = database::ready_agent_persistence(cx).map_err(|error| error.to_string());
        let openai_sessions = self.openai_sessions.clone();
        let cleanup_conversation_id = conversation_id.clone();
        let running_task =
            match std::mem::replace(&mut active.task, ActiveRunTask::Stopping(Task::ready(()))) {
                ActiveRunTask::Running(task) => task,
                ActiveRunTask::Stopping(task) => {
                    active.task = ActiveRunTask::Stopping(task);
                    return false;
                }
            };
        let cleanup = cx.spawn(async move |store, cx| {
            running_task.await;
            openai_sessions
                .close_conversation(&cleanup_conversation_id)
                .await;
            let result = match persistence {
                Ok(persistence) => AgentRuntime::new(persistence)
                    .with_openai_session_pool(openai_sessions.clone())
                    .cancel_non_terminal_runs_for_conversation(&cleanup_conversation_id, None)
                    .await
                    .map(|_| ())
                    .map_err(|error| error.to_string()),
                Err(error) => Err(error),
            };
            finish_stop_from_task(
                store,
                cleanup_conversation_id,
                run_key,
                agent_run_id,
                result,
                cx,
            );
        });
        active.task = ActiveRunTask::Stopping(cleanup);

        self.last_errors.remove(conversation_id);
        cx.notify();
        true
    }

    pub(crate) fn start_run(
        &mut self,
        request: AgentRunRequest,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if self.shutting_down || !matches!(self.recovery, refresh::Operation::Ready(_)) {
            let message = self
                .recovery
                .problem()
                .map(ToString::to_string)
                .unwrap_or_else(|| "conversation runtime is recovering".to_string());
            self.last_errors
                .insert(request.conversation_id.clone(), message.clone());
            cx.notify();
            return Err(message);
        }
        let conversation_id = request.conversation_id.clone();
        if self.active_runs.contains_key(&conversation_id) {
            return Err("conversation already has an active run".to_string());
        }

        self.last_errors.remove(&conversation_id);
        let run_key = self.next_active_run_key();
        let persistence = match database::ready_agent_persistence(cx) {
            Ok(persistence) => persistence,
            Err(error) => {
                self.last_errors
                    .insert(conversation_id.clone(), error.to_string());
                cx.notify();
                return Err(error.to_string());
            }
        };
        let provider = match crate::state::providers::ready_provider(&request.provider_id, cx) {
            Ok(provider) => provider,
            Err(error) => {
                self.last_errors
                    .insert(conversation_id.clone(), error.to_string());
                cx.notify();
                return Err(error.to_string());
            }
        };
        let (tx, rx) = smol::channel::unbounded();
        let event_task = self.spawn_event_listener(rx, cx);
        let approval_broker = Arc::new(ConversationApprovalBroker::new());
        let run_conversation_id = conversation_id.clone();
        let cancellation_token = request.cancellation_token.clone();
        let runtime_approval_broker = approval_broker.clone();
        let openai_sessions = self.openai_sessions.clone();
        let run_task = cx.spawn(async move |store, cx| {
            let result = run_agent_with_saved_provider(
                persistence,
                provider,
                request,
                tx.clone(),
                runtime_approval_broker,
                openai_sessions,
                cx,
            )
            .await;
            if let Err(error) = drain_runtime_publications(&tx).await {
                event!(
                    Level::ERROR,
                    %error,
                    conversation_id = %run_conversation_id,
                    "drain conversation runtime publications failed"
                );
            }
            finish_run_from_task(store, run_conversation_id, run_key, result, cx);
        });

        self.active_runs.insert(
            conversation_id.clone(),
            ActiveRun {
                key: run_key,
                agent_run_id: None,
                cancellation_token,
                approval_broker,
                task: ActiveRunTask::Running(run_task),
                _event_task: event_task,
            },
        );
        super::registry::retain_active(conversation_id.clone(), cx);
        cx.emit(ConversationRuntimeEvent::RunStarted {
            conversation_id: conversation_id.clone(),
        });
        cx.notify();
        Ok(())
    }

    pub(crate) fn shutdown_all(&mut self, cx: &mut Context<Self>) -> Task<()> {
        if self.shutting_down {
            return Task::ready(());
        }
        self.shutting_down = true;
        if self.recovery.is_running() {
            self.recovery.transition(Cancel);
        }

        let active_runs = std::mem::take(&mut self.active_runs);
        let mut tasks = Vec::with_capacity(active_runs.len());
        for (conversation_id, active) in active_runs {
            active.cancellation_token.cancel();
            if let Some(agent_run_id) = active.agent_run_id.as_ref() {
                active.approval_broker.cancel_all_for_run(agent_run_id);
            } else {
                active.approval_broker.cancel_all();
            }
            self.last_errors.remove(&conversation_id);
            cx.emit(ConversationRuntimeEvent::RunFinished { conversation_id });
            tasks.push((active.task, active._event_task));
        }
        cx.notify();

        let openai_sessions = self.openai_sessions.clone();
        cx.spawn(async move |_, _| {
            for (task, event_task) in tasks {
                match task {
                    ActiveRunTask::Running(task) | ActiveRunTask::Stopping(task) => task.await,
                }
                event_task.await;
            }
            openai_sessions.close_all().await;
        })
    }

    pub(crate) fn approve_tool_invocation(
        &mut self,
        conversation_id: ConversationId,
        tool_invocation_id: ToolInvocationId,
        _window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.shutting_down || !matches!(self.recovery, refresh::Operation::Ready(_)) {
            return false;
        }
        self.last_errors.remove(&conversation_id);
        let Some(active) = self.active_runs.get(&conversation_id) else {
            return false;
        };
        if !matches!(active.task, ActiveRunTask::Running(_)) {
            return false;
        }
        let Some(agent_run_id) = active.agent_run_id.as_ref() else {
            return false;
        };
        if !active
            .approval_broker
            .is_pending_for_run(agent_run_id, &tool_invocation_id)
        {
            return false;
        }
        debug_assert!(active.approval_broker.pending_count_for_run(agent_run_id) > 0);
        let Some(outcome) = active.approval_broker.resolve(
            &tool_invocation_id,
            ToolApprovalDecision::Approved {
                decided_by: "user".to_string(),
                reason: None,
            },
        ) else {
            return false;
        };
        debug_assert_eq!(outcome.conversation_id, conversation_id);
        debug_assert_eq!(&outcome.agent_run_id, agent_run_id);
        let _ = outcome.remaining_for_run;
        cx.notify();
        true
    }

    pub(crate) fn deny_tool_invocation(
        &mut self,
        conversation_id: ConversationId,
        tool_invocation_id: ToolInvocationId,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.shutting_down || !matches!(self.recovery, refresh::Operation::Ready(_)) {
            return false;
        }
        self.last_errors.remove(&conversation_id);
        let Some(active) = self.active_runs.get(&conversation_id) else {
            return false;
        };
        if !matches!(active.task, ActiveRunTask::Running(_)) {
            return false;
        }
        let Some(agent_run_id) = active.agent_run_id.as_ref() else {
            return false;
        };
        if !active
            .approval_broker
            .is_pending_for_run(agent_run_id, &tool_invocation_id)
        {
            return false;
        }
        debug_assert!(active.approval_broker.pending_count_for_run(agent_run_id) > 0);
        let Some(outcome) = active.approval_broker.resolve(
            &tool_invocation_id,
            ToolApprovalDecision::Denied {
                decided_by: "user".to_string(),
                reason: None,
            },
        ) else {
            return false;
        };
        debug_assert_eq!(outcome.conversation_id, conversation_id);
        debug_assert_eq!(&outcome.agent_run_id, agent_run_id);
        let _ = outcome.remaining_for_run;
        cx.notify();
        true
    }

    fn next_active_run_key(&mut self) -> ActiveRunKey {
        let key = ActiveRunKey(self.next_run_key);
        self.next_run_key = self.next_run_key.wrapping_add(1);
        key
    }

    fn spawn_event_listener(
        &self,
        rx: Receiver<RuntimePublication>,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        cx.spawn(async move |this, cx| {
            while let Ok(publication) = rx.recv().await {
                match publication {
                    RuntimePublication::Event(runtime_event) => {
                        let Some(this) = this.upgrade() else {
                            break;
                        };
                        this.update(cx, |store, cx| {
                            store.handle_runtime_event(runtime_event, cx);
                        });
                    }
                    RuntimePublication::Drain(acknowledgement) => {
                        let _ = acknowledgement.send(()).await;
                        break;
                    }
                }
            }
        })
    }

    fn handle_runtime_event(
        &mut self,
        runtime_event: jaco_agent::AgentRuntimeEvent,
        cx: &mut Context<Self>,
    ) {
        match runtime_event {
            jaco_agent::AgentRuntimeEvent::ConversationCommitted {
                conversation,
                changes,
            } => {
                let conversation = *conversation;
                let conversation_id = conversation.id.clone();
                super::registry::publish_changes(
                    conversation_id.clone(),
                    Some(conversation.clone()),
                    changes.clone(),
                    cx,
                );
            }
            jaco_agent::AgentRuntimeEvent::ConversationTimelineChanged {
                conversation_id,
                changes,
            } => {
                super::registry::publish_changes(
                    conversation_id.clone(),
                    None,
                    changes.clone(),
                    cx,
                );
            }
            jaco_agent::AgentRuntimeEvent::AgentRunStarted {
                agent_run_id,
                conversation_id,
            } => {
                let Some(active) = self.active_runs.get_mut(&conversation_id) else {
                    return;
                };
                if !matches!(active.task, ActiveRunTask::Running(_)) {
                    return;
                }
                active.agent_run_id = Some(agent_run_id);
            }
            jaco_agent::AgentRuntimeEvent::AgentRunStatusChanged {
                agent_run_id: _,
                status: _,
            } => {}
        }
        cx.notify();
    }

    fn finish_run(
        &mut self,
        conversation_id: ConversationId,
        run_key: ActiveRunKey,
        result: Result<AgentRunHandle, String>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(active) = self.active_runs.get(&conversation_id) else {
            return false;
        };
        if active.key != run_key || !matches!(active.task, ActiveRunTask::Running(_)) {
            return false;
        }

        self.active_runs.remove(&conversation_id);
        super::registry::release_active(&conversation_id, cx);
        if let Err(err) = result {
            let sessions = self.openai_sessions.clone();
            let failed_conversation_id = conversation_id.clone();
            cx.spawn(async move |_, _| {
                sessions.close_conversation(&failed_conversation_id).await;
            })
            .detach();
            self.last_errors.insert(conversation_id.clone(), err);
        }
        cx.emit(ConversationRuntimeEvent::RunFinished { conversation_id });
        cx.notify();
        true
    }

    fn finish_stop(
        &mut self,
        conversation_id: ConversationId,
        run_key: ActiveRunKey,
        agent_run_id: Option<AgentRunId>,
        result: Result<(), String>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(active) = self.active_runs.get(&conversation_id) else {
            return false;
        };
        if active.key != run_key || !matches!(active.task, ActiveRunTask::Stopping(_)) {
            return false;
        }

        self.active_runs.remove(&conversation_id);
        super::registry::release_active(&conversation_id, cx);
        match result {
            Ok(()) => {
                super::registry::refresh_conversation(&conversation_id, cx);
            }
            Err(error) => {
                event!(
                    Level::ERROR,
                    %error,
                    %conversation_id,
                    ?agent_run_id,
                    "cancel active conversation runs failed"
                );
                self.last_errors.insert(conversation_id.clone(), error);
            }
        }
        cx.emit(ConversationRuntimeEvent::RunFinished { conversation_id });
        cx.notify();
        true
    }
}

fn finish_run_from_task(
    store: WeakEntity<ConversationRuntimeStore>,
    conversation_id: ConversationId,
    run_key: ActiveRunKey,
    result: Result<AgentRunHandle, String>,
    cx: &mut AsyncApp,
) {
    if let Err(err) = store.update(cx, |store, cx| {
        store.finish_run(conversation_id, run_key, result, cx);
    }) {
        event!(Level::ERROR, error = ?err, "finish conversation agent run failed");
    }
}

fn finish_stop_from_task(
    store: WeakEntity<ConversationRuntimeStore>,
    conversation_id: ConversationId,
    run_key: ActiveRunKey,
    agent_run_id: Option<AgentRunId>,
    result: Result<(), String>,
    cx: &mut AsyncApp,
) {
    if let Err(err) = store.update(cx, |store, cx| {
        store.finish_stop(conversation_id, run_key, agent_run_id, result, cx);
    }) {
        event!(Level::ERROR, error = ?err, "finish stopping conversation agent run failed");
    }
}

async fn drain_runtime_publications(tx: &Sender<RuntimePublication>) -> Result<(), String> {
    let (acknowledgement_tx, acknowledgement_rx) = smol::channel::bounded(1);
    tx.send(RuntimePublication::Drain(acknowledgement_tx))
        .await
        .map_err(|_| "conversation runtime publication listener is unavailable".to_string())?;
    acknowledgement_rx.recv().await.map_err(|_| {
        "conversation runtime publication listener did not acknowledge drain".to_string()
    })
}

pub(crate) fn create(cx: &mut App) -> JacoResult<Entity<ConversationRuntimeStore>> {
    let store = cx.new(|_| ConversationRuntimeStore::new());
    request_recovery(store.clone(), cx)?;
    Ok(store)
}

pub(crate) fn retry_recovery_if_needed(store: &Entity<ConversationRuntimeStore>, cx: &mut App) {
    let should_retry = {
        let runtime = store.read(cx);
        !runtime.shutting_down
            && matches!(
                runtime.recovery,
                refresh::Operation::Unavailable(_) | refresh::Operation::Degraded(_)
            )
    };
    if should_retry && let Err(error) = request_recovery(store.clone(), cx) {
        event!(
            Level::ERROR,
            ?error,
            "retry conversation runtime recovery failed"
        );
    }
}

fn request_recovery(store: Entity<ConversationRuntimeStore>, cx: &mut App) -> JacoResult<()> {
    let persistence = database::ready_agent_persistence(cx)?;
    let openai_sessions = store.read(cx).openai_sessions.clone();
    let completion_store = store.downgrade();
    let task = cx.spawn(async move |cx| {
        let result = AgentRuntime::new(persistence)
            .with_openai_session_pool(openai_sessions)
            .recover_interrupted_runs()
            .await
            .map_err(|error| ConversationRuntimeProblem(error.to_string()));
        if let Ok(recovered) = &result
            && !recovered.is_empty()
        {
            event!(
                Level::WARN,
                recovered_count = recovered.len(),
                "recovered interrupted jaco agent runs"
            );
        }
        let failed = result.is_err();
        let _ = completion_store.update(cx, |runtime, cx| {
            if runtime.recovery.is_running() {
                runtime.recovery.transition(Complete(result.map(|_| ())));
                cx.notify();
            }
        });
        if failed {
            event!(Level::ERROR, "recover interrupted jaco agent runs failed");
            cx.update(database::request_refresh);
        }
    });
    store.update(cx, |runtime, cx| {
        match runtime.recovery {
            refresh::Operation::Idle(_) => runtime.recovery.transition(Load(task)),
            refresh::Operation::Unavailable(_) | refresh::Operation::Degraded(_) => {
                runtime.recovery.transition(Retry(task));
            }
            refresh::Operation::Loading(_)
            | refresh::Operation::Ready(_)
            | refresh::Operation::Refreshing(_)
            | refresh::Operation::Retrying(_)
            | refresh::Operation::RefreshingDegraded(_) => {}
        }
        cx.notify();
    });
    Ok(())
}

async fn run_agent_with_saved_provider(
    persistence: Arc<dyn AgentPersistence>,
    provider: ProviderRecord,
    request: AgentRunRequest,
    tx: Sender<RuntimePublication>,
    approval_broker: Arc<ConversationApprovalBroker>,
    openai_sessions: OpenAiResponsesSessionPool,
    cx: &mut AsyncApp,
) -> Result<AgentRunHandle, String> {
    let observer = AgentRuntimeObserver::new(move |event| {
        if let Err(err) = tx.send_blocking(RuntimePublication::Event(event)) {
            event!(Level::ERROR, error = ?err, "send conversation runtime event failed");
        }
    });
    let runtime = AgentRuntime::new(persistence)
        .with_openai_session_pool(openai_sessions)
        .with_approval_broker(approval_broker);
    let mut request = match crate::state::mcp::prepare_run_request(request, cx).await {
        Ok(prepared) => prepared.request,
        Err(err) => {
            return gpui_tokio::Tokio::spawn(cx, async move {
                runtime
                    .record_setup_failed_run(err.request, err.message, Some(&observer))
                    .await
            })
            .await
            .map_err(|err| err.to_string())?
            .map_err(|err| err.to_string());
        }
    };
    let agent_run = runtime
        .begin_run(&mut request, Some(observer))
        .await
        .map_err(|err| err.to_string())?;
    let secrets = match ProviderSecretStore::read_values(cx, &provider.secret_refs).await {
        Ok(secrets) => secrets,
        Err(err) => {
            return gpui_tokio::Tokio::spawn(cx, async move {
                runtime
                    .record_setup_failed_started_run(agent_run, err)
                    .await
            })
            .await
            .map_err(|err| err.to_string())?
            .map_err(|err| err.to_string());
        }
    };
    gpui_tokio::Tokio::spawn(cx, async move {
        runtime
            .run_started_with_saved_provider(agent_run, request, provider, secrets)
            .await
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database;
    use gpui::{Subscription, WindowHandle};
    use jaco_agent::AgentRunHandleStatus;
    use jaco_core::{
        AgentEngineKind, AgentRunInput, AgentRunStatus, AgentRunTriggerKind, AgentRuntimeSnapshot,
        ApprovalRequestPayload, ApprovalStatus, ContentPart, ConversationEntryPayload,
        ConversationEntryStatus, ConversationMetadata, ConversationSettingsSnapshot, ProjectKind,
        ProjectMetadata, ProviderSettingsPayload, ToolApprovalMode, ToolApprovalPolicy,
        ToolArguments, ToolExecutionPolicy, ToolInvocationInput, ToolInvocationStatus,
        ToolNameStrategy, ToolPolicySnapshot, ToolSource, TranscriptRole,
        conservative_model_capabilities,
    };
    use jaco_db::{
        FreshRepository, NewAgentRun, NewConversation, NewConversationEntry, NewProject,
        NewToolInvocation, NewToolInvocationApproval,
    };
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    };
    use tempfile::{TempDir, tempdir};

    struct RuntimeEventRecorder {
        events: Arc<Mutex<Vec<ConversationRuntimeEvent>>>,
        _subscription: Subscription,
    }

    impl RuntimeEventRecorder {
        fn new(
            store: Entity<ConversationRuntimeStore>,
            cx: &mut Context<RuntimeEventRecorder>,
        ) -> Self {
            let events = Arc::new(Mutex::new(Vec::new()));
            let observed_events = events.clone();
            let subscription = cx.subscribe(
                &store,
                move |_recorder, _store, event: &ConversationRuntimeEvent, _cx| {
                    observed_events.lock().unwrap().push(event.clone());
                },
            );
            Self {
                events,
                _subscription: subscription,
            }
        }
    }

    fn active_run(key: ActiveRunKey) -> ActiveRun {
        active_run_with_token(key, AgentCancellationToken::new())
    }

    fn active_run_with_token(
        key: ActiveRunKey,
        cancellation_token: AgentCancellationToken,
    ) -> ActiveRun {
        ActiveRun {
            key,
            agent_run_id: Some("run-1".to_string()),
            cancellation_token,
            approval_broker: Arc::new(ConversationApprovalBroker::new()),
            task: ActiveRunTask::Running(Task::ready(())),
            _event_task: Task::ready(()),
        }
    }

    fn active_run_with_agent_id(key: ActiveRunKey, agent_run_id: AgentRunId) -> ActiveRun {
        ActiveRun {
            agent_run_id: Some(agent_run_id),
            ..active_run(key)
        }
    }

    #[gpui::test]
    fn publication_drain_acknowledges_after_queued_events_are_applied(
        cx: &mut gpui::TestAppContext,
    ) {
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new_ready_for_test()));
        let conversation_id = "conversation-1".to_string();
        let agent_run_id = "run-final".to_string();
        let event_applied = Arc::new(AtomicBool::new(false));

        let driver = cx.update(|cx| {
            store.update(cx, |runtime, cx| {
                let (tx, rx) = smol::channel::unbounded();
                let event_task = runtime.spawn_event_listener(rx, cx);
                runtime.active_runs.insert(
                    conversation_id.clone(),
                    ActiveRun {
                        agent_run_id: None,
                        _event_task: event_task,
                        ..active_run(ActiveRunKey(0))
                    },
                );
                tx.send_blocking(RuntimePublication::Event(
                    jaco_agent::AgentRuntimeEvent::AgentRunStarted {
                        agent_run_id: agent_run_id.clone(),
                        conversation_id: conversation_id.clone(),
                    },
                ))
                .expect("queue runtime event");

                let store = store.downgrade();
                let event_applied = event_applied.clone();
                let conversation_id = conversation_id.clone();
                let agent_run_id = agent_run_id.clone();
                cx.spawn(async move |_, cx| {
                    drain_runtime_publications(&tx)
                        .await
                        .expect("drain runtime publications");
                    let applied = store
                        .update(cx, |runtime, _| {
                            runtime.active_agent_run_id(&conversation_id).as_ref()
                                == Some(&agent_run_id)
                        })
                        .unwrap_or(false);
                    event_applied.store(applied, Ordering::SeqCst);
                })
            })
        });

        cx.run_until_parked();
        assert!(event_applied.load(Ordering::SeqCst));
        drop(driver);
    }

    #[gpui::test]
    fn init_recovers_persisted_running_runs(cx: &mut gpui::TestAppContext) {
        let _dir = init_runtime_test(cx);
        let (conversation_id, agent_run_id) = cx.update(|cx| {
            let repository = test_repository(cx);
            let conversation_id = insert_conversation_with_user_item(&repository);
            let agent_run_id = insert_agent_run(&repository, &conversation_id);
            (conversation_id, agent_run_id)
        });

        let runtime = cx.update(|cx| create(cx).expect("initialize conversation runtime"));
        cx.run_until_parked();
        cx.update(|cx| {
            let repository = test_repository(cx);
            let agent_run = repository
                .get_agent_run(&agent_run_id)
                .expect("load recovered run")
                .expect("recovered run exists");
            assert_eq!(agent_run.status, AgentRunStatus::Failed);
            assert_eq!(agent_run.error.as_ref().unwrap().code, "interrupted");
            assert!(!runtime.read(cx).is_running(&conversation_id));
        });
    }

    #[gpui::test]
    fn init_recovers_persisted_waiting_approval_runs(cx: &mut gpui::TestAppContext) {
        let _dir = init_runtime_test(cx);
        let (conversation_id, agent_run_id, approval_id) = cx.update(|cx| {
            let repository = test_repository(cx);
            let conversation_id = insert_conversation_with_user_item(&repository);
            let agent_run_id = insert_agent_run(&repository, &conversation_id);
            let approval_id = insert_approval_for_run(&repository, &agent_run_id);
            (conversation_id, agent_run_id, approval_id)
        });

        let runtime = cx.update(|cx| create(cx).expect("initialize conversation runtime"));
        cx.run_until_parked();
        cx.update(|cx| {
            let repository = test_repository(cx);
            let agent_run = repository
                .get_agent_run(&agent_run_id)
                .expect("load recovered run")
                .expect("recovered run exists");
            assert_eq!(agent_run.status, AgentRunStatus::Failed);
            assert_eq!(agent_run.error.as_ref().unwrap().code, "interrupted");
            assert!(!runtime.read(cx).is_running(&conversation_id));

            let invocation = repository
                .get_tool_invocation(&approval_id)
                .unwrap()
                .unwrap();
            assert_eq!(invocation.status, ToolInvocationStatus::Failed);
            assert_eq!(invocation.error.as_ref().unwrap().code, "interrupted");
            assert_eq!(
                invocation.approval.as_ref().map(|approval| approval.status),
                Some(ApprovalStatus::Canceled)
            );
            assert!(
                invocation
                    .approval
                    .as_ref()
                    .is_some_and(|approval| approval.decision.is_none())
            );
            assert_eq!(
                tool_result_texts(&repository, &conversation_id),
                vec!["agent run was interrupted before reaching a terminal state".to_string()]
            );
        });
    }

    #[gpui::test]
    fn stop_run_keeps_conversation_gated_until_cleanup_finishes(cx: &mut gpui::TestAppContext) {
        let _dir = init_runtime_test(cx);
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new_ready_for_test()));
        let recorder = cx.update(|cx| cx.new(|cx| RuntimeEventRecorder::new(store.clone(), cx)));
        let cancellation_token = AgentCancellationToken::new();
        let (conversation_id, agent_run_id) = cx.update(|cx| {
            let repository = test_repository(cx);
            let conversation_id = insert_conversation_with_user_item(&repository);
            let agent_run_id = insert_agent_run(&repository, &conversation_id);
            (conversation_id, agent_run_id)
        });

        cx.update(|cx| {
            store.update(cx, |store, cx| {
                store.active_runs.insert(
                    conversation_id.clone(),
                    ActiveRun {
                        agent_run_id: Some(agent_run_id.clone()),
                        ..active_run_with_token(ActiveRunKey(0), cancellation_token.clone())
                    },
                );
                store
                    .last_errors
                    .insert(conversation_id.clone(), "runtime canceled".to_string());

                assert!(store.stop_run(&conversation_id, cx));
                assert!(!store.stop_run(&conversation_id, cx));
                assert!(!store.finish_run(
                    conversation_id.clone(),
                    ActiveRunKey(0),
                    Err("stale run completion".to_string()),
                    cx,
                ));
                assert_eq!(
                    store.run_status(&conversation_id),
                    ConversationRunStatus::Stopping
                );
                assert!(store.is_running(&conversation_id));
                assert!(store.take_last_error(&conversation_id).is_none());
            });
        });

        assert!(cancellation_token.is_cancelled());
        cx.update(|cx| {
            assert!(recorder.read(cx).events.lock().unwrap().is_empty());
        });
        cx.run_until_parked();

        cx.update(|cx| {
            let repository = test_repository(cx);
            let run = repository.get_agent_run(&agent_run_id).unwrap().unwrap();
            assert_eq!(run.status, AgentRunStatus::Canceled);
            assert!(run.error.is_none());
            assert_eq!(
                store.read(cx).run_status(&conversation_id),
                ConversationRunStatus::Idle
            );
            let events = recorder.read(cx).events.lock().unwrap().clone();
            assert!(events.contains(&ConversationRuntimeEvent::RunFinished {
                conversation_id: conversation_id.clone(),
            }));
        });
    }

    #[gpui::test]
    fn finish_run_records_uncanceled_error(cx: &mut gpui::TestAppContext) {
        let _dir = init_runtime_test(cx);
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new_ready_for_test()));
        let conversation_id = "conversation-1".to_string();

        cx.update(|cx| {
            store.update(cx, |store, cx| {
                store
                    .active_runs
                    .insert(conversation_id.clone(), active_run(ActiveRunKey(0)));
                store.finish_run(
                    conversation_id.clone(),
                    ActiveRunKey(0),
                    Err("provider failed".to_string()),
                    cx,
                );

                assert_eq!(
                    store.take_last_error(&conversation_id).as_deref(),
                    Some("provider failed")
                );
            });
        });
    }

    #[gpui::test]
    fn finish_run_removes_matching_active_run(cx: &mut gpui::TestAppContext) {
        let _dir = init_runtime_test(cx);
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new_ready_for_test()));
        let conversation_id = "conversation-1".to_string();
        let agent_run = jaco_db::AgentRunRecord {
            id: "run-1".to_string(),
            conversation_id: conversation_id.clone(),
            trigger_entry_id: "user-1".to_string(),
            trigger_kind: AgentRunTriggerKind::User,
            status: AgentRunStatus::Completed,
            input: AgentRunInput {
                prompt_snapshot: None,
                provider_id: "provider".to_string(),
                model_id: "model".to_string(),
                settings_snapshot: run_settings(),
                runtime_snapshot: AgentRuntimeSnapshot {
                    engine: AgentEngineKind::Rig,
                    engine_version: "test".to_string(),
                    skill_catalog_hash: None,
                    tool_name_strategy: ToolNameStrategy::Direct,
                },
                max_steps: 8,
            },
            output: None,
            error: None,
            created_at: time::OffsetDateTime::now_utc(),
            started_at: Some(time::OffsetDateTime::now_utc()),
            completed_at: Some(time::OffsetDateTime::now_utc()),
            updated_at: time::OffsetDateTime::now_utc(),
        };

        cx.update(|cx| {
            store.update(cx, |store, cx| {
                store
                    .active_runs
                    .insert(conversation_id.clone(), active_run(ActiveRunKey(0)));
                assert!(store.finish_run(
                    conversation_id.clone(),
                    ActiveRunKey(0),
                    Ok(AgentRunHandle {
                        agent_run,
                        output: None,
                        status: AgentRunHandleStatus::Finished,
                        events: Vec::new(),
                        steps: Vec::new(),
                    }),
                    cx
                ));
                assert!(!store.active_runs.contains_key(&conversation_id));
            });
        });
    }

    #[gpui::test]
    fn app_owned_completion_does_not_require_window(cx: &mut gpui::TestAppContext) {
        let _dir = init_runtime_test(cx);
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new_ready_for_test()));
        let conversation_id = "conversation-1".to_string();

        let completion = cx.update(|cx| {
            store.update(cx, |store, _cx| {
                store
                    .active_runs
                    .insert(conversation_id.clone(), active_run(ActiveRunKey(0)));
            });
            let store = store.downgrade();
            let conversation_id = conversation_id.clone();
            cx.spawn(async move |cx| {
                finish_run_from_task(
                    store,
                    conversation_id,
                    ActiveRunKey(0),
                    Err("provider failed".to_string()),
                    cx,
                );
            })
        });

        cx.run_until_parked();
        drop(completion);
        cx.update(|cx| {
            store.update(cx, |store, _cx| {
                assert!(!store.active_runs.contains_key(&conversation_id));
                assert_eq!(
                    store.take_last_error(&conversation_id).as_deref(),
                    Some("provider failed")
                );
            });
        });
    }

    #[gpui::test]
    fn deny_tool_invocation_resolves_matching_pending_approval(cx: &mut gpui::TestAppContext) {
        let _dir = init_runtime_test(cx);
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new_ready_for_test()));
        let (conversation_id, agent_run_id, approval_id) = cx.update(|cx| {
            let repository = test_repository(cx);
            let conversation_id = insert_conversation_with_user_item(&repository);
            let agent_run_id = insert_agent_run(&repository, &conversation_id);
            let approval_id = insert_approval_for_run(&repository, &agent_run_id);
            (conversation_id, agent_run_id, approval_id)
        });

        cx.update(|cx| {
            store.update(cx, |store, cx| {
                let active = active_run_with_agent_id(ActiveRunKey(0), agent_run_id.clone());
                let mut receiver = active.approval_broker.register_pending_for_test(
                    conversation_id.clone(),
                    agent_run_id.clone(),
                    approval_id.clone(),
                );
                store.active_runs.insert(conversation_id.clone(), active);

                assert!(store.deny_tool_invocation(
                    conversation_id.clone(),
                    approval_id.clone(),
                    cx
                ));
                assert!(store.active_runs.contains_key(&conversation_id));
                assert_eq!(
                    receiver.try_recv().unwrap(),
                    ToolApprovalDecision::Denied {
                        decided_by: "user".to_string(),
                        reason: None,
                    }
                );
            });
        });
    }

    #[gpui::test]
    fn approve_tool_invocation_without_active_waiting_run_is_ignored(
        cx: &mut gpui::TestAppContext,
    ) {
        let _dir = init_runtime_test(cx);
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new_ready_for_test()));
        let window = open_runtime_test_window(cx);
        let (conversation_id, agent_run_id, approval_id) = cx.update(|cx| {
            let repository = test_repository(cx);
            let conversation_id = insert_conversation_with_user_item(&repository);
            let agent_run_id = insert_agent_run(&repository, &conversation_id);
            let approval_id = insert_approval_for_run(&repository, &agent_run_id);
            (conversation_id, agent_run_id, approval_id)
        });

        let approved = cx.update(|cx| {
            window
                .update(cx, |_view, window, cx| {
                    store.update(cx, |store, cx| {
                        store.approve_tool_invocation(
                            conversation_id.clone(),
                            approval_id.clone(),
                            window,
                            cx,
                        )
                    })
                })
                .unwrap()
        });
        assert!(!approved);

        cx.update(|cx| {
            let repository = test_repository(cx);
            let invocation = repository
                .get_tool_invocation(&approval_id)
                .unwrap()
                .unwrap();
            assert_eq!(invocation.status, ToolInvocationStatus::AwaitingApproval);
            assert_eq!(
                invocation.approval.as_ref().map(|approval| approval.status),
                Some(ApprovalStatus::Pending)
            );
            let agent_run = repository.get_agent_run(&agent_run_id).unwrap().unwrap();
            assert_eq!(agent_run.status, AgentRunStatus::Running);
            assert!(!store.read(cx).is_running(&conversation_id));
            store.update(cx, |store, _cx| {
                assert!(store.take_last_error(&conversation_id).is_none());
            });
        });
    }

    #[gpui::test]
    fn deny_tool_invocation_without_active_waiting_run_is_ignored(cx: &mut gpui::TestAppContext) {
        let _dir = init_runtime_test(cx);
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new_ready_for_test()));
        let (conversation_id, agent_run_id, approval_id) = cx.update(|cx| {
            let repository = test_repository(cx);
            let conversation_id = insert_conversation_with_user_item(&repository);
            let agent_run_id = insert_agent_run(&repository, &conversation_id);
            let approval_id = insert_approval_for_run(&repository, &agent_run_id);
            (conversation_id, agent_run_id, approval_id)
        });

        cx.update(|cx| {
            store.update(cx, |store, cx| {
                assert!(!store.deny_tool_invocation(
                    conversation_id.clone(),
                    approval_id.clone(),
                    cx
                ));
                assert!(!store.active_runs.contains_key(&conversation_id));
                assert!(store.take_last_error(&conversation_id).is_none());
            });
        });

        cx.update(|cx| {
            let repository = test_repository(cx);
            let invocation = repository
                .get_tool_invocation(&approval_id)
                .unwrap()
                .unwrap();
            assert_eq!(invocation.status, ToolInvocationStatus::AwaitingApproval);
            assert_eq!(
                invocation.approval.as_ref().map(|approval| approval.status),
                Some(ApprovalStatus::Pending)
            );
            let agent_run = repository.get_agent_run(&agent_run_id).unwrap().unwrap();
            assert_eq!(agent_run.status, AgentRunStatus::Running);
        });
    }

    #[gpui::test]
    fn deny_tool_invocation_ignores_stale_action_without_pending_broker(
        cx: &mut gpui::TestAppContext,
    ) {
        let _dir = init_runtime_test(cx);
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new_ready_for_test()));
        let (conversation_id, agent_run_id, approval_id) = cx.update(|cx| {
            let repository = test_repository(cx);
            let conversation_id = insert_conversation_with_user_item(&repository);
            let agent_run_id = insert_agent_run(&repository, &conversation_id);
            let approval_id = insert_approval_for_run(&repository, &agent_run_id);
            (conversation_id, agent_run_id, approval_id)
        });

        cx.update(|cx| {
            store.update(cx, |store, cx| {
                store.active_runs.insert(
                    conversation_id.clone(),
                    active_run_with_agent_id(ActiveRunKey(0), agent_run_id.clone()),
                );

                assert!(!store.deny_tool_invocation(
                    conversation_id.clone(),
                    approval_id.clone(),
                    cx
                ));
                let active = store
                    .active_runs
                    .get(&conversation_id)
                    .expect("stale denial must not clear the active run");
                assert_eq!(active.agent_run_id.as_ref(), Some(&agent_run_id));
                assert!(store.take_last_error(&conversation_id).is_none());
            });
        });
    }

    #[gpui::test]
    fn finish_run_ignores_stale_run_key(cx: &mut gpui::TestAppContext) {
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new_ready_for_test()));
        let conversation_id = "conversation-1".to_string();

        cx.update(|cx| {
            store.update(cx, |store, cx| {
                store
                    .active_runs
                    .insert(conversation_id.clone(), active_run(ActiveRunKey(2)));

                assert!(!store.finish_run(
                    conversation_id.clone(),
                    ActiveRunKey(1),
                    Err("old run failed".to_string()),
                    cx
                ));
                assert!(store.active_runs.contains_key(&conversation_id));
                assert!(store.take_last_error(&conversation_id).is_none());
            });
        });
    }

    fn init_runtime_test(cx: &mut gpui::TestAppContext) -> TempDir {
        let dir = tempdir().unwrap();
        cx.update(|cx| {
            database::install_for_test(cx, dir.path());
            crate::features::conversation::resources::ConversationResourcesStore::install_global(
                cx,
                crate::features::conversation::resources::ConversationResourcesState::AwaitingDatabase,
            );
        });
        dir
    }

    fn test_repository(cx: &App) -> FreshRepository {
        database::with_ready_repository(cx, |repository| Ok(repository.clone())).unwrap()
    }

    fn open_runtime_test_window(cx: &mut gpui::TestAppContext) -> WindowHandle<TestView> {
        cx.update(|cx| {
            cx.open_window(Default::default(), |_window, cx| cx.new(|_| TestView))
                .expect("open runtime test window")
        })
    }

    struct TestView;

    impl gpui::Render for TestView {
        fn render(
            &mut self,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<Self>,
        ) -> impl gpui::IntoElement {
            gpui::div()
        }
    }

    fn insert_conversation_with_user_item(repository: &FreshRepository) -> ConversationId {
        let project = repository
            .insert_project(NewProject {
                path: "/tmp/jaco-runtime-test".to_string(),
                display_name: "Runtime Test".to_string(),
                kind: ProjectKind::Normal,
                pinned: false,
                removed: false,
                metadata: ProjectMetadata {
                    scratch_reason: None,
                    git_root: None,
                    last_active_conversation_id: None,
                },
            })
            .unwrap();
        let conversation = repository
            .insert_conversation(NewConversation {
                project_id: project.id,
                title: "Runtime Test".to_string(),
                pinned: false,
                prompt_id: None,
                default_provider_id: None,
                default_model_id: None,
                metadata: ConversationMetadata {
                    summary: None,
                    tags: Vec::new(),
                },
                settings_snapshot: conversation_settings(),
            })
            .unwrap();
        repository
            .append_conversation_entry(NewConversationEntry {
                conversation_id: conversation.id.clone(),
                status: ConversationEntryStatus::Completed,
                agent_run_id: None,
                provider_step_id: None,
                tool_invocation_id: None,
                provider_item_id: None,
                payload: ConversationEntryPayload::Message {
                    role: TranscriptRole::User,
                    content: vec![ContentPart::Text {
                        text: "hello".to_string(),
                    }],
                },
            })
            .unwrap();
        conversation.id
    }

    fn insert_agent_run(
        repository: &FreshRepository,
        conversation_id: &ConversationId,
    ) -> AgentRunId {
        let trigger_entry_id = repository
            .conversation_entries(conversation_id)
            .unwrap()
            .last()
            .unwrap()
            .id
            .clone();
        repository
            .insert_agent_run(NewAgentRun {
                conversation_id: conversation_id.to_string(),
                trigger_entry_id: trigger_entry_id.clone(),
                trigger_kind: AgentRunTriggerKind::User,
                input: AgentRunInput {
                    prompt_snapshot: None,
                    provider_id: "provider".to_string(),
                    model_id: "model".to_string(),
                    settings_snapshot: run_settings(),
                    runtime_snapshot: AgentRuntimeSnapshot {
                        engine: AgentEngineKind::Rig,
                        engine_version: "test".to_string(),
                        skill_catalog_hash: None,
                        tool_name_strategy: ToolNameStrategy::Direct,
                    },
                    max_steps: 8,
                },
            })
            .unwrap()
            .id
    }

    fn insert_approval_for_run(
        repository: &FreshRepository,
        agent_run_id: &AgentRunId,
    ) -> ToolInvocationId {
        let invocation = repository
            .insert_tool_invocation(NewToolInvocation {
                agent_run_id: agent_run_id.clone(),
                provider_step_id: None,
                status: ToolInvocationStatus::AwaitingApproval,
                input: ToolInvocationInput {
                    source: ToolSource::Local,
                    namespace: None,
                    tool_name: "echo".to_string(),
                    runtime_tool_name: "echo".to_string(),
                    call_id: "call-approval".to_string(),
                    arguments: ToolArguments {
                        value: serde_json::json!({"text": "hi"}),
                    },
                    approval_policy: ToolApprovalPolicy::OnRequest,
                    execution_policy: ToolExecutionPolicy::Foreground,
                },
                output: None,
                error: None,
            })
            .unwrap();
        let invocation = repository
            .request_tool_invocation_approval(
                &invocation.id,
                NewToolInvocationApproval {
                    request: ApprovalRequestPayload {
                        reason: "approve echo".to_string(),
                        tool_source: ToolSource::Local,
                        tool_name: "echo".to_string(),
                        arguments_preview: "{\"text\":\"hi\"}".to_string(),
                        access_requests: Vec::new(),
                    },
                    expires_at: None,
                },
            )
            .unwrap();
        invocation.id
    }

    fn tool_result_texts(
        repository: &FreshRepository,
        conversation_id: &ConversationId,
    ) -> Vec<String> {
        repository
            .conversation_entries(conversation_id)
            .unwrap()
            .into_iter()
            .filter_map(|item| match item.payload {
                ConversationEntryPayload::ToolResult(result) => Some(result.content),
                _ => None,
            })
            .flatten()
            .filter_map(|part| match part {
                ContentPart::Text { text } => Some(text),
                _ => None,
            })
            .collect()
    }

    fn conversation_settings() -> ConversationSettingsSnapshot {
        ConversationSettingsSnapshot {
            prompt: None,
            provider_id: Some("provider".to_string()),
            model_id: Some("model".to_string()),
            model_capabilities: Some(conservative_model_capabilities("openai")),
            tool_policy: tool_policy(),
        }
    }

    fn run_settings() -> jaco_core::RunSettingsSnapshot {
        jaco_core::RunSettingsSnapshot {
            prompt: None,
            provider_id: "provider".to_string(),
            model_id: "model".to_string(),
            model_capabilities: conservative_model_capabilities("openai"),
            provider_settings: ProviderSettingsPayload {
                provider_kind: "openai".to_string(),
                fields: Vec::new(),
            },
            reasoning_selection: None,
            tool_policy: tool_policy(),
        }
    }

    fn tool_policy() -> ToolPolicySnapshot {
        ToolPolicySnapshot {
            approval_policy: ToolApprovalPolicy::Never,
            enabled_sources: vec![ToolSource::Local],
            max_steps: 8,
            approval_mode: ToolApprovalMode::RequestApproval,
            permission_scope: None,
        }
    }
}

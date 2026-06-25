use std::collections::HashMap;

use ai_chat_agent::{
    AgentCancellationToken, AgentRunHandle, AgentRunHandleStatus, AgentRunRequest, AgentRuntime,
    AgentRuntimeObserver, RuntimeGuards,
};
use ai_chat_core::{AgentRunId, AgentRunStatus, ConversationId, ToolInvocationId};
use ai_chat_db::FreshRepository;
use ai_chat_db::ToolInvocationApprovalOutcome;
use gpui::{App, AppContext, AsyncWindowContext, Context, Entity, EventEmitter, Global, Task};
use smol::channel::{Receiver, Sender};
use tracing::{Level, event};

use crate::{database, errors::AiChat2Result, state::provider_secrets::ProviderSecretStore};

#[derive(Clone)]
pub(crate) struct ConversationRuntimeGlobal(Entity<ConversationRuntimeStore>);

impl ConversationRuntimeGlobal {
    pub(crate) fn entity(&self) -> Entity<ConversationRuntimeStore> {
        self.0.clone()
    }
}

impl Global for ConversationRuntimeGlobal {}

pub(crate) struct ConversationRuntimeStore {
    active_runs: HashMap<ConversationId, ActiveRun>,
    last_errors: HashMap<ConversationId, String>,
    next_run_key: u64,
}

struct ActiveRun {
    key: ActiveRunKey,
    agent_run_id: Option<AgentRunId>,
    phase: ActiveRunPhase,
    cancellation_token: AgentCancellationToken,
    run_task: Option<Task<()>>,
    _event_task: Task<()>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ActiveRunPhase {
    Running,
    WaitingForApproval,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ActiveRunKey(u64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ConversationRuntimeEvent {
    RunStarted { conversation_id: ConversationId },
    ConversationChanged { conversation_id: ConversationId },
    RunFinished { conversation_id: ConversationId },
}

impl EventEmitter<ConversationRuntimeEvent> for ConversationRuntimeStore {}

impl ConversationRuntimeStore {
    fn new() -> Self {
        Self {
            active_runs: HashMap::new(),
            last_errors: HashMap::new(),
            next_run_key: 0,
        }
    }

    pub(crate) fn is_running(&self, conversation_id: &ConversationId) -> bool {
        self.active_runs.contains_key(conversation_id)
    }

    pub(crate) fn take_last_error(&mut self, conversation_id: &ConversationId) -> Option<String> {
        self.last_errors.remove(conversation_id)
    }

    pub(crate) fn stop_run(
        &mut self,
        conversation_id: &ConversationId,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(mut active) = self.active_runs.remove(conversation_id) else {
            return false;
        };

        active.cancellation_token.cancel();
        active.run_task.take();
        let repository = database::repository(cx);
        if let Err(err) = AgentRuntime::new(repository)
            .cancel_non_terminal_runs_for_conversation(conversation_id, None)
        {
            event!(
                Level::ERROR,
                error = ?err,
                conversation_id = %conversation_id,
                agent_run_id = ?active.agent_run_id,
                "cancel active conversation runs failed"
            );
        }

        self.last_errors.remove(conversation_id);
        cx.emit(ConversationRuntimeEvent::ConversationChanged {
            conversation_id: conversation_id.clone(),
        });
        cx.emit(ConversationRuntimeEvent::RunFinished {
            conversation_id: conversation_id.clone(),
        });
        cx.notify();
        true
    }

    pub(crate) fn start_run(
        &mut self,
        request: AgentRunRequest,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let conversation_id = request.conversation_id.clone();
        if self.active_runs.contains_key(&conversation_id) {
            return false;
        }

        self.last_errors.remove(&conversation_id);
        let run_key = self.next_active_run_key();
        let repository = database::repository(cx);
        let (tx, rx) = smol::channel::unbounded();
        let event_task = self.spawn_event_listener(rx, cx);
        let store = cx.entity().downgrade();
        let run_conversation_id = conversation_id.clone();
        let cancellation_token = request.cancellation_token.clone();
        let run_task = window.spawn(cx, async move |cx| {
            let result = run_agent_with_saved_provider(repository, request, tx, cx).await;
            if let Err(err) = store.update_in(cx, |store, _window, cx| {
                store.finish_run(run_conversation_id.clone(), run_key, result, cx);
            }) {
                event!(Level::ERROR, error = ?err, "finish conversation agent run failed");
            }
        });

        self.active_runs.insert(
            conversation_id.clone(),
            ActiveRun {
                key: run_key,
                agent_run_id: None,
                phase: ActiveRunPhase::Running,
                cancellation_token,
                run_task: Some(run_task),
                _event_task: event_task,
            },
        );
        cx.emit(ConversationRuntimeEvent::RunStarted {
            conversation_id: conversation_id.clone(),
        });
        cx.emit(ConversationRuntimeEvent::ConversationChanged { conversation_id });
        cx.notify();
        true
    }

    pub(crate) fn approve_tool_invocation(
        &mut self,
        conversation_id: ConversationId,
        tool_invocation_id: ToolInvocationId,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.last_errors.remove(&conversation_id);
        let repository = database::repository(cx);
        let agent_run_id = match agent_run_id_for_tool_invocation(&repository, &tool_invocation_id)
        {
            Ok(agent_run_id) => agent_run_id,
            Err(err) => {
                self.last_errors
                    .insert(conversation_id.clone(), err.to_string());
                cx.emit(ConversationRuntimeEvent::ConversationChanged { conversation_id });
                cx.notify();
                return false;
            }
        };
        if !self.active_waiting_approval_matches(&conversation_id, &agent_run_id) {
            return false;
        }

        let run_key = self.next_active_run_key();
        let (tx, rx) = smol::channel::unbounded();
        let event_task = self.spawn_event_listener(rx, cx);
        let store = cx.entity().downgrade();
        let run_conversation_id = conversation_id.clone();
        let cancellation_token = AgentCancellationToken::new();
        let active_cancellation_token = cancellation_token.clone();
        let run_task = window.spawn(cx, async move |cx| {
            let result = approve_tool_with_runtime(
                repository,
                tool_invocation_id,
                cancellation_token,
                tx,
                cx,
            )
            .await;
            if let Err(err) = store.update_in(cx, |store, _window, cx| {
                store.finish_run(run_conversation_id.clone(), run_key, result, cx);
            }) {
                event!(Level::ERROR, error = ?err, "finish approved tool run failed");
            }
        });

        self.active_runs.insert(
            conversation_id.clone(),
            ActiveRun {
                key: run_key,
                agent_run_id: Some(agent_run_id),
                phase: ActiveRunPhase::Running,
                cancellation_token: active_cancellation_token,
                run_task: Some(run_task),
                _event_task: event_task,
            },
        );
        cx.emit(ConversationRuntimeEvent::RunStarted {
            conversation_id: conversation_id.clone(),
        });
        cx.emit(ConversationRuntimeEvent::ConversationChanged { conversation_id });
        cx.notify();
        true
    }

    pub(crate) fn deny_tool_invocation(
        &mut self,
        conversation_id: ConversationId,
        tool_invocation_id: ToolInvocationId,
        cx: &mut Context<Self>,
    ) -> bool {
        self.last_errors.remove(&conversation_id);
        let repository = database::repository(cx);
        let agent_run_id = match agent_run_id_for_tool_invocation(&repository, &tool_invocation_id)
        {
            Ok(agent_run_id) => agent_run_id,
            Err(err) => {
                self.last_errors
                    .insert(conversation_id.clone(), err.to_string());
                cx.emit(ConversationRuntimeEvent::ConversationChanged { conversation_id });
                cx.notify();
                return false;
            }
        };
        if !self.active_waiting_approval_matches(&conversation_id, &agent_run_id) {
            return false;
        }

        let result = AgentRuntime::new(repository).decide_approval(
            &tool_invocation_id,
            ToolInvocationApprovalOutcome::Denied {
                decided_by: "user".to_string(),
                reason: None,
            },
        );
        if let Err(err) = result {
            self.last_errors
                .insert(conversation_id.clone(), err.to_string());
            cx.emit(ConversationRuntimeEvent::ConversationChanged { conversation_id });
            cx.notify();
            return false;
        }
        self.active_runs.remove(&conversation_id);
        cx.emit(ConversationRuntimeEvent::ConversationChanged {
            conversation_id: conversation_id.clone(),
        });
        cx.emit(ConversationRuntimeEvent::RunFinished { conversation_id });
        cx.notify();
        true
    }

    fn next_active_run_key(&mut self) -> ActiveRunKey {
        let key = ActiveRunKey(self.next_run_key);
        self.next_run_key = self.next_run_key.wrapping_add(1);
        key
    }

    fn active_waiting_approval_matches(
        &self,
        conversation_id: &ConversationId,
        agent_run_id: &AgentRunId,
    ) -> bool {
        self.active_runs.get(conversation_id).is_some_and(|active| {
            active.agent_run_id.as_ref() == Some(agent_run_id)
                && active.phase == ActiveRunPhase::WaitingForApproval
        })
    }

    fn spawn_event_listener(
        &self,
        rx: Receiver<ai_chat_agent::AgentRuntimeEvent>,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        cx.spawn(async move |this, cx| {
            while let Ok(runtime_event) = rx.recv().await {
                let Some(this) = this.upgrade() else {
                    break;
                };
                this.update(cx, |store, cx| {
                    store.handle_runtime_event(runtime_event, cx);
                });
            }
        })
    }

    fn handle_runtime_event(
        &mut self,
        runtime_event: ai_chat_agent::AgentRuntimeEvent,
        cx: &mut Context<Self>,
    ) {
        match runtime_event {
            ai_chat_agent::AgentRuntimeEvent::AgentRunStarted {
                agent_run_id,
                conversation_id,
            } => {
                let Some(active) = self.active_runs.get_mut(&conversation_id) else {
                    return;
                };
                active.agent_run_id = Some(agent_run_id);
                cx.emit(ConversationRuntimeEvent::RunStarted {
                    conversation_id: conversation_id.clone(),
                });
                cx.emit(ConversationRuntimeEvent::ConversationChanged { conversation_id });
            }
            ai_chat_agent::AgentRuntimeEvent::AgentRunStatusChanged {
                agent_run_id,
                status,
            } => {
                if let Some(conversation_id) = self.conversation_id_for_agent_run(&agent_run_id) {
                    if is_terminal_status(status) {
                        cx.emit(ConversationRuntimeEvent::RunFinished {
                            conversation_id: conversation_id.clone(),
                        });
                    }
                    cx.emit(ConversationRuntimeEvent::ConversationChanged { conversation_id });
                }
            }
            ai_chat_agent::AgentRuntimeEvent::ConversationItemAppended {
                conversation_id, ..
            }
            | ai_chat_agent::AgentRuntimeEvent::ConversationItemUpdated {
                conversation_id, ..
            } => {
                cx.emit(ConversationRuntimeEvent::ConversationChanged { conversation_id });
            }
            ai_chat_agent::AgentRuntimeEvent::ProviderStepChanged { agent_run_id, .. }
            | ai_chat_agent::AgentRuntimeEvent::ToolInvocationChanged { agent_run_id, .. } => {
                if let Some(conversation_id) = self.conversation_id_for_agent_run(&agent_run_id) {
                    cx.emit(ConversationRuntimeEvent::ConversationChanged { conversation_id });
                }
            }
        }
        cx.notify();
    }

    fn conversation_id_for_agent_run(&self, agent_run_id: &AgentRunId) -> Option<ConversationId> {
        self.active_runs
            .iter()
            .find(|(_, active)| active.agent_run_id.as_ref() == Some(agent_run_id))
            .map(|(conversation_id, _)| conversation_id.clone())
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
        if active.key != run_key {
            return false;
        }

        if let Ok(AgentRunHandle {
            agent_run,
            status:
                AgentRunHandleStatus::WaitingForApproval {
                    tool_invocation_id: _,
                },
            ..
        }) = &result
        {
            if let Some(active) = self.active_runs.get_mut(&conversation_id) {
                active.agent_run_id = Some(agent_run.id.clone());
                active.phase = ActiveRunPhase::WaitingForApproval;
            }
            cx.emit(ConversationRuntimeEvent::ConversationChanged { conversation_id });
            cx.notify();
            return true;
        }

        self.active_runs.remove(&conversation_id);
        if let Err(err) = result {
            self.last_errors.insert(conversation_id.clone(), err);
        }
        cx.emit(ConversationRuntimeEvent::ConversationChanged {
            conversation_id: conversation_id.clone(),
        });
        cx.emit(ConversationRuntimeEvent::RunFinished { conversation_id });
        cx.notify();
        true
    }
}

pub(crate) fn init(cx: &mut App) -> AiChat2Result<()> {
    let recovered = AgentRuntime::new(database::repository(cx)).recover_interrupted_runs()?;
    if !recovered.is_empty() {
        event!(
            Level::WARN,
            recovered_count = recovered.len(),
            "recovered interrupted ai-chat2 agent runs"
        );
    }
    let store = cx.new(|_| ConversationRuntimeStore::new());
    cx.set_global(ConversationRuntimeGlobal(store));
    Ok(())
}

pub(crate) fn runtime(cx: &App) -> Entity<ConversationRuntimeStore> {
    cx.global::<ConversationRuntimeGlobal>().entity()
}

async fn run_agent_with_saved_provider(
    repository: FreshRepository,
    request: AgentRunRequest,
    tx: Sender<ai_chat_agent::AgentRuntimeEvent>,
    cx: &mut AsyncWindowContext,
) -> Result<AgentRunHandle, String> {
    let observer = AgentRuntimeObserver::new(move |event| {
        if let Err(err) = tx.send_blocking(event) {
            event!(Level::ERROR, error = ?err, "send conversation runtime event failed");
        }
    });
    let runtime = AgentRuntime::new(repository.clone());
    let mut request = match crate::state::mcp::prepare_run_request(request, cx).await {
        Ok(prepared) => prepared.request,
        Err(err) => {
            return gpui_tokio::Tokio::spawn(cx, async move {
                runtime.record_setup_failed_run(err.request, err.message, Some(&observer))
            })
            .await
            .map_err(|err| err.to_string())?
            .map_err(|err| err.to_string());
        }
    };
    let agent_run = runtime
        .begin_run(&mut request, Some(&observer))
        .map_err(|err| err.to_string())?;
    let provider = match repository
        .get_provider(&request.provider_id)
        .map_err(|err| err.to_string())?
    {
        Some(provider) => provider,
        None => {
            let message = format!("provider `{}` was not found", request.provider_id);
            return gpui_tokio::Tokio::spawn(cx, async move {
                runtime.record_setup_failed_started_run(&agent_run, message, Some(&observer))
            })
            .await
            .map_err(|err| err.to_string())?
            .map_err(|err| err.to_string());
        }
    };
    let secrets = match ProviderSecretStore::read_values(cx, &provider.secret_refs).await {
        Ok(secrets) => secrets,
        Err(err) => {
            return gpui_tokio::Tokio::spawn(cx, async move {
                runtime.record_setup_failed_started_run(&agent_run, err, Some(&observer))
            })
            .await
            .map_err(|err| err.to_string())?
            .map_err(|err| err.to_string());
        }
    };
    gpui_tokio::Tokio::spawn(cx, async move {
        runtime
            .run_started_with_saved_provider_observed(
                agent_run,
                request,
                provider,
                secrets,
                Some(observer),
            )
            .await
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(|err| err.to_string())
}

async fn approve_tool_with_runtime(
    repository: FreshRepository,
    tool_invocation_id: ToolInvocationId,
    cancellation_token: AgentCancellationToken,
    tx: Sender<ai_chat_agent::AgentRuntimeEvent>,
    cx: &mut AsyncWindowContext,
) -> Result<AgentRunHandle, String> {
    let approval_tx = tx.clone();
    let observer = AgentRuntimeObserver::new(move |event| {
        if let Err(err) = approval_tx.send_blocking(event) {
            event!(Level::ERROR, error = ?err, "send conversation runtime event failed");
        }
    });
    let runtime = AgentRuntime::new(repository.clone());
    let approval_cancellation_token = cancellation_token.clone();
    let approval_handle = gpui_tokio::Tokio::spawn(cx, async move {
        runtime
            .approve_and_resume_tool(
                &tool_invocation_id,
                "user".to_string(),
                None,
                RuntimeGuards::default().tool_timeout,
                approval_cancellation_token,
                Some(&observer),
            )
            .await
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(|err| err.to_string())?;

    if approval_handle.output.stopped_reason != ai_chat_core::AgentStoppedReason::Completed {
        return Ok(AgentRunHandle {
            agent_run: approval_handle.agent_run,
            output: Some(approval_handle.output),
            status: AgentRunHandleStatus::Finished,
            events: approval_handle.events,
            steps: approval_handle.steps,
        });
    }

    if cancellation_token.is_cancelled() {
        return Ok(AgentRunHandle {
            agent_run: approval_handle.agent_run,
            output: Some(approval_handle.output),
            status: AgentRunHandleStatus::Finished,
            events: approval_handle.events,
            steps: approval_handle.steps,
        });
    }

    let mut resume_request = resume_request_after_approval(&repository, &approval_handle.agent_run)
        .map_err(|err| err.to_string())?;
    resume_request.cancellation_token = cancellation_token;
    run_agent_with_saved_provider(repository, resume_request, tx, cx).await
}

fn is_terminal_status(status: AgentRunStatus) -> bool {
    matches!(
        status,
        AgentRunStatus::Completed | AgentRunStatus::Failed | AgentRunStatus::Canceled
    )
}

fn agent_run_id_for_tool_invocation(
    repository: &FreshRepository,
    tool_invocation_id: &ToolInvocationId,
) -> ai_chat_db::Result<AgentRunId> {
    let invocation = repository
        .get_tool_invocation(tool_invocation_id)?
        .ok_or_else(|| {
            ai_chat_db::DbError::Invariant(format!(
                "tool invocation {tool_invocation_id} is missing"
            ))
        })?;
    Ok(invocation.agent_run_id)
}

fn resume_request_after_approval(
    repository: &FreshRepository,
    parent_run: &ai_chat_db::AgentRunRecord,
) -> ai_chat_db::Result<AgentRunRequest> {
    let conversation = repository
        .get_conversation(&parent_run.conversation_id)?
        .ok_or_else(|| {
            ai_chat_db::DbError::Invariant(format!(
                "conversation {} is missing",
                parent_run.conversation_id
            ))
        })?;
    let project = repository
        .get_project(&conversation.project_id)?
        .ok_or_else(|| {
            ai_chat_db::DbError::Invariant(format!(
                "project {} is missing",
                conversation.project_id
            ))
        })?;
    let mut request = AgentRunRequest::new(
        parent_run.conversation_id.clone(),
        parent_run.input.user_item_id.clone(),
        parent_run.input.provider_id.clone(),
        parent_run.input.model_id.clone(),
        parent_run.input.settings_snapshot.clone(),
        parent_run.input.runtime_snapshot.clone(),
    );
    request.trigger_kind = ai_chat_core::AgentRunTriggerKind::Resume;
    request.parent_agent_run_id = Some(parent_run.id.clone());
    request.prompt_snapshot = parent_run.input.prompt_snapshot.clone();
    request.project_root = Some(std::path::PathBuf::from(project.path));
    request.guards.max_steps = parent_run.input.max_steps.max(1);
    Ok(request)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::FreshStoreGlobal;
    use ai_chat_core::{
        AgentEngineKind, AgentRunInput, AgentRunTriggerKind, AgentRuntimeSnapshot,
        ApprovalRequestPayload, ApprovalStatus, ContentPart, ConversationItemPayload,
        ConversationItemStatus, ConversationMetadata, ConversationSettingsSnapshot, ProjectKind,
        ProjectMetadata, ProviderSettingsPayload, ToolApprovalMode, ToolApprovalPolicy,
        ToolArguments, ToolExecutionPolicy, ToolInvocationInput, ToolInvocationStatus,
        ToolNameStrategy, ToolPolicySnapshot, ToolSource, TranscriptRole,
        conservative_model_capabilities,
    };
    use ai_chat_db::{
        NewAgentRun, NewConversation, NewConversationItem, NewProject, NewToolInvocation,
        NewToolInvocationApproval, ToolInvocationApprovalOutcome,
    };
    use gpui::{Subscription, WindowHandle};
    use std::sync::{Arc, Mutex};
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
            phase: ActiveRunPhase::Running,
            cancellation_token,
            run_task: Some(Task::ready(())),
            _event_task: Task::ready(()),
        }
    }

    fn active_run_with_agent_id(key: ActiveRunKey, agent_run_id: AgentRunId) -> ActiveRun {
        ActiveRun {
            agent_run_id: Some(agent_run_id),
            ..active_run(key)
        }
    }

    fn active_waiting_approval_run(key: ActiveRunKey, agent_run_id: AgentRunId) -> ActiveRun {
        ActiveRun {
            agent_run_id: Some(agent_run_id),
            phase: ActiveRunPhase::WaitingForApproval,
            ..active_run(key)
        }
    }

    #[gpui::test]
    fn init_recovers_persisted_running_runs(cx: &mut gpui::TestAppContext) {
        let _dir = init_runtime_test(cx);
        let (conversation_id, agent_run_id) = cx.update(|cx| {
            let repository = database::repository(cx);
            let conversation_id = insert_conversation_with_user_item(&repository);
            let agent_run_id =
                insert_agent_run(&repository, &conversation_id, AgentRunStatus::Running);
            (conversation_id, agent_run_id)
        });

        cx.update(|cx| {
            init(cx).expect("initialize conversation runtime");
            let repository = database::repository(cx);
            let agent_run = repository
                .get_agent_run(&agent_run_id)
                .expect("load recovered run")
                .expect("recovered run exists");
            assert_eq!(agent_run.status, AgentRunStatus::Failed);
            assert_eq!(agent_run.error.as_ref().unwrap().code, "interrupted");
            assert!(!runtime(cx).read(cx).is_running(&conversation_id));
        });
    }

    #[gpui::test]
    fn init_recovers_persisted_waiting_approval_runs(cx: &mut gpui::TestAppContext) {
        let _dir = init_runtime_test(cx);
        let (conversation_id, agent_run_id, approval_id) = cx.update(|cx| {
            let repository = database::repository(cx);
            let conversation_id = insert_conversation_with_user_item(&repository);
            let agent_run_id =
                insert_agent_run(&repository, &conversation_id, AgentRunStatus::Running);
            let approval_id = insert_approval_for_run(&repository, &agent_run_id, None);
            (conversation_id, agent_run_id, approval_id)
        });

        cx.update(|cx| {
            init(cx).expect("initialize conversation runtime");
            let repository = database::repository(cx);
            let agent_run = repository
                .get_agent_run(&agent_run_id)
                .expect("load recovered run")
                .expect("recovered run exists");
            assert_eq!(agent_run.status, AgentRunStatus::Failed);
            assert_eq!(agent_run.error.as_ref().unwrap().code, "interrupted");
            assert!(!runtime(cx).read(cx).is_running(&conversation_id));

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
    fn stop_run_cancels_active_run_immediately(cx: &mut gpui::TestAppContext) {
        let _dir = init_runtime_test(cx);
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new()));
        let recorder = cx.update(|cx| cx.new(|cx| RuntimeEventRecorder::new(store.clone(), cx)));
        let cancellation_token = AgentCancellationToken::new();
        let (conversation_id, agent_run_id) = cx.update(|cx| {
            let repository = database::repository(cx);
            let conversation_id = insert_conversation_with_user_item(&repository);
            let agent_run_id =
                insert_agent_run(&repository, &conversation_id, AgentRunStatus::Running);
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
                assert!(!store.active_runs.contains_key(&conversation_id));
                assert!(store.take_last_error(&conversation_id).is_none());
            });
        });

        assert!(cancellation_token.is_cancelled());

        cx.update(|cx| {
            let repository = database::repository(cx);
            let run = repository.get_agent_run(&agent_run_id).unwrap().unwrap();
            assert_eq!(run.status, AgentRunStatus::Canceled);
            assert!(run.error.is_none());
            let events = recorder.read(cx).events.lock().unwrap().clone();
            assert!(events.contains(&ConversationRuntimeEvent::RunFinished {
                conversation_id: conversation_id.clone(),
            }));
        });
    }

    #[gpui::test]
    fn finish_run_records_uncanceled_error(cx: &mut gpui::TestAppContext) {
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new()));
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
    fn finish_run_keeps_waiting_for_approval_active(cx: &mut gpui::TestAppContext) {
        let _dir = init_runtime_test(cx);
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new()));
        let recorder = cx.update(|cx| cx.new(|cx| RuntimeEventRecorder::new(store.clone(), cx)));
        let (conversation_id, agent_run) = cx.update(|cx| {
            let repository = database::repository(cx);
            let conversation_id = insert_conversation_with_user_item(&repository);
            let agent_run_id =
                insert_agent_run(&repository, &conversation_id, AgentRunStatus::Running);
            let agent_run = repository.get_agent_run(&agent_run_id).unwrap().unwrap();
            (conversation_id, agent_run)
        });

        cx.update(|cx| {
            store.update(cx, |store, cx| {
                store
                    .active_runs
                    .insert(conversation_id.clone(), active_run(ActiveRunKey(0)));
                assert!(store.finish_run(
                    conversation_id.clone(),
                    ActiveRunKey(0),
                    Ok(AgentRunHandle {
                        agent_run: agent_run.clone(),
                        output: None,
                        status: AgentRunHandleStatus::WaitingForApproval {
                            tool_invocation_id: "tool-1".to_string(),
                        },
                        events: Vec::new(),
                        steps: Vec::new(),
                    }),
                    cx
                ));

                let active = store
                    .active_runs
                    .get(&conversation_id)
                    .expect("waiting approval remains active");
                assert_eq!(active.agent_run_id.as_ref(), Some(&agent_run.id));
                assert_eq!(active.phase, ActiveRunPhase::WaitingForApproval);
            });
        });

        let events = cx.update(|cx| recorder.read(cx).events.lock().unwrap().clone());
        assert!(
            events.contains(&ConversationRuntimeEvent::ConversationChanged {
                conversation_id: conversation_id.clone(),
            })
        );
        assert!(!events.contains(&ConversationRuntimeEvent::RunFinished { conversation_id }));
    }

    #[gpui::test]
    fn deny_tool_invocation_success_removes_matching_waiting_run(cx: &mut gpui::TestAppContext) {
        let _dir = init_runtime_test(cx);
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new()));
        let (conversation_id, agent_run_id, approval_id) = cx.update(|cx| {
            let repository = database::repository(cx);
            let conversation_id = insert_conversation_with_user_item(&repository);
            let agent_run_id =
                insert_agent_run(&repository, &conversation_id, AgentRunStatus::Running);
            let approval_id = insert_approval_for_run(&repository, &agent_run_id, None);
            (conversation_id, agent_run_id, approval_id)
        });

        cx.update(|cx| {
            store.update(cx, |store, cx| {
                store.active_runs.insert(
                    conversation_id.clone(),
                    active_waiting_approval_run(ActiveRunKey(0), agent_run_id.clone()),
                );

                assert!(store.deny_tool_invocation(
                    conversation_id.clone(),
                    approval_id.clone(),
                    cx
                ));
                assert!(!store.active_runs.contains_key(&conversation_id));
            });
        });

        cx.update(|cx| {
            let repository = database::repository(cx);
            let invocation = repository
                .get_tool_invocation(&approval_id)
                .unwrap()
                .unwrap();
            assert_eq!(invocation.status, ToolInvocationStatus::Denied);
            assert_eq!(
                invocation.approval.as_ref().map(|approval| approval.status),
                Some(ApprovalStatus::Denied)
            );
        });
    }

    #[gpui::test]
    fn approve_tool_invocation_without_active_waiting_run_is_ignored(
        cx: &mut gpui::TestAppContext,
    ) {
        let _dir = init_runtime_test(cx);
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new()));
        let window = open_runtime_test_window(cx);
        let (conversation_id, agent_run_id, approval_id) = cx.update(|cx| {
            let repository = database::repository(cx);
            let conversation_id = insert_conversation_with_user_item(&repository);
            let agent_run_id =
                insert_agent_run(&repository, &conversation_id, AgentRunStatus::Running);
            let approval_id = insert_approval_for_run(&repository, &agent_run_id, None);
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
            let repository = database::repository(cx);
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
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new()));
        let (conversation_id, agent_run_id, approval_id) = cx.update(|cx| {
            let repository = database::repository(cx);
            let conversation_id = insert_conversation_with_user_item(&repository);
            let agent_run_id =
                insert_agent_run(&repository, &conversation_id, AgentRunStatus::Running);
            let approval_id = insert_approval_for_run(&repository, &agent_run_id, None);
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
            let repository = database::repository(cx);
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
    fn deny_tool_invocation_error_keeps_matching_waiting_run(cx: &mut gpui::TestAppContext) {
        let _dir = init_runtime_test(cx);
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new()));
        let (conversation_id, agent_run_id, approval_id) = cx.update(|cx| {
            let repository = database::repository(cx);
            let conversation_id = insert_conversation_with_user_item(&repository);
            let agent_run_id =
                insert_agent_run(&repository, &conversation_id, AgentRunStatus::Running);
            let approval_id = insert_approval_for_run(
                &repository,
                &agent_run_id,
                Some(ToolInvocationApprovalOutcome::Approved {
                    decided_by: "user".to_string(),
                    reason: None,
                }),
            );
            (conversation_id, agent_run_id, approval_id)
        });

        cx.update(|cx| {
            store.update(cx, |store, cx| {
                store.active_runs.insert(
                    conversation_id.clone(),
                    active_waiting_approval_run(ActiveRunKey(0), agent_run_id.clone()),
                );

                assert!(!store.deny_tool_invocation(
                    conversation_id.clone(),
                    approval_id.clone(),
                    cx
                ));
                let active = store
                    .active_runs
                    .get(&conversation_id)
                    .expect("failed denial must keep the active run");
                assert_eq!(active.agent_run_id.as_ref(), Some(&agent_run_id));
                assert_eq!(active.phase, ActiveRunPhase::WaitingForApproval);
                assert!(store.take_last_error(&conversation_id).is_some());
            });
        });
    }

    #[gpui::test]
    fn deny_tool_invocation_ignores_stale_action_for_running_resume(cx: &mut gpui::TestAppContext) {
        let _dir = init_runtime_test(cx);
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new()));
        let (conversation_id, agent_run_id, approval_id) = cx.update(|cx| {
            let repository = database::repository(cx);
            let conversation_id = insert_conversation_with_user_item(&repository);
            let agent_run_id =
                insert_agent_run(&repository, &conversation_id, AgentRunStatus::Running);
            let approval_id = insert_approval_for_run(
                &repository,
                &agent_run_id,
                Some(ToolInvocationApprovalOutcome::Approved {
                    decided_by: "user".to_string(),
                    reason: None,
                }),
            );
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
                    .expect("stale denial must not clear the running resume");
                assert_eq!(active.agent_run_id.as_ref(), Some(&agent_run_id));
                assert_eq!(active.phase, ActiveRunPhase::Running);
                assert!(store.take_last_error(&conversation_id).is_none());
            });
        });
    }

    #[gpui::test]
    fn conversation_item_updated_emits_conversation_changed(cx: &mut gpui::TestAppContext) {
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new()));
        let recorder = cx.update(|cx| cx.new(|cx| RuntimeEventRecorder::new(store.clone(), cx)));
        let conversation_id = "conversation-1".to_string();

        cx.update(|cx| {
            store.update(cx, |store, cx| {
                store.handle_runtime_event(
                    ai_chat_agent::AgentRuntimeEvent::ConversationItemUpdated {
                        conversation_id: conversation_id.clone(),
                        item_id: "item-1".to_string(),
                    },
                    cx,
                );
            });
        });

        let events = cx.update(|cx| recorder.read(cx).events.lock().unwrap().clone());
        assert!(
            events.contains(&ConversationRuntimeEvent::ConversationChanged { conversation_id })
        );
    }

    #[gpui::test]
    fn finish_run_ignores_stale_run_key(cx: &mut gpui::TestAppContext) {
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new()));
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
            cx.set_global(FreshStoreGlobal::open_in_dir(dir.path()).unwrap());
        });
        dir
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
                path: "/tmp/ai-chat2-runtime-test".to_string(),
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
            .append_conversation_item(NewConversationItem {
                conversation_id: conversation.id.clone(),
                status: ConversationItemStatus::Completed,
                agent_run_id: None,
                provider_step_id: None,
                tool_invocation_id: None,
                provider_item_id: None,
                payload: ConversationItemPayload::Message {
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
        status: AgentRunStatus,
    ) -> AgentRunId {
        let user_item_id = repository
            .conversation_items(conversation_id)
            .unwrap()
            .last()
            .unwrap()
            .id
            .clone();
        repository
            .insert_agent_run(NewAgentRun {
                trigger_kind: AgentRunTriggerKind::User,
                status,
                input: AgentRunInput {
                    user_item_id,
                    parent_agent_run_id: None,
                    prompt_snapshot: None,
                    provider_id: "provider".to_string(),
                    model_id: "model".to_string(),
                    settings_snapshot: run_settings(),
                    runtime_snapshot: AgentRuntimeSnapshot {
                        engine: AgentEngineKind::Rig,
                        engine_version: "test".to_string(),
                        skill_catalog_hash: None,
                        mcp_config_hash: None,
                        mcp_config_snapshot: None,
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
        outcome: Option<ToolInvocationApprovalOutcome>,
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
        if let Some(outcome) = outcome {
            repository
                .update_tool_invocation_approval(
                    &invocation.id,
                    outcome,
                    ToolInvocationStatus::AwaitingApproval,
                )
                .unwrap()
                .id
        } else {
            invocation.id
        }
    }

    fn tool_result_texts(
        repository: &FreshRepository,
        conversation_id: &ConversationId,
    ) -> Vec<String> {
        repository
            .conversation_items(conversation_id)
            .unwrap()
            .into_iter()
            .filter_map(|item| match item.payload {
                ConversationItemPayload::ToolResult(result) => Some(result.content),
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

    fn run_settings() -> ai_chat_core::RunSettingsSnapshot {
        ai_chat_core::RunSettingsSnapshot {
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

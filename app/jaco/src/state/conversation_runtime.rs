mod approval;

use std::{collections::HashMap, sync::Arc};

use gpui::{App, AppContext, AsyncWindowContext, Context, Entity, EventEmitter, Global, Task};
use jaco_agent::{
    AgentCancellationToken, AgentRunHandle, AgentRunRequest, AgentRuntime, AgentRuntimeObserver,
    ToolApprovalDecision,
};
use jaco_core::{AgentRunId, AgentRunStatus, ConversationId, ToolInvocationId};
use jaco_db::FreshRepository;
use smol::channel::{Receiver, Sender};
use tracing::{Level, event};

use self::approval::ConversationApprovalBroker;
use crate::{database, errors::JacoResult, state::provider_secrets::ProviderSecretStore};

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
    cancellation_token: AgentCancellationToken,
    approval_broker: Arc<ConversationApprovalBroker>,
    run_task: Option<Task<()>>,
    _event_task: Task<()>,
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

    pub(crate) fn active_agent_run_id(
        &self,
        conversation_id: &ConversationId,
    ) -> Option<AgentRunId> {
        self.active_runs
            .get(conversation_id)
            .and_then(|active| active.agent_run_id.clone())
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
        if let Some(agent_run_id) = active.agent_run_id.as_ref() {
            active.approval_broker.cancel_all_for_run(agent_run_id);
        } else {
            active.approval_broker.cancel_all();
        }
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
        let approval_broker = Arc::new(ConversationApprovalBroker::new());
        let store = cx.entity().downgrade();
        let run_conversation_id = conversation_id.clone();
        let cancellation_token = request.cancellation_token.clone();
        let runtime_approval_broker = approval_broker.clone();
        let run_task = window.spawn(cx, async move |cx| {
            let result =
                run_agent_with_saved_provider(repository, request, tx, runtime_approval_broker, cx)
                    .await;
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
                cancellation_token,
                approval_broker,
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
        _window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.last_errors.remove(&conversation_id);
        let Some(active) = self.active_runs.get(&conversation_id) else {
            return false;
        };
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
        let Some(active) = self.active_runs.get(&conversation_id) else {
            return false;
        };
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
        cx.emit(ConversationRuntimeEvent::ConversationChanged { conversation_id });
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
        rx: Receiver<jaco_agent::AgentRuntimeEvent>,
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
        runtime_event: jaco_agent::AgentRuntimeEvent,
        cx: &mut Context<Self>,
    ) {
        match runtime_event {
            jaco_agent::AgentRuntimeEvent::AgentRunStarted {
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
            jaco_agent::AgentRuntimeEvent::AgentRunStatusChanged {
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
            jaco_agent::AgentRuntimeEvent::ConversationEntryAppended {
                conversation_id, ..
            }
            | jaco_agent::AgentRuntimeEvent::ConversationEntryUpdated {
                conversation_id, ..
            } => {
                cx.emit(ConversationRuntimeEvent::ConversationChanged { conversation_id });
            }
            jaco_agent::AgentRuntimeEvent::ProviderStepChanged { agent_run_id, .. }
            | jaco_agent::AgentRuntimeEvent::ToolInvocationChanged { agent_run_id, .. }
            | jaco_agent::AgentRuntimeEvent::ToolApprovalRequested { agent_run_id, .. } => {
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

pub(crate) fn init(cx: &mut App) -> JacoResult<()> {
    let recovered = AgentRuntime::new(database::repository(cx)).recover_interrupted_runs()?;
    if !recovered.is_empty() {
        event!(
            Level::WARN,
            recovered_count = recovered.len(),
            "recovered interrupted jaco agent runs"
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
    tx: Sender<jaco_agent::AgentRuntimeEvent>,
    approval_broker: Arc<ConversationApprovalBroker>,
    cx: &mut AsyncWindowContext,
) -> Result<AgentRunHandle, String> {
    let observer = AgentRuntimeObserver::new(move |event| {
        if let Err(err) = tx.send_blocking(event) {
            event!(Level::ERROR, error = ?err, "send conversation runtime event failed");
        }
    });
    let runtime = AgentRuntime::new(repository.clone()).with_approval_broker(approval_broker);
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

fn is_terminal_status(status: AgentRunStatus) -> bool {
    matches!(
        status,
        AgentRunStatus::Completed | AgentRunStatus::Failed | AgentRunStatus::Canceled
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::FreshStoreGlobal;
    use gpui::{Subscription, WindowHandle};
    use jaco_agent::AgentRunHandleStatus;
    use jaco_core::{
        AgentEngineKind, AgentRunInput, AgentRunTriggerKind, AgentRuntimeSnapshot,
        ApprovalRequestPayload, ApprovalStatus, ContentPart, ConversationEntryPayload,
        ConversationEntryStatus, ConversationMetadata, ConversationSettingsSnapshot, ProjectKind,
        ProjectMetadata, ProviderSettingsPayload, ToolApprovalMode, ToolApprovalPolicy,
        ToolArguments, ToolExecutionPolicy, ToolInvocationInput, ToolInvocationStatus,
        ToolNameStrategy, ToolPolicySnapshot, ToolSource, TranscriptRole,
        conservative_model_capabilities,
    };
    use jaco_db::{
        NewAgentRun, NewConversation, NewConversationEntry, NewProject, NewToolInvocation,
        NewToolInvocationApproval,
    };
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
            cancellation_token,
            approval_broker: Arc::new(ConversationApprovalBroker::new()),
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
            let approval_id = insert_approval_for_run(&repository, &agent_run_id);
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
    fn finish_run_removes_matching_active_run(cx: &mut gpui::TestAppContext) {
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new()));
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
    fn deny_tool_invocation_resolves_matching_pending_approval(cx: &mut gpui::TestAppContext) {
        let _dir = init_runtime_test(cx);
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new()));
        let (conversation_id, agent_run_id, approval_id) = cx.update(|cx| {
            let repository = database::repository(cx);
            let conversation_id = insert_conversation_with_user_item(&repository);
            let agent_run_id =
                insert_agent_run(&repository, &conversation_id, AgentRunStatus::Running);
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
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new()));
        let window = open_runtime_test_window(cx);
        let (conversation_id, agent_run_id, approval_id) = cx.update(|cx| {
            let repository = database::repository(cx);
            let conversation_id = insert_conversation_with_user_item(&repository);
            let agent_run_id =
                insert_agent_run(&repository, &conversation_id, AgentRunStatus::Running);
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
    fn deny_tool_invocation_ignores_stale_action_without_pending_broker(
        cx: &mut gpui::TestAppContext,
    ) {
        let _dir = init_runtime_test(cx);
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new()));
        let (conversation_id, agent_run_id, approval_id) = cx.update(|cx| {
            let repository = database::repository(cx);
            let conversation_id = insert_conversation_with_user_item(&repository);
            let agent_run_id =
                insert_agent_run(&repository, &conversation_id, AgentRunStatus::Running);
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
    fn conversation_entry_updated_emits_conversation_changed(cx: &mut gpui::TestAppContext) {
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new()));
        let recorder = cx.update(|cx| cx.new(|cx| RuntimeEventRecorder::new(store.clone(), cx)));
        let conversation_id = "conversation-1".to_string();

        cx.update(|cx| {
            store.update(cx, |store, cx| {
                store.handle_runtime_event(
                    jaco_agent::AgentRuntimeEvent::ConversationEntryUpdated {
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
        status: AgentRunStatus,
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
                status,
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

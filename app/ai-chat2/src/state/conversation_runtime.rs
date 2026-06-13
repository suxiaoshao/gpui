use std::{collections::HashMap, time::Duration};

use ai_chat_agent::{AgentRunHandle, AgentRunRequest, AgentRuntime, AgentRuntimeObserver};
use ai_chat_core::{AgentRunId, AgentRunStatus, ConversationId};
use ai_chat_db::FreshRepository;
use gpui::{App, AppContext, AsyncWindowContext, Context, Entity, EventEmitter, Global, Task};
use smol::channel::{Receiver, Sender};
use tracing::{Level, event};

use crate::{database, state::provider_secrets::ProviderSecretStore};

const STOP_GRACE: Duration = Duration::from_millis(100);

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
    cancel_requested: bool,
    cancel: Box<dyn Fn() + Send + Sync>,
    _run_task: Task<()>,
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

    pub(crate) fn has_active_run(&self) -> bool {
        !self.active_runs.is_empty()
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

        if !active.cancel_requested {
            (active.cancel)();
            active.cancel_requested = true;
            let run_key = active.key;
            let conversation_id_for_stop = conversation_id.clone();
            cx.spawn(async move |store, cx| {
                cx.background_executor().timer(STOP_GRACE).await;
                let Some(store) = store.upgrade() else {
                    return;
                };

                store.update(cx, |store, cx| {
                    let repository = database::repository(cx);
                    store.force_finish_stopped_run(
                        conversation_id_for_stop.clone(),
                        run_key,
                        repository,
                        cx,
                    );
                });
            })
            .detach();
            cx.emit(ConversationRuntimeEvent::ConversationChanged {
                conversation_id: conversation_id.clone(),
            });
            cx.notify();
        }

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
        let cancel = Box::new(move || cancellation_token.cancel());
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
                cancel_requested: false,
                cancel,
                _run_task: run_task,
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

    fn next_active_run_key(&mut self) -> ActiveRunKey {
        let key = ActiveRunKey(self.next_run_key);
        self.next_run_key = self.next_run_key.wrapping_add(1);
        key
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

        let cancel_requested = self
            .active_runs
            .remove(&conversation_id)
            .is_some_and(|active| active.cancel_requested);
        if let Err(err) = result
            && !cancel_requested
        {
            self.last_errors.insert(conversation_id.clone(), err);
        }
        cx.emit(ConversationRuntimeEvent::ConversationChanged {
            conversation_id: conversation_id.clone(),
        });
        cx.emit(ConversationRuntimeEvent::RunFinished { conversation_id });
        cx.notify();
        true
    }

    fn force_finish_stopped_run(
        &mut self,
        conversation_id: ConversationId,
        run_key: ActiveRunKey,
        repository: FreshRepository,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(active) = self.active_runs.get(&conversation_id) else {
            return false;
        };
        if active.key != run_key || !active.cancel_requested {
            return false;
        }

        let agent_run_id = active.agent_run_id.clone().or_else(|| {
            match latest_non_terminal_agent_run_id(&repository, &conversation_id) {
                Ok(agent_run_id) => agent_run_id,
                Err(err) => {
                    event!(Level::ERROR, error = ?err, conversation_id = %conversation_id, "load active agent run for forced stop failed");
                    None
                }
            }
        });
        if let Some(agent_run_id) = agent_run_id
            && let Err(err) = AgentRuntime::new(repository).cancel_run(&agent_run_id, None)
        {
            event!(Level::ERROR, error = ?err, agent_run_id = %agent_run_id, "cancel active agent run failed");
        }

        self.active_runs.remove(&conversation_id);
        self.last_errors.remove(&conversation_id);
        cx.emit(ConversationRuntimeEvent::ConversationChanged {
            conversation_id: conversation_id.clone(),
        });
        cx.emit(ConversationRuntimeEvent::RunFinished { conversation_id });
        cx.notify();
        true
    }
}

pub(crate) fn init(cx: &mut App) {
    let store = cx.new(|_| ConversationRuntimeStore::new());
    cx.set_global(ConversationRuntimeGlobal(store));
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
    let provider = repository
        .get_provider(&request.provider_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| format!("provider `{}` was not found", request.provider_id))?;
    let secrets = ProviderSecretStore::read_values(cx, &provider.secret_refs).await?;
    let observer = AgentRuntimeObserver::new(move |event| {
        if let Err(err) = tx.send_blocking(event) {
            event!(Level::ERROR, error = ?err, "send conversation runtime event failed");
        }
    });
    let runtime = AgentRuntime::new(repository);
    gpui_tokio::Tokio::spawn(cx, async move {
        runtime
            .run_with_saved_provider_observed(request, provider, secrets, Some(observer))
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

fn latest_non_terminal_agent_run_id(
    repository: &FreshRepository,
    conversation_id: &ConversationId,
) -> ai_chat_db::Result<Option<AgentRunId>> {
    Ok(repository
        .agent_runs_for_conversation(conversation_id)?
        .into_iter()
        .rev()
        .find(|run| !is_terminal_status(run.status))
        .map(|run| run.id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::FreshStoreGlobal;
    use ai_chat_core::{
        AgentEngineKind, AgentRunInput, AgentRunTriggerKind, AgentRuntimeSnapshot, ContentPart,
        ConversationItemPayload, ConversationItemStatus, ConversationMetadata,
        ConversationSettingsSnapshot, ProjectKind, ProjectMetadata, ProviderSettingsPayload,
        ToolApprovalPolicy, ToolNameStrategy, ToolPolicySnapshot, ToolSource, TranscriptRole,
        conservative_model_capabilities,
    };
    use ai_chat_db::{NewAgentRun, NewConversation, NewConversationItem, NewProject};
    use gpui::Subscription;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
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

    fn active_run(
        key: ActiveRunKey,
        cancel_count: Arc<AtomicUsize>,
        cancel_requested: bool,
    ) -> ActiveRun {
        ActiveRun {
            key,
            agent_run_id: None,
            cancel_requested,
            cancel: Box::new(move || {
                cancel_count.fetch_add(1, Ordering::SeqCst);
            }),
            _run_task: Task::ready(()),
            _event_task: Task::ready(()),
        }
    }

    fn active_run_with_agent_id(
        key: ActiveRunKey,
        agent_run_id: AgentRunId,
        cancel_count: Arc<AtomicUsize>,
        cancel_requested: bool,
    ) -> ActiveRun {
        ActiveRun {
            agent_run_id: Some(agent_run_id),
            ..active_run(key, cancel_count, cancel_requested)
        }
    }

    #[gpui::test]
    fn stop_run_cancels_active_run_once(cx: &mut gpui::TestAppContext) {
        let _dir = init_runtime_test(cx);
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new()));
        let conversation_id = "conversation-1".to_string();
        let cancel_count = Arc::new(AtomicUsize::new(0));

        cx.update(|cx| {
            store.update(cx, |store, cx| {
                store.active_runs.insert(
                    conversation_id.clone(),
                    active_run(ActiveRunKey(0), cancel_count.clone(), false),
                );

                assert!(store.stop_run(&conversation_id, cx));
                assert!(store.stop_run(&conversation_id, cx));
                assert!(
                    store
                        .active_runs
                        .get(&conversation_id)
                        .is_some_and(|run| run.cancel_requested)
                );
            });
        });

        assert_eq!(cancel_count.load(Ordering::SeqCst), 1);
    }

    #[gpui::test]
    fn stop_run_removes_active_run_after_grace(cx: &mut gpui::TestAppContext) {
        let _dir = init_runtime_test(cx);
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new()));
        let recorder = cx.update(|cx| cx.new(|cx| RuntimeEventRecorder::new(store.clone(), cx)));
        let conversation_id = "conversation-1".to_string();
        let cancel_count = Arc::new(AtomicUsize::new(0));

        cx.update(|cx| {
            store.update(cx, |store, cx| {
                store.active_runs.insert(
                    conversation_id.clone(),
                    active_run(ActiveRunKey(0), cancel_count.clone(), false),
                );
                assert!(store.stop_run(&conversation_id, cx));
            });
        });

        cx.background_executor
            .advance_clock(STOP_GRACE + Duration::from_millis(1));
        cx.run_until_parked();

        cx.update(|cx| {
            store.update(cx, |store, _cx| {
                assert!(!store.active_runs.contains_key(&conversation_id));
            });
            let events = recorder.read(cx).events.lock().unwrap().clone();
            assert!(events.contains(&ConversationRuntimeEvent::RunFinished {
                conversation_id: conversation_id.clone(),
            }));
        });
        assert_eq!(cancel_count.load(Ordering::SeqCst), 1);
    }

    #[gpui::test]
    fn finish_run_suppresses_error_after_cancel_request(cx: &mut gpui::TestAppContext) {
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new()));
        let conversation_id = "conversation-1".to_string();

        cx.update(|cx| {
            store.update(cx, |store, cx| {
                store.active_runs.insert(
                    conversation_id.clone(),
                    active_run(ActiveRunKey(0), Arc::new(AtomicUsize::new(0)), true),
                );
                store.finish_run(
                    conversation_id.clone(),
                    ActiveRunKey(0),
                    Err("runtime canceled".to_string()),
                    cx,
                );

                assert!(store.take_last_error(&conversation_id).is_none());
            });
        });
    }

    #[gpui::test]
    fn finish_run_records_uncanceled_error(cx: &mut gpui::TestAppContext) {
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new()));
        let conversation_id = "conversation-1".to_string();

        cx.update(|cx| {
            store.update(cx, |store, cx| {
                store.active_runs.insert(
                    conversation_id.clone(),
                    active_run(ActiveRunKey(0), Arc::new(AtomicUsize::new(0)), false),
                );
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
    fn finish_run_ignores_stale_run_key(cx: &mut gpui::TestAppContext) {
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new()));
        let conversation_id = "conversation-1".to_string();

        cx.update(|cx| {
            store.update(cx, |store, cx| {
                store.active_runs.insert(
                    conversation_id.clone(),
                    active_run(ActiveRunKey(2), Arc::new(AtomicUsize::new(0)), false),
                );

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

    #[gpui::test]
    fn force_finish_stopped_run_cancels_active_run_and_removes_it(cx: &mut gpui::TestAppContext) {
        let _dir = init_runtime_test(cx);
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new()));
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
                    active_run_with_agent_id(
                        ActiveRunKey(0),
                        agent_run_id.clone(),
                        Arc::new(AtomicUsize::new(0)),
                        true,
                    ),
                );
                store
                    .last_errors
                    .insert(conversation_id.clone(), "runtime canceled".to_string());
                let repository = database::repository(cx);

                assert!(store.force_finish_stopped_run(
                    conversation_id.clone(),
                    ActiveRunKey(0),
                    repository,
                    cx
                ));
                assert!(!store.active_runs.contains_key(&conversation_id));
                assert!(store.take_last_error(&conversation_id).is_none());
            });
        });

        cx.update(|cx| {
            let repository = database::repository(cx);
            let run = repository.get_agent_run(&agent_run_id).unwrap().unwrap();
            assert_eq!(run.status, AgentRunStatus::Canceled);
            assert!(run.error.is_none());
        });
    }

    #[gpui::test]
    fn force_finish_stopped_run_falls_back_to_latest_non_terminal_run(
        cx: &mut gpui::TestAppContext,
    ) {
        let _dir = init_runtime_test(cx);
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new()));
        let (conversation_id, completed_run_id, running_run_id) = cx.update(|cx| {
            let repository = database::repository(cx);
            let conversation_id = insert_conversation_with_user_item(&repository);
            let completed_run_id =
                insert_agent_run(&repository, &conversation_id, AgentRunStatus::Completed);
            let running_run_id =
                insert_agent_run(&repository, &conversation_id, AgentRunStatus::Running);
            (conversation_id, completed_run_id, running_run_id)
        });

        cx.update(|cx| {
            store.update(cx, |store, cx| {
                store.active_runs.insert(
                    conversation_id.clone(),
                    active_run(ActiveRunKey(0), Arc::new(AtomicUsize::new(0)), true),
                );
                let repository = database::repository(cx);

                assert!(store.force_finish_stopped_run(
                    conversation_id.clone(),
                    ActiveRunKey(0),
                    repository,
                    cx
                ));
            });
        });

        cx.update(|cx| {
            let repository = database::repository(cx);
            assert_eq!(
                repository
                    .get_agent_run(&completed_run_id)
                    .unwrap()
                    .unwrap()
                    .status,
                AgentRunStatus::Completed
            );
            assert_eq!(
                repository
                    .get_agent_run(&running_run_id)
                    .unwrap()
                    .unwrap()
                    .status,
                AgentRunStatus::Canceled
            );
        });
    }

    fn init_runtime_test(cx: &mut gpui::TestAppContext) -> TempDir {
        let dir = tempdir().unwrap();
        cx.update(|cx| {
            cx.set_global(FreshStoreGlobal::open_in_dir(dir.path()).unwrap());
        });
        dir
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
                        tool_name_strategy: ToolNameStrategy::Direct,
                    },
                    max_steps: 8,
                },
            })
            .unwrap()
            .id
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
        }
    }
}

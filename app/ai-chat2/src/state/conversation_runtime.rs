use std::collections::HashMap;

use ai_chat_agent::{AgentRunHandle, AgentRunRequest, AgentRuntime, AgentRuntimeObserver};
use ai_chat_core::{AgentRunId, AgentRunStatus, ConversationId};
use ai_chat_db::FreshRepository;
use gpui::{App, AppContext, AsyncWindowContext, Context, Entity, EventEmitter, Global, Task};
use smol::channel::{Receiver, Sender};
use tracing::{Level, event};

use crate::{database, state::provider_secrets::ProviderSecretStore};

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
}

struct ActiveRun {
    agent_run_id: Option<AgentRunId>,
    _run_task: Task<()>,
    _event_task: Task<()>,
}

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
        }
    }

    pub(crate) fn is_running(&self, conversation_id: &ConversationId) -> bool {
        self.active_runs.contains_key(conversation_id)
    }

    pub(crate) fn take_last_error(&mut self, conversation_id: &ConversationId) -> Option<String> {
        self.last_errors.remove(conversation_id)
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
        let repository = database::repository(cx);
        let (tx, rx) = smol::channel::unbounded();
        let event_task = self.spawn_event_listener(rx, cx);
        let store = cx.entity().downgrade();
        let run_conversation_id = conversation_id.clone();
        let run_task = window.spawn(cx, async move |cx| {
            let result = run_agent_with_saved_provider(repository, request, tx, cx).await;
            if let Err(err) = store.update_in(cx, |store, _window, cx| {
                store.finish_run(run_conversation_id.clone(), result, cx);
            }) {
                event!(Level::ERROR, error = ?err, "finish conversation agent run failed");
            }
        });

        self.active_runs.insert(
            conversation_id.clone(),
            ActiveRun {
                agent_run_id: None,
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
                if let Some(active) = self.active_runs.get_mut(&conversation_id) {
                    active.agent_run_id = Some(agent_run_id);
                }
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
        result: Result<AgentRunHandle, String>,
        cx: &mut Context<Self>,
    ) {
        self.active_runs.remove(&conversation_id);
        if let Err(err) = result {
            self.last_errors.insert(conversation_id.clone(), err);
        }
        cx.emit(ConversationRuntimeEvent::ConversationChanged {
            conversation_id: conversation_id.clone(),
        });
        cx.emit(ConversationRuntimeEvent::RunFinished { conversation_id });
        cx.notify();
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

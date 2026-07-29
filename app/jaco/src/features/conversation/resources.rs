use gpui::{App, AppContext, Entity, Global, Subscription, Task};
use gpui_store::Store;

use crate::{
    database,
    errors::JacoResult,
    features::conversation::{registry::ConversationRegistry, runtime},
};

#[derive(Clone)]
pub(crate) struct ConversationResources {
    pub(crate) conversations: Entity<ConversationRegistry>,
    pub(crate) runtime: Entity<runtime::ConversationRuntimeStore>,
}

pub(crate) enum ConversationResourcesState {
    AwaitingDatabase,
    Ready(ConversationResources),
    Failed(String),
}

pub(crate) type ConversationResourcesStore = Store<ConversationResourcesState>;

struct ConversationResourcesOwner {
    mutation_tasks: Vec<Task<()>>,
    shutdown_task: Option<Task<()>>,
    _database_subscription: Subscription,
}

struct ConversationResourcesOwnerGlobal(Entity<ConversationResourcesOwner>);

impl Global for ConversationResourcesOwnerGlobal {}

pub(crate) fn init(cx: &mut App) {
    ConversationResourcesStore::install_global(cx, ConversationResourcesState::AwaitingDatabase);
    let owner = cx.new(|cx| {
        let subscription = database::store(cx).observe_select(
            cx,
            database::SelectDatabaseReady,
            |owner: &mut ConversationResourcesOwner, ready, cx| owner.sync(*ready, cx),
        );
        ConversationResourcesOwner {
            mutation_tasks: Vec::new(),
            shutdown_task: None,
            _database_subscription: subscription,
        }
    });
    cx.set_global(ConversationResourcesOwnerGlobal(owner.clone()));
    let ready = database::is_ready(cx);
    owner.update(cx, |owner, cx| owner.sync(ready, cx));
}

impl ConversationResourcesOwner {
    fn sync(&mut self, ready: bool, cx: &mut App) {
        if !ready {
            self.retire(cx);
            return;
        }
        let already_ready = store(cx).read(cx, |state| {
            matches!(state, ConversationResourcesState::Ready(_))
        });
        if already_ready {
            return;
        }
        match initialize(cx) {
            Ok(resources) => store(cx).set(cx, ConversationResourcesState::Ready(resources)),
            Err(error) => {
                tracing::error!(?error, "initialize conversation resources failed");
                store(cx).set(cx, ConversationResourcesState::Failed(error.to_string()));
            }
        }
    }

    fn retire(&mut self, cx: &mut App) {
        self.mutation_tasks.clear();
        let runtime = ready_runtime(cx);
        store(cx).set(cx, ConversationResourcesState::AwaitingDatabase);
        let Some(runtime) = runtime else {
            return;
        };
        let shutdown = runtime.update(cx, |runtime, cx| runtime.shutdown_all(cx));
        let previous = self.shutdown_task.take();
        self.shutdown_task = Some(cx.spawn(async move |_| {
            if let Some(previous) = previous {
                previous.await;
            }
            shutdown.await;
        }));
    }

    fn retain(&mut self, task: Task<()>) {
        self.mutation_tasks.retain(|task| !task.is_ready());
        self.mutation_tasks.push(task);
    }
}

fn initialize(cx: &mut App) -> JacoResult<ConversationResources> {
    let executor = database::ready_executor(cx)?;
    let conversations = cx.new(|cx| ConversationRegistry::new(executor, cx));
    conversations
        .read(cx)
        .catalog()
        .update(cx, |catalog, cx| catalog.refresh(cx));
    let runtime = runtime::create(cx)?;
    runtime::retry_recovery_if_needed(&runtime, cx);
    Ok(ConversationResources {
        conversations,
        runtime,
    })
}

pub(crate) fn store(cx: &impl AppContext) -> ConversationResourcesStore {
    ConversationResourcesStore::global(cx)
}

pub(crate) fn ready_data(cx: &impl AppContext) -> Option<ConversationResources> {
    store(cx).read(cx, |state| match state {
        ConversationResourcesState::Ready(resources) => Some(resources.clone()),
        ConversationResourcesState::AwaitingDatabase | ConversationResourcesState::Failed(_) => {
            None
        }
    })
}

pub(crate) fn ready_runtime(
    cx: &impl AppContext,
) -> Option<Entity<runtime::ConversationRuntimeStore>> {
    ready_data(cx).map(|resources| resources.runtime)
}

pub(crate) fn ready_conversations(cx: &impl AppContext) -> Option<Entity<ConversationRegistry>> {
    ready_data(cx).map(|resources| resources.conversations)
}

pub(crate) fn request_retry(cx: &mut App) {
    if !database::is_ready(cx) {
        return;
    }
    let owner = cx.global::<ConversationResourcesOwnerGlobal>().0.clone();
    owner.update(cx, |owner, cx| owner.sync(true, cx));
}

pub(crate) fn retain_task(task: Task<()>, cx: &mut App) {
    #[cfg(test)]
    if !cx.has_global::<ConversationResourcesOwnerGlobal>() {
        crate::app::tasks::retain_application(task, cx);
        return;
    }
    let owner = cx.global::<ConversationResourcesOwnerGlobal>().0.clone();
    owner.update(cx, |owner, _| owner.retain(task));
}

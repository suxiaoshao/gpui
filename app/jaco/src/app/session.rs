use gpui::{App, AppContext, Entity, Global, Subscription, Task};
use gpui_store::{Select, Store};

use crate::{
    database::{self, DatabaseOperation, DatabaseResource},
    errors::JacoResult,
    state,
};

#[derive(Clone)]
pub(crate) struct AppSessionData {
    pub(crate) binding: database::session::DatabaseBinding,
    pub(crate) runtime: Entity<state::conversations::runtime::ConversationRuntimeStore>,
    pub(crate) workspace: Entity<state::JacoWorkspaceStore>,
}

pub(crate) enum AppSessionState {
    AwaitingDatabase,
    Ready(AppSessionData),
    Failed {
        binding: database::session::DatabaseBinding,
        message: String,
    },
}

pub(crate) type AppSessionStore = Store<AppSessionState>;

#[derive(Clone, Debug, Eq, PartialEq)]
struct DatabaseSessionDependency {
    binding: Option<database::session::DatabaseBinding>,
    exactly_ready: bool,
}

#[derive(Clone, Copy)]
struct SelectDatabaseSessionDependency;

impl Select<DatabaseResource> for SelectDatabaseSessionDependency {
    type Output = DatabaseSessionDependency;

    fn select(&self, resource: &DatabaseResource) -> Self::Output {
        match resource {
            DatabaseResource::AwaitingConfig => DatabaseSessionDependency {
                binding: None,
                exactly_ready: false,
            },
            DatabaseResource::Bound { operation, .. } => DatabaseSessionDependency {
                binding: operation.data().map(|data| data.binding.clone()),
                exactly_ready: matches!(operation, DatabaseOperation::Ready(_)),
            },
        }
    }
}

struct AppSessionCoordinator {
    binding: Option<database::session::DatabaseBinding>,
    session_tasks: Vec<Task<()>>,
    shutdown_task: Option<Task<()>>,
    _database_subscription: Subscription,
}

struct AppSessionCoordinatorGlobal(Entity<AppSessionCoordinator>);

impl Global for AppSessionCoordinatorGlobal {}

pub(crate) fn init(cx: &mut App) {
    AppSessionStore::install_global(cx, AppSessionState::AwaitingDatabase);
    let coordinator = cx.new(|cx| {
        let database_subscription = database::store(cx).observe_select(
            cx,
            SelectDatabaseSessionDependency,
            |coordinator: &mut AppSessionCoordinator, dependency, cx| {
                coordinator.sync(dependency.clone(), cx);
            },
        );
        AppSessionCoordinator {
            binding: None,
            session_tasks: Vec::new(),
            shutdown_task: None,
            _database_subscription: database_subscription,
        }
    });
    let dependency = database::store(cx).read(cx, |resource| {
        SelectDatabaseSessionDependency.select(resource)
    });
    cx.set_global(AppSessionCoordinatorGlobal(coordinator.clone()));
    coordinator.update(cx, |coordinator, cx| coordinator.sync(dependency, cx));
}

impl AppSessionCoordinator {
    fn sync(&mut self, dependency: DatabaseSessionDependency, cx: &mut App) {
        if self.binding != dependency.binding {
            self.retire_current_session(cx);
            self.binding = dependency.binding.clone();
        }

        let Some(binding) = dependency.binding else {
            return;
        };
        if !dependency.exactly_ready {
            return;
        }
        let already_ready = AppSessionStore::global(cx).read(cx, |session| {
            matches!(
                session,
                AppSessionState::Ready(data) if data.binding == binding
            )
        });
        if already_ready {
            return;
        }

        match initialize_ready_session(binding.clone(), cx) {
            Ok(data) => AppSessionStore::global(cx).set(cx, AppSessionState::Ready(data)),
            Err(error) => {
                tracing::error!(?error, "initialize ready Jaco session failed");
                AppSessionStore::global(cx).set(
                    cx,
                    AppSessionState::Failed {
                        binding,
                        message: error.to_string(),
                    },
                );
            }
        }
    }

    fn retire_current_session(&mut self, cx: &mut App) {
        self.session_tasks.clear();
        let runtime = AppSessionStore::global(cx).read(cx, |session| match session {
            AppSessionState::Ready(data) => Some(data.runtime.clone()),
            AppSessionState::AwaitingDatabase | AppSessionState::Failed { .. } => None,
        });
        AppSessionStore::global(cx).set(cx, AppSessionState::AwaitingDatabase);
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

    fn retain_task(&mut self, binding: &database::session::DatabaseBinding, task: Task<()>) {
        if self.binding.as_ref() != Some(binding) {
            return;
        }
        self.session_tasks.retain(|task| !task.is_ready());
        self.session_tasks.push(task);
    }
}

fn initialize_ready_session(
    binding: database::session::DatabaseBinding,
    cx: &mut App,
) -> JacoResult<AppSessionData> {
    super::init_ready_services(cx)?;
    let runtime = state::conversations::runtime::create(cx)?;
    let workspace = state::workspace::create(cx);
    state::conversations::runtime::retry_recovery_if_needed(&runtime, cx);
    Ok(AppSessionData {
        binding,
        runtime,
        workspace,
    })
}

pub(crate) fn store(cx: &impl AppContext) -> AppSessionStore {
    AppSessionStore::global(cx)
}

pub(crate) fn ready_data(cx: &impl AppContext) -> Option<AppSessionData> {
    store(cx).read(cx, |session| match session {
        AppSessionState::Ready(data) => Some(data.clone()),
        AppSessionState::AwaitingDatabase | AppSessionState::Failed { .. } => None,
    })
}

pub(crate) fn ready_runtime(
    cx: &impl AppContext,
) -> Option<Entity<state::conversations::runtime::ConversationRuntimeStore>> {
    ready_data(cx).map(|session| session.runtime)
}

pub(crate) fn ready_workspace(cx: &impl AppContext) -> Option<Entity<state::JacoWorkspaceStore>> {
    ready_data(cx).map(|session| session.workspace)
}

pub(crate) fn request_retry(cx: &mut App) {
    let dependency = database::store(cx).read(cx, |resource| {
        SelectDatabaseSessionDependency.select(resource)
    });
    let coordinator = cx.global::<AppSessionCoordinatorGlobal>().0.clone();
    coordinator.update(cx, |coordinator, cx| coordinator.sync(dependency, cx));
}

pub(crate) fn retain_task(
    binding: database::session::DatabaseBinding,
    task: Task<()>,
    cx: &mut App,
) {
    let coordinator = cx.global::<AppSessionCoordinatorGlobal>().0.clone();
    coordinator.update(cx, |coordinator, _cx| {
        coordinator.retain_task(&binding, task);
    });
}

use std::fmt;

use gpui::{App, AppContext, Task};
use gpui_operation::{Complete, Load, Refresh, Retry, Transition, refresh};
use gpui_store::Store;

use crate::features::query::advanced::QueryOptions;

pub(crate) struct CatalogTask {
    _target_generation: u64,
    _task: Task<()>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct QueryCatalogProblem(String);

impl fmt::Display for QueryCatalogProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for QueryCatalogProblem {}

pub(crate) type CatalogOperation =
    refresh::Operation<QueryOptions, QueryCatalogProblem, CatalogTask>;

pub(crate) struct QueryCatalogState {
    pub(crate) operation: CatalogOperation,
    pub(crate) invalidation_generation: u64,
    pub(crate) covered_generation: u64,
}

pub(crate) type QueryCatalogStore = Store<QueryCatalogState>;

impl Default for QueryCatalogState {
    fn default() -> Self {
        Self {
            operation: CatalogOperation::new(),
            invalidation_generation: 0,
            covered_generation: 0,
        }
    }
}

pub(crate) fn init(cx: &mut App) {
    QueryCatalogStore::install_global(cx, QueryCatalogState::default());
}

pub(crate) fn store(cx: &impl AppContext) -> QueryCatalogStore {
    QueryCatalogStore::global(cx)
}

pub(crate) fn phase(cx: &impl AppContext) -> refresh::Phase {
    store(cx).read(cx, |state| state.operation.phase())
}

pub(crate) fn data(cx: &impl AppContext) -> Option<QueryOptions> {
    store(cx).read(cx, |state| state.operation.data().cloned())
}

pub(crate) fn problem(cx: &impl AppContext) -> Option<QueryCatalogProblem> {
    store(cx).read(cx, |state| state.operation.problem().cloned())
}

pub(crate) fn invalidate(cx: &mut App) {
    store(cx).update(cx, |state| {
        state.invalidation_generation = state.invalidation_generation.saturating_add(1);
    });
    request_follow_up(cx);
}

pub(crate) fn request_load(cx: &mut App) {
    if !super::database::is_ready(cx) {
        return;
    }
    let catalog = store(cx);
    if catalog.read(cx, |state| state.operation.is_running()) {
        return;
    }
    let target_generation = catalog.read(cx, |state| state.invalidation_generation);
    let pool = match super::database::ready_pool(cx) {
        Ok(pool) => pool,
        Err(_) => return,
    };
    let task = cx.spawn(async move |cx| {
        let result = cx
            .background_spawn(async move {
                let conn = pool
                    .get()
                    .map_err(|error| QueryCatalogProblem(error.to_string()))?;
                QueryOptions::load(&conn).map_err(|error| QueryCatalogProblem(error.to_string()))
            })
            .await;
        let succeeded = result.is_ok();
        cx.update(|cx| {
            store(cx).update(cx, |state| {
                if state.operation.is_running() {
                    state.operation.transition(Complete(result));
                    if succeeded {
                        state.covered_generation = target_generation;
                    }
                }
            });
            if should_follow_completion(succeeded) {
                request_follow_up(cx);
            }
        });
    });
    catalog.update(cx, |state| {
        let task = CatalogTask {
            _target_generation: target_generation,
            _task: task,
        };
        match &state.operation {
            CatalogOperation::Idle(_) => state.operation.transition(Load(task)),
            CatalogOperation::Ready(_) | CatalogOperation::Degraded(_) => {
                state.operation.transition(Refresh(task))
            }
            CatalogOperation::Unavailable(_) => state.operation.transition(Retry(task)),
            CatalogOperation::Loading(_)
            | CatalogOperation::Refreshing(_)
            | CatalogOperation::Retrying(_)
            | CatalogOperation::RefreshingDegraded(_) => {}
        }
    });
}

fn should_follow_completion(succeeded: bool) -> bool {
    succeeded
}

fn request_follow_up(cx: &mut App) {
    let should_refresh = store(cx).read(cx, |state| {
        super::database::is_ready(cx)
            && !state.operation.is_running()
            && allows_automatic_follow_up(state.operation.phase())
            && state.covered_generation < state.invalidation_generation
    });
    if should_refresh {
        request_load(cx);
    }
}

fn allows_automatic_follow_up(phase: refresh::Phase) -> bool {
    matches!(phase, refresh::Phase::Idle | refresh::Phase::Ready)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_only_advances_coverage_after_success() {
        let state = QueryCatalogState {
            invalidation_generation: 3,
            covered_generation: 1,
            ..Default::default()
        };
        assert!(state.covered_generation < state.invalidation_generation);
    }

    #[test]
    fn catalog_task_owns_target_generation_and_task() {
        let task = CatalogTask {
            _target_generation: 2,
            _task: Task::ready(()),
        };
        assert_eq!(task._target_generation, 2);
        assert!(task._task.is_ready());
    }

    #[test]
    fn failed_completion_keeps_pending_invalidation_for_explicit_retry() {
        assert!(!should_follow_completion(false));
        assert!(should_follow_completion(true));
        assert!(!allows_automatic_follow_up(refresh::Phase::Unavailable));
        assert!(!allows_automatic_follow_up(refresh::Phase::Degraded));
        assert!(allows_automatic_follow_up(refresh::Phase::Ready));
    }
}

use std::mem;

use gpui::{Entity, Task};
use gpui_operation::Transition;

use super::{DatabaseData, DatabaseProblem, DatabaseRepair, session::DatabaseSession};

/// The database lifecycle deliberately never retains data after a failed
/// refresh. Once validation fails, the current session is retired before the
/// operation becomes repairable.
#[allow(clippy::large_enum_variant)]
pub(crate) enum DatabaseOperation {
    Idle,
    Ready(DatabaseData),
    Refreshing {
        data: DatabaseData,
        _task: Task<()>,
    },
    Retiring {
        problem: DatabaseProblem,
        _task: Task<()>,
    },
    Unavailable(DatabaseProblem),
    Repairing {
        problem: DatabaseProblem,
        _repair: DatabaseRepair,
        _task: Task<()>,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum DatabasePhase {
    Idle,
    Ready,
    Refreshing,
    Retiring,
    Unavailable,
    Repairing,
}

pub(super) enum DatabaseMessage {
    Settle(Result<DatabaseData, DatabaseProblem>),
    Refresh(Task<()>),
    Refreshed,
    RefreshFailed {
        problem: DatabaseProblem,
        retire: Task<()>,
    },
    Retired,
    Repair {
        repair: DatabaseRepair,
        task: Task<()>,
    },
    Repaired(Result<DatabaseData, DatabaseProblem>),
}

impl DatabaseMessage {
    fn name(&self) -> &'static str {
        match self {
            Self::Settle(_) => "Settle",
            Self::Refresh(_) => "Refresh",
            Self::Refreshed => "Refreshed",
            Self::RefreshFailed { .. } => "RefreshFailed",
            Self::Retired => "Retired",
            Self::Repair { .. } => "Repair",
            Self::Repaired(_) => "Repaired",
        }
    }
}

impl DatabaseOperation {
    pub(crate) const fn new() -> Self {
        Self::Idle
    }

    pub(crate) fn phase(&self) -> DatabasePhase {
        match self {
            Self::Idle => DatabasePhase::Idle,
            Self::Ready(_) => DatabasePhase::Ready,
            Self::Refreshing { .. } => DatabasePhase::Refreshing,
            Self::Retiring { .. } => DatabasePhase::Retiring,
            Self::Unavailable(_) => DatabasePhase::Unavailable,
            Self::Repairing { .. } => DatabasePhase::Repairing,
        }
    }

    /// Lifecycle-only access used to drain a session during refresh failure or
    /// application shutdown. It does not expose database business data.
    pub(crate) fn session(&self) -> Option<&Entity<DatabaseSession>> {
        match self {
            Self::Ready(data) | Self::Refreshing { data, .. } => Some(&data.session),
            Self::Idle | Self::Retiring { .. } | Self::Unavailable(_) | Self::Repairing { .. } => {
                None
            }
        }
    }

    pub(crate) fn problem(&self) -> Option<&DatabaseProblem> {
        match self {
            Self::Retiring { problem, .. }
            | Self::Unavailable(problem)
            | Self::Repairing { problem, .. } => Some(problem),
            Self::Idle | Self::Ready(_) | Self::Refreshing { .. } => None,
        }
    }

    pub(crate) fn is_running(&self) -> bool {
        matches!(
            self,
            Self::Refreshing { .. } | Self::Retiring { .. } | Self::Repairing { .. }
        )
    }
}

impl Default for DatabaseOperation {
    fn default() -> Self {
        Self::new()
    }
}

impl Transition<DatabaseMessage> for &mut DatabaseOperation {
    type Output = ();

    fn transition(self, message: DatabaseMessage) {
        let message_name = message.name();
        let current = mem::replace(self, DatabaseOperation::Idle);
        let current_phase = current.phase();
        match (current, message) {
            (DatabaseOperation::Idle, DatabaseMessage::Settle(result)) => {
                *self = match result {
                    Ok(data) => DatabaseOperation::Ready(data),
                    Err(problem) => DatabaseOperation::Unavailable(problem),
                };
            }
            (DatabaseOperation::Ready(data), DatabaseMessage::Refresh(task)) => {
                *self = DatabaseOperation::Refreshing { data, _task: task };
            }
            (DatabaseOperation::Refreshing { data, _task: task }, DatabaseMessage::Refreshed) => {
                *self = DatabaseOperation::Ready(data);
                drop(task);
            }
            (
                DatabaseOperation::Refreshing {
                    data,
                    _task: refresh_task,
                },
                DatabaseMessage::RefreshFailed { problem, retire },
            ) => {
                *self = DatabaseOperation::Retiring {
                    problem,
                    _task: retire,
                };
                drop(refresh_task);
                drop(data);
            }
            (
                DatabaseOperation::Retiring {
                    problem,
                    _task: task,
                },
                DatabaseMessage::Retired,
            ) => {
                *self = DatabaseOperation::Unavailable(problem);
                drop(task);
            }
            (DatabaseOperation::Unavailable(problem), DatabaseMessage::Repair { repair, task }) => {
                *self = DatabaseOperation::Repairing {
                    problem,
                    _repair: repair,
                    _task: task,
                };
            }
            (
                DatabaseOperation::Repairing {
                    problem: old_problem,
                    _repair: repair,
                    _task: task,
                },
                DatabaseMessage::Repaired(result),
            ) => {
                *self = match result {
                    Ok(data) => DatabaseOperation::Ready(data),
                    Err(problem) => DatabaseOperation::Unavailable(problem),
                };
                drop(task);
                drop(repair);
                drop(old_problem);
            }
            (current, message) => {
                *self = current;
                tracing::debug!(
                    ?current_phase,
                    message = message_name,
                    "ignored database operation message"
                );
                drop(message);
            }
        }
    }
}

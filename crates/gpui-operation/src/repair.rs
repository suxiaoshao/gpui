//! Fetch lifecycle that requires a caller-selected repair after a problem.
//!
//! Use [`Operation`] when an owner needs to retain the complete lifecycle.
//! The named states and [`Transition`] implementations remain available when
//! a caller directly owns one exact state.
//!
//! Unlike [`crate::refresh`], this family does not offer [`crate::Retry`] or
//! a plain [`crate::Refresh`] from a problem-bearing state. The caller must
//! choose a concrete [`crate::Repair`] action.
//!
//! Settled states carry a `Repair` type marker via `PhantomData` to keep the
//! family together without inheriting ownership or auto-trait bounds.
//!
//! # Transitions
//!
//! | Current | Message | Output |
//! |---|---|---|
//! | `Idle<Repair>` | `Settle<Result<Data, Problem>>` | `FetchCompleted<Data, Problem, Repair>` |
//! | `Idle<Repair>` | `Load<Task>` | `Fetching<Idle<Repair>, Task>` |
//! | `Ready<Data, Repair>` | `Refresh<Task>` | `Fetching<Ready<Data, Repair>, Task>` |
//! | `Unavailable<Problem, Repair>` | `Repair<Repair, Task>` | `Repairing<Unavailable<Problem, Repair>, Repair, Task>` |
//! | `Degraded<Data, Problem, Repair>` | `Repair<Repair, Task>` | `Repairing<Degraded<Data, Problem, Repair>, Repair, Task>` |
//! | `Fetching<Previous, Task>` | `Cancel` | `Previous` |
//! | `Repairing<Previous, Repair, Task>` | `Cancel` | `Previous` |

use std::marker::PhantomData;

use crate::{Cancel, Complete, Load, Refresh, Settle, Transition};

/// No fetch has ever been requested.
#[must_use = "operation states must be retained or transitioned"]
pub struct Idle<Repair> {
    marker: PhantomData<fn() -> Repair>,
}

impl<Repair> Idle<Repair> {
    /// Creates an idle operation state.
    pub const fn new() -> Self {
        Self {
            marker: PhantomData,
        }
    }
}

impl<Repair> Default for Idle<Repair> {
    fn default() -> Self {
        Self::new()
    }
}

/// Valid data is available.
#[must_use = "operation states must be retained or transitioned"]
pub struct Ready<Data, Repair> {
    data: Data,
    marker: PhantomData<fn() -> Repair>,
}

impl<Data, Repair> Ready<Data, Repair> {
    /// Borrows the current valid data.
    pub fn data(&self) -> &Data {
        &self.data
    }
}

impl<Data, Repair, Message> Transition<Message> for &mut Ready<Data, Repair>
where
    for<'data> &'data mut Data: Transition<Message, Output = ()>,
{
    type Output = ();

    fn transition(self, message: Message) {
        (&mut self.data).transition(message);
    }
}

/// A problem is available and no valid data exists.
#[must_use = "operation states must be retained or transitioned"]
pub struct Unavailable<Problem: std::error::Error, Repair> {
    problem: Problem,
    marker: PhantomData<fn() -> Repair>,
}

impl<Problem: std::error::Error, Repair> Unavailable<Problem, Repair> {
    /// Borrows the latest problem.
    pub fn problem(&self) -> &Problem {
        &self.problem
    }
}

/// Valid, potentially stale data and the latest problem are available.
#[must_use = "operation states must be retained or transitioned"]
pub struct Degraded<Data, Problem: std::error::Error, Repair> {
    data: Data,
    problem: Problem,
    marker: PhantomData<fn() -> Repair>,
}

impl<Data, Problem: std::error::Error, Repair> Degraded<Data, Problem, Repair> {
    /// Borrows the last-known-good data.
    pub fn data(&self) -> &Data {
        &self.data
    }

    /// Borrows the latest problem.
    pub fn problem(&self) -> &Problem {
        &self.problem
    }
}

/// A normal fetch is running.
#[must_use = "dropping a running operation state cancels its active task"]
pub struct Fetching<Previous, Task> {
    task: Task,
    previous: Previous,
}

impl<Previous, Task> Fetching<Previous, Task> {
    /// Borrows the state that was active before the fetch started.
    pub fn previous(&self) -> &Previous {
        &self.previous
    }
}

/// A caller-selected repair is running.
#[must_use = "dropping a running operation state cancels its active task"]
pub struct Repairing<Previous, Repair, Task> {
    task: Task,
    repair: Repair,
    previous: Previous,
}

impl<Previous, Repair, Task> Repairing<Previous, Repair, Task> {
    /// Borrows the state that was active before the repair started.
    pub fn previous(&self) -> &Previous {
        &self.previous
    }

    /// Borrows the caller-selected repair.
    pub fn repair(&self) -> &Repair {
        &self.repair
    }
}

/// Result of a completion when no valid data existed before (load path).
#[must_use]
pub enum FetchCompleted<Data, Problem: std::error::Error, Repair> {
    /// Fetch succeeded.
    Ready(Ready<Data, Repair>),
    /// Fetch failed.
    Unavailable(Unavailable<Problem, Repair>),
}

/// Result of a completion when valid data existed before (refresh path).
#[must_use]
pub enum RefreshCompleted<Data, Problem: std::error::Error, Repair> {
    /// Fetch succeeded; old data is discarded.
    Ready(Ready<Data, Repair>),
    /// Fetch failed; old data is retained alongside the new problem.
    Degraded(Degraded<Data, Problem, Repair>),
}

/// Result of a repair completion when no valid data existed.
#[must_use]
pub enum RepairWithoutDataCompleted<Data, Problem: std::error::Error, Repair> {
    /// Repair succeeded.
    Ready(Ready<Data, Repair>),
    /// Repair failed; old problem is replaced.
    Unavailable(Unavailable<Problem, Repair>),
}

/// Result of a repair completion when valid data existed.
#[must_use]
pub enum RepairWithDataCompleted<Data, Problem: std::error::Error, Repair> {
    /// Repair succeeded; old data and problem are discarded.
    Ready(Ready<Data, Repair>),
    /// Repair failed; old data is retained, old problem is replaced.
    Degraded(Degraded<Data, Problem, Repair>),
}

/// The complete repair-capable lifecycle for long-term storage in an owner.
///
/// Normal fetches are accepted from `Idle` and `Ready`. Problem-bearing states
/// require a caller-selected Repair. The enum owns all runtime state matching
/// and movement while the caller remains responsible for constructing Tasks
/// and routing completion.
#[must_use = "operation state must be retained"]
pub enum Operation<Data, Problem: std::error::Error, Repair, Task> {
    /// No fetch has been requested.
    Idle(Idle<Repair>),
    /// The first fetch is running.
    Loading(Fetching<Idle<Repair>, Task>),
    /// Valid data is available.
    Ready(Ready<Data, Repair>),
    /// A normal refresh is running while valid data remains available.
    Refreshing(Fetching<Ready<Data, Repair>, Task>),
    /// No valid data exists and the latest fetch failed.
    Unavailable(Unavailable<Problem, Repair>),
    /// A caller-selected repair is running without valid data.
    RepairingUnavailable(Repairing<Unavailable<Problem, Repair>, Repair, Task>),
    /// Last-known-good data and the latest problem are available.
    Degraded(Degraded<Data, Problem, Repair>),
    /// A caller-selected repair is running while degraded data remains
    /// available.
    RepairingDegraded(Repairing<Degraded<Data, Problem, Repair>, Repair, Task>),
}

/// A clone-free, comparable projection of [`Operation`]'s current phase.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Phase {
    /// No fetch has been requested.
    Idle,
    /// The first fetch is running.
    Loading,
    /// Valid data is available.
    Ready,
    /// A normal refresh is running while valid data remains available.
    Refreshing,
    /// No valid data exists and the latest fetch failed.
    Unavailable,
    /// A caller-selected repair is running without valid data.
    RepairingUnavailable,
    /// Last-known-good data and the latest problem are available.
    Degraded,
    /// A caller-selected repair is running while degraded data remains
    /// available.
    RepairingDegraded,
}

impl<Data, Problem: std::error::Error, Repair, Task> Operation<Data, Problem, Repair, Task> {
    /// Creates an idle operation.
    pub const fn new() -> Self {
        Self::Idle(Idle::new())
    }

    /// Returns the current phase without borrowing any payload.
    pub fn phase(&self) -> Phase {
        match self {
            Self::Idle(_) => Phase::Idle,
            Self::Loading(_) => Phase::Loading,
            Self::Ready(_) => Phase::Ready,
            Self::Refreshing(_) => Phase::Refreshing,
            Self::Unavailable(_) => Phase::Unavailable,
            Self::RepairingUnavailable(_) => Phase::RepairingUnavailable,
            Self::Degraded(_) => Phase::Degraded,
            Self::RepairingDegraded(_) => Phase::RepairingDegraded,
        }
    }

    /// Borrows the current valid data, including retained data during refresh,
    /// degraded, and degraded-repair states.
    pub fn data(&self) -> Option<&Data> {
        match self {
            Self::Ready(state) => Some(state.data()),
            Self::Refreshing(state) => Some(state.previous().data()),
            Self::Degraded(state) => Some(state.data()),
            Self::RepairingDegraded(state) => Some(state.previous().data()),
            Self::Idle(_)
            | Self::Loading(_)
            | Self::Unavailable(_)
            | Self::RepairingUnavailable(_) => None,
        }
    }

    /// Borrows the latest problem, including the retained problem while a
    /// repair is running.
    pub fn problem(&self) -> Option<&Problem> {
        match self {
            Self::Unavailable(state) => Some(state.problem()),
            Self::RepairingUnavailable(state) => Some(state.previous().problem()),
            Self::Degraded(state) => Some(state.problem()),
            Self::RepairingDegraded(state) => Some(state.previous().problem()),
            Self::Idle(_) | Self::Loading(_) | Self::Ready(_) | Self::Refreshing(_) => None,
        }
    }

    /// Borrows the caller-selected Repair while one is running.
    pub fn active_repair(&self) -> Option<&Repair> {
        match self {
            Self::RepairingUnavailable(state) => Some(state.repair()),
            Self::RepairingDegraded(state) => Some(state.repair()),
            _ => None,
        }
    }

    /// Returns whether this operation currently owns a running Task.
    pub fn is_running(&self) -> bool {
        matches!(
            self,
            Self::Loading(_)
                | Self::Refreshing(_)
                | Self::RepairingUnavailable(_)
                | Self::RepairingDegraded(_)
        )
    }
}

impl<Data, Problem: std::error::Error, Repair, Task> Default
    for Operation<Data, Problem, Repair, Task>
{
    fn default() -> Self {
        Self::new()
    }
}

fn trace_ignored<Message>(phase: Phase) {
    #[cfg(feature = "tracing")]
    tracing::debug!(
        family = "repair",
        ?phase,
        message = std::any::type_name::<Message>(),
        "ignored operation message"
    );

    #[cfg(not(feature = "tracing"))]
    let _ = phase;
}

// ── Complete runtime enum transitions ──────────────────────────────────

impl<Data, Problem: std::error::Error, Repair, Task> Transition<Settle<Data, Problem>>
    for &mut Operation<Data, Problem, Repair, Task>
{
    type Output = ();

    fn transition(self, message: Settle<Data, Problem>) {
        let current = std::mem::take(self);
        match current {
            Operation::Idle(_) => {
                *self = match message.0 {
                    Ok(data) => Operation::Ready(Ready {
                        data,
                        marker: PhantomData,
                    }),
                    Err(problem) => Operation::Unavailable(Unavailable {
                        problem,
                        marker: PhantomData,
                    }),
                };
            }
            current => {
                *self = current;
                trace_ignored::<Settle<Data, Problem>>(self.phase());
                drop(message);
            }
        }
    }
}

impl<Data, Problem: std::error::Error, Repair, Task> Transition<Load<Task>>
    for &mut Operation<Data, Problem, Repair, Task>
{
    type Output = ();

    fn transition(self, message: Load<Task>) {
        let current = std::mem::take(self);
        match current {
            Operation::Idle(state) => {
                *self = Operation::Loading(state.transition(message));
            }
            current => {
                *self = current;
                trace_ignored::<Load<Task>>(self.phase());
                drop(message);
            }
        }
    }
}

impl<Data, Problem: std::error::Error, Repair, Task> Transition<Refresh<Task>>
    for &mut Operation<Data, Problem, Repair, Task>
{
    type Output = ();

    fn transition(self, message: Refresh<Task>) {
        let current = std::mem::take(self);
        match current {
            Operation::Ready(state) => {
                *self = Operation::Refreshing(state.transition(message));
            }
            current => {
                *self = current;
                trace_ignored::<Refresh<Task>>(self.phase());
                drop(message);
            }
        }
    }
}

impl<Data, Problem: std::error::Error, Repair, Task> Transition<crate::Repair<Repair, Task>>
    for &mut Operation<Data, Problem, Repair, Task>
{
    type Output = ();

    fn transition(self, message: crate::Repair<Repair, Task>) {
        let current = std::mem::take(self);
        match current {
            Operation::Unavailable(state) => {
                *self = Operation::RepairingUnavailable(state.transition(message));
            }
            Operation::Degraded(state) => {
                *self = Operation::RepairingDegraded(state.transition(message));
            }
            current => {
                *self = current;
                trace_ignored::<crate::Repair<Repair, Task>>(self.phase());
                drop(message);
            }
        }
    }
}

impl<Data, Problem: std::error::Error, Repair, Task> Transition<Complete<Data, Problem>>
    for &mut Operation<Data, Problem, Repair, Task>
{
    type Output = ();

    fn transition(self, message: Complete<Data, Problem>) {
        let Complete(result) = message;
        let current = std::mem::take(self);

        match current {
            Operation::Loading(Fetching { task, previous }) => {
                *self = match result {
                    Ok(data) => Operation::Ready(Ready {
                        data,
                        marker: PhantomData,
                    }),
                    Err(problem) => Operation::Unavailable(Unavailable {
                        problem,
                        marker: PhantomData,
                    }),
                };
                drop(task);
                drop(previous);
            }
            Operation::Refreshing(Fetching {
                task,
                previous: Ready { data: old_data, .. },
            }) => match result {
                Ok(data) => {
                    *self = Operation::Ready(Ready {
                        data,
                        marker: PhantomData,
                    });
                    drop(task);
                    drop(old_data);
                }
                Err(problem) => {
                    *self = Operation::Degraded(Degraded {
                        data: old_data,
                        problem,
                        marker: PhantomData,
                    });
                    drop(task);
                }
            },
            Operation::RepairingUnavailable(Repairing {
                task,
                repair,
                previous:
                    Unavailable {
                        problem: old_problem,
                        ..
                    },
            }) => {
                *self = match result {
                    Ok(data) => Operation::Ready(Ready {
                        data,
                        marker: PhantomData,
                    }),
                    Err(problem) => Operation::Unavailable(Unavailable {
                        problem,
                        marker: PhantomData,
                    }),
                };
                drop(task);
                drop(repair);
                drop(old_problem);
            }
            Operation::RepairingDegraded(Repairing {
                task,
                repair,
                previous:
                    Degraded {
                        data: old_data,
                        problem: old_problem,
                        ..
                    },
            }) => match result {
                Ok(data) => {
                    *self = Operation::Ready(Ready {
                        data,
                        marker: PhantomData,
                    });
                    drop(task);
                    drop(repair);
                    drop(old_data);
                    drop(old_problem);
                }
                Err(problem) => {
                    *self = Operation::Degraded(Degraded {
                        data: old_data,
                        problem,
                        marker: PhantomData,
                    });
                    drop(task);
                    drop(repair);
                    drop(old_problem);
                }
            },
            current => {
                *self = current;
                trace_ignored::<Complete<Data, Problem>>(self.phase());
                drop(result);
            }
        }
    }
}

impl<Data, Problem: std::error::Error, Repair, Task> Transition<Cancel>
    for &mut Operation<Data, Problem, Repair, Task>
{
    type Output = ();

    fn transition(self, _message: Cancel) {
        let current = std::mem::take(self);

        match current {
            Operation::Loading(Fetching { task, previous }) => {
                *self = Operation::Idle(previous);
                drop(task);
            }
            Operation::Refreshing(Fetching { task, previous }) => {
                *self = Operation::Ready(previous);
                drop(task);
            }
            Operation::RepairingUnavailable(Repairing {
                task,
                repair,
                previous,
            }) => {
                *self = Operation::Unavailable(previous);
                drop(task);
                drop(repair);
            }
            Operation::RepairingDegraded(Repairing {
                task,
                repair,
                previous,
            }) => {
                *self = Operation::Degraded(previous);
                drop(task);
                drop(repair);
            }
            current => {
                *self = current;
                trace_ignored::<Cancel>(self.phase());
            }
        }
    }
}

// ── Transition implementations ──────────────────────────────────────────

impl<Data, Problem: std::error::Error, Repair> Transition<Settle<Data, Problem>> for Idle<Repair> {
    type Output = FetchCompleted<Data, Problem, Repair>;

    fn transition(self, message: Settle<Data, Problem>) -> Self::Output {
        match message.0 {
            Ok(data) => FetchCompleted::Ready(Ready {
                data,
                marker: PhantomData,
            }),
            Err(problem) => FetchCompleted::Unavailable(Unavailable {
                problem,
                marker: PhantomData,
            }),
        }
    }
}

impl<Repair, Task> Transition<Load<Task>> for Idle<Repair> {
    type Output = Fetching<Idle<Repair>, Task>;

    fn transition(self, message: Load<Task>) -> Self::Output {
        Fetching {
            task: message.0,
            previous: self,
        }
    }
}

impl<Data, Repair, Task> Transition<Refresh<Task>> for Ready<Data, Repair> {
    type Output = Fetching<Ready<Data, Repair>, Task>;

    fn transition(self, message: Refresh<Task>) -> Self::Output {
        Fetching {
            task: message.0,
            previous: self,
        }
    }
}

impl<Problem: std::error::Error, Repair, Task> Transition<crate::Repair<Repair, Task>>
    for Unavailable<Problem, Repair>
{
    type Output = Repairing<Unavailable<Problem, Repair>, Repair, Task>;

    fn transition(self, message: crate::Repair<Repair, Task>) -> Self::Output {
        Repairing {
            task: message.task,
            repair: message.repair,
            previous: self,
        }
    }
}

impl<Data, Problem: std::error::Error, Repair, Task> Transition<crate::Repair<Repair, Task>>
    for Degraded<Data, Problem, Repair>
{
    type Output = Repairing<Degraded<Data, Problem, Repair>, Repair, Task>;

    fn transition(self, message: crate::Repair<Repair, Task>) -> Self::Output {
        Repairing {
            task: message.task,
            repair: message.repair,
            previous: self,
        }
    }
}

// ── Cancel transitions ──────────────────────────────────────────────────

impl<Previous, Task> Transition<Cancel> for Fetching<Previous, Task> {
    type Output = Previous;

    fn transition(self, _message: Cancel) -> Self::Output {
        let Self { task, previous } = self;
        drop(task);
        previous
    }
}

impl<Previous, Repair, Task> Transition<Cancel> for Repairing<Previous, Repair, Task> {
    type Output = Previous;

    fn transition(self, _message: Cancel) -> Self::Output {
        let Self {
            task,
            repair,
            previous,
        } = self;
        drop(task);
        drop(repair);
        previous
    }
}

// ── Completion transitions ──────────────────────────────────────────────

impl<Data, Problem: std::error::Error, Repair, Task> Transition<Complete<Data, Problem>>
    for Fetching<Idle<Repair>, Task>
{
    type Output = FetchCompleted<Data, Problem, Repair>;

    fn transition(self, message: Complete<Data, Problem>) -> Self::Output {
        let Self { task, previous: _ } = self;
        drop(task);
        match message.0 {
            Ok(data) => FetchCompleted::Ready(Ready {
                data,
                marker: PhantomData,
            }),
            Err(problem) => FetchCompleted::Unavailable(Unavailable {
                problem,
                marker: PhantomData,
            }),
        }
    }
}

impl<Data, Problem: std::error::Error, Repair, Task> Transition<Complete<Data, Problem>>
    for Fetching<Ready<Data, Repair>, Task>
{
    type Output = RefreshCompleted<Data, Problem, Repair>;

    fn transition(self, message: Complete<Data, Problem>) -> Self::Output {
        let Self {
            task,
            previous: Ready { data: old_data, .. },
        } = self;
        drop(task);
        match message.0 {
            Ok(new_data) => RefreshCompleted::Ready(Ready {
                data: new_data,
                marker: PhantomData,
            }),
            Err(new_problem) => RefreshCompleted::Degraded(Degraded {
                data: old_data,
                problem: new_problem,
                marker: PhantomData,
            }),
        }
    }
}

impl<Data, Problem: std::error::Error, Repair, Task> Transition<Complete<Data, Problem>>
    for Repairing<Unavailable<Problem, Repair>, Repair, Task>
{
    type Output = RepairWithoutDataCompleted<Data, Problem, Repair>;

    fn transition(self, message: Complete<Data, Problem>) -> Self::Output {
        let Self {
            task,
            repair,
            previous: Unavailable { .. },
        } = self;
        drop(task);
        drop(repair);
        match message.0 {
            Ok(data) => RepairWithoutDataCompleted::Ready(Ready {
                data,
                marker: PhantomData,
            }),
            Err(problem) => RepairWithoutDataCompleted::Unavailable(Unavailable {
                problem,
                marker: PhantomData,
            }),
        }
    }
}

impl<Data, Problem: std::error::Error, Repair, Task> Transition<Complete<Data, Problem>>
    for Repairing<Degraded<Data, Problem, Repair>, Repair, Task>
{
    type Output = RepairWithDataCompleted<Data, Problem, Repair>;

    fn transition(self, message: Complete<Data, Problem>) -> Self::Output {
        let Self {
            task,
            repair,
            previous: Degraded { data: old_data, .. },
        } = self;
        drop(task);
        drop(repair);
        match message.0 {
            Ok(new_data) => RepairWithDataCompleted::Ready(Ready {
                data: new_data,
                marker: PhantomData,
            }),
            Err(new_problem) => RepairWithDataCompleted::Degraded(Degraded {
                data: old_data,
                problem: new_problem,
                marker: PhantomData,
            }),
        }
    }
}

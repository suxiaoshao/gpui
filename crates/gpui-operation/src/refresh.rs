//! Repeatable fetch/retry lifecycle with no explicit repair step.
//!
//! Use [`Operation`] when an owner needs to retain the complete lifecycle.
//! The named states and [`Transition`] implementations remain available when
//! a caller directly owns one exact state.
//!
//! Settled states: [`Idle`], [`Ready`], [`Unavailable`], [`Degraded`].
//! Running state: [`Fetching`], which owns a Task and the previous state.
//!
//! # Transitions
//!
//! | Current | Message | Output |
//! |---|---|---|
//! | `Idle` | `Settle<Result<Data, Problem>>` | `FetchCompleted<Data, Problem>` |
//! | `Idle` | `Load<Task>` | `Fetching<Idle, Task>` |
//! | `Ready<Data>` | `Refresh<Task>` | `Fetching<Ready<Data>, Task>` |
//! | `Unavailable<Problem>` | `Retry<Task>` | `Fetching<Unavailable<Problem>, Task>` |
//! | `Degraded<Data, Problem>` | `Refresh<Task>` | `Fetching<Degraded<Data, Problem>, Task>` |
//! | `Fetching<Previous, Task>` | `Cancel` | `Previous` |
//! | `Fetching<Idle, Task>` | `Complete<Data, Problem>` | `FetchCompleted<Data, Problem>` |
//! | `Fetching<Unavailable<Problem>, Task>` | `Complete<Data, Problem>` | `FetchCompleted<Data, Problem>` |
//! | `Fetching<Ready<Data>, Task>` | `Complete<Data, Problem>` | `RefreshCompleted<Data, Problem>` |
//! | `Fetching<Degraded<Data, Problem>, Task>` | `Complete<Data, Problem>` | `RefreshCompleted<Data, Problem>` |

use crate::{Cancel, Complete, Load, Refresh, Retry, Settle, Transition};

/// No fetch has ever been requested.
///
/// Valid transitions are [`Settle`] for synchronous initial work and [`Load`]
/// for asynchronous initial work.
///
/// ```compile_fail
/// use gpui_operation::{Cancel, Transition};
/// use gpui_operation::refresh::Idle;
///
/// // Idle cannot be cancelled — there is nothing running.
/// Idle::new().transition(Cancel);
/// ```
#[must_use = "operation states must be retained or transitioned"]
pub struct Idle {
    _private: (),
}

impl Idle {
    /// Creates an idle operation state.
    pub const fn new() -> Self {
        Self { _private: () }
    }
}

impl Default for Idle {
    fn default() -> Self {
        Self::new()
    }
}

/// Valid data is available.
///
/// Domain messages are delegated to `&mut Data`, preserving the delegated
/// transition's output.
#[must_use = "operation states must be retained or transitioned"]
pub struct Ready<Data> {
    data: Data,
}

impl<Data> Ready<Data> {
    /// Borrows the current valid data.
    pub fn data(&self) -> &Data {
        &self.data
    }
}

impl<'ready, Data, Message> Transition<Message> for &'ready mut Ready<Data>
where
    &'ready mut Data: Transition<Message>,
{
    type Output = <&'ready mut Data as Transition<Message>>::Output;

    fn transition(self, message: Message) -> Self::Output {
        (&mut self.data).transition(message)
    }
}

/// A problem is available and no valid data exists.
#[must_use = "operation states must be retained or transitioned"]
pub struct Unavailable<Problem: std::error::Error> {
    problem: Problem,
}

impl<Problem: std::error::Error> Unavailable<Problem> {
    /// Borrows the latest problem.
    pub fn problem(&self) -> &Problem {
        &self.problem
    }
}

/// Valid, potentially stale data and the latest problem are available.
#[must_use = "operation states must be retained or transitioned"]
pub struct Degraded<Data, Problem: std::error::Error> {
    data: Data,
    problem: Problem,
}

impl<Data, Problem: std::error::Error> Degraded<Data, Problem> {
    /// Borrows the last-known-good data.
    pub fn data(&self) -> &Data {
        &self.data
    }

    /// Borrows the latest problem.
    pub fn problem(&self) -> &Problem {
        &self.problem
    }
}

/// A fetch is running. The owned field order guarantees that the Task is
/// dropped before the previous state when this value is directly discarded.
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

/// The result of a completion where no valid data existed before.
#[must_use]
pub enum FetchCompleted<Data, Problem: std::error::Error> {
    /// Fetch succeeded.
    Ready(Ready<Data>),
    /// Fetch failed.
    Unavailable(Unavailable<Problem>),
}

/// The result of a completion where valid data existed before.
#[must_use]
pub enum RefreshCompleted<Data, Problem: std::error::Error> {
    /// Fetch succeeded; old data is discarded.
    Ready(Ready<Data>),
    /// Fetch failed; old data is retained alongside the new problem.
    Degraded(Degraded<Data, Problem>),
}

/// The complete refresh-only lifecycle for long-term storage in an owner.
///
/// The enum owns the only authoritative operation state. It is suitable for
/// storage in an Entity, Global, `gpui-store::Store`, or an ordinary field.
/// Callers construct Tasks and route completion, while this type owns all
/// state matching and movement.
#[must_use = "operation state must be retained"]
pub enum Operation<Data, Problem: std::error::Error, Task> {
    /// No fetch has been requested.
    Idle(Idle),
    /// The first fetch is running.
    Loading(Fetching<Idle, Task>),
    /// Valid data is available.
    Ready(Ready<Data>),
    /// A refresh is running while valid data remains available.
    Refreshing(Fetching<Ready<Data>, Task>),
    /// No valid data exists and the latest fetch failed.
    Unavailable(Unavailable<Problem>),
    /// A retry is running after a fetch failed without valid data.
    Retrying(Fetching<Unavailable<Problem>, Task>),
    /// Last-known-good data and the latest refresh problem are available.
    Degraded(Degraded<Data, Problem>),
    /// A refresh is running while degraded data remains available.
    RefreshingDegraded(Fetching<Degraded<Data, Problem>, Task>),
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
    /// A refresh is running while valid data remains available.
    Refreshing,
    /// No valid data exists and the latest fetch failed.
    Unavailable,
    /// A retry is running after a fetch failed without valid data.
    Retrying,
    /// Last-known-good data and the latest refresh problem are available.
    Degraded,
    /// A refresh is running while degraded data remains available.
    RefreshingDegraded,
}

impl<Data, Problem: std::error::Error, Task> Operation<Data, Problem, Task> {
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
            Self::Retrying(_) => Phase::Retrying,
            Self::Degraded(_) => Phase::Degraded,
            Self::RefreshingDegraded(_) => Phase::RefreshingDegraded,
        }
    }

    /// Borrows the current valid data, including retained data during refresh
    /// and degraded states.
    pub fn data(&self) -> Option<&Data> {
        match self {
            Self::Ready(state) => Some(state.data()),
            Self::Refreshing(state) => Some(state.previous().data()),
            Self::Degraded(state) => Some(state.data()),
            Self::RefreshingDegraded(state) => Some(state.previous().data()),
            Self::Idle(_) | Self::Loading(_) | Self::Unavailable(_) | Self::Retrying(_) => None,
        }
    }

    /// Borrows the latest problem, including the retained problem while a
    /// retry or degraded refresh is running.
    pub fn problem(&self) -> Option<&Problem> {
        match self {
            Self::Unavailable(state) => Some(state.problem()),
            Self::Retrying(state) => Some(state.previous().problem()),
            Self::Degraded(state) => Some(state.problem()),
            Self::RefreshingDegraded(state) => Some(state.previous().problem()),
            Self::Idle(_) | Self::Loading(_) | Self::Ready(_) | Self::Refreshing(_) => None,
        }
    }

    /// Returns whether this operation currently owns a running Task.
    pub fn is_running(&self) -> bool {
        matches!(
            self,
            Self::Loading(_)
                | Self::Refreshing(_)
                | Self::Retrying(_)
                | Self::RefreshingDegraded(_)
        )
    }
}

impl<Data, Problem: std::error::Error, Task> Default for Operation<Data, Problem, Task> {
    fn default() -> Self {
        Self::new()
    }
}

fn trace_ignored<Message>(phase: Phase) {
    #[cfg(feature = "tracing")]
    tracing::debug!(
        family = "refresh",
        ?phase,
        message = std::any::type_name::<Message>(),
        "ignored operation message"
    );

    #[cfg(not(feature = "tracing"))]
    let _ = phase;
}

// ── Complete runtime enum transitions ──────────────────────────────────

impl<Data, Problem: std::error::Error, Task> Transition<Settle<Data, Problem>>
    for &mut Operation<Data, Problem, Task>
{
    type Output = ();

    fn transition(self, message: Settle<Data, Problem>) {
        let current = std::mem::take(self);
        match current {
            Operation::Idle(_) => {
                *self = match message.0 {
                    Ok(data) => Operation::Ready(Ready { data }),
                    Err(problem) => Operation::Unavailable(Unavailable { problem }),
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

impl<Data, Problem: std::error::Error, Task> Transition<Load<Task>>
    for &mut Operation<Data, Problem, Task>
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

impl<Data, Problem: std::error::Error, Task> Transition<Refresh<Task>>
    for &mut Operation<Data, Problem, Task>
{
    type Output = ();

    fn transition(self, message: Refresh<Task>) {
        let current = std::mem::take(self);
        match current {
            Operation::Ready(state) => {
                *self = Operation::Refreshing(state.transition(message));
            }
            Operation::Degraded(state) => {
                *self = Operation::RefreshingDegraded(state.transition(message));
            }
            current => {
                *self = current;
                trace_ignored::<Refresh<Task>>(self.phase());
                drop(message);
            }
        }
    }
}

impl<Data, Problem: std::error::Error, Task> Transition<Retry<Task>>
    for &mut Operation<Data, Problem, Task>
{
    type Output = ();

    fn transition(self, message: Retry<Task>) {
        let current = std::mem::take(self);
        match current {
            Operation::Unavailable(state) => {
                *self = Operation::Retrying(state.transition(message));
            }
            current => {
                *self = current;
                trace_ignored::<Retry<Task>>(self.phase());
                drop(message);
            }
        }
    }
}

impl<Data, Problem: std::error::Error, Task> Transition<Complete<Data, Problem>>
    for &mut Operation<Data, Problem, Task>
{
    type Output = ();

    fn transition(self, message: Complete<Data, Problem>) {
        let Complete(result) = message;
        let current = std::mem::take(self);

        match current {
            Operation::Loading(Fetching { task, previous }) => {
                *self = match result {
                    Ok(data) => Operation::Ready(Ready { data }),
                    Err(problem) => Operation::Unavailable(Unavailable { problem }),
                };
                drop(task);
                drop(previous);
            }
            Operation::Refreshing(Fetching {
                task,
                previous: Ready { data: old_data },
            }) => match result {
                Ok(data) => {
                    *self = Operation::Ready(Ready { data });
                    drop(task);
                    drop(old_data);
                }
                Err(problem) => {
                    *self = Operation::Degraded(Degraded {
                        data: old_data,
                        problem,
                    });
                    drop(task);
                }
            },
            Operation::Retrying(Fetching {
                task,
                previous: Unavailable {
                    problem: old_problem,
                },
            }) => {
                *self = match result {
                    Ok(data) => Operation::Ready(Ready { data }),
                    Err(problem) => Operation::Unavailable(Unavailable { problem }),
                };
                drop(task);
                drop(old_problem);
            }
            Operation::RefreshingDegraded(Fetching {
                task,
                previous:
                    Degraded {
                        data: old_data,
                        problem: old_problem,
                    },
            }) => match result {
                Ok(data) => {
                    *self = Operation::Ready(Ready { data });
                    drop(task);
                    drop(old_data);
                    drop(old_problem);
                }
                Err(problem) => {
                    *self = Operation::Degraded(Degraded {
                        data: old_data,
                        problem,
                    });
                    drop(task);
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

impl<Data, Problem: std::error::Error, Task> Transition<Cancel>
    for &mut Operation<Data, Problem, Task>
{
    type Output = ();

    fn transition(self, _message: Cancel) {
        let current = std::mem::take(self);

        let task = match current {
            Operation::Loading(Fetching { task, previous }) => {
                *self = Operation::Idle(previous);
                task
            }
            Operation::Refreshing(Fetching { task, previous }) => {
                *self = Operation::Ready(previous);
                task
            }
            Operation::Retrying(Fetching { task, previous }) => {
                *self = Operation::Unavailable(previous);
                task
            }
            Operation::RefreshingDegraded(Fetching { task, previous }) => {
                *self = Operation::Degraded(previous);
                task
            }
            current => {
                *self = current;
                trace_ignored::<Cancel>(self.phase());
                return;
            }
        };

        drop(task);
    }
}

// ── Transition implementations ──────────────────────────────────────────

impl<Data, Problem: std::error::Error> Transition<Settle<Data, Problem>> for Idle {
    type Output = FetchCompleted<Data, Problem>;

    fn transition(self, message: Settle<Data, Problem>) -> Self::Output {
        match message.0 {
            Ok(data) => FetchCompleted::Ready(Ready { data }),
            Err(problem) => FetchCompleted::Unavailable(Unavailable { problem }),
        }
    }
}

impl<Task> Transition<Load<Task>> for Idle {
    type Output = Fetching<Idle, Task>;

    fn transition(self, message: Load<Task>) -> Self::Output {
        Fetching {
            task: message.0,
            previous: self,
        }
    }
}

impl<Data, Task> Transition<Refresh<Task>> for Ready<Data> {
    type Output = Fetching<Ready<Data>, Task>;

    fn transition(self, message: Refresh<Task>) -> Self::Output {
        Fetching {
            task: message.0,
            previous: self,
        }
    }
}

impl<Problem: std::error::Error, Task> Transition<Retry<Task>> for Unavailable<Problem> {
    type Output = Fetching<Unavailable<Problem>, Task>;

    fn transition(self, message: Retry<Task>) -> Self::Output {
        Fetching {
            task: message.0,
            previous: self,
        }
    }
}

impl<Data, Problem: std::error::Error, Task> Transition<Refresh<Task>> for Degraded<Data, Problem> {
    type Output = Fetching<Degraded<Data, Problem>, Task>;

    fn transition(self, message: Refresh<Task>) -> Self::Output {
        Fetching {
            task: message.0,
            previous: self,
        }
    }
}

impl<Previous, Task> Transition<Cancel> for Fetching<Previous, Task> {
    type Output = Previous;

    fn transition(self, _message: Cancel) -> Self::Output {
        let Self { task, previous } = self;
        drop(task);
        previous
    }
}

impl<Data, Problem: std::error::Error, Task> Transition<Complete<Data, Problem>>
    for Fetching<Idle, Task>
{
    type Output = FetchCompleted<Data, Problem>;

    fn transition(self, message: Complete<Data, Problem>) -> Self::Output {
        let Self { task, previous: _ } = self;
        drop(task);
        match message.0 {
            Ok(data) => FetchCompleted::Ready(Ready { data }),
            Err(problem) => FetchCompleted::Unavailable(Unavailable { problem }),
        }
    }
}

impl<Data, Problem: std::error::Error, Task> Transition<Complete<Data, Problem>>
    for Fetching<Ready<Data>, Task>
{
    type Output = RefreshCompleted<Data, Problem>;

    fn transition(self, message: Complete<Data, Problem>) -> Self::Output {
        let Self {
            task,
            previous: Ready { data: old_data },
        } = self;
        drop(task);
        match message.0 {
            Ok(new_data) => RefreshCompleted::Ready(Ready { data: new_data }),
            Err(new_problem) => {
                let degraded = Degraded {
                    data: old_data,
                    problem: new_problem,
                };
                RefreshCompleted::Degraded(degraded)
            }
        }
    }
}

impl<Data, Problem: std::error::Error, Task> Transition<Complete<Data, Problem>>
    for Fetching<Unavailable<Problem>, Task>
{
    type Output = FetchCompleted<Data, Problem>;

    fn transition(self, message: Complete<Data, Problem>) -> Self::Output {
        let Self {
            task,
            previous: Unavailable { problem: _ },
        } = self;
        drop(task);
        match message.0 {
            Ok(data) => FetchCompleted::Ready(Ready { data }),
            Err(problem) => FetchCompleted::Unavailable(Unavailable { problem }),
        }
    }
}

impl<Data, Problem: std::error::Error, Task> Transition<Complete<Data, Problem>>
    for Fetching<Degraded<Data, Problem>, Task>
{
    type Output = RefreshCompleted<Data, Problem>;

    fn transition(self, message: Complete<Data, Problem>) -> Self::Output {
        let Self {
            task,
            previous:
                Degraded {
                    data: old_data,
                    problem: _,
                },
        } = self;
        drop(task);
        match message.0 {
            Ok(new_data) => RefreshCompleted::Ready(Ready { data: new_data }),
            Err(new_problem) => {
                let degraded = Degraded {
                    data: old_data,
                    problem: new_problem,
                };
                RefreshCompleted::Degraded(degraded)
            }
        }
    }
}

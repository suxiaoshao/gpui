/// Kicks off the first fetch.
#[must_use]
pub struct Load<Task>(pub Task);

/// Kicks off a normal fetch while valid data is retained.
#[must_use]
pub struct Refresh<Task>(pub Task);

/// Retries a fetch that previously failed.
#[must_use]
pub struct Retry<Task>(pub Task);

/// Kicks off a caller-selected repair.
#[must_use]
pub struct Repair<Kind, Task> {
    /// The caller-selected repair to execute.
    pub repair: Kind,
    /// The async work that carries out the repair.
    pub task: Task,
}

/// The result of one completed attempt.
#[must_use]
pub struct Complete<Data, Problem: std::error::Error>(pub Result<Data, Problem>);

/// Cancels the active attempt and restores the previous settled state.
#[must_use]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct Cancel;

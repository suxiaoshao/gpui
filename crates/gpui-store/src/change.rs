/// Carries a business result together with the caller's decision about
/// whether the mutation should publish a change notification.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreChange<R> {
    /// The state was meaningfully changed; `Store::update_if` will notify.
    Changed(R),
    /// No meaningful change occurred; `Store::update_if` will **not** notify.
    Unchanged(R),
}

impl<R> StoreChange<R> {
    /// Shorthand for `StoreChange::Changed(result)`.
    pub fn changed(result: R) -> Self {
        Self::Changed(result)
    }

    /// Shorthand for `StoreChange::Unchanged(result)`.
    pub fn unchanged(result: R) -> Self {
        Self::Unchanged(result)
    }

    /// Returns `true` when the variant is `Changed`.
    pub fn is_changed(&self) -> bool {
        matches!(self, Self::Changed(_))
    }

    /// Unwraps the inner result regardless of the decision.
    pub fn into_result(self) -> R {
        match self {
            Self::Changed(r) | Self::Unchanged(r) => r,
        }
    }
}

/// Selects a projected value from a store snapshot.
///
/// Implementations must be pure and deterministic — they are called both
/// synchronously during selection creation and on every subsequent store
/// publication.
pub trait Select<S: ?Sized> {
    /// The type of the projected value.
    type Output;

    /// Compute the projection from the current store state.
    fn select(&self, source: &S) -> Self::Output;
}

/// Every closure `Fn(&S) -> T` is a valid [`Select`].
impl<S: ?Sized, F, T> Select<S> for F
where
    F: Fn(&S) -> T,
{
    type Output = T;

    fn select(&self, source: &S) -> T {
        self(source)
    }
}

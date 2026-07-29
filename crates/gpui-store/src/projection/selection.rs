use std::rc::Rc;

use gpui::Subscription;

use crate::store::SelectionCell;

/// An owner-bound read-only projection of store state that stays in sync
/// automatically.
///
/// Created by [`Store::select`].
///
/// A `StoreSelection` holds a [`Subscription`] — dropping the selection
/// unsubscribes from further updates. When the source `Store` is dropped
/// first, the selection retains its last-known value but stops updating.
///
/// [`Store::select`]: crate::Store::select
#[must_use]
pub struct StoreSelection<T> {
    pub(crate) snapshot: Rc<SelectionCell<T>>,
    pub(crate) _subscription: Subscription,
}

impl<T> StoreSelection<T> {
    /// Reads a projection from the current selected value.
    pub fn read<R>(&self, read: impl FnOnce(&T) -> R) -> R {
        self.snapshot.read(read)
    }

    /// Returns a clone of the current selected value.
    pub fn cloned(&self) -> T
    where
        T: Clone,
    {
        self.read(Clone::clone)
    }
}

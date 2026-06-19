use std::{
    borrow::Borrow,
    cell::{Cell, UnsafeCell},
    fmt,
    ops::Deref,
    rc::Rc,
};

use gpui::Subscription;

use crate::{StoreRevision, StoreState};

pub(crate) struct SnapshotCell<T> {
    value: UnsafeCell<T>,
}

impl<T> SnapshotCell<T> {
    pub(crate) fn new(value: T) -> Self {
        Self {
            value: UnsafeCell::new(value),
        }
    }

    pub(crate) fn get(&self) -> &T {
        // The store runs on GPUI's single-threaded app executor. This cell never
        // exposes `&mut T`; updates replace the snapshot only inside GPUI
        // notification callbacks.
        unsafe { &*self.value.get() }
    }

    pub(crate) fn replace(&self, value: T) {
        // No mutable reference is handed out by the public API. Mutation is
        // serialized by GPUI callbacks and direct store operations.
        unsafe {
            *self.value.get() = value;
        }
    }
}

pub struct StoreSelection<T> {
    pub(crate) snapshot: Rc<SnapshotCell<T>>,
    pub(crate) store_revision: Rc<Cell<StoreRevision>>,
    pub(crate) _subscription: Subscription,
}

impl<T> StoreSelection<T> {
    pub(crate) fn from_parts(
        snapshot: Rc<SnapshotCell<T>>,
        store_revision: Rc<Cell<StoreRevision>>,
        subscription: Subscription,
    ) -> Self {
        Self {
            snapshot,
            store_revision,
            _subscription: subscription,
        }
    }

    pub fn get(&self) -> &T {
        self.snapshot.get()
    }

    pub fn store_revision(&self) -> StoreRevision {
        self.store_revision.get()
    }
}

impl<T> Deref for StoreSelection<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl<T> AsRef<T> for StoreSelection<T> {
    fn as_ref(&self) -> &T {
        self.get()
    }
}

impl<T> Borrow<T> for StoreSelection<T> {
    fn borrow(&self) -> &T {
        self.get()
    }
}

impl<T: fmt::Debug> fmt::Debug for StoreSelection<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.get().fmt(f)
    }
}

impl<T: fmt::Display> fmt::Display for StoreSelection<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.get().fmt(f)
    }
}

impl<T: PartialEq> PartialEq for StoreSelection<T> {
    fn eq(&self, other: &Self) -> bool {
        self.get() == other.get()
    }
}

impl<T: Eq> Eq for StoreSelection<T> {}

impl<T: PartialEq> PartialEq<T> for StoreSelection<T> {
    fn eq(&self, other: &T) -> bool {
        self.get() == other
    }
}

impl<T> StoreSelection<T> {
    pub(crate) fn new<S, Source, Owner>(
        store: crate::SharedStore<S, Source>,
        cx: &mut gpui::Context<Owner>,
        select: impl Fn(&S) -> T + 'static,
    ) -> Self
    where
        S: StoreState,
        Source: crate::StoreBackend<S>,
        Owner: 'static,
        T: PartialEq + 'static,
    {
        let entity = store.entity();
        let selector = Rc::new(select);
        let (snapshot, revision) = entity.read_with(cx, |runtime, _| {
            (selector(runtime.state()), runtime.revision())
        });
        let snapshot = Rc::new(SnapshotCell::new(snapshot));
        let store_revision = Rc::new(Cell::new(revision));

        let observed_snapshot = snapshot.clone();
        let observed_revision = store_revision.clone();
        let observed_selector = selector.clone();
        let subscription = cx.observe(&entity, move |_owner, observed, cx| {
            let (next_snapshot, next_revision) = observed.read_with(cx, |runtime, _| {
                (observed_selector(runtime.state()), runtime.revision())
            });
            observed_revision.set(next_revision);

            if observed_snapshot.get() != &next_snapshot {
                observed_snapshot.replace(next_snapshot);
                cx.notify();
            }
        });

        Self::from_parts(snapshot, store_revision, subscription)
    }
}

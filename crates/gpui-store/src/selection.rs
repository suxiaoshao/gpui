use std::{
    cell::{Cell, RefCell},
    fmt,
    rc::Rc,
};

use gpui::Subscription;

use crate::{StoreRevision, StoreState};

pub(crate) struct SnapshotCell<T> {
    value: RefCell<Rc<T>>,
}

impl<T> SnapshotCell<T> {
    pub(crate) fn new(value: T) -> Self {
        Self {
            value: RefCell::new(Rc::new(value)),
        }
    }

    pub(crate) fn snapshot(&self) -> Rc<T> {
        self.value.borrow().clone()
    }

    pub(crate) fn read<R>(&self, read: impl FnOnce(&T) -> R) -> R {
        let snapshot = self.snapshot();
        read(snapshot.as_ref())
    }

    pub(crate) fn replace(&self, value: T) {
        *self.value.borrow_mut() = Rc::new(value);
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

    pub fn snapshot(&self) -> Rc<T> {
        self.snapshot.snapshot()
    }

    pub fn read<R>(&self, read: impl FnOnce(&T) -> R) -> R {
        self.snapshot.read(read)
    }

    pub fn cloned(&self) -> T
    where
        T: Clone,
    {
        self.read(Clone::clone)
    }

    pub fn store_revision(&self) -> StoreRevision {
        self.store_revision.get()
    }
}

impl<T: fmt::Debug> fmt::Debug for StoreSelection<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.read(|value| value.fmt(f))
    }
}

impl<T: fmt::Display> fmt::Display for StoreSelection<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.read(|value| value.fmt(f))
    }
}

impl<T: PartialEq> PartialEq for StoreSelection<T> {
    fn eq(&self, other: &Self) -> bool {
        let left = self.snapshot();
        let right = other.snapshot();
        left.as_ref() == right.as_ref()
    }
}

impl<T: Eq> Eq for StoreSelection<T> {}

impl<T: PartialEq> PartialEq<T> for StoreSelection<T> {
    fn eq(&self, other: &T) -> bool {
        self.read(|value| value == other)
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

            if observed_snapshot.read(|snapshot| snapshot != &next_snapshot) {
                observed_snapshot.replace(next_snapshot);
                cx.notify();
            }
        });

        Self::from_parts(snapshot, store_revision, subscription)
    }
}

use std::{borrow::Borrow, cell::Cell, fmt, ops::Deref, rc::Rc};

use gpui::{App, Context, Subscription};

use crate::{SharedStore, SnapshotCell, StoreRevision, StoreSource, StoreState, StoreUpdate};

type StoreBindingWriter<T> = dyn Fn(T, &mut App) -> StoreUpdate + 'static;

pub struct StoreBinding<T> {
    snapshot: Rc<SnapshotCell<T>>,
    store_revision: Rc<Cell<StoreRevision>>,
    writer: Rc<StoreBindingWriter<T>>,
    _subscription: Subscription,
}

impl<T> StoreBinding<T> {
    pub(crate) fn new<S, Source, Owner>(
        store: SharedStore<S, Source>,
        cx: &mut Context<Owner>,
        get: impl Fn(&S) -> T + 'static,
        set: impl Fn(&mut S, T) + 'static,
    ) -> Self
    where
        S: StoreState,
        Source: StoreSource<S>,
        Owner: 'static,
        T: Clone + PartialEq + 'static,
    {
        let entity = store.entity();
        let getter = Rc::new(get);
        let setter = Rc::new(set);
        let (snapshot, revision) = entity.read_with(cx, |runtime, _| {
            (getter(runtime.state()), runtime.revision())
        });
        let snapshot = Rc::new(SnapshotCell::new(snapshot));
        let store_revision = Rc::new(Cell::new(revision));

        let observed_snapshot = snapshot.clone();
        let observed_revision = store_revision.clone();
        let observed_getter = getter.clone();
        let subscription = cx.observe(&entity, move |_owner, observed, cx| {
            let (next_snapshot, next_revision) = observed.read_with(cx, |runtime, _| {
                (observed_getter(runtime.state()), runtime.revision())
            });
            observed_revision.set(next_revision);

            if observed_snapshot.get() != &next_snapshot {
                observed_snapshot.replace(next_snapshot);
                cx.notify();
            }
        });

        let writer_entity = entity;
        let writer_getter = getter;
        let writer_setter = setter;
        let writer = Rc::new(move |value: T, cx: &mut App| {
            writer_entity.update(cx, |runtime, cx| {
                runtime.update_if(
                    |state| {
                        let before = writer_getter(state);
                        if before == value {
                            return false;
                        }

                        writer_setter(state, value);
                        writer_getter(state) != before
                    },
                    cx,
                )
            })
        });

        Self {
            snapshot,
            store_revision,
            writer,
            _subscription: subscription,
        }
    }

    pub fn get(&self) -> &T {
        self.snapshot.get()
    }

    pub fn store_revision(&self) -> StoreRevision {
        self.store_revision.get()
    }

    pub fn set<Owner>(&self, cx: &mut Context<Owner>, value: T) -> StoreUpdate
    where
        Owner: 'static,
        T: Clone + PartialEq,
    {
        (self.writer)(value, cx)
    }

    pub fn update<Owner>(&self, cx: &mut Context<Owner>, update: impl FnOnce(&mut T)) -> StoreUpdate
    where
        Owner: 'static,
        T: Clone + PartialEq,
    {
        let mut value = self.get().clone();
        update(&mut value);
        self.set(cx, value)
    }
}

impl<T> Deref for StoreBinding<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl<T> AsRef<T> for StoreBinding<T> {
    fn as_ref(&self) -> &T {
        self.get()
    }
}

impl<T> Borrow<T> for StoreBinding<T> {
    fn borrow(&self) -> &T {
        self.get()
    }
}

impl<T: fmt::Debug> fmt::Debug for StoreBinding<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.get().fmt(f)
    }
}

impl<T: fmt::Display> fmt::Display for StoreBinding<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.get().fmt(f)
    }
}

impl<T: PartialEq> PartialEq for StoreBinding<T> {
    fn eq(&self, other: &Self) -> bool {
        self.get() == other.get()
    }
}

impl<T: Eq> Eq for StoreBinding<T> {}

impl<T: PartialEq> PartialEq<T> for StoreBinding<T> {
    fn eq(&self, other: &T) -> bool {
        self.get() == other
    }
}

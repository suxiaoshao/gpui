use std::{borrow::Borrow, cell::Cell, convert::Infallible, fmt, ops::Deref, rc::Rc};

use gpui::{App, Context, Subscription};

use crate::{
    MemoryBackend, SharedStore, SnapshotCell, StoreBackend, StoreCommitBackend, StoreRevision,
    StoreState, StoreUpdate,
};

type StoreBindingWriter<T, Error> = dyn Fn(T, &mut App) -> Result<StoreUpdate, Error> + 'static;

pub struct StoreBinding<T, Error = Infallible> {
    snapshot: Rc<SnapshotCell<T>>,
    store_revision: Rc<Cell<StoreRevision>>,
    writer: Rc<StoreBindingWriter<T, Error>>,
    _subscription: Subscription,
}

impl<T, Error> StoreBinding<T, Error> {
    pub fn get(&self) -> &T {
        self.snapshot.get()
    }

    pub fn store_revision(&self) -> StoreRevision {
        self.store_revision.get()
    }
}

impl<T, Error> StoreBinding<T, Error>
where
    T: Clone + PartialEq + 'static,
    Error: 'static,
{
    fn from_parts<S, Backend, Owner>(
        store: SharedStore<S, Backend>,
        cx: &mut Context<Owner>,
        getter: Rc<dyn Fn(&S) -> T>,
        writer: Rc<StoreBindingWriter<T, Error>>,
    ) -> Self
    where
        S: StoreState,
        Backend: StoreBackend<S>,
        Owner: 'static,
    {
        let entity = store.entity();
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

        Self {
            snapshot,
            store_revision,
            writer,
            _subscription: subscription,
        }
    }

    pub(crate) fn new_memory<S, Owner>(
        store: SharedStore<S, MemoryBackend>,
        cx: &mut Context<Owner>,
        get: impl Fn(&S) -> T + 'static,
        set: impl Fn(&mut S, T) + 'static,
    ) -> Self
    where
        S: StoreState,
        Owner: 'static,
    {
        let getter: Rc<dyn Fn(&S) -> T> = Rc::new(get);
        let setter = Rc::new(set);
        let writer_store = store.clone();
        let writer_getter = getter.clone();
        let writer_setter = setter.clone();
        let writer = Rc::new(move |value: T, cx: &mut App| {
            Ok(writer_store.update_if(cx, |state| {
                let before = writer_getter(state);
                if before == value {
                    return false;
                }

                writer_setter(state, value.clone());
                writer_getter(state) != before
            }))
        });

        Self::from_parts(store, cx, getter, writer)
    }

    pub(crate) fn new_committed<S, Backend, Owner>(
        store: SharedStore<S, Backend>,
        cx: &mut Context<Owner>,
        get: impl Fn(&S) -> T + 'static,
        set: impl Fn(&mut S, T) + 'static,
    ) -> Self
    where
        S: StoreState + Clone + PartialEq,
        Backend: StoreCommitBackend<S, Error = Error>,
        Owner: 'static,
    {
        let getter: Rc<dyn Fn(&S) -> T> = Rc::new(get);
        let setter = Rc::new(set);
        let writer_store = store.clone();
        let writer_getter = getter.clone();
        let writer_setter = setter.clone();
        let writer = Rc::new(move |value: T, cx: &mut App| {
            writer_store.try_update_if(cx, |state| {
                let before = writer_getter(state);
                if before == value {
                    return false;
                }

                writer_setter(state, value.clone());
                writer_getter(state) != before
            })
        });

        Self::from_parts(store, cx, getter, writer)
    }

    pub fn try_set<Owner>(&self, cx: &mut Context<Owner>, value: T) -> Result<StoreUpdate, Error>
    where
        Owner: 'static,
    {
        (self.writer)(value, cx)
    }

    pub fn try_update<Owner>(
        &self,
        cx: &mut Context<Owner>,
        update: impl FnOnce(&mut T),
    ) -> Result<StoreUpdate, Error>
    where
        Owner: 'static,
    {
        let mut value = self.get().clone();
        update(&mut value);
        self.try_set(cx, value)
    }
}

impl<T> StoreBinding<T, Infallible>
where
    T: Clone + PartialEq + 'static,
{
    pub fn set<Owner>(&self, cx: &mut Context<Owner>, value: T) -> StoreUpdate
    where
        Owner: 'static,
    {
        match self.try_set(cx, value) {
            Ok(update) => update,
            Err(error) => match error {},
        }
    }

    pub fn update<Owner>(&self, cx: &mut Context<Owner>, update: impl FnOnce(&mut T)) -> StoreUpdate
    where
        Owner: 'static,
    {
        match self.try_update(cx, update) {
            Ok(update) => update,
            Err(error) => match error {},
        }
    }
}

impl<T, Error> Deref for StoreBinding<T, Error> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl<T, Error> AsRef<T> for StoreBinding<T, Error> {
    fn as_ref(&self) -> &T {
        self.get()
    }
}

impl<T, Error> Borrow<T> for StoreBinding<T, Error> {
    fn borrow(&self) -> &T {
        self.get()
    }
}

impl<T: fmt::Debug, Error> fmt::Debug for StoreBinding<T, Error> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.get().fmt(f)
    }
}

impl<T: fmt::Display, Error> fmt::Display for StoreBinding<T, Error> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.get().fmt(f)
    }
}

impl<T: PartialEq, Error> PartialEq for StoreBinding<T, Error> {
    fn eq(&self, other: &Self) -> bool {
        self.get() == other.get()
    }
}

impl<T: Eq, Error> Eq for StoreBinding<T, Error> {}

impl<T: PartialEq, Error> PartialEq<T> for StoreBinding<T, Error> {
    fn eq(&self, other: &T) -> bool {
        self.get() == other
    }
}

use std::{convert::Infallible, fmt};

use gpui::App;

use crate::StoreState;

pub type StoreBackendFuture<T, Error> = Result<T, Error>;
pub type StoreBackendCallback<Event> = Box<dyn FnMut(Event, &mut App) + 'static>;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StoreBackendId(String);

impl StoreBackendId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for StoreBackendId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for StoreBackendId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for StoreBackendId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoreCommitAck<Snapshot> {
    snapshot: Option<Snapshot>,
}

impl<Snapshot> StoreCommitAck<Snapshot> {
    pub fn without_snapshot() -> Self {
        Self { snapshot: None }
    }

    pub fn with_snapshot(snapshot: Snapshot) -> Self {
        Self {
            snapshot: Some(snapshot),
        }
    }

    pub fn snapshot(&self) -> Option<&Snapshot> {
        self.snapshot.as_ref()
    }

    pub fn into_snapshot(self) -> Option<Snapshot> {
        self.snapshot
    }
}

pub trait StoreBackend<S: StoreState>: 'static {
    type Snapshot: Clone + PartialEq + 'static;
    type Event: 'static;
    type Subscription: 'static;
    type Error: 'static;

    fn backend_id(&self) -> StoreBackendId;

    fn load(&self) -> StoreBackendFuture<Option<Self::Snapshot>, Self::Error> {
        Ok(None)
    }

    fn subscribe(
        &self,
        _on_change: StoreBackendCallback<Self::Event>,
    ) -> StoreBackendFuture<Option<Self::Subscription>, Self::Error> {
        Ok(None)
    }

    fn load_after_event(
        &self,
        _event: Self::Event,
    ) -> StoreBackendFuture<Option<Self::Snapshot>, Self::Error> {
        self.load()
    }

    fn reconcile(&self, state: &mut S, snapshot: Self::Snapshot) -> bool;
}

pub trait StoreCommitBackend<S: StoreState>: StoreBackend<S> {
    fn commit_snapshot(
        &self,
        draft: &S,
    ) -> StoreBackendFuture<Option<StoreCommitAck<Self::Snapshot>>, Self::Error>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MemoryBackend;

impl<S: StoreState> StoreBackend<S> for MemoryBackend {
    type Snapshot = ();
    type Event = ();
    type Subscription = ();
    type Error = Infallible;

    fn backend_id(&self) -> StoreBackendId {
        StoreBackendId::new("memory:runtime")
    }

    fn reconcile(&self, _state: &mut S, _snapshot: Self::Snapshot) -> bool {
        false
    }
}

type LoadFn<Snapshot, Error> =
    Box<dyn Fn() -> StoreBackendFuture<Option<Snapshot>, Error> + 'static>;
type SubscribeFn<Event, Subscription, Error> = Box<
    dyn Fn(StoreBackendCallback<Event>) -> StoreBackendFuture<Option<Subscription>, Error>
        + 'static,
>;
type LoadAfterEventFn<Event, Snapshot, Error> =
    Box<dyn Fn(Event) -> StoreBackendFuture<Option<Snapshot>, Error> + 'static>;
type ReconcileFn<S, Snapshot> = Box<dyn Fn(&mut S, Snapshot) -> bool + 'static>;
type CommitSnapshotFn<S, Snapshot, Error> =
    Box<dyn Fn(&S) -> StoreBackendFuture<Option<StoreCommitAck<Snapshot>>, Error> + 'static>;

pub struct StoreBackendBuilder<S, Snapshot = (), Event = (), Subscription = (), Error = Infallible>
{
    backend_id: StoreBackendId,
    load: Option<LoadFn<Snapshot, Error>>,
    subscribe: Option<SubscribeFn<Event, Subscription, Error>>,
    load_after_event: Option<LoadAfterEventFn<Event, Snapshot, Error>>,
    reconcile: Option<ReconcileFn<S, Snapshot>>,
}

pub struct StoreCommitBackendBuilder<
    S,
    Snapshot = (),
    Event = (),
    Subscription = (),
    Error = Infallible,
> {
    backend: StoreBackendBuilder<S, Snapshot, Event, Subscription, Error>,
    commit_snapshot: CommitSnapshotFn<S, Snapshot, Error>,
}

impl StoreBackendBuilder<(), (), (), (), Infallible> {
    pub fn new(backend_id: impl Into<StoreBackendId>) -> Self {
        Self {
            backend_id: backend_id.into(),
            load: None,
            subscribe: None,
            load_after_event: None,
            reconcile: None,
        }
    }
}

impl<S, Snapshot, Event, Subscription, Error>
    StoreBackendBuilder<S, Snapshot, Event, Subscription, Error>
{
    pub fn load<NextSnapshot, NextError>(
        self,
        load: impl Fn() -> StoreBackendFuture<Option<NextSnapshot>, NextError> + 'static,
    ) -> StoreBackendBuilder<S, NextSnapshot, Event, Subscription, NextError> {
        StoreBackendBuilder {
            backend_id: self.backend_id,
            load: Some(Box::new(load)),
            subscribe: None,
            load_after_event: None,
            reconcile: None,
        }
    }

    pub fn subscribe<NextEvent, NextSubscription>(
        self,
        subscribe: impl Fn(
            StoreBackendCallback<NextEvent>,
        ) -> StoreBackendFuture<Option<NextSubscription>, Error>
        + 'static,
    ) -> StoreBackendBuilder<S, Snapshot, NextEvent, NextSubscription, Error> {
        StoreBackendBuilder {
            backend_id: self.backend_id,
            load: self.load,
            subscribe: Some(Box::new(subscribe)),
            load_after_event: None,
            reconcile: self.reconcile,
        }
    }

    pub fn load_after_event(
        self,
        load_after_event: impl Fn(Event) -> StoreBackendFuture<Option<Snapshot>, Error> + 'static,
    ) -> Self {
        Self {
            backend_id: self.backend_id,
            load: self.load,
            subscribe: self.subscribe,
            load_after_event: Some(Box::new(load_after_event)),
            reconcile: self.reconcile,
        }
    }

    pub fn reconcile<NextS>(
        self,
        reconcile: impl Fn(&mut NextS, Snapshot) -> bool + 'static,
    ) -> StoreBackendBuilder<NextS, Snapshot, Event, Subscription, Error> {
        StoreBackendBuilder {
            backend_id: self.backend_id,
            load: self.load,
            subscribe: self.subscribe,
            load_after_event: self.load_after_event,
            reconcile: Some(Box::new(reconcile)),
        }
    }

    pub fn reconcile_replace<NextS>(
        self,
    ) -> StoreBackendBuilder<NextS, Snapshot, Event, Subscription, Error>
    where
        NextS: PartialEq<Snapshot> + From<Snapshot>,
    {
        self.reconcile(|state: &mut NextS, snapshot| {
            if *state == snapshot {
                return false;
            }

            *state = snapshot.into();
            true
        })
    }

    pub fn reconcile_field<NextS>(
        self,
        field: impl Fn(&mut NextS) -> &mut Snapshot + 'static,
    ) -> StoreBackendBuilder<NextS, Snapshot, Event, Subscription, Error>
    where
        Snapshot: PartialEq,
    {
        self.reconcile(move |state: &mut NextS, snapshot| {
            let field = field(state);
            if field == &snapshot {
                return false;
            }

            *field = snapshot;
            true
        })
    }

    pub fn commit_snapshot(
        self,
        commit_snapshot: impl Fn(&S) -> StoreBackendFuture<Option<StoreCommitAck<Snapshot>>, Error>
        + 'static,
    ) -> StoreCommitBackendBuilder<S, Snapshot, Event, Subscription, Error> {
        StoreCommitBackendBuilder {
            backend: self,
            commit_snapshot: Box::new(commit_snapshot),
        }
    }
}

impl<S, Snapshot, Event, Subscription, Error> StoreBackend<S>
    for StoreBackendBuilder<S, Snapshot, Event, Subscription, Error>
where
    S: StoreState,
    Snapshot: Clone + PartialEq + 'static,
    Event: 'static,
    Subscription: 'static,
    Error: 'static,
{
    type Snapshot = Snapshot;
    type Event = Event;
    type Subscription = Subscription;
    type Error = Error;

    fn backend_id(&self) -> StoreBackendId {
        self.backend_id.clone()
    }

    fn load(&self) -> StoreBackendFuture<Option<Self::Snapshot>, Self::Error> {
        match &self.load {
            Some(load) => load(),
            None => Ok(None),
        }
    }

    fn subscribe(
        &self,
        on_change: StoreBackendCallback<Self::Event>,
    ) -> StoreBackendFuture<Option<Self::Subscription>, Self::Error> {
        match &self.subscribe {
            Some(subscribe) => subscribe(on_change),
            None => Ok(None),
        }
    }

    fn load_after_event(
        &self,
        event: Self::Event,
    ) -> StoreBackendFuture<Option<Self::Snapshot>, Self::Error> {
        match &self.load_after_event {
            Some(load_after_event) => load_after_event(event),
            None => self.load(),
        }
    }

    fn reconcile(&self, state: &mut S, snapshot: Self::Snapshot) -> bool {
        match &self.reconcile {
            Some(reconcile) => reconcile(state, snapshot),
            None => false,
        }
    }
}

impl<S, Snapshot, Event, Subscription, Error> StoreBackend<S>
    for StoreCommitBackendBuilder<S, Snapshot, Event, Subscription, Error>
where
    S: StoreState,
    Snapshot: Clone + PartialEq + 'static,
    Event: 'static,
    Subscription: 'static,
    Error: 'static,
{
    type Snapshot = Snapshot;
    type Event = Event;
    type Subscription = Subscription;
    type Error = Error;

    fn backend_id(&self) -> StoreBackendId {
        <StoreBackendBuilder<S, Snapshot, Event, Subscription, Error> as StoreBackend<
            S,
        >>::backend_id(&self.backend)
    }

    fn load(&self) -> StoreBackendFuture<Option<Self::Snapshot>, Self::Error> {
        <StoreBackendBuilder<S, Snapshot, Event, Subscription, Error> as StoreBackend<S>>::load(
            &self.backend,
        )
    }

    fn subscribe(
        &self,
        on_change: StoreBackendCallback<Self::Event>,
    ) -> StoreBackendFuture<Option<Self::Subscription>, Self::Error> {
        <StoreBackendBuilder<S, Snapshot, Event, Subscription, Error> as StoreBackend<S>>::subscribe(
            &self.backend,
            on_change,
        )
    }

    fn load_after_event(
        &self,
        event: Self::Event,
    ) -> StoreBackendFuture<Option<Self::Snapshot>, Self::Error> {
        <StoreBackendBuilder<S, Snapshot, Event, Subscription, Error> as StoreBackend<
            S,
        >>::load_after_event(&self.backend, event)
    }

    fn reconcile(&self, state: &mut S, snapshot: Self::Snapshot) -> bool {
        <StoreBackendBuilder<S, Snapshot, Event, Subscription, Error> as StoreBackend<S>>::reconcile(
            &self.backend,
            state,
            snapshot,
        )
    }
}

impl<S, Snapshot, Event, Subscription, Error> StoreCommitBackend<S>
    for StoreCommitBackendBuilder<S, Snapshot, Event, Subscription, Error>
where
    S: StoreState,
    Snapshot: Clone + PartialEq + 'static,
    Event: 'static,
    Subscription: 'static,
    Error: 'static,
{
    fn commit_snapshot(
        &self,
        draft: &S,
    ) -> StoreBackendFuture<Option<StoreCommitAck<Self::Snapshot>>, Self::Error> {
        (self.commit_snapshot)(draft)
    }
}

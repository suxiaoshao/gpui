use std::{convert::Infallible, fmt};

use gpui::App;

use crate::StoreState;

pub type StoreSourceFuture<T, Error> = Result<T, Error>;
pub type StoreSourceCallback<Event> = Box<dyn FnMut(Event, &mut App) + 'static>;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StoreSourceId(String);

impl StoreSourceId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for StoreSourceId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for StoreSourceId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for StoreSourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreSourcePolicy {
    MemoryOnly,
    StoreBacked,
    ExternalBacked,
    Projection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoreSourceWriteAck<Snapshot> {
    snapshot: Option<Snapshot>,
}

impl<Snapshot> StoreSourceWriteAck<Snapshot> {
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

pub trait StoreSource<S: StoreState>: 'static {
    type Snapshot: Clone + PartialEq + 'static;
    type Event: 'static;
    type Subscription: 'static;
    type Error: 'static;

    fn source_id(&self) -> StoreSourceId;

    fn policy(&self) -> StoreSourcePolicy;

    fn load(&self) -> StoreSourceFuture<Option<Self::Snapshot>, Self::Error> {
        Ok(None)
    }

    fn subscribe(
        &self,
        _on_change: StoreSourceCallback<Self::Event>,
    ) -> StoreSourceFuture<Option<Self::Subscription>, Self::Error> {
        Ok(None)
    }

    fn load_after_event(
        &self,
        _event: Self::Event,
    ) -> StoreSourceFuture<Option<Self::Snapshot>, Self::Error> {
        self.load()
    }

    fn reconcile(&self, state: &mut S, snapshot: Self::Snapshot) -> bool;

    fn write_snapshot(
        &self,
        _state: &S,
    ) -> StoreSourceFuture<Option<StoreSourceWriteAck<Self::Snapshot>>, Self::Error> {
        Ok(None)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MemorySource;

impl<S: StoreState> StoreSource<S> for MemorySource {
    type Snapshot = ();
    type Event = ();
    type Subscription = ();
    type Error = Infallible;

    fn source_id(&self) -> StoreSourceId {
        StoreSourceId::new("memory:runtime")
    }

    fn policy(&self) -> StoreSourcePolicy {
        StoreSourcePolicy::MemoryOnly
    }

    fn reconcile(&self, _state: &mut S, _snapshot: Self::Snapshot) -> bool {
        false
    }
}

type LoadFn<Snapshot, Error> =
    Box<dyn Fn() -> StoreSourceFuture<Option<Snapshot>, Error> + 'static>;
type SubscribeFn<Event, Subscription, Error> = Box<
    dyn Fn(StoreSourceCallback<Event>) -> StoreSourceFuture<Option<Subscription>, Error> + 'static,
>;
type LoadAfterEventFn<Event, Snapshot, Error> =
    Box<dyn Fn(Event) -> StoreSourceFuture<Option<Snapshot>, Error> + 'static>;
type ReconcileFn<S, Snapshot> = Box<dyn Fn(&mut S, Snapshot) -> bool + 'static>;
type WriteSnapshotFn<S, Snapshot, Error> =
    Box<dyn Fn(&S) -> StoreSourceFuture<Option<StoreSourceWriteAck<Snapshot>>, Error> + 'static>;

pub struct StoreSourceBuilder<S, Snapshot = (), Event = (), Subscription = (), Error = Infallible> {
    source_id: StoreSourceId,
    policy: StoreSourcePolicy,
    load: Option<LoadFn<Snapshot, Error>>,
    subscribe: Option<SubscribeFn<Event, Subscription, Error>>,
    load_after_event: Option<LoadAfterEventFn<Event, Snapshot, Error>>,
    reconcile: Option<ReconcileFn<S, Snapshot>>,
    write_snapshot: Option<WriteSnapshotFn<S, Snapshot, Error>>,
}

impl StoreSourceBuilder<(), (), (), (), Infallible> {
    pub fn memory(source_id: impl Into<StoreSourceId>) -> Self {
        Self::new(source_id, StoreSourcePolicy::MemoryOnly)
    }

    pub fn store_backed(source_id: impl Into<StoreSourceId>) -> Self {
        Self::new(source_id, StoreSourcePolicy::StoreBacked)
    }

    pub fn external_backed(source_id: impl Into<StoreSourceId>) -> Self {
        Self::new(source_id, StoreSourcePolicy::ExternalBacked)
    }

    pub fn projection(source_id: impl Into<StoreSourceId>) -> Self {
        Self::new(source_id, StoreSourcePolicy::Projection)
    }
}

impl<S, Snapshot, Event, Subscription, Error>
    StoreSourceBuilder<S, Snapshot, Event, Subscription, Error>
{
    fn new(source_id: impl Into<StoreSourceId>, policy: StoreSourcePolicy) -> Self {
        Self {
            source_id: source_id.into(),
            policy,
            load: None,
            subscribe: None,
            load_after_event: None,
            reconcile: None,
            write_snapshot: None,
        }
    }

    pub fn load<NextSnapshot, NextError>(
        self,
        load: impl Fn() -> StoreSourceFuture<Option<NextSnapshot>, NextError> + 'static,
    ) -> StoreSourceBuilder<S, NextSnapshot, Event, Subscription, NextError> {
        StoreSourceBuilder {
            source_id: self.source_id,
            policy: self.policy,
            load: Some(Box::new(load)),
            subscribe: None,
            load_after_event: None,
            reconcile: None,
            write_snapshot: None,
        }
    }

    pub fn subscribe<NextEvent, NextSubscription>(
        self,
        subscribe: impl Fn(
            StoreSourceCallback<NextEvent>,
        ) -> StoreSourceFuture<Option<NextSubscription>, Error>
        + 'static,
    ) -> StoreSourceBuilder<S, Snapshot, NextEvent, NextSubscription, Error> {
        StoreSourceBuilder {
            source_id: self.source_id,
            policy: self.policy,
            load: self.load,
            subscribe: Some(Box::new(subscribe)),
            load_after_event: None,
            reconcile: self.reconcile,
            write_snapshot: self.write_snapshot,
        }
    }

    pub fn load_after_event<NextEvent>(
        self,
        load_after_event: impl Fn(NextEvent) -> StoreSourceFuture<Option<Snapshot>, Error> + 'static,
    ) -> StoreSourceBuilder<S, Snapshot, NextEvent, Subscription, Error> {
        StoreSourceBuilder {
            source_id: self.source_id,
            policy: self.policy,
            load: self.load,
            subscribe: None,
            load_after_event: Some(Box::new(load_after_event)),
            reconcile: self.reconcile,
            write_snapshot: self.write_snapshot,
        }
    }

    pub fn reconcile<NextS>(
        self,
        reconcile: impl Fn(&mut NextS, Snapshot) -> bool + 'static,
    ) -> StoreSourceBuilder<NextS, Snapshot, Event, Subscription, Error> {
        StoreSourceBuilder {
            source_id: self.source_id,
            policy: self.policy,
            load: self.load,
            subscribe: self.subscribe,
            load_after_event: self.load_after_event,
            reconcile: Some(Box::new(reconcile)),
            write_snapshot: None,
        }
    }

    pub fn write_snapshot(
        self,
        write_snapshot: impl Fn(&S) -> StoreSourceFuture<Option<StoreSourceWriteAck<Snapshot>>, Error>
        + 'static,
    ) -> Self {
        Self {
            write_snapshot: Some(Box::new(write_snapshot)),
            ..self
        }
    }
}

impl<S, Snapshot, Event, Subscription, Error> StoreSource<S>
    for StoreSourceBuilder<S, Snapshot, Event, Subscription, Error>
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

    fn source_id(&self) -> StoreSourceId {
        self.source_id.clone()
    }

    fn policy(&self) -> StoreSourcePolicy {
        self.policy
    }

    fn load(&self) -> StoreSourceFuture<Option<Self::Snapshot>, Self::Error> {
        match &self.load {
            Some(load) => load(),
            None => Ok(None),
        }
    }

    fn subscribe(
        &self,
        on_change: StoreSourceCallback<Self::Event>,
    ) -> StoreSourceFuture<Option<Self::Subscription>, Self::Error> {
        match &self.subscribe {
            Some(subscribe) => subscribe(on_change),
            None => Ok(None),
        }
    }

    fn load_after_event(
        &self,
        event: Self::Event,
    ) -> StoreSourceFuture<Option<Self::Snapshot>, Self::Error> {
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

    fn write_snapshot(
        &self,
        state: &S,
    ) -> StoreSourceFuture<Option<StoreSourceWriteAck<Self::Snapshot>>, Self::Error> {
        match &self.write_snapshot {
            Some(write_snapshot) => write_snapshot(state),
            None => Ok(None),
        }
    }
}

use gpui::{App, AppContext, Context, Entity, Global};

use crate::{
    MemorySource, StoreBinding, StoreCore, StoreRevision, StoreSelection, StoreSource,
    StoreSourceCallback, StoreSourceWriteAck, StoreState, StoreUpdate, StoreUpdateOrigin,
};

pub struct StoreRuntime<S, Source = MemorySource>
where
    S: StoreState,
    Source: StoreSource<S>,
{
    core: StoreCore<S>,
    source: Source,
    last_external_snapshot: Option<Source::Snapshot>,
    source_subscription: Option<Source::Subscription>,
    last_persisted_revision: Option<StoreRevision>,
    last_write_ack: Option<StoreSourceWriteAck<Source::Snapshot>>,
    last_error: Option<Source::Error>,
}

impl<S> StoreRuntime<S, MemorySource>
where
    S: StoreState,
{
    pub fn memory(initial: S) -> Self {
        Self::new(initial, MemorySource)
    }
}

impl<S, Source> StoreRuntime<S, Source>
where
    S: StoreState,
    Source: StoreSource<S>,
{
    pub fn new(initial: S, source: Source) -> Self {
        Self {
            core: StoreCore::new(initial),
            source,
            last_external_snapshot: None,
            source_subscription: None,
            last_persisted_revision: None,
            last_write_ack: None,
            last_error: None,
        }
    }

    pub fn state(&self) -> &S {
        self.core.state()
    }

    pub fn revision(&self) -> StoreRevision {
        self.core.revision()
    }

    pub fn source(&self) -> &Source {
        &self.source
    }

    pub fn last_external_snapshot(&self) -> Option<&Source::Snapshot> {
        self.last_external_snapshot.as_ref()
    }

    pub fn last_write_ack(&self) -> Option<&StoreSourceWriteAck<Source::Snapshot>> {
        self.last_write_ack.as_ref()
    }

    pub fn last_error(&self) -> Option<&Source::Error> {
        self.last_error.as_ref()
    }

    pub fn take_last_error(&mut self) -> Option<Source::Error> {
        self.last_error.take()
    }

    pub fn set<T: PartialEq>(
        &mut self,
        field: impl FnOnce(&mut S) -> &mut T,
        value: T,
        cx: &mut Context<Self>,
    ) -> StoreUpdate {
        let update = self.core.set(field, value);
        self.after_update(update, cx);
        update
    }

    pub fn update(&mut self, f: impl FnOnce(&mut S), cx: &mut Context<Self>) -> StoreUpdate
    where
        S: Clone + PartialEq,
    {
        let update = self.core.update(f);
        self.after_update(update, cx);
        update
    }

    pub fn update_if(
        &mut self,
        f: impl FnOnce(&mut S) -> bool,
        cx: &mut Context<Self>,
    ) -> StoreUpdate {
        let update = self.core.update_if(f);
        self.after_update(update, cx);
        update
    }

    pub fn sync_initial(&mut self, cx: &mut Context<Self>) -> Result<StoreUpdate, Source::Error> {
        match self.source.load()? {
            Some(snapshot) => self.sync_snapshot(snapshot, cx),
            None => Ok(StoreUpdate::unchanged(
                self.core.revision(),
                StoreUpdateOrigin::External,
            )),
        }
    }

    pub fn subscribe(&mut self, cx: &mut Context<Self>) -> Result<(), Source::Error> {
        let weak_runtime = cx.weak_entity();
        let callback: StoreSourceCallback<Source::Event> = Box::new(move |event, cx| {
            let _ = weak_runtime.update(cx, |runtime, cx| {
                if let Err(error) = runtime.handle_source_event(event, cx) {
                    runtime.last_error = Some(error);
                }
            });
        });

        self.source_subscription = self.source.subscribe(callback)?;
        Ok(())
    }

    pub fn handle_source_event(
        &mut self,
        event: Source::Event,
        cx: &mut Context<Self>,
    ) -> Result<StoreUpdate, Source::Error> {
        match self.source.load_after_event(event)? {
            Some(snapshot) => self.sync_snapshot(snapshot, cx),
            None => Ok(StoreUpdate::unchanged(
                self.core.revision(),
                StoreUpdateOrigin::External,
            )),
        }
    }

    pub fn sync_snapshot(
        &mut self,
        snapshot: Source::Snapshot,
        cx: &mut Context<Self>,
    ) -> Result<StoreUpdate, Source::Error> {
        if self
            .last_external_snapshot
            .as_ref()
            .is_some_and(|last_snapshot| last_snapshot == &snapshot)
        {
            return Ok(StoreUpdate::unchanged(
                self.core.revision(),
                StoreUpdateOrigin::External,
            ));
        }

        let snapshot_for_reconcile = snapshot.clone();
        let update = self
            .core
            .update_if_with_origin(StoreUpdateOrigin::External, |state| {
                self.source.reconcile(state, snapshot_for_reconcile)
            });
        self.last_external_snapshot = Some(snapshot);

        if update.changed_state() {
            cx.notify();
        }

        Ok(update)
    }

    fn after_update(&mut self, update: StoreUpdate, cx: &mut Context<Self>) {
        if !update.changed_state() {
            return;
        }

        cx.notify();

        if update.origin() != StoreUpdateOrigin::Local {
            return;
        }

        if Some(update.revision()) == self.last_persisted_revision {
            return;
        }

        match self.source.write_snapshot(self.core.state()) {
            Ok(Some(ack)) => {
                self.last_persisted_revision = Some(update.revision());
                if let Some(snapshot) = ack.snapshot().cloned() {
                    self.last_external_snapshot = Some(snapshot);
                }
                self.last_write_ack = Some(ack);
            }
            Ok(None) => {
                self.last_persisted_revision = Some(update.revision());
            }
            Err(error) => {
                self.last_error = Some(error);
            }
        }
    }
}

pub struct SharedStore<S, Source = MemorySource>
where
    S: StoreState,
    Source: StoreSource<S>,
{
    entity: Entity<StoreRuntime<S, Source>>,
}

impl<S, Source> Clone for SharedStore<S, Source>
where
    S: StoreState,
    Source: StoreSource<S>,
{
    fn clone(&self) -> Self {
        Self {
            entity: self.entity.clone(),
        }
    }
}

impl<S, Source> Global for SharedStore<S, Source>
where
    S: StoreState,
    Source: StoreSource<S>,
{
}

impl<S> SharedStore<S, MemorySource>
where
    S: StoreState,
{
    pub fn new(cx: &mut impl AppContext, initial: S) -> Self {
        Self {
            entity: cx.new(|_| StoreRuntime::memory(initial)),
        }
    }

    pub fn install_global(cx: &mut App, initial: S) -> Self {
        let store = Self::new(cx, initial);
        cx.set_global(store.clone());
        store
    }

    pub fn global(cx: &impl AppContext) -> Self {
        cx.read_global(|store: &Self, _| store.clone())
    }
}

impl<S, Source> SharedStore<S, Source>
where
    S: StoreState,
    Source: StoreSource<S>,
{
    pub fn new_with_source(
        cx: &mut impl AppContext,
        initial: S,
        source: Source,
    ) -> Result<Self, Source::Error> {
        let store = Self {
            entity: cx.new(|_| StoreRuntime::new(initial, source)),
        };

        store.sync_initial(cx)?;
        store.subscribe(cx)?;
        Ok(store)
    }

    pub fn install_global_from_source(
        cx: &mut App,
        initial: S,
        source: Source,
    ) -> Result<Self, Source::Error> {
        let store = Self::new_with_source(cx, initial, source)?;
        cx.set_global(store.clone());
        Ok(store)
    }

    pub fn global_with_source(cx: &impl AppContext) -> Self {
        cx.read_global(|store: &Self, _| store.clone())
    }

    pub fn entity(&self) -> Entity<StoreRuntime<S, Source>> {
        self.entity.clone()
    }

    pub fn read<R>(&self, cx: &impl AppContext, f: impl FnOnce(&S) -> R) -> R {
        self.entity
            .read_with(cx, |runtime, _| f(runtime.core.state()))
    }

    pub fn revision(&self, cx: &impl AppContext) -> StoreRevision {
        self.entity
            .read_with(cx, |runtime, _| runtime.core.revision())
    }

    pub fn set<T: PartialEq>(
        &self,
        cx: &mut impl AppContext,
        field: impl FnOnce(&mut S) -> &mut T,
        value: T,
    ) -> StoreUpdate {
        self.entity
            .update(cx, |runtime, cx| runtime.set(field, value, cx))
    }

    pub fn update(&self, cx: &mut impl AppContext, f: impl FnOnce(&mut S)) -> StoreUpdate
    where
        S: Clone + PartialEq,
    {
        self.entity.update(cx, |runtime, cx| runtime.update(f, cx))
    }

    pub fn update_if(
        &self,
        cx: &mut impl AppContext,
        f: impl FnOnce(&mut S) -> bool,
    ) -> StoreUpdate {
        self.entity
            .update(cx, |runtime, cx| runtime.update_if(f, cx))
    }

    pub fn sync_initial(&self, cx: &mut impl AppContext) -> Result<StoreUpdate, Source::Error> {
        self.entity
            .update(cx, |runtime, cx| runtime.sync_initial(cx))
    }

    pub fn subscribe(&self, cx: &mut impl AppContext) -> Result<(), Source::Error> {
        self.entity.update(cx, |runtime, cx| runtime.subscribe(cx))
    }

    pub fn handle_source_event(
        &self,
        cx: &mut impl AppContext,
        event: Source::Event,
    ) -> Result<StoreUpdate, Source::Error> {
        self.entity
            .update(cx, |runtime, cx| runtime.handle_source_event(event, cx))
    }

    pub fn sync_snapshot(
        &self,
        cx: &mut impl AppContext,
        snapshot: Source::Snapshot,
    ) -> Result<StoreUpdate, Source::Error> {
        self.entity
            .update(cx, |runtime, cx| runtime.sync_snapshot(snapshot, cx))
    }

    pub fn select<Owner, T>(
        &self,
        cx: &mut Context<Owner>,
        select: impl Fn(&S) -> T + 'static,
    ) -> StoreSelection<T>
    where
        Owner: 'static,
        T: PartialEq + 'static,
    {
        StoreSelection::new(self.clone(), cx, select)
    }

    pub fn bind<Owner, T>(
        &self,
        cx: &mut Context<Owner>,
        get: impl Fn(&S) -> T + 'static,
        set: impl Fn(&mut S, T) + 'static,
    ) -> StoreBinding<T>
    where
        Owner: 'static,
        T: Clone + PartialEq + 'static,
    {
        StoreBinding::new(self.clone(), cx, get, set)
    }
}

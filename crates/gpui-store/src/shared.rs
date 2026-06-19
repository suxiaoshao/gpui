use std::rc::Rc;

use gpui::{App, AppContext, Context, Entity, Global, Subscription, Window};

use crate::{
    MemoryBackend, SnapshotCell, StoreBackend, StoreBackendCallback, StoreBinding, StoreCommitAck,
    StoreCommitBackend, StoreCore, StoreRevision, StoreSelection, StoreState, StoreUpdate,
    StoreUpdateOrigin,
};

pub struct StoreRuntime<S, Backend = MemoryBackend>
where
    S: StoreState,
    Backend: StoreBackend<S>,
{
    core: StoreCore<S>,
    backend: Backend,
    last_external_snapshot: Option<Backend::Snapshot>,
    backend_subscription: Option<Backend::Subscription>,
    last_commit_ack: Option<StoreCommitAck<Backend::Snapshot>>,
    last_error: Option<Backend::Error>,
}

impl<S> StoreRuntime<S, MemoryBackend>
where
    S: StoreState,
{
    pub fn memory(initial: S) -> Self {
        Self::new(initial, MemoryBackend)
    }

    pub(crate) fn set<T: PartialEq>(
        &mut self,
        field: impl FnOnce(&mut S) -> &mut T,
        value: T,
        cx: &mut Context<Self>,
    ) -> StoreUpdate {
        let update = self.core.set(field, value);
        Self::notify_after_update(update, cx);
        update
    }

    pub(crate) fn update(&mut self, f: impl FnOnce(&mut S), cx: &mut Context<Self>) -> StoreUpdate
    where
        S: Clone + PartialEq,
    {
        let update = self.core.update(f);
        Self::notify_after_update(update, cx);
        update
    }

    pub(crate) fn update_if(
        &mut self,
        f: impl FnOnce(&mut S) -> bool,
        cx: &mut Context<Self>,
    ) -> StoreUpdate {
        let update = self.core.update_if(f);
        Self::notify_after_update(update, cx);
        update
    }
}

impl<S, Backend> StoreRuntime<S, Backend>
where
    S: StoreState,
    Backend: StoreBackend<S>,
{
    pub fn new(initial: S, backend: Backend) -> Self {
        Self {
            core: StoreCore::new(initial),
            backend,
            last_external_snapshot: None,
            backend_subscription: None,
            last_commit_ack: None,
            last_error: None,
        }
    }

    pub fn state(&self) -> &S {
        self.core.state()
    }

    pub fn revision(&self) -> StoreRevision {
        self.core.revision()
    }

    pub fn backend(&self) -> &Backend {
        &self.backend
    }

    pub fn last_external_snapshot(&self) -> Option<&Backend::Snapshot> {
        self.last_external_snapshot.as_ref()
    }

    pub fn last_commit_ack(&self) -> Option<&StoreCommitAck<Backend::Snapshot>> {
        self.last_commit_ack.as_ref()
    }

    pub fn last_error(&self) -> Option<&Backend::Error> {
        self.last_error.as_ref()
    }

    pub fn take_last_error(&mut self) -> Option<Backend::Error> {
        self.last_error.take()
    }

    pub(crate) fn sync_initial(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Result<StoreUpdate, Backend::Error> {
        self.refresh_from_backend(cx)
    }

    pub(crate) fn refresh_from_backend(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Result<StoreUpdate, Backend::Error> {
        match self.backend.load()? {
            Some(snapshot) => self.sync_snapshot(snapshot, cx),
            None => Ok(StoreUpdate::unchanged(
                self.core.revision(),
                StoreUpdateOrigin::External,
            )),
        }
    }

    pub(crate) fn subscribe(&mut self, cx: &mut Context<Self>) -> Result<(), Backend::Error> {
        let weak_runtime = cx.weak_entity();
        let callback: StoreBackendCallback<Backend::Event> = Box::new(move |event, cx| {
            let _ = weak_runtime.update(cx, |runtime, cx| {
                if let Err(error) = runtime.handle_backend_event(event, cx) {
                    runtime.last_error = Some(error);
                }
            });
        });

        self.backend_subscription = self.backend.subscribe(callback)?;
        Ok(())
    }

    pub(crate) fn handle_backend_event(
        &mut self,
        event: Backend::Event,
        cx: &mut Context<Self>,
    ) -> Result<StoreUpdate, Backend::Error> {
        match self.backend.load_after_event(event)? {
            Some(snapshot) => self.sync_snapshot(snapshot, cx),
            None => Ok(StoreUpdate::unchanged(
                self.core.revision(),
                StoreUpdateOrigin::External,
            )),
        }
    }

    pub(crate) fn sync_snapshot(
        &mut self,
        snapshot: Backend::Snapshot,
        cx: &mut Context<Self>,
    ) -> Result<StoreUpdate, Backend::Error> {
        Ok(self.sync_snapshot_with_origin(StoreUpdateOrigin::External, snapshot, cx))
    }

    fn sync_snapshot_with_origin(
        &mut self,
        origin: StoreUpdateOrigin,
        snapshot: Backend::Snapshot,
        cx: &mut Context<Self>,
    ) -> StoreUpdate {
        if origin == StoreUpdateOrigin::External
            && self
                .last_external_snapshot
                .as_ref()
                .is_some_and(|last_snapshot| last_snapshot == &snapshot)
        {
            return StoreUpdate::unchanged(self.core.revision(), origin);
        }

        let snapshot_for_reconcile = snapshot.clone();
        let update = self.core.update_if_with_origin(origin, |state| {
            self.backend.reconcile(state, snapshot_for_reconcile)
        });
        self.last_external_snapshot = Some(snapshot);

        Self::notify_after_update(update, cx);
        update
    }

    fn notify_after_update(update: StoreUpdate, cx: &mut Context<Self>) {
        if update.changed_state() {
            cx.notify();
        }
    }
}

impl<S, Backend> StoreRuntime<S, Backend>
where
    S: StoreState + Clone + PartialEq,
    Backend: StoreCommitBackend<S>,
{
    pub(crate) fn try_set<T: PartialEq>(
        &mut self,
        field: impl FnOnce(&mut S) -> &mut T,
        value: T,
        cx: &mut Context<Self>,
    ) -> Result<StoreUpdate, Backend::Error> {
        let mut draft = self.core.state().clone();
        let field = field(&mut draft);
        if *field == value {
            return Ok(StoreUpdate::unchanged(
                self.core.revision(),
                StoreUpdateOrigin::Local,
            ));
        }

        *field = value;
        self.commit_draft(draft, cx)
    }

    pub(crate) fn try_update(
        &mut self,
        f: impl FnOnce(&mut S),
        cx: &mut Context<Self>,
    ) -> Result<StoreUpdate, Backend::Error> {
        let mut draft = self.core.state().clone();
        f(&mut draft);
        self.commit_draft(draft, cx)
    }

    pub(crate) fn try_update_if(
        &mut self,
        f: impl FnOnce(&mut S) -> bool,
        cx: &mut Context<Self>,
    ) -> Result<StoreUpdate, Backend::Error> {
        let mut draft = self.core.state().clone();
        if !f(&mut draft) {
            return Ok(StoreUpdate::unchanged(
                self.core.revision(),
                StoreUpdateOrigin::Local,
            ));
        }

        self.commit_draft(draft, cx)
    }

    fn commit_draft(
        &mut self,
        draft: S,
        cx: &mut Context<Self>,
    ) -> Result<StoreUpdate, Backend::Error> {
        if self.core.state() == &draft {
            return Ok(StoreUpdate::unchanged(
                self.core.revision(),
                StoreUpdateOrigin::Local,
            ));
        }

        let ack = self.backend.commit_snapshot(&draft)?;
        let update = match ack.as_ref().and_then(|ack| ack.snapshot()).cloned() {
            Some(snapshot) => {
                self.sync_snapshot_with_origin(StoreUpdateOrigin::Local, snapshot, cx)
            }
            None => {
                let update = self
                    .core
                    .replace_with_origin(StoreUpdateOrigin::Local, draft);
                Self::notify_after_update(update, cx);
                update
            }
        };

        self.last_commit_ack = ack;
        Ok(update)
    }
}

pub struct SharedStore<S, Backend = MemoryBackend>
where
    S: StoreState,
    Backend: StoreBackend<S>,
{
    entity: Entity<StoreRuntime<S, Backend>>,
}

impl<S, Backend> Clone for SharedStore<S, Backend>
where
    S: StoreState,
    Backend: StoreBackend<S>,
{
    fn clone(&self) -> Self {
        Self {
            entity: self.entity.clone(),
        }
    }
}

impl<S, Backend> Global for SharedStore<S, Backend>
where
    S: StoreState,
    Backend: StoreBackend<S>,
{
}

impl<S> SharedStore<S, MemoryBackend>
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
        StoreBinding::new_memory(self.clone(), cx, get, set)
    }
}

impl<S, Backend> SharedStore<S, Backend>
where
    S: StoreState,
    Backend: StoreBackend<S>,
{
    pub fn new_with_backend(
        cx: &mut impl AppContext,
        initial: S,
        backend: Backend,
    ) -> Result<Self, Backend::Error> {
        let store = Self {
            entity: cx.new(|_| StoreRuntime::new(initial, backend)),
        };

        store.sync_initial(cx)?;
        store.subscribe(cx)?;
        Ok(store)
    }

    pub fn install_global_with_backend(
        cx: &mut App,
        initial: S,
        backend: Backend,
    ) -> Result<Self, Backend::Error> {
        let store = Self::new_with_backend(cx, initial, backend)?;
        cx.set_global(store.clone());
        Ok(store)
    }

    pub fn install_global_with_default(
        cx: &mut App,
        backend: Backend,
    ) -> Result<Self, Backend::Error>
    where
        S: Default,
    {
        Self::install_global_with_backend(cx, S::default(), backend)
    }

    pub fn global(cx: &impl AppContext) -> Self {
        cx.read_global(|store: &Self, _| store.clone())
    }

    pub fn entity(&self) -> Entity<StoreRuntime<S, Backend>> {
        self.entity.clone()
    }

    pub fn read<R>(&self, cx: &impl AppContext, f: impl FnOnce(&S) -> R) -> R {
        self.entity
            .read_with(cx, |runtime, _| f(runtime.core.state()))
    }

    pub fn read_cloned<T: Clone>(&self, cx: &impl AppContext, f: impl FnOnce(&S) -> &T) -> T {
        self.read(cx, |state| f(state).clone())
    }

    pub fn revision(&self, cx: &impl AppContext) -> StoreRevision {
        self.entity
            .read_with(cx, |runtime, _| runtime.core.revision())
    }

    pub fn sync_initial(&self, cx: &mut impl AppContext) -> Result<StoreUpdate, Backend::Error> {
        self.entity
            .update(cx, |runtime, cx| runtime.sync_initial(cx))
    }

    pub fn refresh_from_backend(
        &self,
        cx: &mut impl AppContext,
    ) -> Result<StoreUpdate, Backend::Error> {
        self.entity
            .update(cx, |runtime, cx| runtime.refresh_from_backend(cx))
    }

    pub fn subscribe(&self, cx: &mut impl AppContext) -> Result<(), Backend::Error> {
        self.entity.update(cx, |runtime, cx| runtime.subscribe(cx))
    }

    pub fn handle_backend_event(
        &self,
        cx: &mut impl AppContext,
        event: Backend::Event,
    ) -> Result<StoreUpdate, Backend::Error> {
        self.entity
            .update(cx, |runtime, cx| runtime.handle_backend_event(event, cx))
    }

    pub fn sync_snapshot(
        &self,
        cx: &mut impl AppContext,
        snapshot: Backend::Snapshot,
    ) -> Result<StoreUpdate, Backend::Error> {
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

    pub fn select_cloned<Owner, T>(
        &self,
        cx: &mut Context<Owner>,
        select: impl Fn(&S) -> &T + 'static,
    ) -> StoreSelection<T>
    where
        Owner: 'static,
        T: Clone + PartialEq + 'static,
    {
        self.select(cx, move |state| select(state).clone())
    }

    pub fn observe_select<Owner, T>(
        &self,
        cx: &mut Context<Owner>,
        select: impl Fn(&S) -> T + 'static,
        observe: impl Fn(&mut Owner, &T, &mut Context<Owner>) + 'static,
    ) -> Subscription
    where
        Owner: 'static,
        T: PartialEq + 'static,
    {
        let entity = self.entity();
        let selector = Rc::new(select);
        let snapshot = entity.read_with(cx, |runtime, _| selector(runtime.state()));
        let snapshot = Rc::new(SnapshotCell::new(snapshot));

        let observed_snapshot = snapshot.clone();
        let observed_selector = selector.clone();
        cx.observe(&entity, move |owner, observed, cx| {
            let next_snapshot =
                observed.read_with(cx, |runtime, _| observed_selector(runtime.state()));
            if observed_snapshot.get() != &next_snapshot {
                observed_snapshot.replace(next_snapshot);
                observe(owner, observed_snapshot.get(), cx);
            }
        })
    }

    pub fn observe_select_in<Owner, T>(
        &self,
        cx: &mut Context<Owner>,
        window: &mut Window,
        select: impl Fn(&S) -> T + 'static,
        observe: impl Fn(&mut Owner, &T, &mut Window, &mut Context<Owner>) + 'static,
    ) -> Subscription
    where
        Owner: 'static,
        T: PartialEq + 'static,
    {
        let entity = self.entity();
        let selector = Rc::new(select);
        let snapshot = entity.read_with(cx, |runtime, _| selector(runtime.state()));
        let snapshot = Rc::new(SnapshotCell::new(snapshot));

        let observed_snapshot = snapshot.clone();
        let observed_selector = selector.clone();
        cx.observe_in(&entity, window, move |owner, observed, window, cx| {
            let next_snapshot =
                observed.read_with(cx, |runtime, _| observed_selector(runtime.state()));
            if observed_snapshot.get() != &next_snapshot {
                observed_snapshot.replace(next_snapshot);
                observe(owner, observed_snapshot.get(), window, cx);
            }
        })
    }
}

impl<S, Backend> SharedStore<S, Backend>
where
    S: StoreState + Clone + PartialEq,
    Backend: StoreCommitBackend<S>,
{
    pub fn try_set<T: PartialEq>(
        &self,
        cx: &mut impl AppContext,
        field: impl FnOnce(&mut S) -> &mut T,
        value: T,
    ) -> Result<StoreUpdate, Backend::Error> {
        self.entity
            .update(cx, |runtime, cx| runtime.try_set(field, value, cx))
    }

    pub fn try_update(
        &self,
        cx: &mut impl AppContext,
        f: impl FnOnce(&mut S),
    ) -> Result<StoreUpdate, Backend::Error> {
        self.entity
            .update(cx, |runtime, cx| runtime.try_update(f, cx))
    }

    pub fn try_update_if(
        &self,
        cx: &mut impl AppContext,
        f: impl FnOnce(&mut S) -> bool,
    ) -> Result<StoreUpdate, Backend::Error> {
        self.entity
            .update(cx, |runtime, cx| runtime.try_update_if(f, cx))
    }

    pub fn try_update_field<T>(
        &self,
        cx: &mut impl AppContext,
        field: impl FnOnce(&mut S) -> &mut T,
        update: impl FnOnce(&mut T),
    ) -> Result<StoreUpdate, Backend::Error> {
        self.try_update(cx, |state| update(field(state)))
    }

    pub fn bind_committed<Owner, T>(
        &self,
        cx: &mut Context<Owner>,
        get: impl Fn(&S) -> T + 'static,
        set: impl Fn(&mut S, T) + 'static,
    ) -> StoreBinding<T, Backend::Error>
    where
        Owner: 'static,
        T: Clone + PartialEq + 'static,
    {
        StoreBinding::new_committed(self.clone(), cx, get, set)
    }

    pub fn bind_committed_field<Owner, T>(
        &self,
        cx: &mut Context<Owner>,
        get: impl Fn(&S) -> T + 'static,
        set: impl Fn(&mut S, T) + 'static,
    ) -> StoreBinding<T, Backend::Error>
    where
        Owner: 'static,
        T: Clone + PartialEq + 'static,
    {
        self.bind_committed(cx, get, set)
    }
}

use gpui::Context;

use crate::{
    MemoryBackend, StoreBackend, StoreBackendCallback, StoreCommitAck, StoreCommitBackend,
    StoreCore, StoreRevision, StoreState, StoreUpdate, StoreUpdateOrigin,
};

pub struct LocalStore<S, Backend = MemoryBackend>
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

impl<S> LocalStore<S, MemoryBackend>
where
    S: StoreState,
{
    pub fn new(initial: S) -> Self {
        Self::with_backend_unchecked(initial, MemoryBackend)
    }

    pub fn set<Owner, T: PartialEq>(
        &mut self,
        cx: &mut Context<Owner>,
        field: impl FnOnce(&mut S) -> &mut T,
        value: T,
    ) -> StoreUpdate
    where
        Owner: 'static,
    {
        let update = self.core.set(field, value);
        Self::notify_after_update(update, cx);
        update
    }

    pub fn update<Owner>(&mut self, cx: &mut Context<Owner>, f: impl FnOnce(&mut S)) -> StoreUpdate
    where
        Owner: 'static,
        S: Clone + PartialEq,
    {
        let update = self.core.update(f);
        Self::notify_after_update(update, cx);
        update
    }

    pub fn update_if<Owner>(
        &mut self,
        cx: &mut Context<Owner>,
        f: impl FnOnce(&mut S) -> bool,
    ) -> StoreUpdate
    where
        Owner: 'static,
    {
        let update = self.core.update_if(f);
        Self::notify_after_update(update, cx);
        update
    }
}

impl<S, Backend> LocalStore<S, Backend>
where
    S: StoreState,
    Backend: StoreBackend<S>,
{
    pub fn with_backend<Owner>(
        cx: &mut Context<Owner>,
        initial: S,
        backend: Backend,
    ) -> Result<Self, Backend::Error>
    where
        Owner: 'static,
    {
        let mut store = Self::with_backend_unchecked(initial, backend);
        store.sync_initial(cx)?;
        Ok(store)
    }

    fn with_backend_unchecked(initial: S, backend: Backend) -> Self {
        Self {
            core: StoreCore::new(initial),
            backend,
            last_external_snapshot: None,
            backend_subscription: None,
            last_commit_ack: None,
            last_error: None,
        }
    }

    pub fn read(&self) -> &S {
        self.core.state()
    }

    pub fn read_cloned<T: Clone>(&self, f: impl FnOnce(&S) -> &T) -> T {
        f(self.core.state()).clone()
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

    pub fn sync_initial<Owner>(
        &mut self,
        cx: &mut Context<Owner>,
    ) -> Result<StoreUpdate, Backend::Error>
    where
        Owner: 'static,
    {
        self.refresh_from_backend(cx)
    }

    pub fn refresh_from_backend<Owner>(
        &mut self,
        cx: &mut Context<Owner>,
    ) -> Result<StoreUpdate, Backend::Error>
    where
        Owner: 'static,
    {
        match self.backend.load()? {
            Some(snapshot) => self.sync_snapshot(cx, snapshot),
            None => Ok(StoreUpdate::unchanged(
                self.core.revision(),
                StoreUpdateOrigin::External,
            )),
        }
    }

    pub fn handle_backend_event<Owner>(
        &mut self,
        cx: &mut Context<Owner>,
        event: Backend::Event,
    ) -> Result<StoreUpdate, Backend::Error>
    where
        Owner: 'static,
    {
        match self.backend.load_after_event(event)? {
            Some(snapshot) => self.sync_snapshot(cx, snapshot),
            None => Ok(StoreUpdate::unchanged(
                self.core.revision(),
                StoreUpdateOrigin::External,
            )),
        }
    }

    pub fn subscribe<Owner>(
        &mut self,
        cx: &mut Context<Owner>,
        access: impl Fn(&mut Owner) -> &mut Self + 'static,
    ) -> Result<(), Backend::Error>
    where
        Owner: 'static,
    {
        let weak_owner = cx.weak_entity();
        let callback: StoreBackendCallback<Backend::Event> = Box::new(move |event, cx| {
            let _ = weak_owner.update(cx, |owner, cx| {
                let store = access(owner);
                if let Err(error) = store.handle_backend_event(cx, event) {
                    store.last_error = Some(error);
                }
            });
        });

        self.backend_subscription = self.backend.subscribe(callback)?;
        Ok(())
    }

    pub fn sync_snapshot<Owner>(
        &mut self,
        cx: &mut Context<Owner>,
        snapshot: Backend::Snapshot,
    ) -> Result<StoreUpdate, Backend::Error>
    where
        Owner: 'static,
    {
        Ok(self.sync_snapshot_with_origin(StoreUpdateOrigin::External, snapshot, cx))
    }

    fn sync_snapshot_with_origin<Owner>(
        &mut self,
        origin: StoreUpdateOrigin,
        snapshot: Backend::Snapshot,
        cx: &mut Context<Owner>,
    ) -> StoreUpdate
    where
        Owner: 'static,
    {
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

    fn notify_after_update<Owner>(update: StoreUpdate, cx: &mut Context<Owner>)
    where
        Owner: 'static,
    {
        if update.changed_state() {
            cx.notify();
        }
    }
}

impl<S, Backend> LocalStore<S, Backend>
where
    S: StoreState + Clone + PartialEq,
    Backend: StoreCommitBackend<S>,
{
    pub fn try_set<Owner, T: PartialEq>(
        &mut self,
        cx: &mut Context<Owner>,
        field: impl FnOnce(&mut S) -> &mut T,
        value: T,
    ) -> Result<StoreUpdate, Backend::Error>
    where
        Owner: 'static,
    {
        let mut draft = self.core.state().clone();
        let field = field(&mut draft);
        if *field == value {
            return Ok(StoreUpdate::unchanged(
                self.core.revision(),
                StoreUpdateOrigin::Local,
            ));
        }

        *field = value;
        self.commit_draft(cx, draft)
    }

    pub fn try_update<Owner>(
        &mut self,
        cx: &mut Context<Owner>,
        f: impl FnOnce(&mut S),
    ) -> Result<StoreUpdate, Backend::Error>
    where
        Owner: 'static,
    {
        let mut draft = self.core.state().clone();
        f(&mut draft);
        self.commit_draft(cx, draft)
    }

    pub fn try_update_if<Owner>(
        &mut self,
        cx: &mut Context<Owner>,
        f: impl FnOnce(&mut S) -> bool,
    ) -> Result<StoreUpdate, Backend::Error>
    where
        Owner: 'static,
    {
        let mut draft = self.core.state().clone();
        if !f(&mut draft) {
            return Ok(StoreUpdate::unchanged(
                self.core.revision(),
                StoreUpdateOrigin::Local,
            ));
        }

        self.commit_draft(cx, draft)
    }

    pub fn try_update_field<Owner, T>(
        &mut self,
        cx: &mut Context<Owner>,
        field: impl FnOnce(&mut S) -> &mut T,
        update: impl FnOnce(&mut T),
    ) -> Result<StoreUpdate, Backend::Error>
    where
        Owner: 'static,
    {
        self.try_update(cx, |state| update(field(state)))
    }

    fn commit_draft<Owner>(
        &mut self,
        cx: &mut Context<Owner>,
        draft: S,
    ) -> Result<StoreUpdate, Backend::Error>
    where
        Owner: 'static,
    {
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

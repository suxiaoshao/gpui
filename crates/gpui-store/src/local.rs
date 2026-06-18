use gpui::Context;

use crate::{
    MemorySource, StoreCore, StoreRevision, StoreSource, StoreSourceCallback, StoreSourceWriteAck,
    StoreState, StoreUpdate, StoreUpdateOrigin,
};

pub struct LocalStore<S, Source = MemorySource>
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

impl<S> LocalStore<S, MemorySource>
where
    S: StoreState,
{
    pub fn new(initial: S) -> Self {
        Self::with_source_unchecked(initial, MemorySource)
    }
}

impl<S, Source> LocalStore<S, Source>
where
    S: StoreState,
    Source: StoreSource<S>,
{
    pub fn with_source<Owner>(
        cx: &mut Context<Owner>,
        initial: S,
        source: Source,
    ) -> Result<Self, Source::Error>
    where
        Owner: 'static,
    {
        let mut store = Self::with_source_unchecked(initial, source);
        store.sync_initial(cx)?;
        Ok(store)
    }

    fn with_source_unchecked(initial: S, source: Source) -> Self {
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

    pub fn read(&self) -> &S {
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
        self.after_update(update, cx);
        update
    }

    pub fn update<Owner>(&mut self, cx: &mut Context<Owner>, f: impl FnOnce(&mut S)) -> StoreUpdate
    where
        Owner: 'static,
        S: Clone + PartialEq,
    {
        let update = self.core.update(f);
        self.after_update(update, cx);
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
        self.after_update(update, cx);
        update
    }

    pub fn sync_initial<Owner>(
        &mut self,
        cx: &mut Context<Owner>,
    ) -> Result<StoreUpdate, Source::Error>
    where
        Owner: 'static,
    {
        match self.source.load()? {
            Some(snapshot) => self.sync_snapshot(cx, snapshot),
            None => Ok(StoreUpdate::unchanged(
                self.core.revision(),
                StoreUpdateOrigin::External,
            )),
        }
    }

    pub fn handle_source_event<Owner>(
        &mut self,
        cx: &mut Context<Owner>,
        event: Source::Event,
    ) -> Result<StoreUpdate, Source::Error>
    where
        Owner: 'static,
    {
        match self.source.load_after_event(event)? {
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
    ) -> Result<(), Source::Error>
    where
        Owner: 'static,
    {
        let weak_owner = cx.weak_entity();
        let callback: StoreSourceCallback<Source::Event> = Box::new(move |event, cx| {
            let _ = weak_owner.update(cx, |owner, cx| {
                let store = access(owner);
                if let Err(error) = store.handle_source_event(cx, event) {
                    store.last_error = Some(error);
                }
            });
        });

        self.source_subscription = self.source.subscribe(callback)?;
        Ok(())
    }

    pub fn sync_snapshot<Owner>(
        &mut self,
        cx: &mut Context<Owner>,
        snapshot: Source::Snapshot,
    ) -> Result<StoreUpdate, Source::Error>
    where
        Owner: 'static,
    {
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

    fn after_update<Owner>(&mut self, update: StoreUpdate, cx: &mut Context<Owner>)
    where
        Owner: 'static,
    {
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

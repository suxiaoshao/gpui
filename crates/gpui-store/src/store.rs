pub trait StoreState: 'static {}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct StoreRevision(u64);

impl StoreRevision {
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn get(self) -> u64 {
        self.0
    }

    fn next(&mut self) {
        self.0 += 1;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreUpdateOrigin {
    Local,
    External,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoreUpdate {
    revision: StoreRevision,
    changed: bool,
    origin: StoreUpdateOrigin,
}

impl StoreUpdate {
    pub fn unchanged(revision: StoreRevision, origin: StoreUpdateOrigin) -> Self {
        Self {
            revision,
            changed: false,
            origin,
        }
    }

    pub fn new_changed(revision: StoreRevision, origin: StoreUpdateOrigin) -> Self {
        Self {
            revision,
            changed: true,
            origin,
        }
    }

    pub fn revision(self) -> StoreRevision {
        self.revision
    }

    pub fn changed_state(self) -> bool {
        self.changed
    }

    pub fn changed(self) -> bool {
        self.changed
    }

    pub fn origin(self) -> StoreUpdateOrigin {
        self.origin
    }
}

pub struct StoreCore<S> {
    state: S,
    revision: StoreRevision,
    last_origin: Option<StoreUpdateOrigin>,
}

impl<S> StoreCore<S> {
    pub fn new(state: S) -> Self {
        Self {
            state,
            revision: StoreRevision::default(),
            last_origin: None,
        }
    }

    pub fn state(&self) -> &S {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut S {
        &mut self.state
    }

    pub fn revision(&self) -> StoreRevision {
        self.revision
    }

    pub fn last_origin(&self) -> Option<StoreUpdateOrigin> {
        self.last_origin
    }

    pub fn set<T: PartialEq>(
        &mut self,
        field: impl FnOnce(&mut S) -> &mut T,
        value: T,
    ) -> StoreUpdate {
        self.set_with_origin(StoreUpdateOrigin::Local, field, value)
    }

    pub fn set_with_origin<T: PartialEq>(
        &mut self,
        origin: StoreUpdateOrigin,
        field: impl FnOnce(&mut S) -> &mut T,
        value: T,
    ) -> StoreUpdate {
        let field = field(&mut self.state);
        if *field == value {
            return StoreUpdate::unchanged(self.revision, origin);
        }

        *field = value;
        self.mark_changed(origin)
    }

    pub fn update(&mut self, f: impl FnOnce(&mut S)) -> StoreUpdate
    where
        S: Clone + PartialEq,
    {
        self.update_with_origin(StoreUpdateOrigin::Local, f)
    }

    pub fn update_with_origin(
        &mut self,
        origin: StoreUpdateOrigin,
        f: impl FnOnce(&mut S),
    ) -> StoreUpdate
    where
        S: Clone + PartialEq,
    {
        let previous = self.state.clone();
        f(&mut self.state);

        if previous == self.state {
            StoreUpdate::unchanged(self.revision, origin)
        } else {
            self.mark_changed(origin)
        }
    }

    pub fn update_if(&mut self, f: impl FnOnce(&mut S) -> bool) -> StoreUpdate {
        self.update_if_with_origin(StoreUpdateOrigin::Local, f)
    }

    pub fn update_if_with_origin(
        &mut self,
        origin: StoreUpdateOrigin,
        f: impl FnOnce(&mut S) -> bool,
    ) -> StoreUpdate {
        if f(&mut self.state) {
            self.mark_changed(origin)
        } else {
            StoreUpdate::unchanged(self.revision, origin)
        }
    }

    pub fn replace(&mut self, state: S) -> StoreUpdate
    where
        S: PartialEq,
    {
        self.replace_with_origin(StoreUpdateOrigin::Local, state)
    }

    pub fn replace_with_origin(&mut self, origin: StoreUpdateOrigin, state: S) -> StoreUpdate
    where
        S: PartialEq,
    {
        if self.state == state {
            return StoreUpdate::unchanged(self.revision, origin);
        }

        self.state = state;
        self.mark_changed(origin)
    }

    pub fn mark_external_changed(&mut self) -> StoreUpdate {
        self.mark_changed(StoreUpdateOrigin::External)
    }

    fn mark_changed(&mut self, origin: StoreUpdateOrigin) -> StoreUpdate {
        self.revision.next();
        self.last_origin = Some(origin);
        StoreUpdate::new_changed(self.revision, origin)
    }
}

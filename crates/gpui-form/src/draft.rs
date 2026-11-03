use std::{
    any::{Any, TypeId},
    borrow::Cow,
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
};

use crate::{PathKey, ValidationMessage, topology::CanonicalAddress};

/// Application-stable identity of one editor at a path. Distinct editors need distinct keys.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EditorKey(Cow<'static, str>);
impl EditorKey {
    pub fn new(key: impl Into<Cow<'static, str>>) -> Self {
        Self(key.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl From<&'static str> for EditorKey {
    fn from(key: &'static str) -> Self {
        Self::new(key)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorTypeMismatch;
impl std::fmt::Display for EditorTypeMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("editor key has a different display or mapping type")
    }
}
impl std::error::Error for EditorTypeMismatch {}

pub enum EditResult<S, D> {
    Parsed(S),
    Draft {
        display: D,
        code: Cow<'static, str>,
        message: ValidationMessage,
    },
}
impl<S, D> EditResult<S, D> {
    pub fn draft(
        display: D,
        code: impl Into<Cow<'static, str>>,
        message: impl Into<ValidationMessage>,
    ) -> Self {
        Self::Draft {
            display,
            code: code.into(),
            message: message.into(),
        }
    }
}

pub(crate) struct DraftRecord {
    pub(crate) address: CanonicalAddress,
    pub(crate) control: u64,
    pub(crate) display: Box<dyn Any>,
    pub(crate) mapping: TypeId,
    equal: fn(&dyn Any, &dyn Any) -> bool,
}
impl DraftRecord {
    pub(crate) fn new<D: Clone + PartialEq + 'static>(
        address: CanonicalAddress,
        control: u64,
        display: D,
        mapping: TypeId,
    ) -> Self {
        Self {
            address,
            control,
            display: Box::new(display),
            mapping,
            equal: |a, b| a.downcast_ref::<D>() == b.downcast_ref::<D>(),
        }
    }
    pub(crate) fn same_display(&self, other: &Self) -> bool {
        self.mapping == other.mapping && (self.equal)(&*self.display, &*other.display)
    }
}
pub(crate) type Drafts = HashMap<(PathKey, EditorKey), DraftRecord>;

#[derive(Clone, Default)]
pub(crate) struct EditClock(Rc<RefCell<ClockState>>);
#[derive(Default)]
struct ClockState {
    epoch: u64,
    pending: HashMap<u64, (u64, CanonicalAddress)>,
}
pub(crate) struct EditTicket {
    clock: EditClock,
    id: u64,
}
impl EditClock {
    pub(crate) fn epoch(&self) -> u64 {
        self.0.borrow().epoch
    }
    pub(crate) fn is_pending(&self) -> bool {
        !self.0.borrow().pending.is_empty()
    }
    pub(crate) fn reserve(&self, control: u64, address: CanonicalAddress) -> EditTicket {
        let mut state = self.0.borrow_mut();
        state.epoch = state.epoch.checked_add(1).expect("edit epoch exhausted");
        let id = state.epoch;
        state.pending.insert(id, (control, address));
        EditTicket {
            clock: self.clone(),
            id,
        }
    }
    pub(crate) fn cancel_control(&self, control: u64) {
        self.0
            .borrow_mut()
            .pending
            .retain(|_, (c, _)| *c != control);
    }
    pub(crate) fn cancel_all(&self) {
        self.0.borrow_mut().pending.clear();
    }
    pub(crate) fn cancel_at(&self, address: &CanonicalAddress) -> bool {
        let mut state = self.0.borrow_mut();
        let before = state.pending.len();
        state.pending.retain(|_, (_, a)| !address.is_prefix_of(a));
        state.pending.len() != before
    }
}
impl EditTicket {
    pub(crate) fn accept(self) -> bool {
        self.clock.0.borrow_mut().pending.remove(&self.id).is_some()
    }
}
impl Drop for EditTicket {
    fn drop(&mut self) {
        self.clock.0.borrow_mut().pending.remove(&self.id);
    }
}

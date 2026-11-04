use std::{
    any::{Any, TypeId},
    borrow::Cow,
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
};

use crate::{PathKey, ValidationIssue, ValidationMessage, topology::CanonicalAddress};

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
impl From<&EditorKey> for EditorKey {
    fn from(key: &EditorKey) -> Self {
        key.clone()
    }
}

/// Accepted unresolved state for one (path, EditorKey), independent of mounted controls.
/// Pending native input is not an accepted draft. Issues belong only to this editor.
#[derive(Clone, Debug, PartialEq)]
pub struct EditorStatus {
    pub(crate) has_draft: bool,
    pub(crate) issues: Vec<ValidationIssue>,
}
impl EditorStatus {
    pub fn has_draft(&self) -> bool {
        self.has_draft
    }
    pub fn issues(&self) -> &[ValidationIssue] {
        &self.issues
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

/// A recoverable admission problem. The source display is retained as an editor draft.
#[derive(Clone, Debug, PartialEq)]
pub struct EditProblem {
    code: Cow<'static, str>,
    message: ValidationMessage,
}
impl EditProblem {
    pub fn new(code: impl Into<Cow<'static, str>>, message: impl Into<ValidationMessage>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
    pub fn code(&self) -> &str {
        &self.code
    }
    pub fn message(&self) -> &ValidationMessage {
        &self.message
    }
    pub(crate) fn into_parts(self) -> (String, ValidationMessage) {
        (self.code.into_owned(), self.message)
    }
}

type UpdateFn<S> = dyn FnOnce(&S) -> Result<S, EditProblem> + 'static;

/// A one-shot candidate calculation over the latest storage at admission.
///
/// Capture parsed intent and owned context/baseline fragments at native event entry.
/// The callback must not reparse text, read external state, cause side effects or panic.
/// Cancelled edits never run it; a failed update is retained as a draft and never retried.
pub struct StorageUpdate<S>(Box<UpdateFn<S>>);
impl<S> StorageUpdate<S> {
    pub fn new(update: impl FnOnce(&S) -> Result<S, EditProblem> + 'static) -> Self {
        Self(Box::new(update))
    }
    pub(crate) fn apply(self, latest: &S) -> Result<S, EditProblem> {
        (self.0)(latest)
    }
}

pub enum EditResult<S, D> {
    /// Replace the entire bound value with this candidate, in admission order.
    Parsed(S),
    /// Apply parsed intent to the latest storage once, at admission.
    Update(StorageUpdate<S>),
    Draft {
        display: D,
        code: Cow<'static, str>,
        message: ValidationMessage,
    },
}
impl<S, D> EditResult<S, D> {
    pub fn update(update: impl FnOnce(&S) -> Result<S, EditProblem> + 'static) -> Self {
        Self::Update(StorageUpdate::new(update))
    }
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
    pending: HashMap<u64, PendingEdit>,
}
struct PendingEdit {
    control: u64,
    address: CanonicalAddress,
    editor: EditorKey,
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
    pub(crate) fn is_control_pending(&self, control: u64) -> bool {
        self.0
            .borrow()
            .pending
            .values()
            .any(|edit| edit.control == control)
    }
    pub(crate) fn reserve(
        &self,
        control: u64,
        address: CanonicalAddress,
        editor: EditorKey,
    ) -> EditTicket {
        let mut state = self.0.borrow_mut();
        state.epoch = state.epoch.checked_add(1).expect("edit epoch exhausted");
        let id = state.epoch;
        state.pending.insert(
            id,
            PendingEdit {
                control,
                address,
                editor,
            },
        );
        EditTicket {
            clock: self.clone(),
            id,
        }
    }
    pub(crate) fn cancel_control(&self, control: u64) {
        self.0
            .borrow_mut()
            .pending
            .retain(|_, edit| edit.control != control);
    }
    pub(crate) fn cancel_all(&self) {
        self.0.borrow_mut().pending.clear();
    }
    pub(crate) fn cancel_at(&self, address: &CanonicalAddress) -> bool {
        let mut state = self.0.borrow_mut();
        let before = state.pending.len();
        state
            .pending
            .retain(|_, edit| !address.is_prefix_of(&edit.address));
        state.pending.len() != before
    }
    pub(crate) fn cancel_editor_at(
        &self,
        address: &CanonicalAddress,
        editor: &EditorKey,
    ) -> Vec<u64> {
        let mut controls = Vec::new();
        self.0.borrow_mut().pending.retain(|_, edit| {
            if &edit.address == address && &edit.editor == editor {
                controls.push(edit.control);
                false
            } else {
                true
            }
        });
        controls
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

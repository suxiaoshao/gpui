use std::{any::Any, rc::Rc};

use crate::{FieldChangeCause, FieldPath};

/// A typed, form-level notification for observers that need to invalidate a
/// page or section without reading component state during rendering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormStoreEvent<Field> {
    FieldChanged {
        field: Field,
        cause: FieldChangeCause,
    },
}

/// Internal runtime event used by generated field handles. The draft is
/// owned/type-erased so the core crate does not need a generated event enum.
pub struct FormDraftEvent {
    path: FieldPath,
    draft: Rc<dyn Any>,
    cause: FieldChangeCause,
}

impl FormDraftEvent {
    pub fn new<Draft>(path: FieldPath, draft: Draft, cause: FieldChangeCause) -> Self
    where
        Draft: Any,
    {
        Self {
            path,
            draft: Rc::new(draft),
            cause,
        }
    }

    pub fn path(&self) -> &FieldPath {
        &self.path
    }

    pub fn cause(&self) -> FieldChangeCause {
        self.cause
    }

    pub fn draft<Draft: Any>(&self) -> Option<&Draft> {
        self.draft.downcast_ref()
    }
}

pub struct FieldDraftEvent<Draft> {
    pub draft: Draft,
    pub cause: FieldChangeCause,
}

impl<Draft: Clone> Clone for FieldDraftEvent<Draft> {
    fn clone(&self) -> Self {
        Self {
            draft: self.draft.clone(),
            cause: self.cause,
        }
    }
}

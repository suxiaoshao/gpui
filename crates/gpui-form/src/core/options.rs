use crate::{FieldError, ValidationTrigger};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptionsSnapshot<Item> {
    items: Vec<Item>,
    revision: u64,
}

impl<Item> Default for OptionsSnapshot<Item> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            revision: 0,
        }
    }
}

impl<Item> OptionsSnapshot<Item> {
    pub fn new(items: Vec<Item>) -> Self {
        Self { items, revision: 0 }
    }

    pub fn items(&self) -> &[Item] {
        &self.items
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn replace(&mut self, items: Vec<Item>) {
        self.items = items;
        self.revision = self.revision.saturating_add(1);
    }

    pub fn into_items(self) -> Vec<Item> {
        self.items
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct OptionMismatch {
    pub error: FieldError,
}

impl OptionMismatch {
    pub fn new(error: FieldError) -> Self {
        Self { error }
    }

    pub fn trigger(&self) -> ValidationTrigger {
        self.error.trigger
    }
}

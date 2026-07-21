use std::hash::Hash;

use crate::{path::FieldPath, trigger::ValidationTrigger};

pub trait FormFieldId: Clone + Copy + Eq + Hash + 'static {
    fn path(self) -> FieldPath;
    fn schema(self) -> &'static FieldSchema;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ValidationTriggers {
    pub mount: bool,
    pub change: bool,
    pub blur: bool,
    pub dynamic: bool,
    pub submit: bool,
}

impl ValidationTriggers {
    pub const fn includes(self, trigger: ValidationTrigger) -> bool {
        match trigger {
            ValidationTrigger::Mount => self.mount,
            ValidationTrigger::Change => self.change,
            ValidationTrigger::Blur => self.blur,
            ValidationTrigger::Dynamic => self.dynamic,
            ValidationTrigger::Submit => self.submit,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldSchema {
    name: &'static str,
    required: bool,
    triggers: ValidationTriggers,
}

impl FieldSchema {
    pub const fn new(name: &'static str, required: bool, triggers: ValidationTriggers) -> Self {
        Self {
            name,
            required,
            triggers,
        }
    }

    pub const fn name(self) -> &'static str {
        self.name
    }

    pub const fn is_required(self) -> bool {
        self.required
    }

    pub const fn triggers(self) -> ValidationTriggers {
        self.triggers
    }
}

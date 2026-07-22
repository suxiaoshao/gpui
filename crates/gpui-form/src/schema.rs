use std::{fmt, hash::Hash};

use crate::{
    array::FormItemId,
    path::{FieldPath, FieldPathSegment},
    trigger::ValidationTrigger,
};

#[doc(hidden)]
pub trait FormModelSchema {
    fn schema_at_path(
        &self,
        segments: &[FieldPathSegment],
    ) -> Result<&'static FieldSchema, FormSchemaPathError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[doc(hidden)]
pub enum FormSchemaPathError {
    EmptyPath,
    UnknownField,
    UnexpectedItem,
    MissingItem(FormItemId),
    DuplicateItem(FormItemId),
    Projection,
    TrailingSegments,
}

impl fmt::Display for FormSchemaPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPath => f.write_str("field schema paths cannot be empty"),
            Self::UnknownField => f.write_str("the field is not declared by this form model"),
            Self::UnexpectedItem => {
                f.write_str("an array item segment does not follow an identified array field")
            }
            Self::MissingItem(id) => write!(f, "array item #{id} is missing"),
            Self::DuplicateItem(id) => write!(f, "array item #{id} is duplicated"),
            Self::Projection => f.write_str("projection paths do not have model schemas"),
            Self::TrailingSegments => {
                f.write_str("the field schema path has segments after a leaf field")
            }
        }
    }
}

impl std::error::Error for FormSchemaPathError {}

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

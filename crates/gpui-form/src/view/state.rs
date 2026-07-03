use crate::{FormField, FormMeta, ValidationSeverity};

use super::render::FieldErrorViewState;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FieldViewState {
    pub required: bool,
    pub errors: Vec<FieldErrorViewState>,
    pub has_error: bool,
}

impl FieldViewState {
    pub fn from_field<Field>(field: &Field, form_meta: &FormMeta) -> Self
    where
        Field: FormField,
    {
        let errors = field
            .visible_errors(form_meta)
            .into_iter()
            .map(FieldErrorViewState::from_error)
            .collect::<Vec<_>>();
        let has_error = errors
            .iter()
            .any(|error| error.severity == ValidationSeverity::Error);

        Self {
            required: field.is_required(),
            errors,
            has_error,
        }
    }
}

use gpui::{App, Context, Window};

use crate::{
    DraftFieldStore, FieldChangeCause, FieldCodec, FieldError, FieldPath, FormItemId, FormMeta,
    FormValidationReport, ValidationScope, ValidationSource, ValidationTriggers,
};

#[doc(hidden)]
pub fn field_path(name: &'static str) -> FieldPath {
    FieldPath::from_static(name)
}

#[doc(hidden)]
pub fn draft_field<Value, Codec>(
    _name: &'static str,
    value: Value,
    triggers: ValidationTriggers,
    required: bool,
) -> DraftFieldStore<Value, Codec>
where
    Value: Clone + PartialEq + 'static,
    Codec: FieldCodec<Value>,
{
    let mut field = DraftFieldStore::<Value, Codec>::new(value);
    field.core_mut().set_validation_triggers(triggers);
    field.core_mut().set_required(required);
    field
}

#[doc(hidden)]
pub fn scope_contains_path(scope: &ValidationScope, path: &FieldPath) -> bool {
    match scope {
        ValidationScope::Form => true,
        ValidationScope::Field(field_path) => path == field_path || field_path.starts_with(path),
        ValidationScope::Group(group_path) => path.starts_with(group_path),
        ValidationScope::ArrayItem {
            path: array_path,
            id,
        } => path.starts_with(&array_item_path(array_path, *id)),
    }
}

#[doc(hidden)]
pub fn merge_field_errors_preserving_internal(
    existing: &[FieldError],
    report: &FormValidationReport,
    path: &FieldPath,
) -> Vec<FieldError> {
    let mut errors = existing
        .iter()
        .filter(|error| error.source == ValidationSource::Internal)
        .cloned()
        .collect::<Vec<_>>();
    for error in report
        .field_errors()
        .iter()
        .filter(|error| &error.path == path)
    {
        if !errors.iter().any(|existing| existing == error) {
            errors.push(error.clone());
        }
    }
    errors
}

fn array_item_path(path: &FieldPath, id: FormItemId) -> FieldPath {
    path.join_item(id)
}

pub trait GeneratedFormStore<Input>: Sized + 'static {
    fn from_value(value: Input, window: &mut Window, cx: &mut Context<Self>) -> Self;
    fn draft(&self) -> Input;
    fn replace_from_value(&mut self, value: Input, cx: &mut Context<Self>);
    fn field_paths(&self) -> &[FieldPath];
    fn write_draft(
        &mut self,
        value: Input,
        cause: FieldChangeCause,
        window: &mut Window,
        cx: &mut App,
    );
    fn meta(&self) -> &FormMeta;
    fn apply_validation_report(
        &mut self,
        report: &FormValidationReport,
        scope: &ValidationScope,
        cx: &mut App,
    );
    fn prepare_submit(&mut self, _cx: &mut App) -> FormValidationReport {
        FormValidationReport::empty()
    }
    fn current_validation_report(&self, _cx: &App) -> FormValidationReport {
        FormValidationReport::empty()
    }

    fn clear_all_errors(&mut self, _cx: &mut App) {}
}

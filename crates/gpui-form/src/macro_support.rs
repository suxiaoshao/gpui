use gpui::{App, Context, Entity, Window};

use crate::{
    ComponentFieldEventOutcome, ComponentFieldStore, FieldChangeCause, FieldError, FieldPath,
    FormComponentBinding, FormItemId, FormMeta, FormValidationReport, ValidationScope,
    ValidationSource, ValidationTriggers, ValueFieldStore,
};

#[doc(hidden)]
pub fn field_path(name: &'static str) -> FieldPath {
    FieldPath::from_static(name)
}

#[doc(hidden)]
pub fn value_field<T>(_name: &'static str, value: T) -> ValueFieldStore<T>
where
    T: Clone + PartialEq + 'static,
{
    ValueFieldStore::new(value)
}

#[doc(hidden)]
pub fn component_field<Value, Binding>(
    value: Value,
    state: Entity<Binding::State>,
    triggers: ValidationTriggers,
    required: bool,
) -> ComponentFieldStore<Value, Binding>
where
    Value: Clone + PartialEq + 'static,
    Binding: FormComponentBinding<Value>,
{
    let mut field = ComponentFieldStore::<Value, Binding>::new(value, state);
    field.core_mut().set_validation_triggers(triggers);
    field.core_mut().set_required(required);
    field
}

#[doc(hidden)]
pub fn component_field_event_trigger(
    outcome: ComponentFieldEventOutcome,
) -> Option<crate::ValidationTrigger> {
    outcome.validation_trigger()
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
    errors.extend(
        report
            .field_errors()
            .iter()
            .filter(|error| &error.path == path)
            .cloned(),
    );
    errors
}

fn array_item_path(path: &FieldPath, id: FormItemId) -> FieldPath {
    path.join_item(id)
}

pub trait GeneratedFormStore<Input>: Sized + 'static {
    fn from_value(value: Input, window: &mut Window, cx: &mut Context<Self>) -> Self;
    fn draft(&self) -> Input;
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

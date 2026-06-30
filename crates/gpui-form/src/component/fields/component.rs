use std::marker::PhantomData;

use gpui::{App, Entity, Window};

use crate::{
    AnyFormField, FieldChangeCause, FieldCore, FieldError, FieldMeta, FieldValidationReport,
    FormComponentBinding, FormField, FormMeta, ValidationTrigger,
};

pub struct ComponentFieldStore<Value, Binding>
where
    Value: Clone + PartialEq + 'static,
    Binding: FormComponentBinding<Value>,
{
    core: FieldCore<Value>,
    state: Entity<Binding::State>,
    _binding: PhantomData<fn() -> Binding>,
}

impl<Value, Binding> ComponentFieldStore<Value, Binding>
where
    Value: Clone + PartialEq + 'static,
    Binding: FormComponentBinding<Value>,
{
    pub fn new(value: Value, state: Entity<Binding::State>) -> Self {
        Self {
            core: FieldCore::new(value),
            state,
            _binding: PhantomData,
        }
    }

    pub fn state(&self) -> Entity<Binding::State> {
        self.state.clone()
    }

    pub fn core(&self) -> &FieldCore<Value> {
        &self.core
    }

    pub fn core_mut(&mut self) -> &mut FieldCore<Value> {
        &mut self.core
    }

    pub fn read_component_value(&self, cx: &App) -> Value {
        Binding::read_value(&self.state, cx)
    }

    pub fn write_component_value(
        &self,
        value: &Value,
        cause: FieldChangeCause,
        window: &mut Window,
        cx: &mut App,
    ) {
        Binding::write_value(&self.state, value, cause, window, cx);
    }

    pub fn set_required(&mut self, required: bool, window: &mut Window, cx: &mut App) {
        self.core.set_required(required);
        Binding::set_required(&self.state, required, window, cx);
    }
}

impl<Value, Binding> FormField for ComponentFieldStore<Value, Binding>
where
    Value: Clone + PartialEq + 'static,
    Binding: FormComponentBinding<Value>,
{
    type Value = Value;
    type ComponentState = Binding::State;

    fn value(&self) -> &Self::Value {
        self.core.value()
    }

    fn set_value(&mut self, value: Self::Value, cause: FieldChangeCause) {
        self.core.set_value(value, cause);
    }

    fn reset(&mut self, window: &mut Window, cx: &mut App) {
        self.core.reset();
        let value = self.core.value().clone();
        Binding::write_value(&self.state, &value, FieldChangeCause::Reset, window, cx);
    }

    fn component_state(&self) -> Option<Entity<Self::ComponentState>> {
        Some(self.state.clone())
    }

    fn meta(&self) -> &FieldMeta {
        self.core.meta()
    }

    fn is_required(&self) -> bool {
        self.core.is_required()
    }

    fn errors(&self) -> &[FieldError] {
        self.core.errors()
    }

    fn visible_errors(&self, form_meta: &FormMeta) -> Vec<&FieldError> {
        self.core.visible_errors(form_meta)
    }

    fn set_errors(&mut self, errors: Vec<FieldError>) {
        self.core.set_errors(errors);
    }

    fn clear_errors(&mut self) {
        self.core.clear_errors();
    }

    fn mark_touched(&mut self) {
        self.core.meta_mut().mark_touched();
    }

    fn mark_blurred(&mut self) {
        self.core.meta_mut().mark_blurred();
    }

    fn validate(&mut self, _trigger: ValidationTrigger) -> FieldValidationReport {
        FieldValidationReport::new(self.core.errors().to_vec())
    }

    fn focus(&mut self, window: &mut Window, cx: &mut App) -> bool {
        Binding::focus(&self.state, window, cx)
    }
}

impl<Value, Binding> AnyFormField for ComponentFieldStore<Value, Binding>
where
    Value: Clone + PartialEq + 'static,
    Binding: FormComponentBinding<Value>,
{
    fn meta(&self) -> &FieldMeta {
        self.core.meta()
    }

    fn is_required(&self) -> bool {
        self.core.is_required()
    }

    fn errors(&self) -> &[FieldError] {
        self.core.errors()
    }

    fn visible_errors(&self, form_meta: &FormMeta) -> Vec<&FieldError> {
        self.core.visible_errors(form_meta)
    }

    fn set_errors(&mut self, errors: Vec<FieldError>) {
        self.core.set_errors(errors);
    }

    fn clear_errors(&mut self) {
        self.core.clear_errors();
    }

    fn focus_any(&mut self, window: &mut Window, cx: &mut App) -> bool {
        self.focus(window, cx)
    }
}

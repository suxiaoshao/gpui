use gpui::{App, AppContext as _, Entity, Window};

use crate::{
    AnyFormField, ComponentStateOptions, FieldChangeCause, FieldCore, FieldError, FieldMeta,
    FieldValidationReport, FormComponentBinding, FormField, FormMeta, ValidationTrigger,
};

#[derive(Debug)]
pub struct BoolComponentState {
    value: bool,
    disabled: bool,
}

impl BoolComponentState {
    pub fn value(&self) -> bool {
        self.value
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }
}

pub struct BoolBinding;

impl FormComponentBinding<bool> for BoolBinding {
    type State = BoolComponentState;
    type Event = ();

    fn new_state(
        initial: &bool,
        options: ComponentStateOptions,
        _window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self::State> {
        let value = *initial;
        cx.new(|_| BoolComponentState {
            value,
            disabled: options.disabled,
        })
    }

    fn read_value(state: &Entity<Self::State>, cx: &App) -> bool {
        state.read(cx).value
    }

    fn write_value(
        state: &Entity<Self::State>,
        value: &bool,
        _cause: FieldChangeCause,
        _window: &mut Window,
        cx: &mut App,
    ) {
        state.update(cx, |state, _| {
            state.value = *value;
        });
    }

    fn set_disabled(
        state: &Entity<Self::State>,
        disabled: bool,
        _window: &mut Window,
        cx: &mut App,
    ) {
        state.update(cx, |state, _| {
            state.disabled = disabled;
        });
    }

    fn focus(_state: &Entity<Self::State>, _window: &mut Window, _cx: &mut App) -> bool {
        false
    }
}

#[derive(Debug)]
pub struct BoolFieldStore {
    core: FieldCore<bool>,
    state: Entity<BoolComponentState>,
}

impl BoolFieldStore {
    pub fn new(value: bool, state: Entity<BoolComponentState>) -> Self {
        Self {
            core: FieldCore::new(value),
            state,
        }
    }

    pub fn state(&self) -> Entity<BoolComponentState> {
        self.state.clone()
    }

    pub fn core(&self) -> &FieldCore<bool> {
        &self.core
    }

    pub fn core_mut(&mut self) -> &mut FieldCore<bool> {
        &mut self.core
    }

    pub fn read_component_value(&self, cx: &App) -> bool {
        BoolBinding::read_value(&self.state, cx)
    }

    pub fn write_component_value(
        &self,
        value: &bool,
        cause: FieldChangeCause,
        window: &mut Window,
        cx: &mut App,
    ) {
        BoolBinding::write_value(&self.state, value, cause, window, cx);
    }
}

impl FormField for BoolFieldStore {
    type Value = bool;
    type ComponentState = BoolComponentState;

    fn value(&self) -> &Self::Value {
        self.core.value()
    }

    fn set_value(&mut self, value: Self::Value, cause: FieldChangeCause) {
        self.core.set_value(value, cause);
    }

    fn reset(&mut self, _window: &mut Window, _cx: &mut App) {
        self.core.reset();
        BoolBinding::write_value(
            &self.state,
            self.core.value(),
            FieldChangeCause::Reset,
            _window,
            _cx,
        );
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

    fn focus(&mut self, _window: &mut Window, _cx: &mut App) -> bool {
        BoolBinding::focus(&self.state, _window, _cx)
    }
}

impl AnyFormField for BoolFieldStore {
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

    fn focus_any(&mut self, _window: &mut Window, _cx: &mut App) -> bool {
        BoolBinding::focus(&self.state, _window, _cx)
    }
}

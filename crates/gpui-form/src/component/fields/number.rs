use std::marker::PhantomData;
use std::str::FromStr;

use gpui::{App, AppContext as _, Entity, Window};
use gpui_component::input::{InputEvent, InputState};

use crate::{
    AnyFormField, ComponentStateOptions, FieldChangeCause, FieldCore, FieldError, FieldMeta,
    FieldValidationReport, FormComponentBinding, FormField, FormMeta, ValidationSource,
    ValidationTrigger, resolve_form_text,
};

pub trait NumberFieldValue: Clone + PartialEq + ToString + FromStr + 'static {}

impl<T> NumberFieldValue for T where T: Clone + PartialEq + ToString + FromStr + 'static {}

pub struct NumberInputBinding<N>(PhantomData<fn() -> N>);

impl<N> FormComponentBinding<N> for NumberInputBinding<N>
where
    N: NumberFieldValue,
{
    type State = InputState;
    type Event = InputEvent;

    fn new_state(
        initial: &N,
        options: ComponentStateOptions,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self::State> {
        cx.new(|cx| {
            let mut input = InputState::new(window, cx).default_value(initial.to_string());
            if let Some(placeholder_key) = options.placeholder_key {
                input = input.placeholder(resolve_form_text(placeholder_key, cx));
            }
            if options.masked {
                input = input.masked(true);
            }
            input
        })
    }

    fn read_value(state: &Entity<Self::State>, cx: &App) -> N {
        match state.read(cx).value().to_string().parse::<N>() {
            Ok(value) => value,
            Err(_) => panic!("gpui-form number input state contains an unparsable value"),
        }
    }

    fn write_value(
        state: &Entity<Self::State>,
        value: &N,
        _cause: FieldChangeCause,
        window: &mut Window,
        cx: &mut App,
    ) {
        state.update(cx, |input, cx| {
            input.set_value(value.to_string(), window, cx);
        });
    }

    fn focus(state: &Entity<Self::State>, window: &mut Window, cx: &mut App) -> bool {
        state.update(cx, |input, cx| {
            input.focus(window, cx);
        });
        true
    }
}

#[derive(Debug)]
pub struct NumberFieldStore<N>
where
    N: NumberFieldValue,
{
    core: FieldCore<N>,
    input_state: Entity<InputState>,
    parse_error: Option<FieldError>,
}

impl<N> NumberFieldStore<N>
where
    N: NumberFieldValue,
{
    pub fn new(value: N, input_state: Entity<InputState>) -> Self {
        Self {
            core: FieldCore::new(value),
            input_state,
            parse_error: None,
        }
    }

    pub fn input_state(&self) -> Entity<InputState> {
        self.input_state.clone()
    }

    pub fn core(&self) -> &FieldCore<N> {
        &self.core
    }

    pub fn core_mut(&mut self) -> &mut FieldCore<N> {
        &mut self.core
    }

    pub fn try_read_component_value(&self, cx: &App) -> Result<N, N::Err> {
        self.input_state.read(cx).value().to_string().parse::<N>()
    }

    pub fn write_component_value(
        &self,
        value: &N,
        cause: FieldChangeCause,
        window: &mut Window,
        cx: &mut App,
    ) {
        NumberInputBinding::<N>::write_value(&self.input_state, value, cause, window, cx);
    }

    pub fn set_parse_error(&mut self, error: Option<FieldError>) {
        self.parse_error = error;
        let mut errors = self
            .core
            .errors()
            .iter()
            .filter(|error| {
                !(error.source == ValidationSource::Internal && error.code.as_ref() == "parse")
            })
            .cloned()
            .collect::<Vec<_>>();
        if let Some(error) = &self.parse_error {
            errors.push(error.clone());
        }
        self.core.set_errors(errors);
    }
}

impl<N> FormField for NumberFieldStore<N>
where
    N: NumberFieldValue,
{
    type Value = N;
    type ComponentState = InputState;

    fn value(&self) -> &Self::Value {
        self.core.value()
    }

    fn set_value(&mut self, value: Self::Value, cause: FieldChangeCause) {
        self.core.set_value(value, cause);
    }

    fn reset(&mut self, window: &mut Window, cx: &mut App) {
        self.core.reset();
        self.write_component_value(self.core.value(), FieldChangeCause::Reset, window, cx);
    }

    fn component_state(&self) -> Option<Entity<Self::ComponentState>> {
        Some(self.input_state.clone())
    }

    fn meta(&self) -> &FieldMeta {
        self.core.meta()
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
        self.parse_error = None;
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
        NumberInputBinding::<N>::focus(&self.input_state, window, cx)
    }
}

impl<N> AnyFormField for NumberFieldStore<N>
where
    N: NumberFieldValue,
{
    fn meta(&self) -> &FieldMeta {
        self.core.meta()
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
        self.parse_error = None;
    }

    fn focus_any(&mut self, window: &mut Window, cx: &mut App) -> bool {
        self.focus(window, cx)
    }
}

use std::marker::PhantomData;

use gpui::{App, AppContext as _, Entity, Window};
use gpui_component::input::{InputEvent, InputState};

use crate::{
    AnyFormField, ComponentStateOptions, FieldChangeCause, FieldCore, FieldError, FieldMeta,
    FieldValidationReport, FormComponentBinding, FormField, FormMeta, ValidationTrigger,
    resolve_form_text,
};

pub trait TextFieldValue: Clone + PartialEq + 'static {
    fn to_text(&self) -> String;
    fn from_text(text: String) -> Self;
}

impl TextFieldValue for String {
    fn to_text(&self) -> String {
        self.clone()
    }

    fn from_text(text: String) -> Self {
        text
    }
}

impl TextFieldValue for Option<String> {
    fn to_text(&self) -> String {
        self.clone().unwrap_or_default()
    }

    fn from_text(text: String) -> Self {
        if text.is_empty() { None } else { Some(text) }
    }
}

pub struct TextInputBinding<T>(PhantomData<fn() -> T>);

impl<T> FormComponentBinding<T> for TextInputBinding<T>
where
    T: TextFieldValue,
{
    type State = InputState;
    type Event = InputEvent;

    fn new_state(
        initial: &T,
        options: ComponentStateOptions,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self::State> {
        cx.new(|cx| {
            let mut input = InputState::new(window, cx).default_value(initial.to_text());
            if let Some(placeholder_key) = options.placeholder_key {
                input = input.placeholder(resolve_form_text(placeholder_key, cx));
            }
            if options.masked {
                input = input.masked(true);
            }
            input
        })
    }

    fn read_value(state: &Entity<Self::State>, cx: &App) -> T {
        T::from_text(state.read(cx).value().to_string())
    }

    fn write_value(
        state: &Entity<Self::State>,
        value: &T,
        _cause: FieldChangeCause,
        window: &mut Window,
        cx: &mut App,
    ) {
        state.update(cx, |input, cx| {
            input.set_value(value.to_text(), window, cx);
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
pub struct TextFieldStore<T>
where
    T: TextFieldValue,
{
    core: FieldCore<T>,
    input_state: Entity<InputState>,
}

impl<T> TextFieldStore<T>
where
    T: TextFieldValue,
{
    pub fn new(value: T, input_state: Entity<InputState>) -> Self {
        Self {
            core: FieldCore::new(value),
            input_state,
        }
    }

    pub fn input_state(&self) -> Entity<InputState> {
        self.input_state.clone()
    }

    pub fn core(&self) -> &FieldCore<T> {
        &self.core
    }

    pub fn core_mut(&mut self) -> &mut FieldCore<T> {
        &mut self.core
    }

    pub fn read_component_value(&self, cx: &App) -> T {
        TextInputBinding::<T>::read_value(&self.input_state, cx)
    }

    pub fn write_component_value(
        &self,
        value: &T,
        cause: FieldChangeCause,
        window: &mut Window,
        cx: &mut App,
    ) {
        TextInputBinding::<T>::write_value(&self.input_state, value, cause, window, cx);
    }
}

impl<T> FormField for TextFieldStore<T>
where
    T: TextFieldValue,
{
    type Value = T;
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
        TextInputBinding::<T>::focus(&self.input_state, window, cx)
    }
}

impl<T> AnyFormField for TextFieldStore<T>
where
    T: TextFieldValue,
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
    }

    fn focus_any(&mut self, window: &mut Window, cx: &mut App) -> bool {
        self.focus(window, cx)
    }
}

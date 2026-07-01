use std::{marker::PhantomData, str::FromStr};

use gpui::{App, AppContext as _, Entity, Window};
use gpui_component::input::{InputEvent, InputState, NumberInput};
use gpui_form::{
    ComponentStateOptions, FieldChangeCause, FieldError, FieldPath, FormComponentBinding,
    FormComponentEvent, ValidationSource, ValidationTrigger, resolve_form_text,
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
    type Draft = String;

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

    fn draft_from_value(value: &N) -> Self::Draft {
        value.to_string()
    }

    fn read_draft(state: &Entity<Self::State>, cx: &App) -> Self::Draft {
        state.read(cx).value().to_string()
    }

    fn parse_draft(
        draft: &Self::Draft,
        path: FieldPath,
        trigger: ValidationTrigger,
        _cx: &App,
    ) -> Result<N, Box<FieldError>> {
        draft.parse::<N>().map_err(|_| {
            Box::new(
                FieldError::new(
                    path,
                    trigger,
                    ValidationSource::Internal,
                    "parse",
                    "gpui-form-error-number-parse",
                )
                .with_param("value", draft.clone()),
            )
        })
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

    fn event_kind(event: &Self::Event) -> Option<FormComponentEvent> {
        match event {
            InputEvent::Change => Some(FormComponentEvent::Change(FieldChangeCause::UserInput)),
            InputEvent::Focus => Some(FormComponentEvent::Focus),
            InputEvent::Blur => Some(FormComponentEvent::Blur),
            InputEvent::PressEnter { .. } => None,
        }
    }

    fn focus(state: &Entity<Self::State>, window: &mut Window, cx: &mut App) -> bool {
        state.update(cx, |input, cx| {
            input.focus(window, cx);
        });
        true
    }
}

pub fn number_input<N>(state: &Entity<InputState>) -> NumberInput
where
    N: NumberFieldValue,
{
    let _ = PhantomData::<fn() -> N>;
    NumberInput::new(state)
}

use std::marker::PhantomData;

use gpui::{App, AppContext as _, Context, Entity, Window};
use gpui_component::input::{InputEvent, InputState};
use gpui_form::{
    ComponentStateOptions, FieldChangeCause, FieldError, FieldPath, FormComponentBinding,
    FormComponentEvent, FormComponentEventSink, SubscriptionSet, ValidationTrigger,
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
    type Draft = String;

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

    fn draft_from_value(value: &T) -> Self::Draft {
        value.to_text()
    }

    fn read_draft(state: &Entity<Self::State>, cx: &App) -> Self::Draft {
        state.read(cx).value().to_string()
    }

    fn parse_draft(
        draft: &Self::Draft,
        _path: FieldPath,
        _trigger: ValidationTrigger,
        _cx: &App,
    ) -> Result<T, Box<FieldError>> {
        Ok(T::from_text(draft.clone()))
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

    fn install_subscriptions<Form>(
        state: Entity<Self::State>,
        sink: FormComponentEventSink<Form>,
        window: &mut Window,
        cx: &mut Context<Form>,
    ) -> SubscriptionSet
    where
        Form: 'static,
    {
        let mut subscriptions = SubscriptionSet::new();
        subscriptions.push(cx.subscribe_in(
            &state,
            window,
            move |form, _state, event: &InputEvent, window, cx| {
                let event = match event {
                    InputEvent::Change => {
                        Some(FormComponentEvent::Change(FieldChangeCause::UserInput))
                    }
                    InputEvent::Focus => Some(FormComponentEvent::Focus),
                    InputEvent::Blur => Some(FormComponentEvent::Blur),
                    InputEvent::PressEnter { .. } => None,
                };
                if let Some(event) = event {
                    sink.emit(form, event, window, cx);
                }
            },
        ));
        subscriptions
    }
}

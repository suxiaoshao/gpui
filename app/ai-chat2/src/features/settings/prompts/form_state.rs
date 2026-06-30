use crate::foundation::I18n;
use gpui::*;
use gpui_component::input::{InputEvent, InputState};
use gpui_form::{
    ComponentStateOptions, FieldChangeCause, FieldError, FormComponentBinding, FormComponentEvent,
    FormField, FormMeta,
};

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = PromptEditFormStore)]
pub(super) struct PromptEditFormInput {
    #[form(component = "input", required, placeholder = "prompt-placeholder-name")]
    pub(super) name: String,
    #[form(
        binding = "PromptContentInputBinding",
        required,
        placeholder = "prompt-placeholder-content"
    )]
    pub(super) content: String,
}

impl PromptEditFormInput {
    pub(super) fn new(name: String, content: String) -> Self {
        Self { name, content }
    }
}

pub(super) struct PromptContentInputBinding;

impl FormComponentBinding<String> for PromptContentInputBinding {
    type State = InputState;
    type Event = InputEvent;

    fn new_state(
        initial: &String,
        options: ComponentStateOptions,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self::State> {
        cx.new(|cx| {
            let mut input = InputState::new(window, cx)
                .multi_line(true)
                .rows(10)
                .default_value(initial.clone());
            if let Some(placeholder_key) = options.placeholder_key {
                input = input.placeholder(cx.global::<I18n>().t(placeholder_key));
            }
            input
        })
    }

    fn read_value(state: &Entity<Self::State>, cx: &App) -> String {
        state.read(cx).value().to_string()
    }

    fn write_value(
        state: &Entity<Self::State>,
        value: &String,
        _cause: FieldChangeCause,
        window: &mut Window,
        cx: &mut App,
    ) {
        state.update(cx, |input, cx| {
            input.set_value(value.clone(), window, cx);
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

pub(super) fn field_errors<Field>(field: &Field) -> Vec<FieldError>
where
    Field: FormField,
{
    field
        .visible_errors(&FormMeta::default())
        .into_iter()
        .cloned()
        .collect()
}

use crate::foundation::I18n;
use gpui::*;
use gpui_component::input::{InputEvent, InputState};
use gpui_form::{
    ComponentStateOptions, FieldChangeCause, FieldError, FormComponentBinding, FormComponentEvent,
    FormComponentEventSink, FormField, FormMeta, SubscriptionSet,
};

type StringInputBinding = gpui_form_gpui_component::TextInputBinding<String>;

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = PromptEditFormStore)]
pub(super) struct PromptEditFormInput {
    #[form(
        binding = "StringInputBinding",
        required,
        placeholder = "prompt-placeholder-name"
    )]
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
    type Draft = String;

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

    fn draft_from_value(value: &String) -> Self::Draft {
        value.clone()
    }

    fn read_draft(state: &Entity<Self::State>, cx: &App) -> Self::Draft {
        state.read(cx).value().to_string()
    }

    fn parse_draft(
        draft: &Self::Draft,
        _path: gpui_form::FieldPath,
        _trigger: gpui_form::ValidationTrigger,
        _cx: &App,
    ) -> Result<String, Box<FieldError>> {
        Ok(draft.clone())
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

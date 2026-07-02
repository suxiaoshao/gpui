use gpui::{App, AppContext as _, Context, Entity, Window};
use gpui_component::input::{InputEvent, InputState};
use gpui_form::{
    ComponentStateOptions, FieldChangeCause, FieldError, FieldPath, FormComponentBinding,
    FormComponentEvent, FormComponentEventSink, SubscriptionSet, ValidationTrigger,
    resolve_form_text,
};

use super::ProviderFormField;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::features::settings::provider) struct ProviderSecretValue {
    pub(in crate::features::settings::provider) field: ProviderFormField,
    pub(in crate::features::settings::provider) value: String,
    pub(in crate::features::settings::provider) changed: bool,
}

impl ProviderSecretValue {
    pub(in crate::features::settings::provider) fn new(
        field: ProviderFormField,
        value: String,
        changed: bool,
    ) -> Self {
        Self {
            field,
            value,
            changed,
        }
    }

    pub(in crate::features::settings::provider) fn key(&self) -> &'static str {
        self.field.key()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::features::settings::provider) struct ProviderSecretDraft {
    field: ProviderFormField,
    value: String,
    changed: bool,
}

pub(in crate::features::settings::provider) struct ProviderSecretInputState {
    input: Entity<InputState>,
    field: ProviderFormField,
    changed: bool,
}

impl ProviderSecretInputState {
    pub(in crate::features::settings::provider) fn input(&self) -> Entity<InputState> {
        self.input.clone()
    }
}

pub(in crate::features::settings::provider) struct ProviderSecretInputBinding;

impl FormComponentBinding<ProviderSecretValue> for ProviderSecretInputBinding {
    type State = ProviderSecretInputState;
    type Draft = ProviderSecretDraft;

    fn new_state(
        initial: &ProviderSecretValue,
        options: ComponentStateOptions,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self::State> {
        let input = cx.new(|cx| {
            let mut input = InputState::new(window, cx).default_value(initial.value.clone());
            if let Some(placeholder_key) = options.placeholder_key {
                input = input.placeholder(resolve_form_text(placeholder_key, cx));
            }
            if options.masked {
                input = input.masked(true);
            }
            input
        });
        cx.new(|_| ProviderSecretInputState {
            input,
            field: initial.field,
            changed: initial.changed,
        })
    }

    fn draft_from_value(value: &ProviderSecretValue) -> Self::Draft {
        ProviderSecretDraft {
            field: value.field,
            value: value.value.clone(),
            changed: value.changed,
        }
    }

    fn read_draft(state: &Entity<Self::State>, cx: &App) -> Self::Draft {
        let state = state.read(cx);
        ProviderSecretDraft {
            field: state.field,
            value: state.input.read(cx).value().to_string(),
            changed: state.changed,
        }
    }

    fn parse_draft(
        draft: &Self::Draft,
        _path: FieldPath,
        _trigger: ValidationTrigger,
        _cx: &App,
    ) -> Result<ProviderSecretValue, Box<FieldError>> {
        Ok(ProviderSecretValue::new(
            draft.field,
            draft.value.clone(),
            draft.changed,
        ))
    }

    fn write_value(
        state: &Entity<Self::State>,
        value: &ProviderSecretValue,
        _cause: FieldChangeCause,
        window: &mut Window,
        cx: &mut App,
    ) {
        state.update(cx, |state, cx| {
            state.field = value.field;
            state.changed = value.changed;
            state.input.update(cx, |input, cx| {
                input.set_value(value.value.clone(), window, cx);
            });
        });
    }

    fn focus(state: &Entity<Self::State>, window: &mut Window, cx: &mut App) -> bool {
        let input = state.read(cx).input();
        input.update(cx, |input, cx| {
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
        let input = state.read(cx).input();
        let mut subscriptions = SubscriptionSet::new();
        subscriptions.push(cx.subscribe_in(
            &input,
            window,
            move |form, _input, event: &InputEvent, window, cx| {
                let event = match event {
                    InputEvent::Change => {
                        state.update(cx, |state, _cx| {
                            state.changed = true;
                        });
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

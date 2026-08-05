use std::ops::Deref;

use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_component::input::{InputEvent, InputState};
use gpui_form::typed::{FieldDef, Form, FormEvent, FormSchema};

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

impl gpui_form::typed::RequiredValue for ProviderSecretValue {
    fn is_missing(&self) -> bool {
        self.value.trim().is_empty()
    }
}

/// Owning control for a provider secret. Its lifetime owns both projection
/// subscriptions, so dropping the control detaches it from the shared form.
pub(in crate::features::settings::provider) struct ProviderSecretInput {
    #[allow(dead_code)]
    subscriptions: Vec<Subscription>,
    input: Entity<InputState>,
    _lease: gpui_form::ControlLease,
}

impl ProviderSecretInput {
    pub(in crate::features::settings::provider) fn new<M, Owner>(
        form: &Entity<Form<M>>,
        field: FieldDef<M, ProviderSecretValue>,
        placeholder: String,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Self
    where
        M: FormSchema,
        Owner: 'static,
    {
        let value = field.value(form, cx);
        let state = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder(placeholder)
        });
        state.update(cx, |input, cx| input.set_value(value.value, window, cx));
        let binding = field.bind_control(form, cx);
        let lease = binding.lease();
        let weak_state = state.downgrade();
        let weak_form = form.downgrade();
        let projected_field = field;
        let form_subscription =
            cx.subscribe_in(form, window, move |_owner, _, _: &FormEvent, window, cx| {
                let weak_state = weak_state.clone();
                let weak_form = weak_form.clone();
                let field = projected_field;
                cx.defer_in(window, move |_owner, window, cx| {
                    let (Some(state), Some(form)) = (weak_state.upgrade(), weak_form.upgrade())
                    else {
                        return;
                    };
                    let value = field.value(&form, cx);
                    state.update(cx, |input, cx| {
                        input.set_value(value.value, window, cx);
                    });
                });
            });
        let weak_form = form.downgrade();
        let input_field = field;
        let input_binding = binding;
        let input_subscription = cx.subscribe_in(
            &state,
            window,
            move |_owner, state, event: &InputEvent, window, cx| match event {
                InputEvent::Change => {
                    let text = state.read(cx).value().to_string();
                    let Some(form) = weak_form.upgrade() else {
                        return;
                    };
                    let mut value = input_field.value(&form, cx);
                    value.value = text;
                    value.changed = true;
                    input_binding.defer_set(value, window, cx);
                }
                InputEvent::Blur => input_binding.defer_blur(window, cx),
                InputEvent::Focus | InputEvent::PressEnter { .. } => {}
            },
        );

        Self {
            subscriptions: vec![form_subscription, input_subscription],
            input: state,
            _lease: lease,
        }
    }
}

impl Deref for ProviderSecretInput {
    type Target = Entity<InputState>;

    fn deref(&self) -> &Self::Target {
        &self.input
    }
}

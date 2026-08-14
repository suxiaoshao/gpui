use std::ops::Deref;

use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_component::input::{InputEvent, InputState};
use gpui_form::{ControlBinding, ControlProjection, FieldDef, Form, FormSchema};

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

impl gpui_form::RequiredValue for ProviderSecretValue {
    fn is_missing(&self) -> bool {
        self.value.trim().is_empty()
    }
}

/// Owning control for a provider secret. The binding owns form projection;
/// native input callbacks only retain its writer capability.
pub(in crate::features::settings::provider) struct ProviderSecretInput {
    _binding: ControlBinding,
    _subscription: Subscription,
    input: Entity<InputState>,
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
        let value = field.get(form, cx);
        let state = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder(placeholder)
        });
        state.update(cx, |input, cx| input.set_value(value.value, window, cx));
        let (binding, writer) = field.bind_control_in(
            form,
            &state,
            |input, projection, window, cx| match projection {
                ControlProjection::Value(value) => input.set_value(value.value, window, cx),
                ControlProjection::Retired => {}
            },
            window,
            cx,
        );
        let weak_form = form.downgrade();
        let input_field = field;
        let input_writer = writer;
        let subscription = cx.subscribe_in(
            &state,
            window,
            move |_owner, state, event: &InputEvent, window, cx| match event {
                InputEvent::Change => {
                    let text = state.read(cx).value().to_string();
                    let Some(form) = weak_form.upgrade() else {
                        return;
                    };
                    let mut value = input_field.get(&form, cx);
                    value.value = text;
                    value.changed = true;
                    input_writer.defer_set(value, window, cx);
                }
                InputEvent::Blur => input_writer.defer_blur(window, cx),
                InputEvent::Focus | InputEvent::PressEnter { .. } => {}
            },
        );

        Self {
            _binding: binding,
            _subscription: subscription,
            input: state,
        }
    }
}

impl Deref for ProviderSecretInput {
    type Target = Entity<InputState>;

    fn deref(&self) -> &Self::Target {
        &self.input
    }
}

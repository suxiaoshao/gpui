use std::ops::Deref;

use gpui::{AppContext as _, Context, Entity, EventEmitter, Subscription, Window};
use gpui_component::input::{InputEvent, InputState};
use gpui_form::typed::{FormField, FormStore};

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
pub(in crate::features::settings::provider) struct ProviderSecretInputState<Form>
where
    Form: FormStore,
{
    subscriptions: Vec<Subscription>,
    input: Entity<InputState>,
    _marker: std::marker::PhantomData<Form>,
}

impl<Form> ProviderSecretInputState<Form>
where
    Form: FormStore + EventEmitter<gpui_form::typed::FormEvent<Form::Field>>,
{
    pub(in crate::features::settings::provider) fn new<Owner>(
        field: FormField<Form, ProviderSecretValue>,
        placeholder: String,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Result<Self, gpui_form_gpui_component::FormControlError>
    where
        Owner: 'static,
    {
        let value = field
            .value(cx)
            .map_err(gpui_form_gpui_component::FormControlError::from)?;
        let state = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder(placeholder)
        });
        state.update(cx, |input, cx| input.set_value(value.value, window, cx));
        let attachment = field.attach_control(cx)?;
        let form_state = state.clone();
        let projected_field = field.clone();
        let form_subscription = field.subscribe_in(window, cx, move |_owner, window, cx| {
            let state = form_state.clone();
            let field = projected_field.clone();
            cx.defer_in(window, move |_owner, window, cx| {
                let Ok(value) = field.value(cx) else { return };
                state.update(cx, |input, cx| {
                    input.set_value(value.value, window, cx);
                });
            });
        })?;
        let component_attachment = attachment;
        let input_subscription = cx.subscribe_in(
            &state,
            window,
            move |_owner, state, event: &InputEvent, window, cx| match event {
                InputEvent::Change => {
                    let text = state.read(cx).value().to_string();
                    let Ok(mut value) = field.value(cx) else {
                        return;
                    };
                    value.value = text;
                    value.changed = true;
                    component_attachment.defer_set_user_value(value, window, cx);
                }
                InputEvent::Blur => component_attachment.defer_blur(window, cx),
                InputEvent::Focus | InputEvent::PressEnter { .. } => {}
            },
        );

        Ok(Self {
            subscriptions: vec![form_subscription, input_subscription],
            input: state,
            _marker: std::marker::PhantomData,
        })
    }
}

impl<Form> Deref for ProviderSecretInputState<Form>
where
    Form: FormStore,
{
    type Target = Entity<InputState>;

    fn deref(&self) -> &Self::Target {
        &self.input
    }
}

impl<Form> Drop for ProviderSecretInputState<Form>
where
    Form: FormStore,
{
    fn drop(&mut self) {
        self.subscriptions.clear();
    }
}

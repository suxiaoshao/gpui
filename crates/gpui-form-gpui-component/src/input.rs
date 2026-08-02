use std::ops::Deref;

use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_component::input::{InputEvent, InputState};
use gpui_form::{FieldAccessError, FormField, FormState, PartialFormField};

pub struct FormInput {
    subscriptions: Vec<Subscription>,
    state: Entity<InputState>,
}

impl FormInput {
    pub fn new<Form, Owner, Build>(
        form: &Entity<Form>,
        field: FormField<Form, String>,
        build: Build,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Self
    where
        Form: FormState,
        Owner: 'static,
        Build: FnOnce(&mut Window, &mut Context<InputState>) -> InputState,
    {
        let value = field.value(form, cx);
        let state = cx.new(|cx| build(window, cx));
        state.update(cx, |state, cx| state.set_value(value, window, cx));
        let binding = field.bind_control(form, cx);

        let weak_state = state.downgrade();
        let weak_form = form.downgrade();
        let projection = field.clone();
        let form_subscription = field.subscribe_in(form, window, cx, move |_, window, cx| {
            let weak_state = weak_state.clone();
            let weak_form = weak_form.clone();
            let projection = projection.clone();
            cx.defer_in(window, move |_, window, cx| {
                let (Some(state), Some(form)) = (weak_state.upgrade(), weak_form.upgrade()) else {
                    return;
                };
                let value = projection.value(&form, cx);
                state.update(cx, |state, cx| state.set_value(value, window, cx));
            });
        });

        let event_binding = binding.clone();
        let state_subscription = cx.subscribe_in(
            &state,
            window,
            move |_, state, event: &InputEvent, window, cx| match event {
                InputEvent::Change => {
                    event_binding.defer_set(state.read(cx).value().to_string(), window, cx);
                }
                InputEvent::Blur => event_binding.defer_blur(window, cx),
                InputEvent::Focus | InputEvent::PressEnter { .. } => {}
            },
        );

        Self {
            subscriptions: vec![form_subscription, state_subscription],
            state,
        }
    }

    pub fn try_new<Form, Owner, Build>(
        form: &Entity<Form>,
        field: PartialFormField<Form, String>,
        build: Build,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Result<Self, FieldAccessError>
    where
        Form: FormState,
        Owner: 'static,
        Build: FnOnce(&mut Window, &mut Context<InputState>) -> InputState,
    {
        let value = field.try_value(form, cx)?;
        let state = cx.new(|cx| build(window, cx));
        state.update(cx, |state, cx| state.set_value(value, window, cx));
        let binding = field.try_bind_control(form, cx)?;

        let weak_state = state.downgrade();
        let weak_form = form.downgrade();
        let projection = field.clone();
        let form_subscription =
            field.try_subscribe_in(form, window, cx, move |_, window, cx| {
                let weak_state = weak_state.clone();
                let weak_form = weak_form.clone();
                let projection = projection.clone();
                cx.defer_in(window, move |_, window, cx| {
                    let (Some(state), Some(form)) = (weak_state.upgrade(), weak_form.upgrade())
                    else {
                        return;
                    };
                    let Ok(value) = projection.try_value(&form, cx) else {
                        return;
                    };
                    state.update(cx, |state, cx| state.set_value(value, window, cx));
                });
            })?;

        let event_binding = binding.clone();
        let state_subscription = cx.subscribe_in(
            &state,
            window,
            move |_, state, event: &InputEvent, window, cx| match event {
                InputEvent::Change => {
                    event_binding.defer_set(state.read(cx).value().to_string(), window, cx);
                }
                InputEvent::Blur => event_binding.defer_blur(window, cx),
                InputEvent::Focus | InputEvent::PressEnter { .. } => {}
            },
        );

        Ok(Self {
            subscriptions: vec![form_subscription, state_subscription],
            state,
        })
    }
}

impl Deref for FormInput {
    type Target = Entity<InputState>;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl Drop for FormInput {
    fn drop(&mut self) {
        self.subscriptions.clear();
    }
}

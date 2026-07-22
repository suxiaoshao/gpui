use std::ops::Deref;

use gpui::{AppContext as _, Context, Entity, EventEmitter, Subscription, Window};
use gpui_component::input::{InputEvent, InputState};
use gpui_form::typed::{FormControl, FormField, FormStore};

use crate::FormControlError;

pub struct FormInput {
    subscriptions: Vec<Subscription>,
    input: Entity<InputState>,
}

impl FormInput {
    pub fn new<Form, Owner, Build>(
        field: FormField<Form, String>,
        build: Build,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Result<Self, FormControlError>
    where
        Form: FormStore + EventEmitter<gpui_form::typed::FormEvent<Form::Field>>,
        Owner: 'static,
        Build: FnOnce(&mut Window, &mut Context<InputState>) -> InputState,
    {
        <Self as FormControl<String>>::new(field, build, window, cx)
    }
}

impl Deref for FormInput {
    type Target = Entity<InputState>;

    fn deref(&self) -> &Self::Target {
        &self.input
    }
}

impl Drop for FormInput {
    fn drop(&mut self) {
        self.subscriptions.clear();
    }
}

impl FormControl<String> for FormInput {
    type State = InputState;
    type Error = FormControlError;

    fn new<Form, Owner, Build>(
        field: FormField<Form, String>,
        build: Build,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Result<Self, Self::Error>
    where
        Form: FormStore,
        Owner: 'static,
        Build: FnOnce(&mut Window, &mut Context<Self::State>) -> Self::State,
    {
        let value = field.value(cx)?;
        let attachment = field.attach_control(cx)?;
        let input = cx.new(|cx| build(window, cx));
        input.update(cx, |input, cx| input.set_value(value, window, cx));

        let weak_input = input.downgrade();
        let projection = field.clone();
        let form_subscription = field.subscribe_in(window, cx, move |_, window, cx| {
            let weak_input = weak_input.clone();
            let projection = projection.clone();
            cx.defer_in(window, move |_, window, cx| {
                let Some(input) = weak_input.upgrade() else {
                    return;
                };
                let Ok(value) = projection.value(cx) else {
                    return;
                };
                input.update(cx, |input, cx| input.set_value(value, window, cx));
            });
        })?;

        let input_attachment = attachment.clone();
        let input_subscription = cx.subscribe_in(
            &input,
            window,
            move |_, input, event: &InputEvent, window, cx| match event {
                InputEvent::Change => {
                    input_attachment.defer_set_user_value(
                        input.read(cx).value().to_string(),
                        window,
                        cx,
                    );
                }
                InputEvent::Blur => input_attachment.defer_blur(window, cx),
                InputEvent::Focus | InputEvent::PressEnter { .. } => {}
            },
        );

        Ok(Self {
            subscriptions: vec![form_subscription, input_subscription],
            input,
        })
    }
}

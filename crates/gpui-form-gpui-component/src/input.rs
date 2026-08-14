use std::ops::Deref;

use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_component::input::{InputEvent, InputState};
use gpui_form::{
    ControlBinding, ControlProjection, DynamicPath, Form, FormSchema, IntoTotalPath, ResolveError,
};

pub struct FormInput {
    #[allow(dead_code)]
    subscriptions: Vec<Subscription>,
    _binding: ControlBinding,
    state: Entity<InputState>,
}

impl FormInput {
    pub fn new<Root, Owner, Path, Build>(
        form: &Entity<Form<Root>>,
        path: Path,
        build: Build,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Self
    where
        Root: FormSchema,
        Owner: 'static,
        Path: IntoTotalPath<Root, String>,
        Build: FnOnce(&mut Window, &mut Context<InputState>) -> InputState,
    {
        let path = path.into_total_path();
        let value = path.get(form, cx);
        let state = cx.new(|cx| build(window, cx));
        state.update(cx, |state, cx| state.set_value(value, window, cx));
        let (binding, writer) = path.bind_control_in(
            form,
            &state,
            |state, projection, window, cx| match projection {
                ControlProjection::Value(value) => state.set_value(value, window, cx),
                ControlProjection::Retired => {}
            },
            window,
            cx,
        );
        let state_subscription = cx.subscribe_in(
            &state,
            window,
            move |_, state, event: &InputEvent, window, cx| match event {
                InputEvent::Change => {
                    writer.defer_set(state.read(cx).value().to_string(), window, cx);
                }
                InputEvent::Blur => writer.defer_blur(window, cx),
                InputEvent::Focus | InputEvent::PressEnter { .. } => {}
            },
        );
        Self {
            subscriptions: vec![state_subscription],
            _binding: binding,
            state,
        }
    }

    pub fn try_new<Root, Owner, Build>(
        form: &Entity<Form<Root>>,
        path: DynamicPath<Root, String>,
        build: Build,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Result<Self, ResolveError>
    where
        Root: FormSchema,
        Owner: 'static,
        Build: FnOnce(&mut Window, &mut Context<InputState>) -> InputState,
    {
        let value = path.try_get(form, cx)?;
        let state = cx.new(|cx| build(window, cx));
        state.update(cx, |state, cx| state.set_value(value, window, cx));
        let (binding, writer) = path.try_bind_control_in(
            form,
            &state,
            |state, projection, window, cx| match projection {
                ControlProjection::Value(value) => state.set_value(value, window, cx),
                ControlProjection::Retired => {}
            },
            window,
            cx,
        )?;
        let state_subscription = cx.subscribe_in(
            &state,
            window,
            move |_, state, event: &InputEvent, window, cx| match event {
                InputEvent::Change => {
                    writer.defer_set(state.read(cx).value().to_string(), window, cx);
                }
                InputEvent::Blur => writer.defer_blur(window, cx),
                InputEvent::Focus | InputEvent::PressEnter { .. } => {}
            },
        );
        Ok(Self {
            subscriptions: vec![state_subscription],
            _binding: binding,
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

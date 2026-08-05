use std::ops::Deref;

use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_component::input::{InputEvent, InputState};
use gpui_form::{
    ControlLease, DynamicPath, Form, FormEvent, FormSchema, IntoTotalPath, ResolveError, TotalPath,
};

pub struct FormInput {
    subscriptions: Vec<Subscription>,
    _lease: ControlLease,
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
        let value = path.value(form, cx);
        let state = cx.new(|cx| build(window, cx));
        state.update(cx, |state, cx| state.set_value(value, window, cx));
        let binding = path.bind_control(form, cx);
        let lease = binding.lease();
        let form_subscription = subscribe_total(form, path, &state, window, cx);
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
            _lease: lease,
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
        let value = path.try_value(form, cx)?;
        let state = cx.new(|cx| build(window, cx));
        state.update(cx, |state, cx| state.set_value(value, window, cx));
        let binding = path.try_bind_control(form, cx)?;
        let lease = binding.lease();
        let form_subscription = subscribe_dynamic(form, path, &state, window, cx);
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
            _lease: lease,
            state,
        })
    }
}

fn subscribe_total<Root: FormSchema, Owner: 'static>(
    form: &Entity<Form<Root>>,
    path: TotalPath<Root, String>,
    state: &Entity<InputState>,
    window: &Window,
    cx: &mut Context<Owner>,
) -> Subscription {
    let weak_form = form.downgrade();
    let weak_state = state.downgrade();
    cx.subscribe_in(form, window, move |_, _, event: &FormEvent, window, cx| {
        if matches!(event, FormEvent::ValidationChanged { .. }) {
            return;
        }
        let weak_form = weak_form.clone();
        let weak_state = weak_state.clone();
        let path = path.clone();
        cx.defer_in(window, move |_, window, cx| {
            let (Some(form), Some(state)) = (weak_form.upgrade(), weak_state.upgrade()) else {
                return;
            };
            let value = path.value(&form, cx);
            state.update(cx, |state, cx| state.set_value(value, window, cx));
        });
    })
}

fn subscribe_dynamic<Root: FormSchema, Owner: 'static>(
    form: &Entity<Form<Root>>,
    path: DynamicPath<Root, String>,
    state: &Entity<InputState>,
    window: &Window,
    cx: &mut Context<Owner>,
) -> Subscription {
    let weak_form = form.downgrade();
    let weak_state = state.downgrade();
    cx.subscribe_in(form, window, move |_, _, event: &FormEvent, window, cx| {
        if matches!(event, FormEvent::ValidationChanged { .. }) {
            return;
        }
        let weak_form = weak_form.clone();
        let weak_state = weak_state.clone();
        let path = path.clone();
        cx.defer_in(window, move |_, window, cx| {
            let (Some(form), Some(state)) = (weak_form.upgrade(), weak_state.upgrade()) else {
                return;
            };
            let Ok(value) = path.try_value(&form, cx) else {
                return;
            };
            state.update(cx, |state, cx| state.set_value(value, window, cx));
        });
    })
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

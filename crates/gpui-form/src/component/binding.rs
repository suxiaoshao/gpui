use std::rc::Rc;

use gpui::{App, AppContext as _, Context, Entity, Window};

use crate::{
    FieldChangeCause, FieldError, FieldPath, NoComponentState, SubscriptionSet, ValidationTrigger,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormComponentEvent {
    Change(FieldChangeCause),
    Focus,
    Blur,
}

type FormComponentEventCallback<Form> =
    dyn Fn(&mut Form, FormComponentEvent, &mut Window, &mut Context<Form>);

pub struct FormComponentEventSink<Form> {
    callback: Rc<FormComponentEventCallback<Form>>,
}

impl<Form> Clone for FormComponentEventSink<Form> {
    fn clone(&self) -> Self {
        Self {
            callback: self.callback.clone(),
        }
    }
}

impl<Form> FormComponentEventSink<Form> {
    pub fn new(
        callback: impl Fn(&mut Form, FormComponentEvent, &mut Window, &mut Context<Form>) + 'static,
    ) -> Self {
        Self {
            callback: Rc::new(callback),
        }
    }

    pub fn emit(
        &self,
        form: &mut Form,
        event: FormComponentEvent,
        window: &mut Window,
        cx: &mut Context<Form>,
    ) {
        (self.callback)(form, event, window, cx);
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ComponentStateOptions {
    pub label_key: Option<&'static str>,
    pub description_key: Option<&'static str>,
    pub placeholder_key: Option<&'static str>,
    pub masked: bool,
    pub disabled: bool,
    pub required: bool,
}

pub trait FormComponentBinding<Value>: Sized + 'static
where
    Value: Clone + PartialEq + 'static,
{
    type State: 'static;
    type Draft: Clone + PartialEq + 'static;

    fn new_state(
        initial: &Value,
        options: ComponentStateOptions,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self::State>;

    fn draft_from_value(value: &Value) -> Self::Draft;

    fn read_draft(state: &Entity<Self::State>, cx: &App) -> Self::Draft;

    fn parse_draft(
        draft: &Self::Draft,
        path: FieldPath,
        trigger: ValidationTrigger,
        cx: &App,
    ) -> Result<Value, Box<FieldError>>;

    fn write_value(
        state: &Entity<Self::State>,
        value: &Value,
        cause: FieldChangeCause,
        window: &mut Window,
        cx: &mut App,
    );

    fn set_disabled(
        _state: &Entity<Self::State>,
        _disabled: bool,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    fn set_required(
        _state: &Entity<Self::State>,
        _required: bool,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    fn focus(state: &Entity<Self::State>, window: &mut Window, cx: &mut App) -> bool;

    fn install_subscriptions<Form>(
        _state: Entity<Self::State>,
        _sink: FormComponentEventSink<Form>,
        _window: &mut Window,
        _cx: &mut Context<Form>,
    ) -> SubscriptionSet
    where
        Form: 'static,
    {
        SubscriptionSet::default()
    }
}

pub struct NoComponentBinding<Value>(std::marker::PhantomData<fn() -> Value>);

impl<Value> FormComponentBinding<Value> for NoComponentBinding<Value>
where
    Value: Clone + PartialEq + 'static,
{
    type State = NoComponentState;
    type Draft = Value;

    fn new_state(
        initial: &Value,
        _options: ComponentStateOptions,
        _window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self::State> {
        let _ = initial;
        cx.new(|_| NoComponentState)
    }

    fn draft_from_value(value: &Value) -> Self::Draft {
        value.clone()
    }

    fn read_draft(_state: &Entity<Self::State>, _cx: &App) -> Self::Draft {
        panic!("gpui-form NoComponentBinding state is not readable")
    }

    fn parse_draft(
        draft: &Self::Draft,
        _path: FieldPath,
        _trigger: ValidationTrigger,
        _cx: &App,
    ) -> Result<Value, Box<FieldError>> {
        Ok(draft.clone())
    }

    fn write_value(
        _state: &Entity<Self::State>,
        _value: &Value,
        _cause: FieldChangeCause,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    fn focus(_state: &Entity<Self::State>, _window: &mut Window, _cx: &mut App) -> bool {
        false
    }
}

use gpui::{App, Context, Entity, Window};

use crate::{FieldChangeCause, SubscriptionSet};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ComponentStateOptions {
    pub label_key: Option<&'static str>,
    pub description_key: Option<&'static str>,
    pub placeholder_key: Option<&'static str>,
    pub masked: bool,
    pub disabled: bool,
}

pub trait FormComponentBinding<Value>: Sized + 'static
where
    Value: Clone + PartialEq + 'static,
{
    type State: 'static;
    type Event: 'static;

    fn new_state(
        initial: &Value,
        options: ComponentStateOptions,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self::State>;

    fn read_value(state: &Entity<Self::State>, cx: &App) -> Value;

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

    fn focus(state: &Entity<Self::State>, window: &mut Window, cx: &mut App) -> bool;

    fn install_subscriptions<Form>(
        _state: Entity<Self::State>,
        _form: Entity<Form>,
        _window: &mut Window,
        _cx: &mut Context<Form>,
    ) -> SubscriptionSet
    where
        Form: 'static,
    {
        SubscriptionSet::default()
    }
}

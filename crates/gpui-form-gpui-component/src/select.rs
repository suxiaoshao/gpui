use std::marker::PhantomData;

use gpui::{App, AppContext as _, Context, Entity, Focusable, Window};
use gpui_component::{
    searchable_list::{SearchableListDelegate, SearchableListItem},
    select::{SelectEvent, SelectState},
};
use gpui_form::{
    ComponentStateOptions, FieldChangeCause, FieldError, FieldPath, FormComponentBinding,
    FormComponentEvent, FormComponentEventSink, SubscriptionSet, ValidationTrigger,
};

pub trait SelectFieldValue: Clone + PartialEq + 'static {
    type Selected: Clone + PartialEq + 'static;

    fn to_selected_value(&self) -> Option<Self::Selected>;
    fn from_selected_value(selected: Option<Self::Selected>, previous: &Self) -> Self;
}

impl<T> SelectFieldValue for Option<T>
where
    T: Clone + PartialEq + 'static,
{
    type Selected = T;

    fn to_selected_value(&self) -> Option<Self::Selected> {
        self.clone()
    }

    fn from_selected_value(selected: Option<Self::Selected>, _previous: &Self) -> Self {
        selected
    }
}

pub struct SelectBinding<T, D>(PhantomData<fn() -> (T, D)>);

impl<T, D> SelectBinding<T, D>
where
    T: SelectFieldValue,
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem<Value = T::Selected>,
{
    pub fn new_state_with_delegate(
        initial: &T,
        delegate: D,
        searchable: bool,
        _options: ComponentStateOptions,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<SelectState<D>> {
        let selected_index = initial
            .to_selected_value()
            .and_then(|value| delegate.position(&value));
        cx.new(|cx| SelectState::new(delegate, selected_index, window, cx).searchable(searchable))
    }

    pub fn set_items(
        state: &Entity<SelectState<D>>,
        delegate: D,
        window: &mut Window,
        cx: &mut App,
    ) {
        state.update(cx, |select, cx| {
            select.set_items(delegate, window, cx);
        });
    }

    pub fn focus(state: &Entity<SelectState<D>>, window: &mut Window, cx: &mut App) -> bool {
        let focus_handle = state.read(cx).focus_handle(cx);
        focus_handle.focus(window, cx);
        true
    }
}

impl<T, D> FormComponentBinding<T> for SelectBinding<T, D>
where
    T: SelectFieldValue + Default,
    D: SearchableListDelegate + Default + 'static,
    D::Item: SearchableListItem<Value = T::Selected>,
{
    type State = SelectState<D>;
    type Draft = T;

    fn new_state(
        initial: &T,
        options: ComponentStateOptions,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self::State> {
        Self::new_state_with_delegate(initial, D::default(), false, options, window, cx)
    }

    fn draft_from_value(value: &T) -> Self::Draft {
        value.clone()
    }

    fn read_draft(state: &Entity<Self::State>, cx: &App) -> Self::Draft {
        T::from_selected_value(state.read(cx).selected_value().cloned(), &T::default())
    }

    fn parse_draft(
        draft: &Self::Draft,
        _path: FieldPath,
        _trigger: ValidationTrigger,
        _cx: &App,
    ) -> Result<T, Box<FieldError>> {
        Ok(draft.clone())
    }

    fn write_value(
        state: &Entity<Self::State>,
        value: &T,
        _cause: FieldChangeCause,
        window: &mut Window,
        cx: &mut App,
    ) {
        let selected = value.to_selected_value();
        state.update(cx, |select, cx| {
            if let Some(value) = selected.as_ref() {
                select.set_selected_value(value, window, cx);
            } else {
                select.set_selected_index(None, window, cx);
            }
        });
    }

    fn focus(state: &Entity<Self::State>, window: &mut Window, cx: &mut App) -> bool {
        Self::focus(state, window, cx)
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
        let mut subscriptions = SubscriptionSet::new();
        subscriptions.push(cx.subscribe_in(
            &state,
            window,
            move |form, _state, event: &SelectEvent<D>, window, cx| {
                let SelectEvent::Confirm(_) = event;
                sink.emit(
                    form,
                    FormComponentEvent::Change(FieldChangeCause::UserInput),
                    window,
                    cx,
                );
            },
        ));
        subscriptions
    }
}

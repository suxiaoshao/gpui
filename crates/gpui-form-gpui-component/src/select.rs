use std::{cell::Cell, rc::Rc};

use crate::binding::ComponentBindError;
use gpui::{App, AppContext as _, Context, Entity, Focusable, Window};
use gpui_component::{
    searchable_list::{SearchableListDelegate, SearchableListItem},
    select::{SelectEvent, SelectState},
};
use gpui_form::{FormDraftEvent, FormFieldHandle, SubscriptionSet};

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

pub fn new_select_state<T, D>(
    initial: &T,
    delegate: D,
    searchable: bool,
    window: &mut Window,
    cx: &mut App,
) -> Entity<SelectState<D>>
where
    T: SelectFieldValue,
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem<Value = T::Selected>,
{
    let selected_index = initial
        .to_selected_value()
        .and_then(|value| delegate.position(&value));
    cx.new(|cx| SelectState::new(delegate, selected_index, window, cx).searchable(searchable))
}

pub fn focus_select<D>(state: &Entity<SelectState<D>>, window: &mut Window, cx: &mut App) -> bool
where
    D: SearchableListDelegate + 'static,
{
    let focus_handle = state.read(cx).focus_handle(cx);
    focus_handle.focus(window, cx);
    true
}

pub fn bind_select<Form, Value, D, Owner>(
    field: FormFieldHandle<Form, Value>,
    state: &Entity<SelectState<D>>,
    window: &mut Window,
    cx: &mut Context<Owner>,
) -> Result<SubscriptionSet, ComponentBindError>
where
    Form: gpui::EventEmitter<FormDraftEvent> + 'static,
    Value: SelectFieldValue + Default,
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem<Value = Value::Selected>,
    Owner: 'static,
{
    let initial = field.draft(cx).map_err(ComponentBindError::from)?;
    state.update(cx, |select, cx| {
        if let Some(value) = initial.to_selected_value().as_ref() {
            select.set_selected_value(value, window, cx);
        } else {
            select.set_selected_index(None, window, cx);
        }
    });

    let sync = Rc::new(Cell::new(false));
    let mut subscriptions = SubscriptionSet::new();
    let form_sync = sync.clone();
    let form_state = state.clone();
    subscriptions.push(
        field.subscribe_in(window, cx, move |_owner, event, window, cx| {
            if form_sync.get() {
                return;
            }
            form_sync.set(true);
            let value = event.draft.to_selected_value();
            form_state.update(cx, |select, cx| {
                if let Some(value) = value.as_ref() {
                    select.set_selected_value(value, window, cx);
                } else {
                    select.set_selected_index(None, window, cx);
                }
            });
            form_sync.set(false);
        })?,
    );

    let component_sync = sync;
    let component_field = field;
    subscriptions.push(cx.subscribe_in(
        state,
        window,
        move |_owner, state, event: &SelectEvent<D>, window, cx| {
            let SelectEvent::Confirm(_) = event;
            if component_sync.get() {
                return;
            }
            let Ok(previous) = component_field.draft(cx) else {
                return;
            };
            let selected = state.read(cx).selected_value().cloned();
            let next = Value::from_selected_value(selected, &previous);
            let sync = component_sync.clone();
            let field = component_field.clone();
            cx.defer_in(window, move |_owner, _window, cx| {
                if sync.get() {
                    return;
                }
                sync.set(true);
                let _ = field.set_user_draft(next, cx);
                sync.set(false);
            });
        },
    ));

    Ok(subscriptions)
}

pub fn set_select_items<Form, Value, D, Owner>(
    field: FormFieldHandle<Form, Value>,
    state: &Entity<SelectState<D>>,
    delegate: D,
    window: &mut Window,
    cx: &mut Context<Owner>,
) -> Result<(), ComponentBindError>
where
    Form: gpui::EventEmitter<FormDraftEvent> + 'static,
    Value: SelectFieldValue + Default,
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem<Value = Value::Selected>,
    Owner: 'static,
{
    state.update(cx, |select, cx| select.set_items(delegate, window, cx));
    let draft = field.draft(cx).map_err(ComponentBindError::from)?;
    state.update(cx, |select, cx| {
        if let Some(value) = draft.to_selected_value().as_ref() {
            select.set_selected_value(value, window, cx);
        } else {
            select.set_selected_index(None, window, cx);
        }
    });
    Ok(())
}

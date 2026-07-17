use std::{cell::Cell, rc::Rc};

use crate::binding::ComponentBindError;
use gpui::{App, AppContext as _, Context, Entity, Focusable, Window};
use gpui_component::{
    IndexPath,
    combobox::{ComboboxEvent, ComboboxState},
    searchable_list::{SearchableListDelegate, SearchableListItem},
};
use gpui_form::{FormDraftEvent, FormFieldHandle, SubscriptionSet};

pub trait ComboboxFieldValue: Clone + PartialEq + 'static {
    type Selected: Clone + PartialEq + 'static;

    fn to_selected_values(&self) -> Vec<Self::Selected>;
    fn from_selected_values(selected: Vec<Self::Selected>, previous: &Self) -> Self;
}

impl<T> ComboboxFieldValue for Vec<T>
where
    T: Clone + PartialEq + 'static,
{
    type Selected = T;

    fn to_selected_values(&self) -> Vec<Self::Selected> {
        self.clone()
    }

    fn from_selected_values(selected: Vec<Self::Selected>, _previous: &Self) -> Self {
        selected
    }
}

impl<T> ComboboxFieldValue for Option<T>
where
    T: Clone + PartialEq + 'static,
{
    type Selected = T;

    fn to_selected_values(&self) -> Vec<Self::Selected> {
        self.clone().into_iter().collect()
    }

    fn from_selected_values(mut selected: Vec<Self::Selected>, _previous: &Self) -> Self {
        selected.drain(..).next()
    }
}

pub fn new_combobox_state<T, D>(
    initial: &T,
    delegate: D,
    multiple: bool,
    searchable: bool,
    window: &mut Window,
    cx: &mut App,
) -> Entity<ComboboxState<D>>
where
    T: ComboboxFieldValue,
    D: SearchableListDelegate + Clone + 'static,
    D::Item: SearchableListItem<Value = T::Selected>,
{
    let selected_indices = selected_indices_for(&delegate, initial);
    cx.new(|cx| {
        ComboboxState::new(delegate, selected_indices, window, cx)
            .multiple(multiple)
            .searchable(searchable)
    })
}

pub fn focus_combobox<D>(
    state: &Entity<ComboboxState<D>>,
    window: &mut Window,
    cx: &mut App,
) -> bool
where
    D: SearchableListDelegate + 'static,
{
    let focus_handle = state.read(cx).focus_handle(cx);
    focus_handle.focus(window, cx);
    true
}

pub fn bind_combobox<Form, Value, D, Owner>(
    field: FormFieldHandle<Form, Value>,
    state: &Entity<ComboboxState<D>>,
    delegate: D,
    window: &mut Window,
    cx: &mut Context<Owner>,
) -> Result<SubscriptionSet, ComponentBindError>
where
    Form: gpui::EventEmitter<FormDraftEvent> + 'static,
    Value: ComboboxFieldValue + Default,
    D: SearchableListDelegate + Clone + 'static,
    D::Item: SearchableListItem<Value = Value::Selected>,
    Owner: 'static,
{
    let initial = field.draft(cx).map_err(ComponentBindError::from)?;
    let projection_delegate = delegate.clone();
    let sync = Rc::new(Cell::new(false));
    state.update(cx, |combobox, cx| {
        combobox.set_selected_indices(
            selected_indices_for(&projection_delegate, &initial),
            window,
            cx,
        );
    });

    let mut subscriptions = SubscriptionSet::new();
    let form_sync = sync.clone();
    let form_state = state.clone();
    let form_delegate = delegate.clone();
    subscriptions.push(
        field.subscribe_in(window, cx, move |_owner, event, window, cx| {
            if form_sync.get() {
                return;
            }
            form_sync.set(true);
            form_state.update(cx, |combobox, cx| {
                combobox.set_selected_indices(
                    selected_indices_for(&form_delegate, &event.draft),
                    window,
                    cx,
                );
            });
            form_sync.set(false);
        })?,
    );

    let component_sync = sync;
    let component_field = field;
    subscriptions.push(cx.subscribe_in(
        state,
        window,
        move |_owner, state, event: &ComboboxEvent<D>, window, cx| {
            if !matches!(event, ComboboxEvent::Change(_)) || component_sync.get() {
                return;
            }
            let Ok(previous) = component_field.draft(cx) else {
                return;
            };
            let next = Value::from_selected_values(state.read(cx).selected_values(), &previous);
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

pub fn set_combobox_items<Form, Value, D, Owner>(
    field: FormFieldHandle<Form, Value>,
    state: &Entity<ComboboxState<D>>,
    delegate: D,
    window: &mut Window,
    cx: &mut Context<Owner>,
) -> Result<(), ComponentBindError>
where
    Form: gpui::EventEmitter<FormDraftEvent> + 'static,
    Value: ComboboxFieldValue + Default,
    D: SearchableListDelegate + Clone + 'static,
    D::Item: SearchableListItem<Value = Value::Selected>,
    Owner: 'static,
{
    state.update(cx, |combobox, cx| {
        combobox.set_items(delegate.clone(), window, cx)
    });
    let draft = field.draft(cx).map_err(ComponentBindError::from)?;
    state.update(cx, |combobox, cx| {
        combobox.set_selected_indices(selected_indices_for(&delegate, &draft), window, cx);
    });
    Ok(())
}

fn selected_indices_for<D, Value>(delegate: &D, value: &Value) -> Vec<IndexPath>
where
    Value: ComboboxFieldValue,
    D: SearchableListDelegate,
    D::Item: SearchableListItem<Value = Value::Selected>,
{
    value
        .to_selected_values()
        .iter()
        .filter_map(|selected| delegate.position(selected))
        .collect()
}

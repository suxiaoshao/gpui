use std::ops::Deref;

use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_component::{
    combobox::{ComboboxEvent, ComboboxState},
    searchable_list::{SearchableListDelegate, SearchableListItem},
};
use gpui_form::{FieldAccessError, FormField, FormState, PartialFormField};

type ComboboxValue<D> = <<D as SearchableListDelegate>::Item as SearchableListItem>::Value;

pub struct FormCombobox<D>
where
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem,
{
    subscriptions: Vec<Subscription>,
    state: Entity<ComboboxState<D>>,
}

impl<D> FormCombobox<D>
where
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem,
{
    pub fn new<Form, Owner, Build>(
        form: &Entity<Form>,
        field: FormField<Form, Vec<ComboboxValue<D>>>,
        build: Build,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Self
    where
        Form: FormState,
        Owner: 'static,
        Build: FnOnce(&mut Window, &mut Context<ComboboxState<D>>) -> ComboboxState<D>,
    {
        let values = field.value(form, cx);
        let state = cx.new(|cx| build(window, cx));
        project_values(&state, &values, window, cx);
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
                let values = projection.value(&form, cx);
                project_values(&state, &values, window, cx);
            });
        });

        let event_binding = binding.clone();
        let state_subscription = cx.subscribe_in(
            &state,
            window,
            move |_, _, event: &ComboboxEvent<D>, window, cx| {
                if let ComboboxEvent::Change(values) = event {
                    event_binding.defer_set(values.clone(), window, cx);
                }
            },
        );

        Self {
            subscriptions: vec![form_subscription, state_subscription],
            state,
        }
    }

    pub fn try_new<Form, Owner, Build>(
        form: &Entity<Form>,
        field: PartialFormField<Form, Vec<ComboboxValue<D>>>,
        build: Build,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Result<Self, FieldAccessError>
    where
        Form: FormState,
        Owner: 'static,
        Build: FnOnce(&mut Window, &mut Context<ComboboxState<D>>) -> ComboboxState<D>,
    {
        let values = field.try_value(form, cx)?;
        let state = cx.new(|cx| build(window, cx));
        project_values(&state, &values, window, cx);
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
                    let Ok(values) = projection.try_value(&form, cx) else {
                        return;
                    };
                    project_values(&state, &values, window, cx);
                });
            })?;

        let event_binding = binding.clone();
        let state_subscription = cx.subscribe_in(
            &state,
            window,
            move |_, _, event: &ComboboxEvent<D>, window, cx| {
                if let ComboboxEvent::Change(values) = event {
                    event_binding.defer_set(values.clone(), window, cx);
                }
            },
        );

        Ok(Self {
            subscriptions: vec![form_subscription, state_subscription],
            state,
        })
    }
}

fn project_values<D>(
    state: &Entity<ComboboxState<D>>,
    values: &[ComboboxValue<D>],
    window: &mut Window,
    cx: &mut impl gpui::AppContext,
) where
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem,
{
    state.update(cx, |state, cx| {
        state.set_selected_values(values, window, cx)
    });
}

impl<D> Deref for FormCombobox<D>
where
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem,
{
    type Target = Entity<ComboboxState<D>>;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl<D> Drop for FormCombobox<D>
where
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem,
{
    fn drop(&mut self) {
        self.subscriptions.clear();
    }
}

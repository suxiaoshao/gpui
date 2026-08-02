use std::ops::Deref;

use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_component::{
    searchable_list::{SearchableListDelegate, SearchableListItem},
    select::{SelectEvent, SelectState},
};
use gpui_form::{FieldAccessError, FormField, FormState, PartialFormField};

type SelectValue<D> = <<D as SearchableListDelegate>::Item as SearchableListItem>::Value;

pub struct FormSelect<D>
where
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem,
{
    subscriptions: Vec<Subscription>,
    state: Entity<SelectState<D>>,
}

impl<D> FormSelect<D>
where
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem,
{
    pub fn new<Form, Owner, Build>(
        form: &Entity<Form>,
        field: FormField<Form, Option<SelectValue<D>>>,
        build: Build,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Self
    where
        Form: FormState,
        Owner: 'static,
        Build: FnOnce(&mut Window, &mut Context<SelectState<D>>) -> SelectState<D>,
    {
        let value = field.value(form, cx);
        let state = cx.new(|cx| build(window, cx));
        project_value(&state, &value, window, cx);
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
                project_value(&state, &value, window, cx);
            });
        });

        let event_binding = binding.clone();
        let state_subscription = cx.subscribe_in(
            &state,
            window,
            move |_, _, event: &SelectEvent<D>, window, cx| {
                let SelectEvent::Confirm(value) = event;
                event_binding.defer_set(value.clone(), window, cx);
            },
        );

        Self {
            subscriptions: vec![form_subscription, state_subscription],
            state,
        }
    }

    pub fn try_new<Form, Owner, Build>(
        form: &Entity<Form>,
        field: PartialFormField<Form, Option<SelectValue<D>>>,
        build: Build,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Result<Self, FieldAccessError>
    where
        Form: FormState,
        Owner: 'static,
        Build: FnOnce(&mut Window, &mut Context<SelectState<D>>) -> SelectState<D>,
    {
        let value = field.try_value(form, cx)?;
        let state = cx.new(|cx| build(window, cx));
        project_value(&state, &value, window, cx);
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
                    project_value(&state, &value, window, cx);
                });
            })?;

        let event_binding = binding.clone();
        let state_subscription = cx.subscribe_in(
            &state,
            window,
            move |_, _, event: &SelectEvent<D>, window, cx| {
                let SelectEvent::Confirm(value) = event;
                event_binding.defer_set(value.clone(), window, cx);
            },
        );

        Ok(Self {
            subscriptions: vec![form_subscription, state_subscription],
            state,
        })
    }
}

fn project_value<D>(
    state: &Entity<SelectState<D>>,
    value: &Option<SelectValue<D>>,
    window: &mut Window,
    cx: &mut impl gpui::AppContext,
) where
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem,
{
    state.update(cx, |state, cx| match value {
        Some(value) => state.set_selected_value(value, window, cx),
        None => state.set_selected_index(None, window, cx),
    });
}

impl<D> Deref for FormSelect<D>
where
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem,
{
    type Target = Entity<SelectState<D>>;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl<D> Drop for FormSelect<D>
where
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem,
{
    fn drop(&mut self) {
        self.subscriptions.clear();
    }
}

use std::ops::Deref;

use gpui::{AppContext as _, Context, Entity, EventEmitter, Subscription, Window};
use gpui_component::{
    searchable_list::{SearchableListDelegate, SearchableListItem},
    select::{SelectEvent, SelectState},
};
use gpui_form::typed::{FormControl, FormField, FormStore};

use crate::FormControlError;

type SelectValue<D> = <<D as SearchableListDelegate>::Item as SearchableListItem>::Value;

pub struct FormSelect<D>
where
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem,
{
    subscriptions: Vec<Subscription>,
    select: Entity<SelectState<D>>,
}

impl<D> FormSelect<D>
where
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem,
{
    pub fn new<Form, Owner, Build>(
        field: FormField<Form, Option<SelectValue<D>>>,
        build: Build,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Result<Self, FormControlError>
    where
        Form: FormStore + EventEmitter<gpui_form::typed::FormEvent<Form::Field>>,
        Owner: 'static,
        Build: FnOnce(&mut Window, &mut Context<SelectState<D>>) -> SelectState<D>,
    {
        <Self as FormControl<Option<SelectValue<D>>>>::new(field, build, window, cx)
    }
}

impl<D> Deref for FormSelect<D>
where
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem,
{
    type Target = Entity<SelectState<D>>;

    fn deref(&self) -> &Self::Target {
        &self.select
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

impl<D> FormControl<Option<SelectValue<D>>> for FormSelect<D>
where
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem,
{
    type State = SelectState<D>;
    type Error = FormControlError;

    fn new<Form, Owner, Build>(
        field: FormField<Form, Option<SelectValue<D>>>,
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
        let select = cx.new(|cx| build(window, cx));
        select.update(cx, |select, cx| match &value {
            Some(value) => select.set_selected_value(value, window, cx),
            None => select.set_selected_index(None, window, cx),
        });

        let weak_select = select.downgrade();
        let projection = field.clone();
        let form_subscription = field.subscribe_in(window, cx, move |_, window, cx| {
            let weak_select = weak_select.clone();
            let projection = projection.clone();
            cx.defer_in(window, move |_, window, cx| {
                let Some(select) = weak_select.upgrade() else {
                    return;
                };
                let Ok(value) = projection.value(cx) else {
                    return;
                };
                select.update(cx, |select, cx| match &value {
                    Some(value) => select.set_selected_value(value, window, cx),
                    None => select.set_selected_index(None, window, cx),
                });
            });
        })?;

        let select_attachment = attachment.clone();
        let select_subscription = cx.subscribe_in(
            &select,
            window,
            move |_, _, event: &SelectEvent<D>, window, cx| {
                let SelectEvent::Confirm(value) = event;
                select_attachment.defer_set_user_value(value.clone(), window, cx);
            },
        );

        Ok(Self {
            subscriptions: vec![form_subscription, select_subscription],
            select,
        })
    }
}

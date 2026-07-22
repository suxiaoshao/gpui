use std::ops::Deref;

use gpui::{AppContext as _, Context, Entity, EventEmitter, Subscription, Window};
use gpui_component::{
    combobox::{ComboboxEvent, ComboboxState},
    searchable_list::{SearchableListDelegate, SearchableListItem},
};
use gpui_form::typed::{FormControl, FormField, FormStore};

use crate::FormControlError;

type ComboboxValue<D> = <<D as SearchableListDelegate>::Item as SearchableListItem>::Value;

pub struct FormCombobox<D>
where
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem,
{
    subscriptions: Vec<Subscription>,
    combobox: Entity<ComboboxState<D>>,
}

impl<D> FormCombobox<D>
where
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem,
{
    pub fn new<Form, Owner, Build>(
        field: FormField<Form, Vec<ComboboxValue<D>>>,
        build: Build,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Result<Self, FormControlError>
    where
        Form: FormStore + EventEmitter<gpui_form::typed::FormEvent<Form::Field>>,
        Owner: 'static,
        Build: FnOnce(&mut Window, &mut Context<ComboboxState<D>>) -> ComboboxState<D>,
    {
        <Self as FormControl<Vec<ComboboxValue<D>>>>::new(field, build, window, cx)
    }
}

impl<D> Deref for FormCombobox<D>
where
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem,
{
    type Target = Entity<ComboboxState<D>>;

    fn deref(&self) -> &Self::Target {
        &self.combobox
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

impl<D> FormControl<Vec<ComboboxValue<D>>> for FormCombobox<D>
where
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem,
{
    type State = ComboboxState<D>;
    type Error = FormControlError;

    fn new<Form, Owner, Build>(
        field: FormField<Form, Vec<ComboboxValue<D>>>,
        build: Build,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Result<Self, Self::Error>
    where
        Form: FormStore,
        Owner: 'static,
        Build: FnOnce(&mut Window, &mut Context<Self::State>) -> Self::State,
    {
        let values = field.value(cx)?;
        let attachment = field.attach_control(cx)?;
        let combobox = cx.new(|cx| build(window, cx));
        combobox.update(cx, |combobox, cx| {
            combobox.set_selected_values(&values, window, cx)
        });

        let weak_combobox = combobox.downgrade();
        let projection = field.clone();
        let form_subscription = field.subscribe_in(window, cx, move |_, window, cx| {
            let weak_combobox = weak_combobox.clone();
            let projection = projection.clone();
            cx.defer_in(window, move |_, window, cx| {
                let Some(combobox) = weak_combobox.upgrade() else {
                    return;
                };
                let Ok(values) = projection.value(cx) else {
                    return;
                };
                combobox.update(cx, |combobox, cx| {
                    combobox.set_selected_values(&values, window, cx)
                });
            });
        })?;

        let combobox_attachment = attachment.clone();
        let combobox_subscription = cx.subscribe_in(
            &combobox,
            window,
            move |_, _, event: &ComboboxEvent<D>, window, cx| {
                if let ComboboxEvent::Change(values) = event {
                    combobox_attachment.defer_set_user_value(values.clone(), window, cx);
                }
            },
        );

        Ok(Self {
            subscriptions: vec![form_subscription, combobox_subscription],
            combobox,
        })
    }
}

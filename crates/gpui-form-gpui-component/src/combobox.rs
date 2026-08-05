use std::ops::Deref;

use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_component::{
    combobox::{ComboboxEvent, ComboboxState},
    searchable_list::{SearchableListDelegate, SearchableListItem},
};
use gpui_form::{
    ControlLease, DynamicPath, Form, FormEvent, FormSchema, IntoTotalPath, ResolveError, TotalPath,
};

type ComboboxValue<D> = <<D as SearchableListDelegate>::Item as SearchableListItem>::Value;

pub struct FormCombobox<D>
where
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem,
{
    subscriptions: Vec<Subscription>,
    _lease: ControlLease,
    state: Entity<ComboboxState<D>>,
}

impl<D> FormCombobox<D>
where
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem,
{
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
        Path: IntoTotalPath<Root, Vec<ComboboxValue<D>>>,
        ComboboxValue<D>: Clone + PartialEq + 'static,
        Build: FnOnce(&mut Window, &mut Context<ComboboxState<D>>) -> ComboboxState<D>,
    {
        let path = path.into_total_path();
        let values = path.value(form, cx);
        let state = cx.new(|cx| build(window, cx));
        sync_selected_values(&state, &values, window, cx);
        let binding = path.bind_control(form, cx);
        let lease = binding.lease();
        let subscription = subscribe_total(form, path, &state, window, cx);
        let event_binding = binding.clone();
        let event_subscription = cx.subscribe_in(
            &state,
            window,
            move |_, _, event: &ComboboxEvent<D>, window, cx| {
                if let ComboboxEvent::Change(values) = event {
                    event_binding.defer_set(values.clone(), window, cx);
                }
            },
        );
        Self {
            subscriptions: vec![subscription, event_subscription],
            _lease: lease,
            state,
        }
    }

    pub fn try_new<Root, Owner, Build>(
        form: &Entity<Form<Root>>,
        path: DynamicPath<Root, Vec<ComboboxValue<D>>>,
        build: Build,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Result<Self, ResolveError>
    where
        Root: FormSchema,
        Owner: 'static,
        ComboboxValue<D>: Clone + PartialEq + 'static,
        Build: FnOnce(&mut Window, &mut Context<ComboboxState<D>>) -> ComboboxState<D>,
    {
        let values = path.try_value(form, cx)?;
        let state = cx.new(|cx| build(window, cx));
        sync_selected_values(&state, &values, window, cx);
        let binding = path.try_bind_control(form, cx)?;
        let lease = binding.lease();
        let subscription = subscribe_dynamic(form, path, &state, window, cx);
        let event_binding = binding.clone();
        let event_subscription = cx.subscribe_in(
            &state,
            window,
            move |_, _, event: &ComboboxEvent<D>, window, cx| {
                if let ComboboxEvent::Change(values) = event {
                    event_binding.defer_set(values.clone(), window, cx);
                }
            },
        );
        Ok(Self {
            subscriptions: vec![subscription, event_subscription],
            _lease: lease,
            state,
        })
    }
}

fn subscribe_total<Root, Owner, D>(
    form: &Entity<Form<Root>>,
    path: TotalPath<Root, Vec<ComboboxValue<D>>>,
    state: &Entity<ComboboxState<D>>,
    window: &Window,
    cx: &mut Context<Owner>,
) -> Subscription
where
    Root: FormSchema,
    Owner: 'static,
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem,
    ComboboxValue<D>: Clone + PartialEq + 'static,
{
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
            sync_selected_values(&state, &path.value(&form, cx), window, cx);
        });
    })
}

fn subscribe_dynamic<Root, Owner, D>(
    form: &Entity<Form<Root>>,
    path: DynamicPath<Root, Vec<ComboboxValue<D>>>,
    state: &Entity<ComboboxState<D>>,
    window: &Window,
    cx: &mut Context<Owner>,
) -> Subscription
where
    Root: FormSchema,
    Owner: 'static,
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem,
    ComboboxValue<D>: Clone + PartialEq + 'static,
{
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
            let Ok(values) = path.try_value(&form, cx) else {
                return;
            };
            sync_selected_values(&state, &values, window, cx);
        });
    })
}

fn sync_selected_values<D>(
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

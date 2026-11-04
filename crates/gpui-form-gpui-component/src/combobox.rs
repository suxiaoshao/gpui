use std::ops::Deref;

use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_component::{
    combobox::{ComboboxEvent, ComboboxState},
    searchable_list::{SearchableListDelegate, SearchableListItem},
};
use gpui_form::{ControlProjection, DynamicPath, Form, FormSchema, IntoTotalPath, ResolveError};

type ComboboxValue<D> = <<D as SearchableListDelegate>::Item as SearchableListItem>::Value;

pub struct FormCombobox<D>
where
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem,
{
    #[allow(dead_code)]
    subscriptions: Vec<Subscription>,
    editor: crate::editor::EditorAccess,
    state: Entity<ComboboxState<D>>,
}

impl<D> FormCombobox<D>
where
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem,
{
    /// Explicitly discard this source editor's accepted draft and queued input.
    pub fn discard_draft(
        &self,
        cx: &mut gpui::App,
    ) -> Result<gpui_form::LifecycleReport, gpui_form::LifecycleError> {
        self.editor.discard_draft(cx)
    }
    pub fn editor_status(
        &self,
        cx: &gpui::App,
    ) -> Result<gpui_form::EditorStatus, gpui_form::LifecycleError> {
        self.editor.editor_status(cx)
    }

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
        let values = path.get(form, cx);
        let state = cx.new(|cx| build(window, cx));
        sync_selected_values(&state, &values, window, cx);
        let (binding, writer) = path.bind_control_in(
            form,
            &state,
            |state, projection, window, cx| match projection {
                ControlProjection::Value(values) => {
                    sync_selected_values_state(state, &values, window, cx)
                }
                ControlProjection::Retired => {}
            },
            window,
            cx,
        );
        let editor = crate::editor::EditorAccess::new(binding, &writer);
        let event_subscription = cx.subscribe_in(
            &state,
            window,
            move |_, _, event: &ComboboxEvent<D>, window, cx| {
                if let ComboboxEvent::Change(values) = event {
                    writer.defer_set(values.clone(), window, cx);
                }
            },
        );
        Self {
            subscriptions: vec![event_subscription],
            editor,
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
        let values = path.try_get(form, cx)?;
        let state = cx.new(|cx| build(window, cx));
        sync_selected_values(&state, &values, window, cx);
        let (binding, writer) = path.try_bind_control_in(
            form,
            &state,
            |state, projection, window, cx| match projection {
                ControlProjection::Value(values) => {
                    sync_selected_values_state(state, &values, window, cx)
                }
                ControlProjection::Retired => {}
            },
            window,
            cx,
        )?;
        let editor = crate::editor::EditorAccess::new(binding, &writer);
        let event_subscription = cx.subscribe_in(
            &state,
            window,
            move |_, _, event: &ComboboxEvent<D>, window, cx| {
                if let ComboboxEvent::Change(values) = event {
                    writer.defer_set(values.clone(), window, cx);
                }
            },
        );
        Ok(Self {
            subscriptions: vec![event_subscription],
            editor,
            state,
        })
    }
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
        sync_selected_values_state(state, values, window, cx)
    });
}

fn sync_selected_values_state<D>(
    state: &mut ComboboxState<D>,
    values: &[ComboboxValue<D>],
    window: &mut Window,
    cx: &mut Context<ComboboxState<D>>,
) where
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem,
{
    state.set_selected_values(values, window, cx);
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

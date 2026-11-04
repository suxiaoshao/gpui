use std::ops::Deref;

use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_component::{
    searchable_list::{SearchableListDelegate, SearchableListItem},
    select::{SelectEvent, SelectState},
};
use gpui_form::{
    ControlBinding, ControlProjection, DynamicPath, Form, FormSchema, IntoTotalPath, ResolveError,
};

type SelectValue<D> = <<D as SearchableListDelegate>::Item as SearchableListItem>::Value;

pub struct FormSelect<D>
where
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem,
{
    #[allow(dead_code)]
    subscriptions: Vec<Subscription>,
    _binding: ControlBinding,
    editor: crate::editor::EditorAccess,
    state: Entity<SelectState<D>>,
}

impl<D> FormSelect<D>
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
        Path: IntoTotalPath<Root, Option<SelectValue<D>>>,
        SelectValue<D>: Clone + PartialEq + 'static,
        Build: FnOnce(&mut Window, &mut Context<SelectState<D>>) -> SelectState<D>,
    {
        let path = path.into_total_path();
        let value = path.get(form, cx);
        let state = cx.new(|cx| build(window, cx));
        sync_selected_value(&state, &value, window, cx);
        let (binding, writer) = path.bind_control_in(
            form,
            &state,
            |state, projection, window, cx| match projection {
                ControlProjection::Value(value) => {
                    sync_selected_value_state(state, &value, window, cx)
                }
                ControlProjection::Retired => {}
            },
            window,
            cx,
        );
        let editor = crate::editor::EditorAccess::new(&writer);
        let event_subscription = cx.subscribe_in(
            &state,
            window,
            move |_, _, event: &SelectEvent<D>, window, cx| {
                let SelectEvent::Confirm(value) = event;
                writer.defer_set(value.clone(), window, cx);
            },
        );
        Self {
            subscriptions: vec![event_subscription],
            _binding: binding,
            editor,
            state,
        }
    }

    pub fn try_new<Root, Owner, Build>(
        form: &Entity<Form<Root>>,
        path: DynamicPath<Root, Option<SelectValue<D>>>,
        build: Build,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Result<Self, ResolveError>
    where
        Root: FormSchema,
        Owner: 'static,
        SelectValue<D>: Clone + PartialEq + 'static,
        Build: FnOnce(&mut Window, &mut Context<SelectState<D>>) -> SelectState<D>,
    {
        let value = path.try_get(form, cx)?;
        let state = cx.new(|cx| build(window, cx));
        sync_selected_value(&state, &value, window, cx);
        let (binding, writer) = path.try_bind_control_in(
            form,
            &state,
            |state, projection, window, cx| match projection {
                ControlProjection::Value(value) => {
                    sync_selected_value_state(state, &value, window, cx)
                }
                ControlProjection::Retired => {}
            },
            window,
            cx,
        )?;
        let editor = crate::editor::EditorAccess::new(&writer);
        let event_subscription = cx.subscribe_in(
            &state,
            window,
            move |_, _, event: &SelectEvent<D>, window, cx| {
                let SelectEvent::Confirm(value) = event;
                writer.defer_set(value.clone(), window, cx);
            },
        );
        Ok(Self {
            subscriptions: vec![event_subscription],
            _binding: binding,
            editor,
            state,
        })
    }
}

fn sync_selected_value<D>(
    state: &Entity<SelectState<D>>,
    value: &Option<SelectValue<D>>,
    window: &mut Window,
    cx: &mut impl gpui::AppContext,
) where
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem,
{
    state.update(cx, |state, cx| {
        sync_selected_value_state(state, value, window, cx)
    });
}

fn sync_selected_value_state<D>(
    state: &mut SelectState<D>,
    value: &Option<SelectValue<D>>,
    window: &mut Window,
    cx: &mut Context<SelectState<D>>,
) where
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem,
{
    match value {
        Some(value) => state.set_selected_value(value, window, cx),
        None => state.set_selected_index(None, window, cx),
    }
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

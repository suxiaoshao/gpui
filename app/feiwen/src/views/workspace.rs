use gpui::*;
use gpui_component::ActiveTheme;

use super::{fetch::FetchView, query::QueryView};

#[derive(Default, Clone, Copy)]
pub(crate) enum RouterType {
    Fetch,
    #[default]
    Query,
}

pub(crate) enum WorkspaceEvent {
    UpdateRouter(RouterType),
}

#[derive(Default)]
pub(crate) struct Workspace {
    router: RouterType,
}

impl EventEmitter<WorkspaceEvent> for Workspace {}

pub(crate) struct WorkspaceView {
    workspace: Entity<Workspace>,
    focus_handle: FocusHandle,
    fetch_view: Entity<FetchView>,
    query_view: Entity<QueryView>,
    _subscriptions: Vec<Subscription>,
}

impl WorkspaceView {
    pub(crate) fn new(window: &mut Window, workspace_cx: &mut Context<Self>) -> Self {
        let workspace = workspace_cx.new(|_cx| Default::default());
        let _subscriptions = vec![workspace_cx.subscribe(&workspace, Self::subscribe)];
        Self {
            focus_handle: workspace_cx.focus_handle(),
            fetch_view: workspace_cx.new(|cx| FetchView::new(window, workspace.clone(), cx)),
            query_view: workspace_cx.new(|cx| QueryView::new(workspace.clone(), cx)),
            workspace,
            _subscriptions,
        }
    }
    fn child_view(&self, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        match self.workspace.read(cx).router {
            RouterType::Fetch => self.fetch_view.clone().into_any_element(),
            RouterType::Query => self.query_view.clone().into_any_element(),
        }
    }
    fn subscribe(
        &mut self,
        subscriber: Entity<Workspace>,
        emitter: &WorkspaceEvent,
        cx: &mut Context<Self>,
    ) {
        match emitter {
            WorkspaceEvent::UpdateRouter(router) => {
                subscriber.update(cx, |data, _| {
                    data.router = *router;
                });
            }
        }
    }
}

impl Render for WorkspaceView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .track_focus(&self.focus_handle)
            .flex()
            .flex_col()
            .bg(theme.background)
            .size_full()
            .child(self.child_view(cx))
    }
}

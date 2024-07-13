use gpui::*;

use super::{fetch::FetchView, query::QueryView};

#[derive(Default, Clone, Copy)]
pub enum RouterType {
    Fetch,
    #[default]
    Query,
}

pub enum WorkspaceEvent {
    UpdateRouter(RouterType),
}

#[derive(Default)]
pub struct Workspace {
    router: RouterType,
}

impl EventEmitter<WorkspaceEvent> for Workspace {}

pub struct WorkspaceView {
    workspace: Model<Workspace>,
    focus_handle: FocusHandle,
    fetch_view: View<FetchView>,
    query_view: View<QueryView>,
}

impl WorkspaceView {
    pub fn new(workspace_cx: &mut ViewContext<Self>) -> Self {
        let workspace = workspace_cx.new_model(|_cx| Default::default());
        workspace_cx.subscribe(&workspace, Self::subscribe).detach();
        Self {
            focus_handle: workspace_cx.focus_handle(),
            fetch_view: workspace_cx.new_view(|cx| FetchView::new(workspace.clone(), cx)),
            query_view: workspace_cx.new_view(|cx| QueryView::new(workspace.clone(), cx)),
            workspace,
        }
    }
    fn child_view(&self, cx: &mut ViewContext<Self>) -> impl gpui::IntoElement {
        match self.workspace.read(cx).router {
            RouterType::Fetch => self.fetch_view.clone().into_any(),
            RouterType::Query => self.query_view.clone().into_any(),
        }
    }
    fn subscribe(
        &mut self,
        subscriber: Model<Workspace>,
        emitter: &WorkspaceEvent,
        cx: &mut ViewContext<Self>,
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
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let theme = cx.global::<theme::Theme>();
        div()
            .track_focus(&self.focus_handle)
            .flex()
            .flex_col()
            .bg(theme.bg_color())
            .size_full()
            .shadow_lg()
            .border_1()
            .text_color(theme.text_color())
            .child(self.child_view(cx))
    }
}

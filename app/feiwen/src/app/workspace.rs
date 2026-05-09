use gpui::*;
use gpui_component::v_flex;
use tracing::{Level, event};

use super::titlebar::{FeiwenTitleBar, route_title, window_title};
use crate::{
    features::{
        fetch::{FetchTaskState, FetchView},
        query::QueryView,
    },
    foundation::I18n,
};

#[derive(Default, Clone, Copy, PartialEq, Eq)]
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
    pub(super) router: RouterType,
}

impl EventEmitter<WorkspaceEvent> for Workspace {}

pub(crate) struct WorkspaceView {
    workspace: Entity<Workspace>,
    focus_handle: FocusHandle,
    _fetch_task: Entity<FetchTaskState>,
    fetch_view: Entity<FetchView>,
    query_view: Entity<QueryView>,
    _subscriptions: Vec<Subscription>,
}

impl WorkspaceView {
    pub(crate) fn new(window: &mut Window, workspace_cx: &mut Context<Self>) -> Self {
        event!(Level::INFO, "creating feiwen workspace");
        let workspace = workspace_cx.new(|_cx| Default::default());
        let fetch_task = workspace_cx.new(|_cx| FetchTaskState::default());
        let fetch_view = workspace_cx.new(|cx| FetchView::new(window, fetch_task.clone(), cx));
        let query_view = workspace_cx
            .new(|cx| QueryView::new(workspace.clone(), fetch_task.clone(), window, cx));
        apply_current_theme(window, workspace_cx);
        let _subscriptions = vec![
            workspace_cx.subscribe(&workspace, Self::subscribe),
            workspace_cx.observe(&query_view, |_, _, cx| {
                cx.notify();
            }),
            workspace_cx.observe(&fetch_task, |_, _, cx| {
                cx.notify();
            }),
            workspace_cx.observe_window_appearance(window, |_state, window, cx| {
                apply_current_theme(window, cx);
                cx.refresh_windows();
            }),
            workspace_cx.observe_global_in::<app_theme::SystemAccentThemeState>(
                window,
                |_state, window, cx| {
                    apply_current_theme(window, cx);
                    cx.refresh_windows();
                },
            ),
        ];
        let this = Self {
            focus_handle: workspace_cx.focus_handle(),
            fetch_view,
            query_view,
            _fetch_task: fetch_task,
            workspace,
            _subscriptions,
        };
        event!(Level::INFO, "feiwen workspace created");
        this
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
                    event!(
                        Level::INFO,
                        from = %data.router.label(),
                        to = %router.label(),
                        "switching feiwen route"
                    );
                    data.router = *router;
                });
            }
        }
    }
}

impl RouterType {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Fetch => "fetch",
            Self::Query => "query",
        }
    }
}

fn apply_current_theme(window: &mut Window, cx: &mut App) {
    app_theme::apply_fixed_system_accent_theme(window, cx);
}

impl Render for WorkspaceView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app_title = cx.global::<I18n>().t("app-title");
        let router = self.workspace.read(cx).router;
        let titlebar_title = route_title(router, cx.global::<I18n>());
        window.set_window_title(&window_title(&titlebar_title, &app_title));

        v_flex()
            .track_focus(&self.focus_handle)
            .size_full()
            .overflow_hidden()
            .child(div().flex_initial().child(FeiwenTitleBar::new(
                app_title,
                router,
                self.workspace.clone(),
                self.query_view.clone(),
                self.fetch_view.clone(),
            )))
            .child(div().flex_1().min_h_0().child(self.child_view(cx)))
    }
}

#[cfg(test)]
mod tests {
    use super::RouterType;

    #[test]
    fn router_type_labels_are_stable_for_titlebar_switcher() {
        assert_eq!(RouterType::Query.label(), "query");
        assert_eq!(RouterType::Fetch.label(), "fetch");
    }
}

use feiwen_fetch::FetchView;
use feiwen_query::QueryView;
use gpui_kit::component::v_flex;
use gpui_kit::*;
use tracing::{Level, event};

use super::resource::{DatabaseResourcePage, notify_backup_completed};
use super::titlebar::{FeiwenTitleBar, route_title, window_title};
use crate::foundation::I18n;

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RouterType {
    Fetch,
    #[default]
    Query,
}

#[derive(Default)]
pub(crate) struct Workspace {
    pub(super) router: RouterType,
}

pub(crate) struct WorkspaceView {
    workspace: Entity<Workspace>,
    focus_handle: FocusHandle,
    fetch_view: Entity<FetchView>,
    query_view: Entity<QueryView>,
    _subscriptions: Vec<Subscription>,
}

impl WorkspaceView {
    pub(crate) fn new(window: &mut Window, workspace_cx: &mut Context<Self>) -> Self {
        event!(Level::INFO, "creating feiwen workspace");
        let workspace = workspace_cx.new(|_cx| Default::default());
        let fetch_view = workspace_cx.new(|cx| FetchView::new(window, cx));
        let query_view = workspace_cx.new(|cx| QueryView::new(window, cx));
        apply_current_theme(window, workspace_cx);
        let _subscriptions = vec![
            workspace_cx.observe(&query_view, |_, _, cx| {
                cx.notify();
            }),
            workspace_cx.observe(&fetch_view, |this, view, cx| {
                let summary = view.read(cx).summary(cx);
                this.query_view
                    .update(cx, |query, cx| query.set_fetch_summary(summary, cx));
                cx.notify();
            }),
            workspace_cx.subscribe(&query_view, |this, _, event, cx| match event {
                feiwen_query::QueryEvent::OpenFetch => {
                    this.workspace.update(cx, |workspace, cx| {
                        workspace.router = RouterType::Fetch;
                        cx.notify();
                    });
                    cx.notify();
                }
            }),
            feiwen_data::database::store(workspace_cx)
                .observe(workspace_cx, |_, _, cx| cx.notify()),
            feiwen_data::database::store(workspace_cx).observe_select_in(
                workspace_cx,
                window,
                |resource: &feiwen_data::database::DatabaseResource| resource.completed_backup(),
                |_, backup, window, cx| {
                    if let Some(backup) = backup {
                        notify_backup_completed(backup, window, cx);
                    }
                },
            ),
            feiwen_data::catalog::store(workspace_cx).observe(workspace_cx, |_, _, cx| cx.notify()),
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
            workspace,
            _subscriptions,
        };
        event!(Level::INFO, "feiwen workspace created");
        this
    }
    fn child_view(&self, cx: &mut Context<Self>) -> AnyElement {
        if feiwen_data::database::phase(cx) != feiwen_data::database::DatabasePhase::Ready {
            return DatabaseResourcePage::new().into_any_element();
        }
        match self.workspace.read(cx).router {
            RouterType::Fetch => self.fetch_view.clone().into_any_element(),
            RouterType::Query => self.query_view.clone().into_any_element(),
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

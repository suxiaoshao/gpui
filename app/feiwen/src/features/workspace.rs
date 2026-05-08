use gpui::*;
use gpui_component::{
    Sizable, StyledExt, TitleBar,
    button::{Toggle, ToggleGroup, ToggleVariants},
    h_flex,
    label::Label,
    v_flex,
};
use tracing::{Level, event};

use super::{
    fetch::{FetchTaskState, FetchView},
    query::QueryView,
};
use crate::foundation::I18n;

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
    router: RouterType,
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
        apply_current_theme(window, workspace_cx);
        let _subscriptions = vec![
            workspace_cx.subscribe(&workspace, Self::subscribe),
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
            fetch_view: workspace_cx.new(|cx| FetchView::new(window, fetch_task.clone(), cx)),
            query_view: workspace_cx
                .new(|cx| QueryView::new(workspace.clone(), fetch_task.clone(), window, cx)),
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
    fn label(self) -> &'static str {
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
        window.set_window_title(&app_title);

        v_flex()
            .track_focus(&self.focus_handle)
            .size_full()
            .overflow_hidden()
            .child(
                div()
                    .flex_initial()
                    .child(TitleBar::new().child(title_bar_content(
                        self.workspace.clone(),
                        app_title,
                        cx,
                    ))),
            )
            .child(div().flex_1().min_h_0().child(self.child_view(cx)))
    }
}

fn title_bar_content(
    workspace: Entity<Workspace>,
    app_title: impl Into<SharedString>,
    cx: &mut Context<WorkspaceView>,
) -> impl IntoElement {
    let router = workspace.read(cx).router;
    h_flex()
        .w_full()
        .h_full()
        .min_w_0()
        .overflow_hidden()
        .gap_2()
        .child(title_bar_title(app_title))
        .child(route_switcher(workspace, router, cx))
}

fn title_bar_title(title: impl Into<SharedString>) -> impl IntoElement {
    h_flex()
        .flex_1()
        .min_w_0()
        .h_full()
        .items_center()
        .overflow_hidden()
        .child(Label::new(title).text_sm().font_medium().truncate())
}

fn route_switcher(
    workspace: Entity<Workspace>,
    current: RouterType,
    cx: &mut Context<WorkspaceView>,
) -> impl IntoElement {
    let (query_label, fetch_label) = {
        let i18n = cx.global::<I18n>();
        (
            i18n.t("titlebar-route-query"),
            i18n.t("titlebar-route-fetch"),
        )
    };

    ToggleGroup::new("feiwen-titlebar-route")
        .segmented()
        .outline()
        .xsmall()
        .child(
            Toggle::new("feiwen-titlebar-route-query")
                .label(query_label)
                .checked(matches!(current, RouterType::Query)),
        )
        .child(
            Toggle::new("feiwen-titlebar-route-fetch")
                .label(fetch_label)
                .checked(matches!(current, RouterType::Fetch)),
        )
        .on_click(cx.listener(move |_this, checked: &Vec<bool>, _, cx| {
            let next = if checked.first().copied().unwrap_or(false) {
                RouterType::Query
            } else if checked.get(1).copied().unwrap_or(false) {
                RouterType::Fetch
            } else {
                current
            };

            workspace.update(cx, |workspace, cx| {
                if workspace.router != next {
                    event!(
                        Level::INFO,
                        from = %workspace.router.label(),
                        to = %next.label(),
                        "switching feiwen route from titlebar"
                    );
                    workspace.router = next;
                    cx.notify();
                }
            });
            cx.notify();
        }))
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

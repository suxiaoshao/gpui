use gpui::{
    App, Entity, InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels, RenderOnce,
    SharedString, Styled, Window, div, point, px,
};
use gpui_component::{
    ActiveTheme, Disableable, Sizable, StyledExt, TitleBar,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    tab::{Tab, TabBar},
};
use tracing::{Level, event};

use super::workspace::{RouterType, Workspace};
use crate::{
    features::{fetch::FetchView, query::QueryView},
    foundation::{I18n, IconName as FeiwenIconName},
};

pub(crate) const FEIWEN_TITLE_BAR_HEIGHT: Pixels = px(44.);
pub(crate) const FEIWEN_TRAFFIC_LIGHT_INSET: Pixels = px(14.);

#[cfg(target_os = "macos")]
const FEIWEN_TITLE_BAR_LEFT_PADDING: Pixels = px(86.);
#[cfg(not(target_os = "macos"))]
const FEIWEN_TITLE_BAR_LEFT_PADDING: Pixels = px(12.);

pub(crate) fn traffic_light_position() -> gpui::Point<Pixels> {
    point(FEIWEN_TRAFFIC_LIGHT_INSET, FEIWEN_TRAFFIC_LIGHT_INSET)
}

pub(crate) fn route_title(router: RouterType, i18n: &I18n) -> SharedString {
    match router {
        RouterType::Query => i18n.t("titlebar-route-query").into(),
        RouterType::Fetch => i18n.t("titlebar-route-fetch").into(),
    }
}

pub(crate) fn window_title(route_title: &SharedString, app_title: &str) -> String {
    if route_title.as_ref() == app_title {
        app_title.to_owned()
    } else {
        format!("{route_title} - {app_title}")
    }
}

fn route_tab_index(router: RouterType) -> usize {
    match router {
        RouterType::Query => 0,
        RouterType::Fetch => 1,
    }
}

fn route_from_tab_index(index: usize) -> Option<RouterType> {
    match index {
        0 => Some(RouterType::Query),
        1 => Some(RouterType::Fetch),
        _ => None,
    }
}

#[derive(IntoElement)]
pub(crate) struct FeiwenTitleBar {
    app_title: SharedString,
    router: RouterType,
    workspace: Entity<Workspace>,
    query_view: Entity<QueryView>,
    fetch_view: Entity<FetchView>,
}

impl FeiwenTitleBar {
    pub(crate) fn new(
        app_title: impl Into<SharedString>,
        router: RouterType,
        workspace: Entity<Workspace>,
        query_view: Entity<QueryView>,
        fetch_view: Entity<FetchView>,
    ) -> Self {
        Self {
            app_title: app_title.into(),
            router,
            workspace,
            query_view,
            fetch_view,
        }
    }

    fn summary(&self, cx: &App) -> String {
        let i18n = cx.global::<I18n>();
        match self.router {
            RouterType::Query => self.query_view.read(cx).titlebar_summary(i18n),
            RouterType::Fetch => self.fetch_view.read(cx).titlebar_summary(i18n, cx),
        }
    }

    fn actions(&self, cx: &mut App) -> impl IntoElement {
        match self.router {
            RouterType::Query => self.query_actions(cx).into_any_element(),
            RouterType::Fetch => self.fetch_actions(cx).into_any_element(),
        }
    }

    fn query_actions(&self, cx: &mut App) -> impl IntoElement {
        let i18n = cx.global::<I18n>();
        let searching = self.query_view.read(cx).is_searching();
        h_flex()
            .gap_2()
            .child(
                Button::new("titlebar-query-reset")
                    .icon(FeiwenIconName::RotateCcw)
                    .label(i18n.t("query-reset-button"))
                    .small()
                    .disabled(searching)
                    .on_click({
                        let query_view = self.query_view.clone();
                        move |_, _, cx| {
                            query_view.update(cx, |view, cx| view.request_reset(cx));
                        }
                    }),
            )
            .child(
                Button::new("titlebar-query-search")
                    .primary()
                    .icon(FeiwenIconName::Search)
                    .label(i18n.t("query-search-button"))
                    .small()
                    .loading(searching)
                    .disabled(searching)
                    .on_click({
                        let query_view = self.query_view.clone();
                        move |_, _, cx| {
                            query_view.update(cx, |view, cx| view.request_search(cx));
                        }
                    }),
            )
    }

    fn fetch_actions(&self, cx: &mut App) -> impl IntoElement {
        let i18n = cx.global::<I18n>();
        let running = self.fetch_view.read(cx).is_running(cx);
        Button::new("titlebar-fetch-start")
            .primary()
            .small()
            .icon(FeiwenIconName::CirclePlay)
            .label(i18n.t("fetch-submit-button"))
            .disabled(running)
            .on_click({
                let fetch_view = self.fetch_view.clone();
                move |_, _, cx| {
                    fetch_view.update(cx, |view, cx| view.request_start_fetch(cx));
                }
            })
    }

    fn route_tabs(&self, cx: &mut App) -> impl IntoElement {
        let i18n = cx.global::<I18n>();
        TabBar::new("feiwen-titlebar-routes")
            .underline()
            .large()
            .h(FEIWEN_TITLE_BAR_HEIGHT)
            .selected_index(route_tab_index(self.router))
            .on_click({
                let workspace = self.workspace.clone();
                move |index, _, cx| {
                    if let Some(target) = route_from_tab_index(*index) {
                        update_router_from_titlebar(&workspace, target, cx);
                    }
                }
            })
            .child(Tab::new().label(i18n.t("titlebar-route-query")))
            .child(Tab::new().label(i18n.t("titlebar-route-fetch")))
    }
}

impl RenderOnce for FeiwenTitleBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        TitleBar::new()
            .h(FEIWEN_TITLE_BAR_HEIGHT)
            .pl(FEIWEN_TITLE_BAR_LEFT_PADDING)
            .child(
                h_flex()
                    .id("feiwen-titlebar-content")
                    .h_full()
                    .min_w_0()
                    .flex_1()
                    .justify_between()
                    .gap_3()
                    .child(self.leading(cx))
                    .child(self.center(cx))
                    .child(self.trailing(cx)),
            )
    }
}

impl FeiwenTitleBar {
    fn leading(&self, cx: &mut App) -> impl IntoElement {
        interactive_region(
            h_flex()
                .h_full()
                .items_center()
                .min_w_0()
                .gap_3()
                .child(
                    Label::new(self.app_title.clone())
                        .text_base()
                        .font_medium()
                        .truncate(),
                )
                .child(self.route_tabs(cx)),
        )
    }

    fn center(&self, cx: &App) -> impl IntoElement {
        h_flex()
            .flex_1()
            .min_w_0()
            .justify_center()
            .overflow_hidden()
            .child(
                Label::new(self.summary(cx))
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .truncate(),
            )
    }

    fn trailing(&self, cx: &mut App) -> impl IntoElement {
        interactive_region(
            h_flex()
                .justify_end()
                .flex_shrink_0()
                .pr_2()
                .child(self.actions(cx)),
        )
    }
}

fn interactive_region(child: impl IntoElement) -> impl IntoElement {
    div()
        .h_full()
        .flex()
        .items_center()
        .on_mouse_down(MouseButton::Left, |_, _, cx| {
            cx.stop_propagation();
        })
        .child(child)
}

fn update_router_from_titlebar(workspace: &Entity<Workspace>, target: RouterType, cx: &mut App) {
    workspace.update(cx, |workspace, cx| {
        if workspace.router == target {
            return;
        }

        event!(
            Level::INFO,
            from = %workspace.router.label(),
            to = %target.label(),
            "switching feiwen route from custom titlebar"
        );
        workspace.router = target;
        cx.notify();
    });
    cx.refresh_windows();
}

#[cfg(test)]
mod tests {
    use super::{route_from_tab_index, route_tab_index, route_title, window_title};
    use crate::{app::RouterType, foundation::i18n::I18n};

    #[test]
    fn route_title_matches_workspace_routes() {
        let i18n = I18n::chinese_for_test();
        assert_eq!(route_title(RouterType::Query, &i18n).as_ref(), "高级检索");
        assert_eq!(route_title(RouterType::Fetch, &i18n).as_ref(), "数据抓取");
    }

    #[test]
    fn window_title_includes_route_when_route_differs_from_app_title() {
        assert_eq!(window_title(&"高级检索".into(), "飞文"), "高级检索 - 飞文");
        assert_eq!(window_title(&"飞文".into(), "飞文"), "飞文");
    }

    #[test]
    fn route_tab_index_matches_workspace_routes() {
        assert_eq!(route_tab_index(RouterType::Query), 0);
        assert_eq!(route_tab_index(RouterType::Fetch), 1);
    }

    #[test]
    fn route_from_tab_index_ignores_unknown_index() {
        assert!(matches!(route_from_tab_index(0), Some(RouterType::Query)));
        assert!(matches!(route_from_tab_index(1), Some(RouterType::Fetch)));
        assert!(route_from_tab_index(2).is_none());
    }
}

use gpui::{
    App, Decorations, Entity, Hsla, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Pixels, Render, RenderOnce, SharedString, StatefulInteractiveElement as _, Styled, Window,
    WindowControlArea, div, point, prelude::FluentBuilder as _, px,
};
use gpui_component::{
    ActiveTheme, Disableable, Icon, IconName, InteractiveElementExt as _, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    tab::{Tab, TabBar},
};
use tracing::{Level, event};

use super::{
    fetch::FetchView,
    query::QueryView,
    workspace::{RouterType, Workspace},
};
use crate::foundation::{I18n, IconName as FeiwenIconName};

pub(crate) const FEIWEN_TITLE_BAR_HEIGHT: Pixels = px(44.);
pub(crate) const FEIWEN_TRAFFIC_LIGHT_INSET: Pixels = px(14.);

#[cfg(target_os = "macos")]
const FEIWEN_TITLE_BAR_LEFT_PADDING: Pixels = px(86.);
#[cfg(not(target_os = "macos"))]
const FEIWEN_TITLE_BAR_LEFT_PADDING: Pixels = px(12.);

const WINDOW_CONTROL_WIDTH: Pixels = px(44.);

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

struct TitleBarState {
    should_move: bool,
}

impl Render for TitleBarState {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
    }
}

impl RenderOnce for FeiwenTitleBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let is_client_decorated = matches!(window.window_decorations(), Decorations::Client { .. });
        let is_web = cfg!(target_family = "wasm");
        let is_linux = cfg!(target_os = "linux");
        let is_macos = cfg!(target_os = "macos");
        let state = window.use_state(cx, |_, _| TitleBarState { should_move: false });

        div().flex_shrink_0().child(
            div()
                .id("feiwen-title-bar")
                .flex()
                .items_center()
                .h(FEIWEN_TITLE_BAR_HEIGHT)
                .pl(FEIWEN_TITLE_BAR_LEFT_PADDING)
                .border_b_1()
                .border_color(cx.theme().title_bar_border)
                .bg(cx.theme().title_bar)
                .when(is_linux, |this| {
                    this.on_double_click(|_, window, _| window.zoom_window())
                })
                .when(is_macos, |this| {
                    this.on_double_click(|_, window, _| window.titlebar_double_click())
                })
                .on_mouse_down_out(window.listener_for(&state, |state, _, _, _| {
                    state.should_move = false;
                }))
                .on_mouse_down(
                    MouseButton::Left,
                    window.listener_for(&state, |state, _, _, _| {
                        state.should_move = true;
                    }),
                )
                .on_mouse_up(
                    MouseButton::Left,
                    window.listener_for(&state, |state, _, _, _| {
                        state.should_move = false;
                    }),
                )
                .on_mouse_move(window.listener_for(&state, |state, _, window, _| {
                    if state.should_move {
                        state.should_move = false;
                        window.start_window_move();
                    }
                }))
                .child(
                    h_flex()
                        .id("feiwen-titlebar-content")
                        .h_full()
                        .min_w_0()
                        .flex_1()
                        .justify_between()
                        .gap_3()
                        .when(!is_web, |this| {
                            this.window_control_area(WindowControlArea::Drag)
                                .when(window.is_fullscreen(), |this| this.pl_3())
                                .when(is_linux && is_client_decorated, |this| {
                                    this.child(
                                        div()
                                            .top_0()
                                            .left_0()
                                            .absolute()
                                            .size_full()
                                            .h_full()
                                            .on_mouse_down(
                                                MouseButton::Right,
                                                move |ev, window, _| {
                                                    window.show_window_menu(ev.position)
                                                },
                                            ),
                                    )
                                })
                        })
                        .child(self.leading(cx))
                        .child(self.center(cx))
                        .child(self.trailing(cx)),
                )
                .child(WindowControls),
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

#[derive(IntoElement, Clone)]
enum ControlIcon {
    Minimize,
    Restore,
    Maximize,
    Close,
}

impl ControlIcon {
    fn id(&self) -> &'static str {
        match self {
            Self::Minimize => "minimize",
            Self::Restore => "restore",
            Self::Maximize => "maximize",
            Self::Close => "close",
        }
    }

    fn icon(&self) -> IconName {
        match self {
            Self::Minimize => IconName::WindowMinimize,
            Self::Restore => IconName::WindowRestore,
            Self::Maximize => IconName::WindowMaximize,
            Self::Close => IconName::WindowClose,
        }
    }

    fn window_control_area(&self) -> WindowControlArea {
        match self {
            Self::Minimize => WindowControlArea::Min,
            Self::Restore | Self::Maximize => WindowControlArea::Max,
            Self::Close => WindowControlArea::Close,
        }
    }

    fn is_close(&self) -> bool {
        matches!(self, Self::Close)
    }

    fn hover_fg(&self, cx: &App) -> Hsla {
        if self.is_close() {
            cx.theme().danger_foreground
        } else {
            cx.theme().secondary_foreground
        }
    }

    fn hover_bg(&self, cx: &App) -> Hsla {
        if self.is_close() {
            cx.theme().danger
        } else {
            cx.theme().secondary_hover
        }
    }

    fn active_bg(&self, cx: &mut App) -> Hsla {
        if self.is_close() {
            cx.theme().danger_active
        } else {
            cx.theme().secondary_active
        }
    }
}

impl RenderOnce for ControlIcon {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let is_linux = cfg!(target_os = "linux");
        let is_windows = cfg!(target_os = "windows");
        let hover_fg = self.hover_fg(cx);
        let hover_bg = self.hover_bg(cx);
        let active_bg = self.active_bg(cx);
        let icon = self.clone();

        div()
            .id(self.id())
            .flex()
            .w(WINDOW_CONTROL_WIDTH)
            .h_full()
            .flex_shrink_0()
            .justify_center()
            .items_center()
            .text_color(cx.theme().foreground)
            .hover(|style| style.bg(hover_bg).text_color(hover_fg))
            .active(|style| style.bg(active_bg).text_color(hover_fg))
            .when(is_windows, |this| {
                this.window_control_area(self.window_control_area())
            })
            .when(is_linux, |this| {
                this.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    window.prevent_default();
                    cx.stop_propagation();
                })
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    match icon {
                        Self::Minimize => window.minimize_window(),
                        Self::Restore | Self::Maximize => window.zoom_window(),
                        Self::Close => window.remove_window(),
                    }
                })
            })
            .child(Icon::new(self.icon()).small())
    }
}

#[derive(IntoElement)]
struct WindowControls;

impl RenderOnce for WindowControls {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        if cfg!(target_os = "macos") || cfg!(target_family = "wasm") {
            return div().id("window-controls");
        }

        h_flex()
            .id("window-controls")
            .items_center()
            .flex_shrink_0()
            .h_full()
            .child(ControlIcon::Minimize)
            .child(if window.is_maximized() {
                ControlIcon::Restore
            } else {
                ControlIcon::Maximize
            })
            .child(ControlIcon::Close)
    }
}

#[cfg(test)]
mod tests {
    use super::{route_from_tab_index, route_tab_index, route_title, window_title};
    use crate::{features::workspace::RouterType, foundation::i18n::I18n};

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

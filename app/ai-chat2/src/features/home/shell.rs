use crate::{
    app::{menus, title_bar_menu},
    foundation, state,
};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Root, StyledExt, TitleBar, h_flex,
    label::Label,
    resizable::{h_resizable, resizable_panel},
    v_flex,
};

use super::{chat_form::ChatForm, sidebar::HomeSidebar};

const KEY_CONTEXT: &str = "AiChat2Home";

pub(crate) struct HomeView {
    focus_handle: FocusHandle,
    app_menu_bar: Entity<title_bar_menu::TitleBarAppMenuBar>,
    layout_state: Entity<state::AiChat2LayoutState>,
    sidebar: Entity<HomeSidebar>,
    chat_form: Entity<ChatForm>,
    _subscriptions: Vec<Subscription>,
}

impl HomeView {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        state::theme::apply_current_theme(window, cx);
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        let app_menu_bar = title_bar_menu::TitleBarAppMenuBar::new(cx);
        let layout_state = cx.global::<state::LayoutStateStore>().entity();
        let sidebar = cx.new(HomeSidebar::new);
        let chat_form = cx.new(|cx| ChatForm::new(window, cx));

        Self {
            focus_handle,
            app_menu_bar,
            layout_state: layout_state.clone(),
            sidebar,
            chat_form,
            _subscriptions: vec![
                cx.observe(&layout_state, |_state, _layout, cx| {
                    cx.notify();
                }),
                cx.observe_window_appearance(window, |_state, window, cx| {
                    state::theme::apply_current_theme(window, cx);
                    cx.refresh_windows();
                }),
                cx.observe_global_in::<state::theme::SystemAccentThemeState>(
                    window,
                    |_state, window, cx| {
                        state::theme::apply_current_theme(window, cx);
                        cx.refresh_windows();
                    },
                ),
                cx.observe_global_in::<state::AiChat2AppSettings>(window, |this, window, cx| {
                    foundation::init_i18n(cx);
                    menus::sync_app_menus(cx);
                    state::theme::apply_current_theme(window, cx);
                    this.reload_app_menu_bar(cx);
                    cx.refresh_windows();
                }),
            ],
        }
    }

    pub(crate) fn reload_app_menu_bar(&mut self, cx: &mut Context<Self>) {
        self.app_menu_bar
            .update(cx, |app_menu_bar, cx| app_menu_bar.reload(cx));
    }

    pub(crate) fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_handle.focus(window, cx);
    }

    fn minimize(&mut self, _: &menus::Minimize, window: &mut Window, _: &mut Context<Self>) {
        window.minimize_window();
    }

    fn zoom(&mut self, _: &menus::Zoom, window: &mut Window, _: &mut Context<Self>) {
        window.zoom_window();
    }
}

impl Render for HomeView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = cx.global::<foundation::I18n>().t("app-title");
        let sidebar_width = self.layout_state.read(cx).sidebar_width();
        let layout_state = self.layout_state.clone();
        let sheet_layer = Root::render_sheet_layer(window, cx);
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);
        window.set_window_title(&title);

        v_flex()
            .track_focus(&self.focus_handle)
            .key_context(KEY_CONTEXT)
            .size_full()
            .overflow_hidden()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(Self::minimize))
            .on_action(cx.listener(Self::zoom))
            .child(
                div()
                    .child(
                        TitleBar::new().child(title_bar_content(self.app_menu_bar.clone(), title)),
                    )
                    .flex_initial(),
            )
            .child(
                div().flex_1().min_h_0().overflow_hidden().child(
                    h_resizable("ai-chat2-home-layout")
                        .on_resize(move |resizable_state, _window, cx| {
                            let width = resizable_state
                                .read(cx)
                                .sizes()
                                .first()
                                .copied()
                                .unwrap_or(state::layout::SIDEBAR_DEFAULT_WIDTH);
                            layout_state.update(cx, |layout, cx| {
                                layout.set_sidebar_width(width, cx);
                            });
                        })
                        .child(
                            resizable_panel()
                                .size(sidebar_width)
                                .size_range(
                                    state::layout::SIDEBAR_MIN_WIDTH
                                        ..state::layout::SIDEBAR_MAX_WIDTH,
                                )
                                .child(self.sidebar.clone()),
                        )
                        .child(
                            resizable_panel().child(
                                div()
                                    .size_full()
                                    .min_w_0()
                                    .p_8()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        div()
                                            .w_full()
                                            .max_w(px(780.))
                                            .child(self.chat_form.clone()),
                                    ),
                            ),
                        ),
                ),
            )
            .children(sheet_layer)
            .children(dialog_layer)
            .children(notification_layer)
    }
}

fn title_bar_content(
    app_menu_bar: Entity<title_bar_menu::TitleBarAppMenuBar>,
    title: impl Into<SharedString>,
) -> impl IntoElement {
    h_flex()
        .w_full()
        .h_full()
        .min_w_0()
        .overflow_hidden()
        .when(menus::should_render_component_menu_bar(), |this| {
            this.child(title_bar_menu::title_bar_leading(app_menu_bar))
        })
        .child(title_bar_title(title))
}

fn title_bar_title(title: impl Into<SharedString>) -> impl IntoElement {
    h_flex()
        .flex_1()
        .min_w_0()
        .h_full()
        .justify_center()
        .overflow_hidden()
        .pr_2()
        .child(Label::new(title).text_sm().font_medium().truncate())
}

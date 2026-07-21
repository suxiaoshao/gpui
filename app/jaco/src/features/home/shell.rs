use crate::{
    app::{menus, title_bar_menu},
    components::conversation_detail::ConversationDetailPage,
    foundation, state,
};
use fluent_bundle::FluentArgs;
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Root, StyledExt, TitleBar, WindowExt as _, h_flex,
    label::Label,
    notification::{Notification, NotificationType},
    resizable::{h_resizable, resizable_panel},
    v_flex,
};
use jaco_core::ConversationId;
use std::collections::HashMap;

use super::{
    actions::{OpenConversationSearch, OpenNewConversation},
    new_conversation::NewConversationPage,
    sidebar::{self, HomeSidebar},
};

pub(crate) const KEY_CONTEXT: &str = "JacoHome";

pub(crate) struct HomeView {
    focus_handle: FocusHandle,
    app_menu_bar: Entity<title_bar_menu::TitleBarAppMenuBar>,
    layout_state: Entity<state::JacoLayoutState>,
    workspace: Entity<state::JacoWorkspaceStore>,
    sidebar: Entity<HomeSidebar>,
    new_conversation: Entity<NewConversationPage>,
    conversation_pages: HashMap<ConversationId, Entity<ConversationDetailPage>>,
    config_load_error_notified: bool,
    _subscriptions: Vec<Subscription>,
}

impl HomeView {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        state::theme::apply_current_theme(window, cx);
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        let app_menu_bar = title_bar_menu::TitleBarAppMenuBar::new(cx);
        let layout_state = cx.global::<state::LayoutStateStore>().entity();
        let config_store = state::config::store(cx);
        let workspace = state::workspace::workspace(cx);
        let sidebar = cx.new(HomeSidebar::new);
        let new_conversation = cx.new(|cx| NewConversationPage::new(window, cx));
        let layout_state_for_bounds = layout_state.clone();
        let new_conversation_for_workspace = new_conversation.clone();

        Self {
            focus_handle,
            app_menu_bar,
            layout_state: layout_state.clone(),
            workspace: workspace.clone(),
            sidebar,
            new_conversation,
            conversation_pages: HashMap::new(),
            config_load_error_notified: false,
            _subscriptions: vec![
                cx.observe(&layout_state, |_state, _layout, cx| {
                    cx.notify();
                }),
                cx.observe_in(&workspace, window, move |_state, workspace, window, cx| {
                    let pending_project_id = workspace.update(cx, |workspace, _cx| {
                        workspace.take_pending_new_conversation_project_id()
                    });
                    if let Some(project_id) = pending_project_id {
                        new_conversation_for_workspace.update(cx, |page, cx| {
                            page.select_project_id_from_sidebar(project_id, window, cx);
                        });
                    }
                    cx.notify();
                }),
                cx.observe_window_bounds(window, move |_state, window, cx| {
                    let window_bounds = window.window_bounds();
                    let display_id = window.display(cx).map(|display| display.id());
                    layout_state_for_bounds.update(cx, |layout, cx| {
                        layout.set_window_bounds(
                            state::layout::WindowPlacementKind::Main,
                            window_bounds,
                            display_id,
                            cx,
                        );
                    });
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
                config_store.observe_select_in(
                    cx,
                    window,
                    |config| {
                        (
                            config.app_settings.language,
                            config.app_settings.theme.clone(),
                        )
                    },
                    |this, _settings, window, cx| {
                        foundation::init_i18n(cx);
                        menus::sync_app_menus(cx);
                        state::theme::apply_current_theme(window, cx);
                        this.reload_app_menu_bar(cx);
                        cx.refresh_windows();
                    },
                ),
            ],
        }
    }

    pub(crate) fn reload_app_menu_bar(&mut self, cx: &mut Context<Self>) {
        self.app_menu_bar
            .update(cx, |app_menu_bar, cx| app_menu_bar.reload(cx));
    }

    pub(crate) fn focus_chat_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.new_conversation
            .update(cx, |page, cx| page.focus_primary(window, cx));
    }

    pub(crate) fn notify_config_load_error(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.config_load_error_notified {
            return;
        }

        let Some(load_error) = state::config::config_load_error(cx) else {
            return;
        };
        self.config_load_error_notified = true;

        let i18n = cx.global::<foundation::I18n>();
        let mut args = FluentArgs::new();
        args.set("path", load_error.path_display());
        args.set("error", load_error.message().to_string());
        window.push_notification(
            Notification::new()
                .title(i18n.t("config-load-error-title"))
                .message(i18n.t_with_args("config-load-error-message", &args))
                .with_type(NotificationType::Error),
            cx,
        );
    }

    fn minimize(&mut self, _: &menus::Minimize, window: &mut Window, _: &mut Context<Self>) {
        window.minimize_window();
    }

    fn zoom(&mut self, _: &menus::Zoom, window: &mut Window, _: &mut Context<Self>) {
        window.zoom_window();
    }

    fn open_new_conversation(
        &mut self,
        _: &OpenNewConversation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.workspace.update(cx, |workspace, cx| {
            workspace.open_new_conversation(cx);
        });
        self.focus_chat_form(window, cx);
    }

    fn open_conversation_search(
        &mut self,
        _: &OpenConversationSearch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        sidebar::search::open_conversation_search_dialog(window, cx);
    }

    fn conversation_page(
        &mut self,
        conversation_id: ConversationId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<ConversationDetailPage> {
        self.conversation_pages
            .entry(conversation_id.clone())
            .or_insert_with(|| {
                cx.new(|cx| ConversationDetailPage::new(conversation_id, window, cx))
            })
            .clone()
    }
}

impl Render for HomeView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = cx.global::<foundation::I18n>().t("app-title");
        let sidebar_width = self.layout_state.read(cx).sidebar_width();
        let layout_state = self.layout_state.clone();
        let route = self.workspace.read(cx).route().clone();
        let sheet_layer = Root::render_sheet_layer(window, cx);
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);
        window.set_window_title(&title);

        v_flex()
            .track_focus(&self.focus_handle)
            .key_context(KEY_CONTEXT)
            .size_full()
            .overflow_hidden()
            .bg(cx.theme().tokens.background.background)
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(Self::minimize))
            .on_action(cx.listener(Self::zoom))
            .on_action(cx.listener(Self::open_new_conversation))
            .on_action(cx.listener(Self::open_conversation_search))
            .child(
                div()
                    .child(
                        TitleBar::new().child(title_bar_content(self.app_menu_bar.clone(), title)),
                    )
                    .flex_initial(),
            )
            .child(
                div().flex_1().min_h_0().overflow_hidden().child(
                    h_resizable("jaco-home-layout")
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
                                div().size_full().min_w_0().child(match route {
                                    state::HomeRoute::NewConversation => {
                                        self.new_conversation.clone().into_any_element()
                                    }
                                    state::HomeRoute::Conversation(conversation_id) => self
                                        .conversation_page(conversation_id, window, cx)
                                        .into_any_element(),
                                }),
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

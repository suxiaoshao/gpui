use crate::{
    app::menus,
    components::{
        add_conversation::open_add_conversation_dialog,
        add_folder::open_add_folder_dialog,
        title_bar_menu::{TitleBarAppMenuBar, title_bar_leading},
    },
    features::home::{sidebar::SidebarView, tabs::TabsView},
    foundation::i18n::I18n,
    state::{self, AiChatConfig, WindowPlacementKind, WorkspaceStore},
};
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    Root, StyledExt, Theme, ThemeRegistry, TitleBar,
    alert::Alert,
    h_flex,
    label::Label,
    resizable::{h_resizable, resizable_panel},
    v_flex,
};
use std::ops::Deref;
pub(crate) use tabs::{
    ConversationPanelView, ConversationTabView, open_copy_conversation_dialog,
    open_export_conversation_prompt,
};

mod search;
mod sidebar;
mod tabs;

use search::{OpenConversationSearch, open_conversation_search_dialog};

actions!(home_view, [AddConversation, AddFolder]);

pub(super) const HOME_CONTEXT: &str = "home_view";

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("secondary-n", AddConversation, Some(HOME_CONTEXT)),
        KeyBinding::new("secondary-shift-n", AddFolder, Some(HOME_CONTEXT)),
    ]);
    search::init(cx);
    sidebar::init(cx);
    tabs::init(cx);
}

fn apply_current_theme(window: &mut Window, cx: &mut App) {
    let theme_registry = ThemeRegistry::global(cx);
    let config = cx.global::<AiChatConfig>();
    let config = config.gpui_theme(theme_registry, window);
    Theme::global_mut(cx).apply_config(&config);
}

pub(crate) struct HomeView {
    sidebar: Entity<SidebarView>,
    tabs: Entity<TabsView>,
    app_menu_bar: Entity<TitleBarAppMenuBar>,
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl HomeView {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        state::chat::init(window, cx);
        state::workspace::init(window, cx);
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        let sidebar = cx.new(|cx| SidebarView::new(window, cx));
        let tabs = cx.new(TabsView::new);
        let app_menu_bar = TitleBarAppMenuBar::new(cx);
        let workspace = cx.global::<WorkspaceStore>().deref().clone();
        apply_current_theme(window, cx);

        Self {
            sidebar,
            tabs,
            app_menu_bar,
            focus_handle,
            _subscriptions: vec![
                cx.observe(&workspace, |_state, _workspace, cx| {
                    cx.notify();
                }),
                cx.observe_window_bounds(window, |_state, window, cx| {
                    let window_bounds = window.window_bounds();
                    let display_id = window.display(cx).map(|display| display.id());
                    cx.global::<WorkspaceStore>()
                        .deref()
                        .clone()
                        .update(cx, |workspace, cx| {
                            workspace.set_window_bounds(
                                WindowPlacementKind::Main,
                                window_bounds,
                                display_id,
                                cx,
                            );
                        });
                }),
                cx.observe_window_appearance(window, |_state, window, cx| {
                    apply_current_theme(window, cx);
                    cx.refresh_windows();
                }),
                cx.observe_global_in::<AiChatConfig>(window, |_state, window, cx| {
                    apply_current_theme(window, cx);
                    cx.refresh_windows();
                }),
                cx.observe_global_in::<state::theme::SystemAccentThemeState>(
                    window,
                    |_state, window, cx| {
                        apply_current_theme(window, cx);
                        cx.refresh_windows();
                    },
                ),
            ],
        }
    }

    pub(crate) fn focus_chat_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(panel) = cx
            .global::<WorkspaceStore>()
            .read(cx)
            .active_conversation_panel()
        else {
            return;
        };
        panel.update(cx, |panel, cx| panel.focus_chat_form(window, cx));
    }

    pub(crate) fn reload_app_menu_bar(&mut self, cx: &mut Context<Self>) {
        self.app_menu_bar
            .update(cx, |app_menu_bar, cx| app_menu_bar.reload(cx));
    }

    fn minimize(&mut self, _: &menus::Minimize, window: &mut Window, _: &mut Context<Self>) {
        window.minimize_window();
    }

    fn zoom(&mut self, _: &menus::Zoom, window: &mut Window, _: &mut Context<Self>) {
        window.zoom_window();
    }

    fn open_conversation_search(
        &mut self,
        _: &OpenConversationSearch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        open_conversation_search_dialog(window, cx);
    }

    fn add_conversation(
        &mut self,
        _: &AddConversation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        open_add_conversation_dialog(None, None, window, cx);
    }

    fn add_folder(&mut self, _: &AddFolder, window: &mut Window, cx: &mut Context<Self>) {
        open_add_folder_dialog(None, window, cx);
    }
}

impl Render for HomeView {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);
        let error_title = cx.global::<I18n>().t("alert-error-title");
        let app_title = cx.global::<I18n>().t("app-title");
        let chat_data = cx.global::<state::ChatData>().read(cx);
        let (sidebar_width, active_tab_title) = {
            let workspace = cx.global::<WorkspaceStore>().read(cx);
            (workspace.sidebar_width(), workspace.active_tab_title())
        };
        let titlebar_title = home_titlebar_title(active_tab_title, &app_title);
        let window_title = home_window_title(&titlebar_title, &app_title);
        window.set_window_title(&window_title);

        v_flex()
            .key_context(HOME_CONTEXT)
            .track_focus(&self.focus_handle)
            .size_full()
            .overflow_hidden()
            .on_action(cx.listener(Self::minimize))
            .on_action(cx.listener(Self::zoom))
            .on_action(cx.listener(Self::open_conversation_search))
            .on_action(cx.listener(Self::add_conversation))
            .on_action(cx.listener(Self::add_folder))
            .child(
                div()
                    .child(
                        TitleBar::new()
                            .child(title_bar_content(self.app_menu_bar.clone(), titlebar_title)),
                    )
                    .flex_initial(),
            )
            .map(|this| match chat_data {
                Ok(_) => this.child(
                    div()
                        .overflow_hidden()
                        .child(
                            h_resizable("vertical-layout")
                                .on_resize(|state, _window, cx| {
                                    let width =
                                        state.read(cx).sizes().first().copied().unwrap_or(px(300.));
                                    cx.global::<WorkspaceStore>().deref().clone().update(
                                        cx,
                                        |workspace, cx| {
                                            workspace.set_sidebar_width(width, cx);
                                        },
                                    );
                                })
                                .child(
                                    resizable_panel()
                                        .size(sidebar_width)
                                        .size_range(
                                            state::workspace::SIDEBAR_MIN_WIDTH
                                                ..state::workspace::SIDEBAR_MAX_WIDTH,
                                        )
                                        .child(self.sidebar.clone()),
                                )
                                .child(self.tabs.clone().into_any_element()),
                        )
                        .flex_1(),
                ),
                Err(err) => {
                    this.child(Alert::error("home-alert", err.to_string()).title(error_title))
                }
            })
            .children(dialog_layer)
            .children(notification_layer)
    }
}

fn title_bar_content(
    app_menu_bar: Entity<TitleBarAppMenuBar>,
    title: impl Into<SharedString>,
) -> impl IntoElement {
    h_flex()
        .w_full()
        .h_full()
        .min_w_0()
        .overflow_hidden()
        .when(menus::should_render_component_menu_bar(), |this| {
            this.child(title_bar_leading(app_menu_bar))
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

fn home_titlebar_title(active_tab_title: Option<SharedString>, app_title: &str) -> SharedString {
    active_tab_title.unwrap_or_else(|| SharedString::from(app_title.to_string()))
}

fn home_window_title(titlebar_title: &SharedString, app_title: &str) -> String {
    if titlebar_title.as_ref() == app_title {
        app_title.to_string()
    } else {
        format!("{titlebar_title} - {app_title}")
    }
}

#[cfg(test)]
mod tests {
    use super::{home_titlebar_title, home_window_title};

    #[test]
    fn home_titlebar_title_uses_active_tab() {
        assert_eq!(
            home_titlebar_title(Some("Conversation A".into()), "AI Chat").as_ref(),
            "Conversation A"
        );
    }

    #[test]
    fn home_titlebar_title_falls_back_to_app_title() {
        assert_eq!(home_titlebar_title(None, "AI Chat").as_ref(), "AI Chat");
    }

    #[test]
    fn home_window_title_includes_active_tab_when_present() {
        assert_eq!(
            home_window_title(&"Conversation A".into(), "AI Chat"),
            "Conversation A - AI Chat"
        );
        assert_eq!(home_window_title(&"AI Chat".into(), "AI Chat"), "AI Chat");
    }
}

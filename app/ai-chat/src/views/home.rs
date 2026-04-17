use crate::{
    app_menus,
    components::{
        add_conversation::open_add_conversation_dialog, add_folder::open_add_folder_dialog,
    },
    i18n::I18n,
    state::{self, AiChatConfig, WorkspaceStore},
    views::home::{sidebar::SidebarView, tabs::TabsView},
};
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    Root, Theme, ThemeRegistry, TitleBar,
    alert::Alert,
    resizable::{h_resizable, resizable_panel},
    v_flex,
};
use std::ops::Deref;
pub(crate) use tabs::{
    ConversationPanelView, ConversationTabView, TemplateDetailView, TemplateListView,
    open_copy_conversation_dialog, open_export_conversation_prompt,
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

pub(crate) struct HomeView {
    sidebar: Entity<SidebarView>,
    tabs: Entity<TabsView>,
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

        Self {
            sidebar,
            tabs,
            focus_handle,
            _subscriptions: vec![
                cx.observe_window_appearance(window, |_state, window, cx| {
                    let theme_registry = ThemeRegistry::global(cx);
                    let config = cx.global::<AiChatConfig>();
                    let config = config.gpui_theme(theme_registry, window);
                    Theme::global_mut(cx).apply_config(&config);
                    cx.refresh_windows();
                }),
                cx.observe_global_in::<AiChatConfig>(window, |_state, window, cx| {
                    let theme_registry = ThemeRegistry::global(cx);
                    let config = cx.global::<AiChatConfig>();
                    let config = config.gpui_theme(theme_registry, window);
                    Theme::global_mut(cx).apply_config(&config);
                    cx.refresh_windows();
                }),
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

    fn minimize(&mut self, _: &app_menus::Minimize, window: &mut Window, _: &mut Context<Self>) {
        window.minimize_window();
    }

    fn zoom(&mut self, _: &app_menus::Zoom, window: &mut Window, _: &mut Context<Self>) {
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
        let chat_data = cx.global::<state::ChatData>().read(cx);
        let sidebar_width = cx.global::<WorkspaceStore>().read(cx).sidebar_width();
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
            .child(div().child(TitleBar::new()).flex_initial())
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

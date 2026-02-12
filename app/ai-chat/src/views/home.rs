use crate::{
    config::AiChatConfig,
    i18n::I18n,
    store,
    views::home::{sidebar::SidebarView, tabs::TabsView},
};
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    Root, Theme, ThemeRegistry, TitleBar,
    alert::Alert,
    resizable::{h_resizable, resizable_panel},
    v_flex,
};
pub(crate) use sidebar::{Add, AddShift};
pub(crate) use tabs::{
    ConversationPanelView, ConversationTabView, TemplateDetailView, TemplateListView,
};

mod sidebar;
mod tabs;

pub fn init(cx: &mut App) {
    sidebar::init(cx);
    tabs::init(cx);
}

pub(crate) struct HomeView {
    sidebar: Entity<SidebarView>,
    tabs: Entity<TabsView>,
    _subscriptions: Vec<Subscription>,
}

impl HomeView {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        store::init(window, cx);
        let sidebar = cx.new(|cx| SidebarView::new(window, cx));
        let tabs = cx.new(TabsView::new);

        Self {
            sidebar,
            tabs,
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
        let chat_data = cx.global::<store::ChatData>().read(cx);
        v_flex()
            .size_full()
            .overflow_hidden()
            .child(div().child(TitleBar::new()).flex_initial())
            .map(|this| match chat_data {
                Ok(_) => this.child(
                    div()
                        .overflow_hidden()
                        .child(
                            h_resizable("vertical-layout")
                                .child(resizable_panel().size(px(300.)).child(self.sidebar.clone()))
                                .child(self.tabs.clone().into_any_element()),
                        )
                        .flex_1(),
                ),
                Err(err) => this.child(Alert::error("home-alert", err.to_string()).title(error_title)),
            })
            .children(dialog_layer)
            .children(notification_layer)
    }
}

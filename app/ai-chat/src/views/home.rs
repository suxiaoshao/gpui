use crate::{
    store,
    views::home::{sidebar::SidebarView, tabs::TabsView},
};
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    Root, TitleBar,
    alert::Alert,
    resizable::{h_resizable, resizable_panel},
    v_flex,
};

mod sidebar;
mod tabs;

pub(crate) use sidebar::{AddConversation, AddFolder};
pub(crate) use tabs::ConversationTabView;

pub fn init(cx: &mut App) {
    sidebar::init(cx);
}

pub(crate) struct HomeView {
    sidebar: Entity<SidebarView>,
    tabs: Entity<TabsView>,
}

impl HomeView {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        store::init(window, cx);
        let sidebar = cx.new(|cx| SidebarView::new(window, cx));
        let tabs = cx.new(|cx| TabsView::new(cx));

        Self { sidebar, tabs }
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
        let chat_data = cx.global::<store::ChatData>().read(cx);
        v_flex()
            .size_full()
            .child(TitleBar::new())
            .map(|this| match chat_data {
                Ok(_) => this.child(
                    h_resizable("vertical-layout")
                        .child(resizable_panel().size(px(300.)).child(self.sidebar.clone()))
                        .child(self.tabs.clone().into_any_element()),
                ),
                Err(err) => this.child(Alert::error("home-alert", err.to_string()).title("Error")),
            })
            .children(dialog_layer)
            .children(notification_layer)
    }
}

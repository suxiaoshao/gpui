use crate::{store, views::home::sidebar::SidebarView};
use gpui::*;
use gpui_component::{
    Root, TitleBar,
    resizable::{h_resizable, resizable_panel},
    v_flex,
};

mod sidebar;

pub fn init(cx: &mut App) {
    sidebar::init(cx);
}

pub(crate) struct HomeView {
    sidebar: Entity<SidebarView>,
}

impl HomeView {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        store::init(window, cx);
        let sidebar = cx.new(|cx| SidebarView::new(window, cx));

        Self { sidebar }
    }
}

impl Render for HomeView {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        let dialog_layer = Root::render_dialog_layer(window, cx);
        v_flex()
            .size_full()
            .child(TitleBar::new())
            .child(
                h_resizable("vertical-layout")
                    .child(resizable_panel().size(px(300.)).child(self.sidebar.clone()))
                    .child(div().child("Bottom Panel").into_any_element()),
            )
            .children(dialog_layer)
    }
}

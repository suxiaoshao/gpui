use gpui::{ParentElement, Render, div};
use gpui_component::{TitleBar, tab::TabBar};

pub(crate) struct HomeView {}

impl HomeView {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

impl Render for HomeView {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        div().child(TitleBar::new())
    }
}

use gpui::{ParentElement, Render};
use gpui_component::{TitleBar, v_flex};

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
        v_flex().child(TitleBar::new())
    }
}

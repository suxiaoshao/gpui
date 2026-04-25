use gpui::{Render, Styled, div};

pub struct FormDataView;

impl FormDataView {
    pub fn new() -> Self {
        FormDataView
    }
}

impl Render for FormDataView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        div().flex_1()
    }
}

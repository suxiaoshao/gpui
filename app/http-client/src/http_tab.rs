use gpui::*;
use gpui_component::tab::{Tab, TabBar};

use crate::{
    http_body::HttpBodyView, http_form::HttpForm, http_headers::HttpHeadersView,
    http_params::HttpParamsView,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HttpTab {
    #[default]
    Params,
    Headers,
    Body,
}

impl HttpTab {
    pub const ALL: [HttpTab; 3] = [HttpTab::Params, HttpTab::Headers, HttpTab::Body];
    pub fn as_str(&self) -> &'static str {
        match self {
            HttpTab::Params => "Params",
            HttpTab::Headers => "Headers",
            HttpTab::Body => "Body",
        }
    }
}

impl From<HttpTab> for usize {
    fn from(value: HttpTab) -> Self {
        match value {
            HttpTab::Params => 0,
            HttpTab::Headers => 1,
            HttpTab::Body => 2,
        }
    }
}

impl From<&usize> for HttpTab {
    fn from(value: &usize) -> Self {
        match value {
            0 => HttpTab::Params,
            1 => HttpTab::Headers,
            2 => HttpTab::Body,
            _ => unimplemented!(),
        }
    }
}

impl From<&mut HttpTabView> for AnyElement {
    fn from(value: &mut HttpTabView) -> Self {
        match value.tab {
            HttpTab::Params => value.params.clone().into_any_element(),
            HttpTab::Headers => value.headers.clone().into_any_element(),
            HttpTab::Body => value.body.clone().into_any_element(),
        }
    }
}

#[derive(Clone)]
pub struct HttpTabView {
    pub tab: HttpTab,
    params: Entity<HttpParamsView>,
    headers: Entity<HttpHeadersView>,
    body: Entity<HttpBodyView>,
}

impl HttpTabView {
    pub fn new(http_form: Entity<HttpForm>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            headers: cx.new(|_cx| HttpHeadersView::new(http_form.clone())),
            params: cx.new(|cx| HttpParamsView::new(http_form.clone(), window, cx)),
            body: cx.new(|cx| HttpBodyView::new(window, cx)),
            tab: HttpTab::Params,
        }
    }
}

impl Render for HttpTabView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected_value = self.tab.into();
        div()
            .flex_1()
            .flex()
            .flex_col()
            .child(
                TabBar::new("tabs")
                    .selected_index(selected_value)
                    .on_click(cx.listener(|this, selected_index, _window, _cx| {
                        let tab = HttpTab::from(selected_index);
                        this.tab = tab;
                    }))
                    .children(HttpTab::ALL.map(|tab| Tab::new().label(tab.as_str()))),
            )
            .child(AnyElement::from(self))
    }
}

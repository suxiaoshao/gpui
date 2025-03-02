use components::{Tab, TabItem, TabList};
use gpui::*;

use crate::{http_form::HttpForm, http_headers::HttpHeadersView, http_params::HttpParams};

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

impl TabItem for HttpTab {
    type Value = Self;

    fn label(&self) -> gpui::SharedString {
        self.as_str().into()
    }

    fn value(&self) -> Self::Value {
        *self
    }
}

#[derive(Clone)]
pub struct HttpTabView {
    pub tab: HttpTab,
    params: Entity<HttpParams>,
    headers: Entity<HttpHeadersView>,
}

impl HttpTabView {
    pub fn new(http_form: Entity<HttpForm>, cx: &mut Context<Tab<Self>>) -> Self {
        Self {
            headers: cx.new(|cx| HttpHeadersView::new(http_form.clone(), cx)),
            params: cx.new(|cx| HttpParams::new(http_form.clone(), cx)),
            tab: HttpTab::Params,
        }
    }
}

impl TabList for HttpTabView {
    type Item = HttpTab;

    fn items(&self) -> impl IntoIterator<Item = Self::Item> {
        HttpTab::ALL
    }

    fn select(&mut self, value: &<Self::Item as TabItem>::Value) {
        self.tab = *value;
    }

    fn get_select_item(&self) -> &Self::Item {
        &self.tab
    }

    fn div(&self, _cx: &mut Window) -> Div {
        div().flex_1()
    }
    fn panel(&self, _cx: &mut Window) -> impl gpui::IntoElement {
        match self.get_select_item() {
            HttpTab::Params => self.params.clone().into_any_element(),
            HttpTab::Headers => self.headers.clone().into_any_element(),
            HttpTab::Body => div().child("Body").into_any_element(),
        }
    }
}

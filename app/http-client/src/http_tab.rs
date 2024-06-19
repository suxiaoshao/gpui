use components::{TabItem, TabList};
use gpui::*;

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

    fn panel(&self) -> impl gpui::IntoElement {
        match self {
            HttpTab::Params => div().child("Params"),
            HttpTab::Headers => div().child("Headers"),
            HttpTab::Body => div().child("Body"),
        }
    }

    fn value(&self) -> Self::Value {
        *self
    }
}

pub struct HttpTabView {
    pub tab: HttpTab,
}

impl HttpTabView {
    pub fn new() -> Self {
        Self {
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

    fn div(&self, _cx: &mut WindowContext) -> Div {
        div().flex_1()
    }
}

use components::{button, SelectItem, SelectList};
use gpui::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HttpMethod {
    #[default]
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
    Options,
    Trace,
    Connect,
}

impl HttpMethod {
    pub const ALL: [HttpMethod; 9] = [
        HttpMethod::Get,
        HttpMethod::Post,
        HttpMethod::Put,
        HttpMethod::Delete,
        HttpMethod::Patch,
        HttpMethod::Head,
        HttpMethod::Options,
        HttpMethod::Trace,
        HttpMethod::Connect,
    ];
    pub fn as_str(&self) -> &'static str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Delete => "DELETE",
            HttpMethod::Patch => "PATCH",
            HttpMethod::Head => "HEAD",
            HttpMethod::Options => "OPTIONS",
            HttpMethod::Trace => "TRACE",
            HttpMethod::Connect => "CONNECT",
        }
    }
}

impl SelectItem for HttpMethod {
    type Value = HttpMethod;

    fn value(&self) -> Self::Value {
        *self
    }

    fn display_item(&self) -> impl IntoElement {
        self.as_str()
    }

    fn id(&self) -> ElementId {
        ElementId::Name(self.as_str().into())
    }

    fn label(&self) -> String {
        self.as_str().to_string()
    }
}

#[derive(Debug, Clone, Default)]
pub struct SelectHttpMethod {
    pub selected: HttpMethod,
}

impl SelectHttpMethod {
    pub fn new(selected: Option<HttpMethod>) -> Self {
        Self {
            selected: selected.unwrap_or_default(),
        }
    }
}

impl SelectList for SelectHttpMethod {
    type Item = HttpMethod;

    type Value = HttpMethod;

    fn items(&self) -> impl IntoIterator<Item = Self::Item> {
        HttpMethod::ALL
    }

    fn select(&mut self, value: &<Self::Item as SelectItem>::Value) {
        self.selected = *value;
    }

    fn get_select_item(&self) -> &Self::Item {
        &self.selected
    }

    fn trigger_element(
        &self,
        cx: &mut WindowContext,
        func: impl Fn(&ClickEvent, &mut WindowContext) + 'static,
    ) -> impl IntoElement {
        button(self.selected.as_str(), cx)
            .on_click(move |event, cx| {
                func(event, cx);
            })
            .rounded_l(px(4.0))
            .rounded_r(rems(0.0))
            .flex()
            .w(px(100.0))
            .items_center()
            .child(self.selected.as_str())
    }
}

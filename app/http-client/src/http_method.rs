use components::{button, SelectItem, SelectList};
use gpui::*;

use crate::http_form::{HttpForm, HttpFormEvent};

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

#[derive(Clone)]
pub struct SelectHttpMethod {
    pub http_form: WeakEntity<HttpForm>,
}

impl SelectHttpMethod {
    pub fn new(http_form: WeakEntity<HttpForm>) -> Self {
        Self { http_form }
    }
    pub fn selected(&self, cx: &mut App) -> HttpMethod {
        self.http_form
            .read_with(cx, |data, _cx| data.http_method)
            .unwrap_or_default()
    }
}

impl SelectList for SelectHttpMethod {
    type Item = HttpMethod;

    type Value = HttpMethod;

    fn items(&self) -> impl IntoIterator<Item = Self::Item> {
        HttpMethod::ALL
    }

    fn select(
        &mut self,
        _window: &mut Window,
        cx: &mut App,
        value: &<Self::Item as SelectItem>::Value,
    ) {
        if let Err(_err) = self
            .http_form
            .update(cx, |_data, cx| cx.emit(HttpFormEvent::SetMethod(*value)))
        {
            // todo log
        };
    }

    fn get_select_item(&self, _window: &mut Window, cx: &mut App) -> Self::Item {
        self.http_form
            .read_with(cx, |data, _cx| data.http_method)
            .unwrap_or_default()
    }

    fn trigger_element(
        &self,
        _window: &mut Window,
        cx: &mut App,
        func: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let http_method = self.selected(cx);
        button(http_method.as_str())
            .on_click(move |event, window, cx| {
                func(event, window, cx);
            })
            .rounded_r(rems(0.0))
            .flex()
            .w(px(100.0))
            .items_center()
            .child(http_method.as_str())
    }
}

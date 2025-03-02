use components::TextInput;
use gpui::*;

use crate::http_form::{HttpForm, HttpFormEvent};

pub struct UrlInput {
    input: Entity<TextInput>,
    form: Entity<HttpForm>,
}

impl UrlInput {
    pub fn new(http_form: Entity<HttpForm>, cx: &mut Context<Self>) -> Self {
        let on_url_change = cx.listener(|this: &mut UrlInput, data: &SharedString, _, cx| {
            this.form.update(cx, |_data, cx| {
                cx.emit(HttpFormEvent::SetUrl(data.to_string()))
            });
        });
        cx.subscribe(&http_form, Self::subscribe).detach();
        Self {
            form: http_form,
            input: cx.new(|cx| TextInput::new(cx, "".to_string(), "Url").on_change(on_url_change)),
        }
    }
    fn subscribe(
        &mut self,
        _subscriber: Entity<HttpForm>,
        emitter: &HttpFormEvent,
        cx: &mut Context<Self>,
    ) {
        if let HttpFormEvent::SetUrlByParams(url) = emitter {
            self.input = cx.new(|cx| TextInput::new(cx, url.clone(), "Url"));
        };
    }
}

impl Render for UrlInput {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.input.clone()
    }
}

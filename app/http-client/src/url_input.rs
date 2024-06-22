use components::Input;
use gpui::*;

use crate::http_form::{HttpForm, HttpFormEvent};

pub struct UrlInput {
    input: View<Input>,
    form: Model<HttpForm>,
}

impl UrlInput {
    pub fn new(http_form: Model<HttpForm>, cx: &mut ViewContext<Self>) -> Self {
        let on_url_change = cx.listener(|this: &mut UrlInput, data: &String, cx| {
            this.form
                .update(cx, |_data, cx| cx.emit(HttpFormEvent::SetUrl(data.clone())));
        });
        cx.subscribe(&http_form, Self::subscribe).detach();
        Self {
            form: http_form,
            input: cx.new_view(|cx| Input::new("".to_string(), cx).on_change(on_url_change)),
        }
    }
    fn subscribe(
        &mut self,
        _subscriber: Model<HttpForm>,
        emitter: &HttpFormEvent,
        cx: &mut ViewContext<Self>,
    ) {
        if let HttpFormEvent::SetUrlByParams(url) = emitter {
            self.input = cx.new_view(|cx| Input::new(url.clone(), cx));
        };
    }
}

impl Render for UrlInput {
    fn render(&mut self, _cx: &mut ViewContext<Self>) -> impl IntoElement {
        self.input.clone()
    }
}

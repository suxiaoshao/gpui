use components::TextInput;
use gpui::*;
use theme::Theme;
use url::Url;

use crate::{
    errors::HttpClientResult,
    http_form::{HttpForm, HttpFormEvent},
};

pub struct HttpParams {
    pub http_form: Model<HttpForm>,
    inputs: Vec<(View<TextInput>, View<TextInput>)>,
}

impl HttpParams {
    fn get_url(&self, cx: &mut WindowContext) -> HttpClientResult<Url> {
        let form = self.http_form.read(cx);
        let url = form.url.as_str();
        let url = url.parse::<url::Url>()?;
        Ok(url)
    }
    fn set_url(&self, index: usize, is_key: bool, value: &str, cx: &mut WindowContext) {
        if let Ok(mut url) = self.get_url(cx) {
            let mut query_pairs: Vec<(String, String)> = url.query_pairs().into_owned().collect();
            match is_key {
                true => {
                    query_pairs[index].0 = value.to_string();
                }
                false => {
                    query_pairs[index].1 = value.to_string();
                }
            }
            url.query_pairs_mut()
                .clear()
                .extend_pairs(query_pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())));
            self.http_form.update(cx, |_this, cx| {
                cx.emit(HttpFormEvent::SetUrlByParams(url.to_string()));
            });
        }
    }
    pub fn new(http_form: Model<HttpForm>, cx: &mut ViewContext<Self>) -> Self {
        let url = http_form.read(cx);
        let url = url.url.clone();
        cx.subscribe(&http_form, Self::subscribe).detach();
        let inputs = Self::get_inputs(&url, cx);
        Self { http_form, inputs }
    }
    fn subscribe(
        &mut self,
        _subscriber: Model<HttpForm>,
        emitter: &HttpFormEvent,
        cx: &mut ViewContext<Self>,
    ) {
        if let HttpFormEvent::SetUrl(url) = emitter {
            self.inputs = Self::get_inputs(url, cx);
        };
    }
    fn get_inputs(
        url: &str,
        params_cx: &mut ViewContext<Self>,
    ) -> Vec<(View<TextInput>, View<TextInput>)> {
        let mut inputs = vec![];
        if let Ok(url) = Url::parse(url) {
            for (index, (key, value)) in url.query_pairs().enumerate() {
                let on_key_change =
                    params_cx.listener(move |this: &mut HttpParams, data: &SharedString, cx| {
                        this.set_url(index, true, data, cx);
                    });
                let on_value_change =
                    params_cx.listener(move |this: &mut HttpParams, data: &SharedString, cx| {
                        this.set_url(index, false, data, cx);
                    });
                let key_input = params_cx
                    .new_view(|cx| TextInput::new(cx, key.to_string()).on_change(on_key_change));
                let value_input = params_cx.new_view(|cx| {
                    TextInput::new(cx, value.to_string()).on_change(on_value_change)
                });
                inputs.push((key_input, value_input));
            }
        }
        inputs
    }
}

impl Render for HttpParams {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>();
        let divider_color = theme.divider_color();
        let mut element = div().p_2().gap_1().child(
            div()
                .gap_1()
                .flex()
                .flex_row()
                .child(div().flex_1().child("Key"))
                .child(div().bg(divider_color).w(px(1.0)))
                .child(div().flex_1().child("Value")),
        );
        for (key_input, value_input) in &self.inputs {
            element = element.child(div().bg(divider_color).h(px(1.0))).child(
                div()
                    .gap_1()
                    .flex()
                    .flex_row()
                    .child(div().flex_1().child(key_input.clone()))
                    .child(div().bg(divider_color).w(px(1.0)))
                    .child(div().flex_1().child(value_input.clone())),
            )
        }
        element
    }
}

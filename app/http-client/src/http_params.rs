use crate::{
    errors::HttpClientResult,
    http_form::{HttpForm, HttpFormEvent},
};
use gpui::*;
use gpui_component::input::{Input, InputState};
use url::Url;

pub struct HttpParams {
    pub http_form: Entity<HttpForm>,
    inputs: Vec<(Entity<InputState>, Entity<InputState>)>,
    _subscriptions: Vec<Subscription>,
}

impl HttpParams {
    fn get_url(&self, cx: &mut Context<Self>) -> HttpClientResult<Url> {
        let form = self.http_form.read(cx);
        let url = form.url.as_str();
        let url = url.parse::<url::Url>()?;
        Ok(url)
    }
    fn set_url(&self, index: usize, is_key: bool, value: &str, cx: &mut Context<Self>) {
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
    pub fn new(http_form: Entity<HttpForm>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let url = http_form.read(cx);
        let url = url.url.clone();
        let _subscriptions = vec![cx.subscribe_in(&http_form, window, Self::subscribe)];
        let inputs = Self::get_inputs(&url, window, cx);
        Self {
            http_form,
            inputs,
            _subscriptions,
        }
    }

    fn subscribe(
        &mut self,
        _subscriber: &Entity<HttpForm>,
        emitter: &HttpFormEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let HttpFormEvent::SetUrl(url) = emitter {
            self.inputs = Self::get_inputs(url, window, cx);
        };
    }
    fn get_inputs(
        url: &str,
        window: &mut Window,
        params_cx: &mut Context<Self>,
    ) -> Vec<(Entity<InputState>, Entity<InputState>)> {
        let mut inputs = vec![];
        if let Ok(url) = Url::parse(url) {
            for (key, value) in url.query_pairs() {
                let key_input = params_cx.new(|cx| {
                    InputState::new(window, cx)
                        .default_value(key.to_string())
                        .placeholder("Key")
                });
                let value_input = params_cx.new(|cx| {
                    InputState::new(window, cx)
                        .default_value(value.to_string())
                        .placeholder("Value")
                });
                inputs.push((key_input, value_input));
            }
        }
        inputs
    }
}

impl Render for HttpParams {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let mut element = div().p_2().gap_1().child(
            div()
                .gap_1()
                .flex()
                .flex_row()
                .child(div().flex_1().child("Key"))
                .child(div().flex_1().child("Value")),
        );
        for (key_input, value_input) in &self.inputs {
            element = element.h(px(1.0)).child(
                div()
                    .gap_1()
                    .flex()
                    .flex_row()
                    .child(div().flex_1().child(Input::new(key_input)))
                    .child(div().flex_1().child(Input::new(value_input))),
            )
        }
        element
    }
}

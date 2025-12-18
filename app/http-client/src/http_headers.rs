use crate::http_form::{HttpForm, HttpFormEvent};
use gpui::*;
use gpui_component::{
    button::Button,
    input::{Input, InputState},
};

#[derive(Clone, Default)]
pub struct HttpHeader {
    key: String,
    value: String,
}

impl HttpHeader {
    pub fn set_value(&mut self, is_key: bool, value: String) {
        match is_key {
            true => {
                self.key = value;
            }
            false => {
                self.value = value;
            }
        }
    }
}

pub struct HttpHeadersView {
    http_form: Entity<HttpForm>,
    inputs: Vec<(Entity<InputState>, Entity<InputState>)>,
    _subscriptions: Vec<Subscription>,
}

impl HttpHeadersView {
    pub fn new(window: &mut Window, http_form: Entity<HttpForm>, cx: &mut Context<Self>) -> Self {
        let _subscriptions = vec![cx.subscribe_in(&http_form, window, Self::subscribe)];
        let headers = http_form.read(cx);
        let headers = &headers.headers;
        let inputs = Self::get_inputs(headers.clone(), window, cx);
        Self {
            http_form,
            inputs,
            _subscriptions,
        }
    }
    fn get_inputs(
        headers: Vec<HttpHeader>,
        window: &mut Window,
        header_cx: &mut Context<Self>,
    ) -> Vec<(Entity<InputState>, Entity<InputState>)> {
        let mut inputs = vec![];
        for HttpHeader { key, value } in headers.into_iter() {
            let key_input = header_cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(key)
                    .placeholder("Key")
            });
            let value_input = header_cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(value)
                    .placeholder("Value")
            });

            inputs.push((key_input, value_input));
        }
        inputs
    }
    fn subscribe(
        &mut self,
        _subscriber: &Entity<HttpForm>,
        emitter: &HttpFormEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match emitter {
            HttpFormEvent::AddHeader => {
                let key_input = cx.new(|cx| InputState::new(window, cx).placeholder("Key"));
                let value_input = cx.new(|cx| InputState::new(window, cx).placeholder("Value"));
                self.inputs.push((key_input, value_input));
            }
            HttpFormEvent::DeleteHeader(index) => {
                if *index < self.inputs.len() {
                    self.inputs.remove(*index);
                }
            }
            _ => (),
        };
    }
}

impl Render for HttpHeadersView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut element = div().p_2().gap_1().child(
            div()
                .gap_1()
                .flex()
                .flex_row()
                .child(div().flex_1().child("Key"))
                .child(div().flex_1().child("Value"))
                .child(Button::new("add_header").label("Add").on_click(cx.listener(
                    |this, _, _, cx| {
                        this.http_form
                            .update(cx, |_, cx| cx.emit(HttpFormEvent::AddHeader));
                    },
                ))),
        );
        for (index, (key_input, value_input)) in self.inputs.iter().enumerate() {
            element = element.child(
                div()
                    .gap_1()
                    .flex()
                    .flex_row()
                    .child(div().flex_1().child(Input::new(key_input)))
                    .child(div().flex_1().child(Input::new(value_input)))
                    .child(
                        Button::new("add_header")
                            .label("Detele")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.http_form.update(cx, |_, cx| {
                                    cx.emit(HttpFormEvent::DeleteHeader(index))
                                });
                            })),
                    ),
            )
        }
        element
    }
}

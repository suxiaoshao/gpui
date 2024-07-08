use components::{button, Input};
use gpui::*;
use theme::Theme;

use crate::http_form::{HttpForm, HttpFormEvent};

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
    http_form: Model<HttpForm>,
    inputs: Vec<(View<Input>, View<Input>)>,
}

impl HttpHeadersView {
    pub fn new(http_form: Model<HttpForm>, cx: &mut ViewContext<Self>) -> Self {
        cx.subscribe(&http_form, Self::subscribe).detach();
        let headers = http_form.read(cx);
        let headers = &headers.headers;
        let inputs = Self::get_inputs(headers.clone(), cx);
        Self { http_form, inputs }
    }
    fn get_inputs(
        headers: Vec<HttpHeader>,
        header_cx: &mut ViewContext<Self>,
    ) -> Vec<(View<Input>, View<Input>)> {
        let mut inputs = vec![];
        for (index, HttpHeader { key, value }) in headers.into_iter().enumerate() {
            let on_key_change =
                header_cx.listener(move |this: &mut HttpHeadersView, data: &String, cx| {
                    this.http_form.update(cx, |_data, cx| {
                        cx.emit(HttpFormEvent::SetHeaderIndex {
                            index,
                            value: data.to_string(),
                            is_key: true,
                        });
                    });
                });
            let on_value_change =
                header_cx.listener(move |this: &mut HttpHeadersView, data: &String, cx| {
                    this.http_form.update(cx, |_data, cx| {
                        cx.emit(HttpFormEvent::SetHeaderIndex {
                            index,
                            value: data.to_string(),
                            is_key: false,
                        });
                    });
                });
            let key_input = header_cx.new_view(|cx| Input::new(key, cx).on_change(on_key_change));
            let value_input =
                header_cx.new_view(|cx| Input::new(value, cx).on_change(on_value_change));
            inputs.push((key_input, value_input));
        }
        inputs
    }
    fn subscribe(
        &mut self,
        _subscriber: Model<HttpForm>,
        emitter: &HttpFormEvent,
        cx: &mut ViewContext<Self>,
    ) {
        match emitter {
            HttpFormEvent::AddHeader => {
                let key_input = cx.new_view(|cx| Input::new(Default::default(), cx));
                let value_input = cx.new_view(|cx| Input::new(Default::default(), cx));
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
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let action_width: f32 = 5.0;
        let theme = cx.global::<Theme>();
        let divider_color = theme.divider_color();
        let mut element = div().p_2().gap_1().child(
            div()
                .gap_1()
                .flex()
                .flex_row()
                .child(div().flex_1().child("Key"))
                .child(div().bg(divider_color).w(px(1.0)))
                .child(div().flex_1().child("Value"))
                .child(div().bg(divider_color).w(px(1.0)))
                .child(
                    button("add_header")
                        .w(rems(action_width))
                        .child("Add")
                        .on_click(cx.listener(|this, _, cx| {
                            this.http_form
                                .update(cx, |_, cx| cx.emit(HttpFormEvent::AddHeader));
                        })),
                ),
        );
        for (index, (key_input, value_input)) in self.inputs.iter().enumerate() {
            element = element.child(div().bg(divider_color).h(px(1.0))).child(
                div()
                    .gap_1()
                    .flex()
                    .flex_row()
                    .child(div().flex_1().child(key_input.clone()))
                    .child(div().bg(divider_color).w(px(1.0)))
                    .child(div().flex_1().child(value_input.clone()))
                    .child(div().bg(divider_color).w(px(1.0)))
                    .child(
                        button("add_header")
                            .w(rems(action_width))
                            .child("Detele")
                            .on_click(cx.listener(move |this, _, cx| {
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

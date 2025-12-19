use crate::http_form::{HttpForm, HttpFormEvent};
use gpui::*;
use gpui_component::{
    button::Button,
    input::{Input, InputState},
};

#[derive(Clone)]
pub struct HttpHeader {
    key: Entity<InputState>,
    value: Entity<InputState>,
}

impl HttpHeader {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            key: cx.new(|cx| InputState::new(window, cx).placeholder("Key")),
            value: cx.new(|cx| InputState::new(window, cx).placeholder("Value")),
        }
    }
}

pub struct HttpHeadersView {
    http_form: Entity<HttpForm>,
    _subscriptions: Vec<Subscription>,
}

impl HttpHeadersView {
    pub fn new(http_form: Entity<HttpForm>) -> Self {
        let _subscriptions = vec![];
        Self {
            http_form,
            _subscriptions,
        }
    }
}

impl Render for HttpHeadersView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let form = self.http_form.read(cx);
        let header = div()
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
            )));
        div()
            .flex()
            .flex_col()
            .p_2()
            .gap_2()
            .child(header)
            .children(form.headers.iter().enumerate().map(|(index, state)| {
                let HttpHeader { key, value } = state.read(cx);
                div()
                    .gap_1()
                    .flex()
                    .flex_row()
                    .child(div().flex_1().child(Input::new(key)))
                    .child(div().flex_1().child(Input::new(value)))
                    .child(
                        Button::new(SharedString::from(format!("delete-{index}")))
                            .label("Delete")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.http_form.update(cx, |_, cx| {
                                    cx.emit(HttpFormEvent::DeleteHeader(index))
                                });
                            })),
                    )
            }))
    }
}

use crate::http_form::{HttpForm, HttpFormEvent};
use crate::i18n::I18n;
use gpui::*;
use gpui_component::{
    button::Button,
    input::{Input, InputState},
    label::Label,
};

#[derive(Clone)]
pub struct HttpHeader {
    key: Entity<InputState>,
    value: Entity<InputState>,
}

impl HttpHeader {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let key_placeholder = cx.global::<I18n>().t("field-key");
        let value_placeholder = cx.global::<I18n>().t("field-value");
        Self {
            key: cx.new(|cx| InputState::new(window, cx).placeholder(key_placeholder)),
            value: cx.new(|cx| InputState::new(window, cx).placeholder(value_placeholder)),
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
        let (key_label, value_label, add_label, delete_label) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("field-key"),
                i18n.t("field-value"),
                i18n.t("button-add"),
                i18n.t("button-delete"),
            )
        };
        let form = self.http_form.read(cx);
        let header = div()
            .gap_2()
            .flex()
            .flex_row()
            .child(div().flex_1().child(Label::new(key_label)))
            .child(div().flex_1().child(Label::new(value_label)))
            .child(
                Button::new("add_header")
                    .label(add_label)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.http_form
                            .update(cx, |_, cx| cx.emit(HttpFormEvent::AddHeader));
                    })),
            );
        div()
            .flex()
            .flex_col()
            .p_2()
            .gap_2()
            .child(header)
            .children(form.headers.iter().enumerate().map(|(index, state)| {
                let HttpHeader { key, value } = state.read(cx);
                div()
                    .gap_2()
                    .flex()
                    .flex_row()
                    .child(div().flex_1().child(Input::new(key)))
                    .child(div().flex_1().child(Input::new(value)))
                    .child(
                        Button::new(SharedString::from(format!("delete-{index}")))
                            .label(delete_label.clone())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.http_form.update(cx, |_, cx| {
                                    cx.emit(HttpFormEvent::DeleteHeader(index))
                                });
                            })),
                    )
            }))
    }
}

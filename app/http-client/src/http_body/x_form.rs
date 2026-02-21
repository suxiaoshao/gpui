use crate::{
    http_body::{HttpBodyEvent, HttpBodyForm},
    i18n::I18n,
};
use gpui::*;
use gpui_component::{
    button::Button,
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    v_flex,
};

#[derive(Clone, Default)]
pub struct XForm {
    key: String,
    value: String,
}

impl XForm {
    pub fn set_key(&mut self, key: String) {
        self.key = key;
    }
    pub fn set_value(&mut self, value: String) {
        self.value = value;
    }
}

struct XFormItem {
    key_input: Entity<InputState>,
    _key_subscription: Subscription,
    value_input: Entity<InputState>,
    _value_subscription: Subscription,
}

impl XFormItem {
    fn new(
        index: usize,
        key: String,
        value: String,
        key_placeholder: String,
        value_placeholder: String,
        window: &mut Window,
        cx: &mut Context<XFormView>,
    ) -> Self {
        let key_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(key)
                .placeholder(key_placeholder)
        });
        let value_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(value)
                .placeholder(value_placeholder)
        });
        let _key_subscription = cx.subscribe_in(
            &key_input,
            window,
            move |this, state, evnet, window, cx| match evnet {
                InputEvent::Change => {
                    let text = state.read(cx).value();
                    this.form.update(cx, |_this, cx| {
                        cx.emit(HttpBodyEvent::SetXFormKey(index, text.to_string()));
                    });
                }
                InputEvent::PressEnter { .. } => {
                    if let Some(XFormItem { value_input, .. }) = this.items.get(index) {
                        value_input.update(cx, |this, cx| {
                            this.focus(window, cx);
                        });
                    }
                }
                _ => {}
            },
        );
        let _value_subscription = cx.subscribe_in(
            &value_input,
            window,
            move |this, state, event, window, cx| match event {
                InputEvent::Change => {
                    let text = state.read(cx).value();
                    this.form.update(cx, |_this, cx| {
                        cx.emit(HttpBodyEvent::SetXFormValue(index, text.to_string()));
                    });
                }
                InputEvent::PressEnter { .. } => {
                    if let Some(XFormItem { key_input, .. }) =
                        this.items.get(index + 1).or(this.items.first())
                    {
                        key_input.update(cx, |this, cx| {
                            this.focus(window, cx);
                        });
                    }
                }
                _ => {}
            },
        );
        Self {
            key_input,
            _key_subscription,
            value_input,
            _value_subscription,
        }
    }
}

pub(crate) struct XFormView {
    form: Entity<HttpBodyForm>,
    items: Vec<XFormItem>,
    _subscriptions: Vec<Subscription>,
}

impl XFormView {
    pub(crate) fn new(
        form: Entity<HttpBodyForm>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let items = Self::get_items(&form, window, cx);
        let _subscriptions = vec![cx.subscribe_in(&form, window, Self::subscribe_in)];

        Self {
            form,
            items,
            _subscriptions,
        }
    }
    fn get_items(
        form: &Entity<HttpBodyForm>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<XFormItem> {
        let x_form = form.read(cx).x_form.clone();
        let key_placeholder = cx.global::<I18n>().t("field-key");
        let value_placeholder = cx.global::<I18n>().t("field-value");
        x_form
            .into_iter()
            .enumerate()
            .map(|(index, XForm { key, value })| {
                XFormItem::new(
                    index,
                    key,
                    value,
                    key_placeholder.clone(),
                    value_placeholder.clone(),
                    window,
                    cx,
                )
            })
            .collect()
    }
    fn subscribe_in(
        &mut self,
        subscriber: &Entity<HttpBodyForm>,
        emitter: &HttpBodyEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match emitter {
            HttpBodyEvent::UpdateXFormByDelete => {
                self.items = Self::get_items(subscriber, window, cx);
                cx.notify();
            }
            HttpBodyEvent::AddXForm => {
                let index = self.items.len();
                let key_placeholder = cx.global::<I18n>().t("field-key");
                let value_placeholder = cx.global::<I18n>().t("field-value");
                self.items.push(XFormItem::new(
                    index,
                    String::default(),
                    String::default(),
                    key_placeholder,
                    value_placeholder,
                    window,
                    cx,
                ));
            }
            _ => {}
        }
    }
}

impl Render for XFormView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        let (key_label, value_label, add_label, delete_label) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("field-key"),
                i18n.t("field-value"),
                i18n.t("button-add"),
                i18n.t("button-delete"),
            )
        };
        let header = h_flex()
            .gap_2()
            .child(div().flex_1().child(Label::new(key_label)))
            .child(div().flex_1().child(Label::new(value_label)))
            .child(
                Button::new("add-x-form")
                    .label(add_label)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.form.update(cx, |_, cx| {
                            cx.emit(HttpBodyEvent::AddXForm);
                        })
                    })),
            );
        v_flex()
            .flex_1()
            .p_2()
            .gap_2()
            .child(header)
            .children(self.items.iter().enumerate().map(
                |(
                    index,
                    XFormItem {
                        key_input,
                        value_input,
                        ..
                    },
                )| {
                    h_flex()
                        .gap_2()
                        .child(Input::new(key_input))
                        .child(Input::new(value_input))
                        .child(
                            Button::new(SharedString::from(format!("delete-x-form-{index}")))
                                .label(delete_label.clone())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.form.update(cx, |_, cx| {
                                        cx.emit(HttpBodyEvent::DeleteXForm(index));
                                    })
                                })),
                        )
                },
            ))
    }
}

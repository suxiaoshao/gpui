use crate::{
    errors::HttpClientResult,
    http_form::{HttpForm, HttpFormEvent},
};
use gpui::*;
use gpui_component::{
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    label::Label,
    popover::Popover,
};
use url::Url;

struct ParamView {
    key: Entity<InputState>,
    _key_subscription: Subscription,
    value: Entity<InputState>,
    _value_subscription: Subscription,
}

pub struct HttpParamsView {
    pub http_form: Entity<HttpForm>,
    inputs: Vec<ParamView>,
    open_popover: bool,
    add_key_input: Entity<InputState>,
    add_value_input: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

impl HttpParamsView {
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
    fn add_params(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let key = self.add_key_input.read(cx).value();
        let value = self.add_value_input.read(cx).value();
        if let Ok(mut url) = self.get_url(cx)
            && !key.is_empty()
            && !value.is_empty()
        {
            url.query_pairs_mut().append_pair(&key, &value);
            let url = url.to_string();
            self.http_form.update(cx, move |_this, cx| {
                cx.emit(HttpFormEvent::SetUrl(url));
            });
        }
        self.add_key_input.update(cx, |this, cx| {
            this.set_value("", window, cx);
        });
        self.add_value_input.update(cx, |this, cx| {
            this.set_value("", window, cx);
        });
        self.open_popover = false;
    }
    fn delete_param(&self, skip_index: usize, cx: &mut Context<Self>) {
        if let Ok(mut url) = self.get_url(cx) {
            let mut query_pairs = url
                .query_pairs()
                .into_owned()
                .collect::<Vec<(String, String)>>();
            query_pairs.remove(skip_index);
            url.query_pairs_mut()
                .clear()
                .extend_pairs(query_pairs.iter());
            let url = url.to_string();
            self.http_form.update(cx, move |_this, cx| {
                cx.emit(HttpFormEvent::SetUrl(url));
            });
        }
    }
    pub fn new(http_form: Entity<HttpForm>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let url = http_form.read(cx);
        let url = url.url.clone();
        let _subscriptions = vec![cx.subscribe_in(&http_form, window, Self::subscribe)];
        let inputs = Self::get_inputs(&url, window, cx);
        let add_key_input = cx.new(|cx| InputState::new(window, cx).placeholder("Key"));
        let add_value_input = cx.new(|cx| InputState::new(window, cx).placeholder("Value"));
        Self {
            http_form,
            inputs,
            _subscriptions,
            add_key_input,
            add_value_input,
            open_popover: false,
        }
    }

    fn subscribe(
        &mut self,
        _subscriber: &Entity<HttpForm>,
        emitter: &HttpFormEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let HttpFormEvent::SetUrlByInput(url) | HttpFormEvent::SetUrl(url) = emitter {
            self.inputs = Self::get_inputs(url, window, cx);
            cx.notify();
        };
    }
    fn get_inputs(url: &str, window: &mut Window, params_cx: &mut Context<Self>) -> Vec<ParamView> {
        let mut inputs = vec![];
        if let Ok(url) = Url::parse(url) {
            for (index, (key, value)) in url.query_pairs().enumerate() {
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
                let key_subscription = params_cx.subscribe_in(
                    &key_input,
                    window,
                    move |this, state, event, window, cx| match event {
                        InputEvent::Change => {
                            let text = state.read(cx).value();
                            this.set_url(index, true, &text, cx);
                        }
                        InputEvent::PressEnter { .. } => {
                            if let Some(ParamView { value, .. }) = this.inputs.get(index) {
                                value.update(cx, |this, cx| {
                                    this.focus(window, cx);
                                });
                            }
                        }
                        _ => {}
                    },
                );

                let value_subscription = params_cx.subscribe_in(
                    &value_input,
                    window,
                    move |this, state, event, window, cx| match event {
                        InputEvent::Change => {
                            let text = state.read(cx).value();
                            this.set_url(index, false, &text, cx);
                        }
                        InputEvent::PressEnter { .. } => {
                            if let Some(ParamView { key, .. }) =
                                this.inputs.get(index + 1).or(this.inputs.first())
                            {
                                key.update(cx, |this, cx| {
                                    this.focus(window, cx);
                                });
                            }
                        }
                        _ => {}
                    },
                );
                inputs.push(ParamView {
                    key: key_input,
                    _key_subscription: key_subscription,
                    value: value_input,
                    _value_subscription: value_subscription,
                });
            }
        }
        inputs
    }
}

impl Render for HttpParamsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let header = div()
            .gap_2()
            .flex()
            .flex_row()
            .child(div().flex_1().child(Label::new("Key")))
            .child(div().flex_1().child(Label::new("Value")))
            .child(
                Popover::new("add-params-popover")
                    .open(self.open_popover)
                    .on_open_change(cx.listener(|this, open: &bool, _, cx| {
                        this.open_popover = *open;
                        cx.notify();
                    }))
                    .trigger(Button::new("add-params-popover-trigger").label("Add"))
                    .w(px(400.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_start()
                            .gap_2()
                            .child(Input::new(&self.add_key_input))
                            .child(Input::new(&self.add_value_input))
                            .child(
                                Button::new("add-params-popover-button")
                                    .label("Confirm")
                                    .success()
                                    .on_click(cx.listener(|this, _event, window, cx| {
                                        this.add_params(window, cx);
                                    })),
                            ),
                    ),
            );
        div()
            .flex()
            .flex_col()
            .p_2()
            .gap_2()
            .child(header)
            .children(self.inputs.iter().enumerate().map(
                |(index, ParamView { key, value, .. })| {
                    div()
                        .gap_2()
                        .flex()
                        .flex_row()
                        .child(div().flex_1().child(Input::new(key)))
                        .child(div().flex_1().child(Input::new(value)))
                        .child(
                            Button::new(SharedString::from(format!("delete-params-{index}")))
                                .label("Delete")
                                .on_click(cx.listener(move |this, _event, _window, cx| {
                                    this.delete_param(index, cx);
                                })),
                        )
                },
            ))
    }
}

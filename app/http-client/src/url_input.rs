use gpui::*;
use gpui_component::input::{Input, InputEvent, InputState};

use crate::{
    http_form::{HttpForm, HttpFormEvent},
    i18n::I18n,
};

pub struct UrlInput {
    input: Entity<InputState>,
    form: Entity<HttpForm>,
    _subscriptions: Vec<Subscription>,
}

impl UrlInput {
    pub fn new(window: &mut Window, http_form: Entity<HttpForm>, cx: &mut Context<Self>) -> Self {
        let url_placeholder = cx.global::<I18n>().t("field-url");
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value("")
                .placeholder(url_placeholder)
        });
        let input_subscription =
            cx.subscribe_in(&input, window, |this, state, event, _window, cx| {
                if let InputEvent::Change = event {
                    let text = state.read(cx).value();
                    this.form.update(cx, |_data, cx| {
                        cx.emit(HttpFormEvent::SetUrlByInput(text.to_string()))
                    });
                }
            });

        let _subscriptions = vec![
            cx.subscribe_in(&http_form, window, Self::subscribe),
            input_subscription,
        ];
        Self {
            form: http_form,
            input,
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
        if let HttpFormEvent::SetUrlByParams(url) | HttpFormEvent::SetUrl(url) = emitter {
            let url_placeholder = cx.global::<I18n>().t("field-url");
            self.input = cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(url)
                    .placeholder(url_placeholder)
            });
        };
    }
}

impl Render for UrlInput {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Input::new(&self.input)
    }
}

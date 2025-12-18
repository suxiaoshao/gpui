use crate::{
    http_headers::HttpHeader,
    http_method::{HttpMethod, SelectHttpMethod},
    http_tab::HttpTabView,
    url_input::UrlInput,
};
use gpui::*;
use gpui_component::{
    IndexPath,
    button::Button,
    select::{Select, SelectState},
};

pub enum HttpFormEvent {
    Send,
    SetUrl(String),
    SetMethod(HttpMethod),
    SetUrlByParams(String),
    AddHeader,
    DeleteHeader(usize),
    SetHeaderIndex {
        index: usize,
        value: String,
        is_key: bool,
    },
}

pub struct HttpForm {
    pub http_method: HttpMethod,
    pub url: String,
    pub headers: Vec<HttpHeader>,
}

impl HttpForm {
    fn new() -> Self {
        Self {
            http_method: HttpMethod::Get,
            url: "".to_string(),
            headers: vec![],
        }
    }
}

impl EventEmitter<HttpFormEvent> for HttpForm {}

pub struct HttpFormView {
    form: Entity<HttpForm>,
    http_method_select: Entity<SelectState<SelectHttpMethod>>,
    http_tab: Entity<HttpTabView>,
    url_input: Entity<UrlInput>,
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl HttpFormView {
    pub fn new(window: &mut Window, form_cx: &mut Context<Self>) -> Self {
        let form = form_cx.new(|_cx| HttpForm::new());
        let _subscriptions = vec![form_cx.subscribe(&form, Self::subscribe)];
        let weak_form = form.downgrade();
        Self {
            url_input: form_cx.new(|cx| UrlInput::new(window, form.clone(), cx)),
            http_tab: form_cx.new(|cx| HttpTabView::new(form.clone(), window, cx)),
            form,
            http_method_select: form_cx.new(|cx| {
                SelectState::new(
                    SelectHttpMethod::new(weak_form),
                    Some(IndexPath::default()),
                    window,
                    cx,
                )
            }),
            focus_handle: form_cx.focus_handle(),
            _subscriptions,
        }
    }
    fn subscribe(
        &mut self,
        subscriber: Entity<HttpForm>,
        emitter: &HttpFormEvent,
        cx: &mut Context<Self>,
    ) {
        match emitter {
            HttpFormEvent::Send => {
                // todo
            }
            HttpFormEvent::SetUrl(url) | HttpFormEvent::SetUrlByParams(url) => {
                subscriber.update(cx, |data, _cx| {
                    data.url.clone_from(url);
                });
            }
            HttpFormEvent::SetMethod(method) => {
                subscriber.update(cx, |data, _cx| {
                    data.http_method = *method;
                });
            }
            HttpFormEvent::AddHeader => {
                subscriber.update(cx, |data, _cx| {
                    data.headers.push(HttpHeader::default());
                });
            }
            HttpFormEvent::DeleteHeader(index) => {
                subscriber.update(cx, |data, _cx| {
                    if *index < data.headers.len() {
                        data.headers.remove(*index);
                    }
                });
            }
            HttpFormEvent::SetHeaderIndex {
                index,
                value,
                is_key,
            } => {
                subscriber.update(cx, |data, _cx| {
                    if let Some(header) = data.headers.get_mut(*index) {
                        header.set_value(*is_key, value.to_string());
                    }
                });
            }
        };
    }
}

impl Render for HttpFormView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let send_button =
            Button::new("Send")
                .label("Send")
                .on_click(cx.listener(|this, _event, _, cx| {
                    this.form.update(cx, |_, cx| {
                        cx.emit(HttpFormEvent::Send);
                    });
                }));
        let header = div()
            .flex()
            .gap_2()
            .p_2()
            .items_start()
            .text_sm()
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_1()
                    .shadow(vec![
                        BoxShadow {
                            color: hsla(0., 0., 0., 0.12),
                            offset: point(px(0.), px(2.)),
                            blur_radius: px(3.),
                            spread_radius: px(0.),
                        },
                        BoxShadow {
                            color: hsla(0., 0., 0., 0.08),
                            offset: point(px(0.), px(3.)),
                            blur_radius: px(6.),
                            spread_radius: px(0.),
                        },
                        BoxShadow {
                            color: hsla(0., 0., 0., 0.04),
                            offset: point(px(0.), px(6.)),
                            blur_radius: px(12.),
                            spread_radius: px(0.),
                        },
                    ])
                    .child(
                        div()
                            .flex_initial()
                            .child(Select::new(&self.http_method_select)),
                    )
                    .child(div().flex_1().child(self.url_input.clone())),
            )
            .child(send_button);
        div()
            .track_focus(&self.focus_handle)
            .flex()
            .flex_col()
            .size_full()
            .shadow_lg()
            .border_1()
            .child(header)
            .child(self.http_tab.clone())
    }
}

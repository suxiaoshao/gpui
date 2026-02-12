use crate::{
    http_headers::HttpHeader,
    http_method::{HttpMethod, SelectHttpMethod},
    http_tab::HttpTabView,
    i18n::I18n,
    url_input::UrlInput,
};
use gpui::*;
use gpui_component::{
    IndexPath,
    button::Button,
    select::{Select, SelectEvent, SelectState},
};

pub enum HttpFormEvent {
    Send,
    SetUrl(String),
    SetUrlByInput(String),
    SetMethod(HttpMethod),
    SetUrlByParams(String),
    AddHeader,
    DeleteHeader(usize),
}

pub struct HttpForm {
    pub http_method: HttpMethod,
    pub url: String,
    pub headers: Vec<Entity<HttpHeader>>,
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
        let http_method_select = form_cx
            .new(|cx| SelectState::new(SelectHttpMethod, Some(IndexPath::default()), window, cx));
        let _subscriptions = vec![
            form_cx.subscribe_in(&form, window, Self::subscribe),
            form_cx.subscribe_in(
                &http_method_select,
                window,
                |this, _state, event, _window, cx| {
                    if let SelectEvent::Confirm(Some(method)) = event {
                        this.form.update(cx, |_form, cx| {
                            cx.emit(HttpFormEvent::SetMethod(*method));
                        });
                    }
                },
            ),
        ];

        Self {
            url_input: form_cx.new(|cx| UrlInput::new(window, form.clone(), cx)),
            http_tab: form_cx.new(|cx| HttpTabView::new(form.clone(), window, cx)),
            form,
            http_method_select,
            focus_handle: form_cx.focus_handle(),
            _subscriptions,
        }
    }
    fn subscribe(
        &mut self,
        subscriber: &Entity<HttpForm>,
        emitter: &HttpFormEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match emitter {
            HttpFormEvent::Send => {
                // todo
            }
            HttpFormEvent::SetUrlByInput(url)
            | HttpFormEvent::SetUrlByParams(url)
            | HttpFormEvent::SetUrl(url) => {
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
                subscriber.update(cx, |data, cx| {
                    data.headers.push(cx.new(|cx| HttpHeader::new(window, cx)));
                });
            }
            HttpFormEvent::DeleteHeader(index) => {
                subscriber.update(cx, |data, _cx| {
                    if *index < data.headers.len() {
                        data.headers.remove(*index);
                    }
                });
            }
        };
    }
}

impl Render for HttpFormView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let send_label = cx.global::<I18n>().t("button-send");
        let send_button =
            Button::new("Send")
                .label(send_label)
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
                    .child(
                        div()
                            .flex_initial()
                            .child(Select::new(&self.http_method_select).w(px(100.))),
                    )
                    .child(div().flex_1().child(self.url_input.clone())),
            )
            .child(send_button);
        div()
            .track_focus(&self.focus_handle)
            .flex()
            .flex_col()
            .size_full()
            .child(header)
            .child(self.http_tab.clone())
    }
}

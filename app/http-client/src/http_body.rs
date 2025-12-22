use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, IndexPath,
    divider::Divider,
    h_flex,
    label::Label,
    select::{Select, SelectEvent, SelectItem, SelectState},
    v_flex,
};
pub use http_text::HttpText;
pub use x_form::XForm;

use crate::http_body::{
    form_data::FormDataView,
    http_text::{HttpTextView, TextType},
    x_form::XFormView,
};

mod form_data;
mod http_text;
mod x_form;

#[derive(Clone, Copy)]
pub enum BodyType {
    None,
    Text,
    XForm,
    FormData,
}

impl SelectItem for BodyType {
    type Value = BodyType;

    fn title(&self) -> SharedString {
        match self {
            BodyType::None => "None".into(),
            BodyType::Text => "text".into(),
            BodyType::XForm => "application/x-www-form-urlencoded".into(),
            BodyType::FormData => "multipart/form-data".into(),
        }
    }

    fn value(&self) -> &Self::Value {
        self
    }
}

pub struct HttpBodyForm {
    body_type: BodyType,
    text: HttpText,
    x_form: Vec<XForm>,
}

enum HttpBodyEvent {
    SetBodyType(BodyType),
    SetTextType(TextType),
    SetText(String),
    DeleteXForm(usize),
    SetXFormValue(usize, String),
    SetXFormKey(usize, String),
    AddXForm,
    UpdateXFormByDelete,
}

impl EventEmitter<HttpBodyEvent> for HttpBodyForm {}

pub struct HttpBodyView {
    form: Entity<HttpBodyForm>,
    body_type_select: Entity<SelectState<Vec<BodyType>>>,
    text_type_select: Entity<SelectState<Vec<TextType>>>,
    http_text_view: Entity<HttpTextView>,
    x_form_view: Entity<XFormView>,
    form_data_view: Entity<FormDataView>,
    _subscriptions: Vec<Subscription>,
}

impl HttpBodyView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let form = cx.new(|_cx| HttpBodyForm {
            body_type: BodyType::None,
            text: HttpText::default(),
            x_form: Vec::new(),
        });
        let body_type_select = cx.new(|cx| {
            SelectState::new(
                vec![
                    BodyType::None,
                    BodyType::Text,
                    BodyType::XForm,
                    BodyType::FormData,
                ],
                Some(IndexPath::default()),
                window,
                cx,
            )
        });
        let text_type_select = cx.new(|cx| {
            SelectState::new(
                vec![
                    TextType::Plaintext,
                    TextType::Json,
                    TextType::Html,
                    TextType::Xml,
                    TextType::Javascript,
                    TextType::Css,
                ],
                Some(IndexPath::default()),
                window,
                cx,
            )
        });
        let http_text_view = cx.new(|cx| HttpTextView::new(form.clone(), window, cx));
        let x_form_view = cx.new(|cx| XFormView::new(form.clone(), window, cx));
        let form_data_view = cx.new(|_cx| FormDataView::new());
        let _subscriptions = vec![
            cx.subscribe_in(
                &body_type_select,
                window,
                |this, _state, event, _window, cx| {
                    if let SelectEvent::Confirm(Some(body_type)) = event {
                        this.form.update(cx, |_form, cx| {
                            cx.emit(HttpBodyEvent::SetBodyType(*body_type));
                        });
                    }
                },
            ),
            cx.subscribe_in(
                &text_type_select,
                window,
                |this, _state, event, _window, cx| {
                    if let SelectEvent::Confirm(Some(text_type)) = event {
                        this.form.update(cx, |_form, cx| {
                            cx.emit(HttpBodyEvent::SetTextType(*text_type));
                        });
                    }
                },
            ),
            cx.subscribe_in(&form, window, Self::subcription_in),
        ];

        Self {
            form,
            body_type_select,
            text_type_select,
            http_text_view,
            x_form_view,
            form_data_view,
            _subscriptions,
        }
    }
    fn subcription_in(
        &mut self,
        subscriber: &Entity<HttpBodyForm>,
        emitter: &HttpBodyEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match emitter {
            HttpBodyEvent::SetBodyType(body_type) => {
                subscriber.update(cx, |form, _cx| {
                    form.body_type = *body_type;
                });
            }
            HttpBodyEvent::SetTextType(text_type) => {
                subscriber.update(cx, |form, _cx| {
                    form.text.text_type = *text_type;
                });
            }
            HttpBodyEvent::SetText(text) => {
                subscriber.update(cx, |form, _cx| {
                    form.text.text = text.clone();
                });
            }
            HttpBodyEvent::DeleteXForm(index) => {
                subscriber.update(cx, |form, cx| {
                    form.x_form.remove(*index);
                    cx.emit(HttpBodyEvent::UpdateXFormByDelete);
                });
            }
            HttpBodyEvent::SetXFormValue(index, value) => {
                subscriber.update(cx, |form, _cx| {
                    if let Some(x_form) = form.x_form.get_mut(*index) {
                        x_form.set_value(value.clone());
                    }
                });
            }
            HttpBodyEvent::SetXFormKey(index, key) => {
                subscriber.update(cx, |form, _cx| {
                    if let Some(x_form) = form.x_form.get_mut(*index) {
                        x_form.set_key(key.clone());
                    }
                });
            }
            HttpBodyEvent::AddXForm => {
                subscriber.update(cx, |form, _cx| {
                    form.x_form.push(XForm::default());
                });
            }
            HttpBodyEvent::UpdateXFormByDelete => {}
        }
    }
}

impl Render for HttpBodyView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        let body_type = self.form.read(cx).body_type;
        let header = h_flex()
            .items_center()
            .p_2()
            .gap_2()
            .child(Label::new("Content-type").text_color(cx.theme().muted_foreground))
            .child(
                div()
                    .flex_initial()
                    .child(Select::new(&self.body_type_select).w(px(280.))),
            )
            .when(matches!(body_type, BodyType::Text), |this| {
                this.child(
                    div()
                        .flex_initial()
                        .child(Select::new(&self.text_type_select).w(px(200.))),
                )
            });
        v_flex()
            .flex_1()
            .child(header)
            .child(Divider::horizontal())
            .child(match body_type {
                BodyType::None => div().into_any_element(),
                BodyType::Text => self.http_text_view.clone().into_any_element(),
                BodyType::XForm => self.x_form_view.clone().into_any_element(),
                BodyType::FormData => self.form_data_view.clone().into_any_element(),
            })
    }
}

use components::{Button, Input, Select};
use gpui::*;

use crate::http_method::{HttpMethod, SelectHttpMethod};

pub struct HttpForm {
    http_method: HttpMethod,
    url: String,
}

impl EventEmitter<()> for HttpForm {}

pub struct HttpFormView {
    form: Model<HttpForm>,
    http_method_select: View<Select<SelectHttpMethod>>,
    url_input: View<Input>,
    focus_handle: FocusHandle,
}

impl HttpFormView {
    pub fn new(cx: &mut ViewContext<Self>) -> Self {
        Self {
            form: cx.new_model(|_cx| HttpForm {
                http_method: HttpMethod::Get,
                url: "".to_string(),
            }),
            http_method_select: cx.new_view(|cx| Select::new(SelectHttpMethod::default(), cx)),
            url_input: cx.new_view(|cx| Input::new("".to_string(), "url_input", cx)),
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Render for HttpFormView {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let theme = cx.global::<theme::Theme>();
        div()
            .track_focus(&self.focus_handle)
            .flex()
            .flex_col()
            .bg(theme.bg_color())
            .size_full()
            .justify_center()
            .items_center()
            .shadow_lg()
            .border_1()
            .text_xl()
            .text_color(theme.text_color())
            .gap_2()
            .child(self.url_input.clone())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .child(self.http_method_select.clone()),
            )
    }
}

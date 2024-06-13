use components::{Input, Select};
use gpui::*;
use smallvec::smallvec;
use theme::ElevationColor;

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
            .p_2()
            .bg(theme.bg_color())
            .size_full()
            .shadow_lg()
            .border_1()
            .text_color(theme.text_color())
            .gap_2()
            .child(
                div()
                    .flex()
                    .rounded_md()
                    .shadow(smallvec![
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
                    .border(px(0.5))
                    .border_color(theme.divider_color())
                    .bg(theme.bg_color().elevation_color(1.0))
                    .child(self.http_method_select.clone())
                    .child(div().my_1().w(px(1.0)).bg(theme.divider_color()))
                    .child(
                        div()
                            .flex_1()
                            .overflow_x_hidden()
                            .child(self.url_input.clone()),
                    ),
            )
            .child(div().flex_1())
    }
}

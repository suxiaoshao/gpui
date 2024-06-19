use components::{button, Input, Select, Tab};
use gpui::*;
use smallvec::smallvec;
use theme::ElevationColor;

use crate::{
    http_method::{HttpMethod, SelectHttpMethod},
    http_tab::HttpTabView,
};

pub enum HttpFormEvent {
    Send,
}

pub struct HttpForm {
    http_method: HttpMethod,
    url: String,
}

impl EventEmitter<HttpFormEvent> for HttpForm {}

pub struct HttpFormView {
    form: Model<HttpForm>,
    http_method_select: View<Select<SelectHttpMethod>>,
    http_tab: View<Tab<HttpTabView>>,
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
            http_tab: cx.new_view(|_cx| Tab::new(HttpTabView::new())),
        }
    }
}

impl Render for HttpFormView {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let theme = cx.global::<theme::Theme>();
        let send_button = button("Send")
            .child("Send")
            .bg(theme.button_bg_color())
            .text_color(theme.button_color())
            .rounded_md()
            .on_click(|_event, _cx| {});
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
                    .bg(theme.bg_color().elevation_color(3.0))
                    .child(self.http_method_select.clone())
                    .child(
                        div()
                            .h(DefiniteLength::Fraction(0.8))
                            .w(px(1.0))
                            .bg(theme.divider_color()),
                    )
                    .child(div().flex_1().child(self.url_input.clone())),
            )
            .child(send_button);
        div()
            .track_focus(&self.focus_handle)
            .flex()
            .flex_col()
            .bg(theme.bg_color())
            .size_full()
            .shadow_lg()
            .border_1()
            .text_color(theme.text_color())
            .child(header)
            .child(self.http_tab.clone())
    }
}

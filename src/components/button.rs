/*
 * @Author: suxiaoshao suxiaoshao@gmail.com
 * @Date: 2024-05-31 01:20:04
 * @LastEditors: suxiaoshao suxiaoshao@gmail.com
 * @LastEditTime: 2024-05-31 03:42:24
 * @FilePath: /gpui-app/src/components/button.rs
 */
use gpui::*;
use prelude::FluentBuilder;

use crate::theme::Theme;

type OnClick = Box<dyn Fn(&ClickEvent, &mut WindowContext) + 'static>;

#[derive(IntoElement)]
pub struct Button {
    text: String,
    id: ElementId,
    on_click: Option<OnClick>,
}

impl Button {
    pub fn new(text: impl Into<String>, id: impl Into<ElementId>) -> Self {
        Self {
            text: text.into(),
            id: id.into(),
            on_click: None,
        }
    }

    pub fn on_click(
        mut self,
        on_click: impl Fn(&ClickEvent, &mut WindowContext) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(on_click));
        self
    }
}

impl RenderOnce for Button {
    fn render(self, cx: &mut WindowContext) -> impl IntoElement {
        let theme = cx.global::<Theme>();
        div()
            .id(self.id)
            .bg(theme.button_bg_color())
            .text_color(theme.button_color())
            .p_1()
            .px_4()
            .rounded_xl()
            .child(self.text)
            .when_some(self.on_click, |this, on_click| {
                this.on_click(move |event, cx| {
                    cx.stop_propagation();
                    (on_click)(event, cx)
                })
            })
            .hover(|style| {
                style
                    .bg(theme.button_hover_bg_color())
                    .text_color(theme.button_hover_color())
            })
            .active(|style| {
                style
                    .bg(theme.button_active_bg_color())
                    .text_color(theme.button_active_color())
            })
    }
}

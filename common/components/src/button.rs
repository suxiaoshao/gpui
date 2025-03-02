use gpui::*;
use prelude::FluentBuilder;

use theme::Theme;

type OnClick = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct Button {
    label: String,
    id: ElementId,
    on_click: Option<OnClick>,
}

impl Button {
    pub fn new(text: impl Into<String>, id: impl Into<ElementId>) -> Self {
        Self {
            label: text.into(),
            id: id.into(),
            on_click: None,
        }
    }

    pub fn on_click(
        mut self,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(on_click));
        self
    }
}

impl RenderOnce for Button {
    fn render(self, _cx: &mut Window, app: &mut App) -> impl IntoElement {
        let theme = app.global::<Theme>();
        div()
            .id(self.id)
            .bg(theme.button_bg_color())
            .text_color(theme.button_color())
            .p_1()
            .px_4()
            .rounded_xl()
            .child(self.label)
            .when_some(self.on_click, |this, on_click| {
                this.on_click(move |event, window, app| {
                    app.stop_propagation();
                    (on_click)(event, window, app)
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

pub fn button(id: impl Into<ElementId>) -> Stateful<Div> {
    div().id(id.into()).p_1().px_2().cursor_pointer()
}

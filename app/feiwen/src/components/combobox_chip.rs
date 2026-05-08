use std::rc::Rc;

use gpui::{
    App, ClickEvent, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _, px,
};
use gpui_component::{
    ActiveTheme, IconName, Sizable, StyledExt as _,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
};

type OnRemove = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub(crate) struct ComboboxChip {
    id: ElementId,
    remove_id: ElementId,
    label: SharedString,
    style: StyleRefinement,
    on_remove: Option<OnRemove>,
}

impl ComboboxChip {
    pub(crate) fn new(
        id: impl Into<ElementId>,
        remove_id: impl Into<ElementId>,
        label: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            remove_id: remove_id.into(),
            label: label.into(),
            style: StyleRefinement::default(),
            on_remove: None,
        }
    }

    pub(crate) fn on_remove(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_remove = Some(Rc::new(handler));
        self
    }
}

impl Styled for ComboboxChip {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ComboboxChip {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let remove_id = self.remove_id;
        let label = self.label;
        let label_for_render = label.clone();

        h_flex()
            .id(self.id)
            .h(px(21.))
            .flex_none()
            .items_center()
            .justify_center()
            .gap_1()
            .rounded(px(4.))
            .bg(cx.theme().muted)
            .px_1p5()
            .text_xs()
            .font_medium()
            .text_color(cx.theme().foreground)
            .whitespace_nowrap()
            .refine_style(&self.style)
            .child(
                div()
                    .flex_none()
                    .child(Label::new(label_for_render).text_xs()),
            )
            .when_some(self.on_remove, |this, on_remove| {
                this.pr_0().child(
                    div().flex_none().child(
                        Button::new(remove_id)
                            .ghost()
                            .xsmall()
                            .ml(px(-4.))
                            .opacity(0.5)
                            .icon(IconName::Close)
                            .tooltip("移除已选项")
                            .on_click(move |event, window, cx| {
                                on_remove(event, window, cx);
                            }),
                    ),
                )
            })
    }
}

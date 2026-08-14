use crate::foundation::assets::IconName as FeiwenIconName;
use gpui::{Context, IntoElement, ParentElement, Render, SharedString, Styled, Window, px};
use gpui_component::{ActiveTheme, Icon, IconName, StyledExt, h_flex, label::Label};
use gpui_form::PathKey;

#[derive(Clone)]
pub(super) struct DragSortRow {
    pub(super) row_id: PathKey,
    priority: usize,
    field_label: SharedString,
    direction_label: SharedString,
    has_error: bool,
}

impl DragSortRow {
    pub(super) fn new(
        row_id: PathKey,
        priority: usize,
        field_label: impl Into<SharedString>,
        direction_label: impl Into<SharedString>,
        has_error: bool,
    ) -> Self {
        Self {
            row_id,
            priority,
            field_label: field_label.into(),
            direction_label: direction_label.into(),
            has_error,
        }
    }
}

impl Render for DragSortRow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .gap_2()
            .items_center()
            .px_3()
            .py_2()
            .min_w(px(360.))
            .border_1()
            .border_color(if self.has_error {
                cx.theme().danger
            } else {
                cx.theme().drag_border
            })
            .rounded_sm()
            .bg(cx.theme().tokens.background.background)
            .shadow_sm()
            .child(Icon::new(IconName::EllipsisVertical))
            .child(
                Label::new(format!("{}", self.priority))
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(
                Label::new(self.field_label.clone())
                    .text_sm()
                    .font_medium()
                    .min_w(px(140.)),
            )
            .child(
                Label::new(self.direction_label.clone())
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(Icon::new(FeiwenIconName::Trash).invisible().ml_auto())
    }
}

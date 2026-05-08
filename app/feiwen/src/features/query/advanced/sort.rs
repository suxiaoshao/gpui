use super::state::SortRow;
use crate::foundation::assets::IconName as FeiwenIconName;
use gpui::{Context, IntoElement, ParentElement, Render, SharedString, Styled, Window, px};
use gpui_component::{ActiveTheme, Icon, IconName, StyledExt, h_flex, label::Label};

pub(super) trait SortRowId {
    fn sort_row_id(&self) -> u64;
}

impl SortRowId for SortRow {
    fn sort_row_id(&self) -> u64 {
        self.id
    }
}

pub(super) fn move_sort_before<T: SortRowId>(rows: &mut Vec<T>, source_id: u64, target_id: u64) {
    if source_id == target_id {
        return;
    }
    let Some(source_index) = rows.iter().position(|row| row.sort_row_id() == source_id) else {
        return;
    };
    let source = rows.remove(source_index);
    let Some(target_index) = rows.iter().position(|row| row.sort_row_id() == target_id) else {
        rows.insert(source_index.min(rows.len()), source);
        return;
    };
    rows.insert(target_index, source);
}

#[derive(Clone)]
pub(super) struct DragSortRow {
    pub(super) row_id: u64,
    priority: usize,
    field_label: SharedString,
    direction_label: SharedString,
    has_error: bool,
}

impl DragSortRow {
    pub(super) fn new(
        row_id: u64,
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
            .bg(cx.theme().background)
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

#[cfg(test)]
mod tests {
    use super::*;

    struct Row(u64);

    impl SortRowId for Row {
        fn sort_row_id(&self) -> u64 {
            self.0
        }
    }

    fn ids(rows: &[Row]) -> Vec<u64> {
        rows.iter().map(|row| row.0).collect()
    }

    #[test]
    fn move_sort_before_reorders_middle_to_front() {
        let mut rows = vec![Row(1), Row(2), Row(3)];
        move_sort_before(&mut rows, 3, 1);
        assert_eq!(ids(&rows), [3, 1, 2]);
    }

    #[test]
    fn move_sort_before_reorders_front_to_middle() {
        let mut rows = vec![Row(1), Row(2), Row(3)];
        move_sort_before(&mut rows, 1, 3);
        assert_eq!(ids(&rows), [2, 1, 3]);
    }

    #[test]
    fn move_sort_before_same_row_is_noop() {
        let mut rows = vec![Row(1), Row(2), Row(3)];
        move_sort_before(&mut rows, 2, 2);
        assert_eq!(ids(&rows), [1, 2, 3]);
    }
}

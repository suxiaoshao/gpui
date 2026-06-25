use std::rc::Rc;

use crate::foundation::assets::IconName;
use gpui::{
    AnyElement, App, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, Styled as _, Window, px,
};
use gpui_component::{
    ActiveTheme, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    input::Input,
    label::Label,
    v_flex,
};

use super::form_state::{KeyValueDraftRow, StringListDraftRow};

type AddRowHandler = Rc<dyn Fn(&mut Window, &mut App) + 'static>;
type RemoveRowHandler = Rc<dyn Fn(u64, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub(super) struct StringListFieldView {
    field_id: &'static str,
    label: SharedString,
    rows: Vec<StringListDraftRow>,
    add_label: SharedString,
    remove_label: SharedString,
    on_add: AddRowHandler,
    on_remove: RemoveRowHandler,
}

impl StringListFieldView {
    pub(super) fn on_add(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_add = Rc::new(handler);
        self
    }

    pub(super) fn on_remove(
        mut self,
        handler: impl Fn(u64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_remove = Rc::new(handler);
        self
    }
}

impl RenderOnce for StringListFieldView {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let add_handler = self.on_add;
        let remove_handler = self.on_remove;
        v_flex()
            .w_full()
            .gap_2()
            .child(Label::new(self.label).text_sm().font_medium())
            .children(self.rows.into_iter().map(|row| {
                let remove_handler = remove_handler.clone();
                h_flex()
                    .id(format!("{}-row-{}", self.field_id, row.id))
                    .w_full()
                    .items_center()
                    .gap_2()
                    .child(Input::new(&row.input).w_full().flex_1())
                    .child(
                        Button::new(format!("{}-remove-{}", self.field_id, row.id))
                            .icon(IconName::Trash)
                            .ghost()
                            .tooltip(self.remove_label.clone())
                            .on_click(move |_, window, cx| {
                                remove_handler(row.id, window, cx);
                            }),
                    )
            }))
            .child(
                Button::new(format!("{}-add", self.field_id))
                    .icon(IconName::Plus)
                    .label(self.add_label.clone())
                    .w_full()
                    .on_click(move |_, window, cx| {
                        add_handler(window, cx);
                    }),
            )
    }
}

#[derive(IntoElement)]
pub(super) struct KeyValueListFieldView {
    field_id: &'static str,
    label: SharedString,
    rows: Vec<KeyValueDraftRow>,
    add_label: SharedString,
    remove_label: SharedString,
    on_add: AddRowHandler,
    on_remove: RemoveRowHandler,
}

impl KeyValueListFieldView {
    pub(super) fn on_add(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_add = Rc::new(handler);
        self
    }

    pub(super) fn on_remove(
        mut self,
        handler: impl Fn(u64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_remove = Rc::new(handler);
        self
    }
}

impl RenderOnce for KeyValueListFieldView {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let add_handler = self.on_add;
        let remove_handler = self.on_remove;
        v_flex()
            .w_full()
            .gap_2()
            .child(Label::new(self.label).text_sm().font_medium())
            .children(self.rows.into_iter().map(|row| {
                let remove_handler = remove_handler.clone();
                h_flex()
                    .id(format!("{}-row-{}", self.field_id, row.id))
                    .w_full()
                    .items_center()
                    .gap_2()
                    .child(Input::new(&row.key_input).w_full().flex_1())
                    .child(Input::new(&row.value_input).w_full().flex_1())
                    .child(
                        Button::new(format!("{}-remove-{}", self.field_id, row.id))
                            .icon(IconName::Trash)
                            .ghost()
                            .tooltip(self.remove_label.clone())
                            .on_click(move |_, window, cx| {
                                remove_handler(row.id, window, cx);
                            }),
                    )
            }))
            .child(
                Button::new(format!("{}-add", self.field_id))
                    .icon(IconName::Plus)
                    .label(self.add_label.clone())
                    .w_full()
                    .on_click(move |_, window, cx| {
                        add_handler(window, cx);
                    }),
            )
    }
}

pub(super) fn render_string_list_field(
    field_id: &'static str,
    label: impl Into<SharedString>,
    rows: Vec<StringListDraftRow>,
    add_label: impl Into<SharedString>,
    remove_label: impl Into<SharedString>,
) -> StringListFieldView {
    StringListFieldView {
        field_id,
        label: label.into(),
        rows,
        add_label: add_label.into(),
        remove_label: remove_label.into(),
        on_add: Rc::new(|_, _| {}),
        on_remove: Rc::new(|_, _, _| {}),
    }
}

pub(super) fn render_key_value_list_field(
    field_id: &'static str,
    label: impl Into<SharedString>,
    rows: Vec<KeyValueDraftRow>,
    add_label: impl Into<SharedString>,
    remove_label: impl Into<SharedString>,
) -> KeyValueListFieldView {
    KeyValueListFieldView {
        field_id,
        label: label.into(),
        rows,
        add_label: add_label.into(),
        remove_label: remove_label.into(),
        on_add: Rc::new(|_, _| {}),
        on_remove: Rc::new(|_, _, _| {}),
    }
}

pub(super) fn validation_error_list(messages: Vec<SharedString>, cx: &mut App) -> AnyElement {
    v_flex()
        .w_full()
        .gap_1()
        .children(messages.into_iter().map(|message| {
            Label::new(message)
                .text_xs()
                .line_height(px(16.))
                .text_color(cx.theme().danger)
        }))
        .into_any_element()
}

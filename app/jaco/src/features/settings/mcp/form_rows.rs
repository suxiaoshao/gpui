use crate::foundation::assets::IconName;
use gpui::{
    Action as _, AnyElement, App, ElementId, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, SharedString, Styled as _, Window, prelude::FluentBuilder as _, px,
};
use gpui_component::{
    ActiveTheme, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    label::Label,
    v_flex,
};
use gpui_form::PathKey;
use serde::Deserialize;
use std::{rc::Rc, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub(super) enum McpRowList {
    Args,
    Env,
    EnvVars,
    Headers,
    EnvHeaders,
}

#[derive(gpui::Action, Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[action(namespace = jaco_mcp_dialog, no_json)]
pub(super) struct AddMcpRow {
    pub(super) list: McpRowList,
}

pub(super) type RemoveRowHandler = Rc<dyn Fn(&mut Window, &mut App)>;
pub(super) type MoveRowHandler = Rc<dyn Fn(&mut Window, &mut App)>;

pub(super) struct RowMoveHandlers {
    pub(super) up: Option<MoveRowHandler>,
    pub(super) down: Option<MoveRowHandler>,
}

pub(super) fn one_input_rows(
    field_id: &'static str,
    label: impl Into<SharedString>,
    rows: impl IntoIterator<
        Item = (
            PathKey,
            Entity<InputState>,
            Vec<SharedString>,
            RowMoveHandlers,
            RemoveRowHandler,
        ),
    >,
    list: McpRowList,
    add_label: impl Into<SharedString>,
    remove_label: impl Into<SharedString>,
    cx: &mut App,
) -> AnyElement {
    let add_label = add_label.into();
    let remove_label = remove_label.into();

    row_container(label)
        .children(
            rows.into_iter()
                .map(|(row_id, input, errors, moves, remove)| {
                    row_with_errors(
                        row_shell(field_id, &row_id)
                            .child(Input::new(&input).w_full().flex_1())
                            .children(move_buttons(field_id, &row_id, moves, cx))
                            .child(remove_button(
                                field_id,
                                row_id,
                                remove_label.clone(),
                                remove,
                            )),
                        errors,
                        cx,
                    )
                }),
        )
        .child(add_button(field_id, list, add_label))
        .into_any_element()
}

pub(super) fn two_input_rows(
    field_id: &'static str,
    label: impl Into<SharedString>,
    rows: impl IntoIterator<
        Item = (
            PathKey,
            Entity<InputState>,
            Vec<SharedString>,
            Entity<InputState>,
            Vec<SharedString>,
            RowMoveHandlers,
            RemoveRowHandler,
        ),
    >,
    list: McpRowList,
    add_label: impl Into<SharedString>,
    remove_label: impl Into<SharedString>,
    cx: &mut App,
) -> AnyElement {
    let add_label = add_label.into();
    let remove_label = remove_label.into();

    row_container(label)
        .children(rows.into_iter().map(
            |(row_id, first_input, first_errors, second_input, second_errors, moves, remove)| {
                row_shell(field_id, &row_id)
                    .child(input_with_errors(first_input, first_errors, cx))
                    .child(input_with_errors(second_input, second_errors, cx))
                    .children(move_buttons(field_id, &row_id, moves, cx))
                    .child(remove_button(
                        field_id,
                        row_id,
                        remove_label.clone(),
                        remove,
                    ))
                    .into_any_element()
            },
        ))
        .child(add_button(field_id, list, add_label))
        .into_any_element()
}

fn move_buttons(
    field_id: &'static str,
    row_id: &PathKey,
    moves: RowMoveHandlers,
    cx: &mut App,
) -> Vec<Button> {
    let mut buttons = Vec::with_capacity(2);
    if let Some(move_up) = moves.up {
        buttons.push(
            Button::new(row_element_id(field_id, row_id, "move-up"))
                .icon(IconName::ChevronUp)
                .ghost()
                .tooltip(cx.global::<crate::foundation::I18n>().t("button-move-up"))
                .on_click(move |_, window, cx| move_up(window, cx)),
        );
    }
    if let Some(move_down) = moves.down {
        buttons.push(
            Button::new(row_element_id(field_id, row_id, "move-down"))
                .icon(IconName::ChevronDown)
                .ghost()
                .tooltip(cx.global::<crate::foundation::I18n>().t("button-move-down"))
                .on_click(move |_, window, cx| move_down(window, cx)),
        );
    }
    buttons
}

fn input_with_errors(
    input: Entity<InputState>,
    errors: Vec<SharedString>,
    cx: &mut App,
) -> AnyElement {
    v_flex()
        .w_full()
        .flex_1()
        .gap_1()
        .child(Input::new(&input).w_full())
        .when(!errors.is_empty(), |this| {
            this.child(validation_error_list(errors, cx))
        })
        .into_any_element()
}

fn row_container(label: impl Into<SharedString>) -> gpui::Div {
    v_flex()
        .w_full()
        .gap_2()
        .child(Label::new(label.into()).text_sm().font_medium())
}

fn add_button(field_id: &'static str, list: McpRowList, add_label: SharedString) -> Button {
    Button::new(format!("{field_id}-add"))
        .icon(IconName::Plus)
        .label(add_label)
        .w_full()
        .on_click(move |_, window, cx| {
            window.dispatch_action(AddMcpRow { list }.boxed_clone(), cx);
        })
}

fn row_shell(field_id: &'static str, row_id: &PathKey) -> gpui::Stateful<gpui::Div> {
    h_flex()
        .id(row_element_id(field_id, row_id, "row"))
        .w_full()
        .items_center()
        .gap_2()
}

fn row_with_errors(
    row: gpui::Stateful<gpui::Div>,
    errors: Vec<SharedString>,
    cx: &mut App,
) -> AnyElement {
    v_flex()
        .w_full()
        .gap_1()
        .child(row)
        .when(!errors.is_empty(), |this| {
            this.child(validation_error_list(errors, cx))
        })
        .into_any_element()
}

fn remove_button(
    field_id: &'static str,
    row_id: PathKey,
    remove_label: SharedString,
    remove: RemoveRowHandler,
) -> Button {
    Button::new(row_element_id(field_id, &row_id, "remove"))
        .icon(IconName::Trash)
        .ghost()
        .tooltip(remove_label)
        .on_click(move |_, window, cx| remove(window, cx))
}

fn row_element_id(field_id: &'static str, row_id: &PathKey, role: &'static str) -> ElementId {
    ElementId::NamedChild(
        Arc::new(ElementId::from(row_id)),
        format!("{field_id}-{role}").into(),
    )
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

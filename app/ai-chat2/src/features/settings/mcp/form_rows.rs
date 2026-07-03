use crate::foundation::assets::IconName;
use gpui::{
    Action as _, AnyElement, App, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, Styled as _, prelude::FluentBuilder as _, px,
};
use gpui_component::{
    ActiveTheme, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    label::Label,
    v_flex,
};
use gpui_form::FormItemId;
use serde::Deserialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub(super) enum McpRowList {
    Args,
    Env,
    EnvVars,
    Headers,
    EnvHeaders,
}

#[derive(gpui::Action, Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[action(namespace = ai_chat2_mcp_dialog, no_json)]
pub(super) struct AddMcpRow {
    pub(super) list: McpRowList,
}

#[derive(gpui::Action, Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[action(namespace = ai_chat2_mcp_dialog, no_json)]
pub(super) struct RemoveMcpRow {
    pub(super) list: McpRowList,
    pub(super) row_id: u64,
}

pub(super) fn one_input_rows(
    field_id: &'static str,
    label: impl Into<SharedString>,
    rows: impl IntoIterator<Item = (FormItemId, Entity<InputState>, Vec<SharedString>)>,
    list: McpRowList,
    add_label: impl Into<SharedString>,
    remove_label: impl Into<SharedString>,
    cx: &mut App,
) -> AnyElement {
    let add_label = add_label.into();
    let remove_label = remove_label.into();

    row_container(label)
        .children(rows.into_iter().map(|(row_id, input, errors)| {
            row_with_errors(
                row_shell(field_id, row_id)
                    .child(Input::new(&input).w_full().flex_1())
                    .child(remove_button(field_id, row_id, list, remove_label.clone())),
                errors,
                cx,
            )
        }))
        .child(add_button(field_id, list, add_label))
        .into_any_element()
}

pub(super) fn two_input_rows(
    field_id: &'static str,
    label: impl Into<SharedString>,
    rows: impl IntoIterator<
        Item = (
            FormItemId,
            Entity<InputState>,
            Entity<InputState>,
            Vec<SharedString>,
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
                .map(|(row_id, first_input, second_input, errors)| {
                    row_with_errors(
                        row_shell(field_id, row_id)
                            .child(Input::new(&first_input).w_full().flex_1())
                            .child(Input::new(&second_input).w_full().flex_1())
                            .child(remove_button(field_id, row_id, list, remove_label.clone())),
                        errors,
                        cx,
                    )
                }),
        )
        .child(add_button(field_id, list, add_label))
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

fn row_shell(field_id: &'static str, row_id: FormItemId) -> gpui::Stateful<gpui::Div> {
    h_flex()
        .id(format!("{field_id}-row-{row_id}"))
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
    row_id: FormItemId,
    list: McpRowList,
    remove_label: SharedString,
) -> Button {
    Button::new(format!("{field_id}-remove-{row_id}"))
        .icon(IconName::Trash)
        .ghost()
        .tooltip(remove_label)
        .on_click(move |_, window, cx| {
            window.dispatch_action(
                RemoveMcpRow {
                    list,
                    row_id: row_id.get(),
                }
                .boxed_clone(),
                cx,
            );
        })
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

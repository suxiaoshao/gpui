use crate::{
    i18n::I18n,
    store::{AddConversationMessage, ChatData, ChatDataEvent},
};
use gpui::*;
use gpui_component::{
    WindowExt,
    button::{Button, ButtonVariants},
    form::{field, v_form},
    input::{Input, InputState},
};
use std::ops::Deref;
use tracing::{Level, event};

pub fn add_conversation_dialog(parent_id: Option<i32>, window: &mut Window, cx: &mut App) {
    add_conversation_dialog_with_messages(parent_id, None, window, cx);
}

pub fn add_conversation_dialog_with_messages(
    parent_id: Option<i32>,
    initial_messages: Option<Vec<AddConversationMessage>>,
    window: &mut Window,
    cx: &mut App,
) {
    let span = tracing::info_span!("add_conversation action");
    let _enter = span.enter();
    event!(Level::INFO, "add_conversation action");

    let (
        dialog_title,
        name_label,
        icon_label,
        info_label,
        cancel_label,
        submit_label,
    ) = {
        let i18n = cx.global::<I18n>();
        (
            i18n.t("dialog-add-conversation-title"),
            i18n.t("field-name"),
            i18n.t("field-icon"),
            i18n.t("field-info"),
            i18n.t("button-cancel"),
            i18n.t("button-submit"),
        )
    };

    let name_input = cx.new(|cx| InputState::new(window, cx).placeholder(name_label.clone()));
    let icon_input = cx.new(|cx| InputState::new(window, cx).placeholder(icon_label.clone()));
    let info_input = cx.new(|cx| InputState::new(window, cx).placeholder(info_label.clone()));
    window.open_dialog(cx, move |dialog, _, _| {
        dialog
            .title(dialog_title.clone())
            .child(
                v_form()
                    .child(
                        field()
                            .required(true)
                            .label(name_label.clone())
                            .child(Input::new(&name_input)),
                    )
                    .child(
                        field()
                            .required(true)
                            .label(icon_label.clone())
                            .child(Input::new(&icon_input)),
                    )
                    .child(
                        field()
                            .label(info_label.clone())
                            .child(Input::new(&info_input)),
                    ),
            )
            .footer({
                let name_input = name_input.clone();
                let icon_input = icon_input.clone();
                let info_input = info_input.clone();
                let cancel_label = cancel_label.clone();
                let submit_label = submit_label.clone();
                let initial_messages = initial_messages.clone();
                move |_this, _state, _window, _cx| {
                    vec![
                        Button::new("cancel").label(cancel_label.clone()).on_click(
                            |_, window, cx| {
                                window.close_dialog(cx);
                            },
                        ),
                        Button::new("ok")
                            .primary()
                            .label(submit_label.clone())
                            .on_click({
                                let name_input = name_input.clone();
                                let icon_input = icon_input.clone();
                                let info_input = info_input.clone();
                                let initial_messages = initial_messages.clone();
                                move |_, window, cx| {
                                    let name = name_input.read(cx).value();
                                    let icon = icon_input.read(cx).value();
                                    let info = info_input.read(cx).value();
                                    if !name.is_empty() {
                                        let chat_data = cx.global::<ChatData>().deref().clone();
                                        let initial_messages = initial_messages.clone();
                                        chat_data.update(cx, move |_this, cx| {
                                            cx.emit(ChatDataEvent::AddConversation {
                                                name,
                                                icon,
                                                info: if info.is_empty() {
                                                    None
                                                } else {
                                                    Some(info)
                                                },
                                                parent_id,
                                                initial_messages: initial_messages.clone(),
                                            });
                                        });
                                    }
                                    window.close_dialog(cx);
                                }
                            }),
                    ]
                }
            })
    });
}

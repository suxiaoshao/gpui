use crate::{
    i18n::I18n,
    state::{AddConversationMessage, ChatData, ChatDataEvent},
};
use gpui::*;
use gpui_component::{
    WindowExt,
    button::{Button, ButtonVariants},
    dialog::{DialogAction, DialogClose, DialogFooter},
    form::{field, v_form},
    input::{Input, InputState},
};
use std::ops::Deref;
use tracing::{Level, event};

#[derive(Clone)]
enum ConversationDialogMode {
    Add {
        parent_id: Option<i32>,
        initial_messages: Option<Vec<AddConversationMessage>>,
    },
    Edit {
        conversation_id: i32,
    },
}

fn open_conversation_dialog(mode: ConversationDialogMode, window: &mut Window, cx: &mut App) {
    let span = tracing::info_span!("conversation_dialog action");
    let _enter = span.enter();

    let is_edit = matches!(mode, ConversationDialogMode::Edit { .. });
    let (dialog_title, name_label, icon_label, info_label, cancel_label, submit_label) = {
        let i18n = cx.global::<I18n>();
        if is_edit {
            (
                i18n.t("dialog-edit-conversation-title"),
                i18n.t("field-name"),
                i18n.t("field-icon"),
                i18n.t("field-info"),
                i18n.t("button-cancel"),
                i18n.t("button-submit"),
            )
        } else {
            (
                i18n.t("dialog-add-conversation-title"),
                i18n.t("field-name"),
                i18n.t("field-icon"),
                i18n.t("field-info"),
                i18n.t("button-cancel"),
                i18n.t("button-submit"),
            )
        }
    };

    let name_input = cx.new(|cx| InputState::new(window, cx).placeholder(name_label.clone()));
    let icon_input = cx.new(|cx| InputState::new(window, cx).placeholder(icon_label.clone()));
    let info_input = cx.new(|cx| InputState::new(window, cx).placeholder(info_label.clone()));

    if let ConversationDialogMode::Edit { conversation_id } = &mode {
        let chat_data = cx.global::<ChatData>();
        let Ok(chat_data) = chat_data.read(cx).as_ref() else {
            event!(
                Level::ERROR,
                "Failed to read chat data for conversation edit dialog"
            );
            return;
        };
        let Some(conversation) = chat_data.conversation(*conversation_id) else {
            event!(
                Level::ERROR,
                "Conversation {conversation_id} not found in chat data"
            );
            return;
        };
        let title = conversation.title.clone();
        let icon = conversation.icon.clone();
        let info = conversation.info.clone();
        name_input.update(cx, |input, _cx| {
            input.set_value(title.clone(), window, _cx);
        });
        icon_input.update(cx, |input, _cx| {
            input.set_value(icon.clone(), window, _cx);
        });
        if let Some(info) = info {
            info_input.update(cx, |input, _cx| {
                input.set_value(info, window, _cx);
            });
        }
    }

    window.open_dialog(cx, move |dialog, _window, _cx| {
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
            .footer(
                DialogFooter::new()
                    .child(
                        DialogClose::new().child(Button::new("cancel").label(cancel_label.clone())),
                    )
                    .child(
                        DialogAction::new().child(
                            Button::new("ok")
                                .primary()
                                .label(submit_label.clone())
                                .on_click({
                                    let name_input = name_input.clone();
                                    let icon_input = icon_input.clone();
                                    let info_input = info_input.clone();
                                    let mode = mode.clone();
                                    move |_, window, cx| {
                                        let name = name_input.read(cx).value();
                                        let icon = icon_input.read(cx).value();
                                        let info = info_input.read(cx).value();
                                        if !name.is_empty() {
                                            let chat_data = cx.global::<ChatData>().deref().clone();
                                            let mode = mode.clone();
                                            chat_data.update(cx, move |_this, cx| {
                                                let info =
                                                    if info.is_empty() { None } else { Some(info) };
                                                match mode {
                                                    ConversationDialogMode::Add {
                                                        parent_id,
                                                        initial_messages,
                                                    } => cx.emit(ChatDataEvent::AddConversation {
                                                        name,
                                                        icon,
                                                        info,
                                                        parent_id,
                                                        initial_messages,
                                                    }),
                                                    ConversationDialogMode::Edit {
                                                        conversation_id,
                                                    } => {
                                                        cx.emit(ChatDataEvent::UpdateConversation {
                                                            id: conversation_id,
                                                            title: name,
                                                            icon,
                                                            info,
                                                        })
                                                    }
                                                }
                                            });
                                        }
                                        window.close_dialog(cx);
                                    }
                                }),
                        ),
                    ),
            )
    });
}

pub fn open_add_conversation_dialog(
    parent_id: Option<i32>,
    initial_messages: Option<Vec<AddConversationMessage>>,
    window: &mut Window,
    cx: &mut App,
) {
    open_conversation_dialog(
        ConversationDialogMode::Add {
            parent_id,
            initial_messages,
        },
        window,
        cx,
    );
}

pub fn open_edit_conversation_dialog(conversation_id: i32, window: &mut Window, cx: &mut App) {
    open_conversation_dialog(ConversationDialogMode::Edit { conversation_id }, window, cx);
}

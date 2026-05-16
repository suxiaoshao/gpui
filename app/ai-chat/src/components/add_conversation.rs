use crate::{
    foundation::assets::IconName,
    foundation::i18n::I18n,
    state::{AddConversationMessage, ChatData, ChatDataEvent},
};
use gpui::*;
use gpui_component::{
    WindowExt,
    button::{Button, ButtonVariants},
    dialog::{DialogClose, DialogFooter},
    form::{field, v_form},
    input::{Input, InputState},
    notification::{Notification, NotificationType},
};
use std::{ops::Deref, rc::Rc};
use tracing::{Level, event};

#[derive(Clone, Debug, Default)]
pub(crate) struct InitialConversationFields {
    pub(crate) name: Option<String>,
    pub(crate) icon: Option<String>,
    pub(crate) info: Option<String>,
}

#[derive(Clone)]
enum ConversationDialogMode {
    Add {
        parent_id: Option<i32>,
        initial_fields: InitialConversationFields,
        initial_messages: Option<Vec<AddConversationMessage>>,
        options: AddConversationDialogOptions,
    },
    Edit {
        conversation_id: i32,
    },
}

#[derive(Clone)]
struct AddConversationDialogOptions {
    title: SharedString,
    failure_title: SharedString,
    success_title: Option<SharedString>,
    on_submit: AddConversationSubmit,
}

pub(crate) struct AddConversationDialogRequest {
    pub(crate) parent_id: Option<i32>,
    pub(crate) initial_fields: InitialConversationFields,
    pub(crate) initial_messages: Option<Vec<AddConversationMessage>>,
    pub(crate) title: SharedString,
    pub(crate) failure_title: SharedString,
    pub(crate) success_title: Option<SharedString>,
    pub(crate) on_submit: AddConversationSubmit,
}

pub(crate) type AddConversationSubmit = Rc<
    dyn Fn(
        ConversationSubmission,
        Option<i32>,
        Option<Vec<AddConversationMessage>>,
        &mut Window,
        &mut App,
    ) -> bool,
>;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ConversationSubmission {
    pub(crate) name: String,
    pub(crate) icon: String,
    pub(crate) info: Option<String>,
}

fn build_conversation_submission(
    name: &str,
    icon: &str,
    info: &str,
    required_message: &str,
) -> Result<ConversationSubmission, String> {
    let name = name.trim().to_string();
    let icon = icon.trim().to_string();
    if name.is_empty() || icon.is_empty() {
        return Err(required_message.to_string());
    }

    let info = info.trim().to_string();
    Ok(ConversationSubmission {
        name,
        icon,
        info: if info.is_empty() { None } else { Some(info) },
    })
}

fn open_conversation_dialog(mode: ConversationDialogMode, window: &mut Window, cx: &mut App) {
    let span = tracing::info_span!("conversation_dialog action");
    let _enter = span.enter();

    let is_edit = matches!(mode, ConversationDialogMode::Edit { .. });
    let (name_label, icon_label, info_label, cancel_label, submit_label, required_message) = {
        let i18n = cx.global::<I18n>();
        (
            i18n.t("field-name"),
            i18n.t("field-icon"),
            i18n.t("field-info"),
            i18n.t("button-cancel"),
            i18n.t("button-submit"),
            i18n.t("conversation-error-name-icon-required"),
        )
    };
    let (dialog_title, failure_title, success_title) = match &mode {
        ConversationDialogMode::Add { options, .. } => (
            options.title.clone(),
            options.failure_title.clone(),
            options.success_title.clone(),
        ),
        ConversationDialogMode::Edit { .. } => {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("dialog-edit-conversation-title").into(),
                i18n.t("notify-update-conversation-failed").into(),
                None,
            )
        }
    };
    let submit_icon = if is_edit {
        IconName::Save
    } else {
        IconName::Upload
    };

    let name_input = cx.new(|cx| InputState::new(window, cx).placeholder(name_label.clone()));
    let icon_input = cx.new(|cx| InputState::new(window, cx).placeholder(icon_label.clone()));
    let info_input = cx.new(|cx| InputState::new(window, cx).placeholder(info_label.clone()));

    match &mode {
        ConversationDialogMode::Add { initial_fields, .. } => {
            if let Some(name) = initial_fields.name.clone() {
                name_input.update(cx, |input, cx| {
                    input.set_value(name, window, cx);
                });
            }
            if let Some(icon) = initial_fields.icon.clone() {
                icon_input.update(cx, |input, cx| {
                    input.set_value(icon, window, cx);
                });
            }
            if let Some(info) = initial_fields.info.clone() {
                info_input.update(cx, |input, cx| {
                    input.set_value(info, window, cx);
                });
            }
        }
        ConversationDialogMode::Edit { conversation_id } => {
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
            name_input.update(cx, |input, cx| {
                input.set_value(title.clone(), window, cx);
            });
            icon_input.update(cx, |input, cx| {
                input.set_value(icon.clone(), window, cx);
            });
            if let Some(info) = info {
                info_input.update(cx, |input, cx| {
                    input.set_value(info, window, cx);
                });
            }
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
                        Button::new("ok")
                            .primary()
                            .icon(submit_icon)
                            .label(submit_label.clone())
                            .on_click({
                                let name_input = name_input.clone();
                                let icon_input = icon_input.clone();
                                let info_input = info_input.clone();
                                let mode = mode.clone();
                                let failure_title = failure_title.clone();
                                let success_title = success_title.clone();
                                let required_message = required_message.clone();
                                move |_, window, cx| {
                                    let name = name_input.read(cx).value();
                                    let icon = icon_input.read(cx).value();
                                    let info = info_input.read(cx).value();
                                    let submission = match build_conversation_submission(
                                        name.as_ref(),
                                        icon.as_ref(),
                                        info.as_ref(),
                                        required_message.as_str(),
                                    ) {
                                        Ok(submission) => submission,
                                        Err(message) => {
                                            window.push_notification(
                                                Notification::new()
                                                    .title(failure_title.clone())
                                                    .message(message)
                                                    .with_type(NotificationType::Error),
                                                cx,
                                            );
                                            return;
                                        }
                                    };
                                    let mode = mode.clone();
                                    let submitted = match mode {
                                        ConversationDialogMode::Add {
                                            parent_id,
                                            initial_fields: _,
                                            initial_messages,
                                            options,
                                        } => options.on_submit.as_ref()(
                                            submission,
                                            parent_id,
                                            initial_messages,
                                            window,
                                            cx,
                                        ),
                                        ConversationDialogMode::Edit { conversation_id } => {
                                            cx.global::<ChatData>().deref().clone().update(
                                                cx,
                                                move |_this, cx| {
                                                    cx.emit(ChatDataEvent::UpdateConversation {
                                                        id: conversation_id,
                                                        title: SharedString::from(submission.name),
                                                        icon: SharedString::from(submission.icon),
                                                        info: submission
                                                            .info
                                                            .map(SharedString::from),
                                                    });
                                                },
                                            );
                                            true
                                        }
                                    };
                                    if submitted {
                                        window.close_dialog(cx);
                                        if let Some(success_title) = success_title.clone() {
                                            window.push_notification(
                                                Notification::new()
                                                    .title(success_title)
                                                    .with_type(NotificationType::Success),
                                                cx,
                                            );
                                        }
                                    }
                                }
                            }),
                    ),
            )
    });
}

fn default_add_conversation_options(cx: &mut App) -> AddConversationDialogOptions {
    let i18n = cx.global::<I18n>();
    AddConversationDialogOptions {
        title: i18n.t("dialog-add-conversation-title").into(),
        failure_title: i18n.t("notify-add-conversation-failed").into(),
        success_title: None,
        on_submit: Rc::new(|submission, parent_id, initial_messages, _window, cx| {
            let name = SharedString::from(submission.name);
            let icon = SharedString::from(submission.icon);
            let info = submission.info.map(SharedString::from);
            let chat_data = cx.global::<ChatData>().deref().clone();
            chat_data.update(cx, move |_this, cx| {
                cx.emit(ChatDataEvent::AddConversation {
                    name,
                    icon,
                    info,
                    parent_id,
                    initial_messages,
                });
            });
            true
        }),
    }
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
            initial_fields: InitialConversationFields::default(),
            initial_messages,
            options: default_add_conversation_options(cx),
        },
        window,
        cx,
    );
}

pub fn open_add_conversation_dialog_with_fields(
    parent_id: Option<i32>,
    initial_fields: InitialConversationFields,
    initial_messages: Option<Vec<AddConversationMessage>>,
    window: &mut Window,
    cx: &mut App,
) {
    open_conversation_dialog(
        ConversationDialogMode::Add {
            parent_id,
            initial_fields,
            initial_messages,
            options: default_add_conversation_options(cx),
        },
        window,
        cx,
    );
}

pub(crate) fn open_add_conversation_dialog_with_options(
    request: AddConversationDialogRequest,
    window: &mut Window,
    cx: &mut App,
) {
    open_conversation_dialog(
        ConversationDialogMode::Add {
            parent_id: request.parent_id,
            initial_fields: request.initial_fields,
            initial_messages: request.initial_messages,
            options: AddConversationDialogOptions {
                title: request.title,
                failure_title: request.failure_title,
                success_title: request.success_title,
                on_submit: request.on_submit,
            },
        },
        window,
        cx,
    );
}

pub fn open_edit_conversation_dialog(conversation_id: i32, window: &mut Window, cx: &mut App) {
    open_conversation_dialog(ConversationDialogMode::Edit { conversation_id }, window, cx);
}

#[cfg(test)]
mod tests {
    use super::build_conversation_submission;

    fn err(result: Result<super::ConversationSubmission, String>) -> String {
        match result {
            Ok(_) => panic!("expected conversation submission validation to fail"),
            Err(err) => err,
        }
    }

    #[test]
    fn submission_requires_name() {
        let err = err(build_conversation_submission(
            "",
            "🤖",
            "",
            "name and icon required",
        ));

        assert_eq!(err, "name and icon required");
    }

    #[test]
    fn submission_requires_icon() {
        let err = err(build_conversation_submission(
            "QA Chat",
            "",
            "",
            "name and icon required",
        ));

        assert_eq!(err, "name and icon required");
    }

    #[test]
    fn submission_rejects_blank_name_and_icon() {
        let err = err(build_conversation_submission(
            " \t ",
            " \n ",
            "",
            "name and icon required",
        ));

        assert_eq!(err, "name and icon required");
    }

    #[test]
    fn submission_maps_blank_info_to_none() {
        let submission =
            build_conversation_submission("QA Chat", "Q", " \n ", "name and icon required")
                .unwrap();

        assert_eq!(submission.info, None);
    }

    #[test]
    fn submission_trims_values() {
        let submission = build_conversation_submission(
            "  QA Chat  ",
            "  Q  ",
            "  QA description  ",
            "name and icon required",
        )
        .unwrap();

        assert_eq!(submission.name, "QA Chat");
        assert_eq!(submission.icon, "Q");
        assert_eq!(submission.info.as_deref(), Some("QA description"));
    }
}

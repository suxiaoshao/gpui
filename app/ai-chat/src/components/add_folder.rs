use gpui::*;
use gpui_component::{
    WindowExt,
    button::{Button, ButtonVariants},
    dialog::{DialogClose, DialogFooter},
    form::{field, v_form},
    input::{Input, InputState},
    notification::{Notification, NotificationType},
};
use std::ops::Deref;
use tracing::{Level, event};

use crate::{
    foundation::assets::IconName,
    foundation::i18n::I18n,
    state::{ChatData, ChatDataEvent},
};

#[derive(Clone, Copy)]
enum FolderDialogMode {
    Add { parent_id: Option<i32> },
    Edit { folder_id: i32 },
}

#[derive(Debug, PartialEq, Eq)]
struct FolderSubmission {
    name: String,
}

fn build_folder_submission(name: &str, required_message: &str) -> Result<FolderSubmission, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(required_message.to_string());
    }
    Ok(FolderSubmission { name })
}

fn open_folder_dialog(mode: FolderDialogMode, window: &mut Window, cx: &mut App) {
    let span = tracing::info_span!("folder_dialog action");
    let _enter = span.enter();

    let is_edit = matches!(mode, FolderDialogMode::Edit { .. });
    let (name_label, dialog_title, cancel_label, submit_label, failure_title, required_message) = {
        let i18n = cx.global::<I18n>();
        if is_edit {
            (
                i18n.t("field-name"),
                i18n.t("dialog-edit-folder-title"),
                i18n.t("button-cancel"),
                i18n.t("button-submit"),
                i18n.t("notify-update-folder-failed"),
                i18n.t("folder-error-name-required"),
            )
        } else {
            (
                i18n.t("field-name"),
                i18n.t("dialog-add-folder-title"),
                i18n.t("button-cancel"),
                i18n.t("button-submit"),
                i18n.t("notify-add-folder-failed"),
                i18n.t("folder-error-name-required"),
            )
        }
    };
    let submit_icon = if is_edit {
        IconName::Save
    } else {
        IconName::Upload
    };

    let folder_input = cx.new(|cx| InputState::new(window, cx).placeholder(name_label.clone()));

    if let FolderDialogMode::Edit { folder_id } = &mode {
        let chat_data = cx.global::<ChatData>();
        let Ok(chat_data) = chat_data.read(cx).as_ref() else {
            event!(
                Level::ERROR,
                "Failed to read chat data for folder edit dialog"
            );
            return;
        };
        let Some(folder) = chat_data.folder(*folder_id) else {
            event!(Level::ERROR, "Folder {folder_id} not found in chat data");
            return;
        };
        let name = folder.name.clone();
        folder_input.update(cx, |input, _cx| {
            input.set_value(name.clone(), window, _cx);
        });
    }

    window.open_dialog(cx, move |dialog, _window, _cx| {
        dialog
            .title(dialog_title.clone())
            .child(
                v_form().child(
                    field()
                        .required(true)
                        .label(name_label.clone())
                        .child(Input::new(&folder_input)),
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
                                let folder_input = folder_input.clone();
                                let mode = mode;
                                let failure_title = failure_title.clone();
                                let required_message = required_message.clone();
                                move |_, window, cx| {
                                    let name = folder_input.read(cx).value();
                                    let submission = match build_folder_submission(
                                        name.as_ref(),
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
                                    let name = SharedString::from(submission.name);
                                    let chat_data = cx.global::<ChatData>().deref().clone();
                                    chat_data.update(cx, move |_this, cx| match mode {
                                        FolderDialogMode::Edit { folder_id } => {
                                            cx.emit(ChatDataEvent::UpdateFolder {
                                                id: folder_id,
                                                name,
                                            });
                                        }
                                        FolderDialogMode::Add { parent_id } => {
                                            cx.emit(ChatDataEvent::AddFolder { name, parent_id });
                                        }
                                    });
                                    window.close_dialog(cx);
                                }
                            }),
                    ),
            )
    });
}

pub fn open_add_folder_dialog(parent_id: Option<i32>, window: &mut Window, cx: &mut App) {
    open_folder_dialog(FolderDialogMode::Add { parent_id }, window, cx);
}

pub fn open_edit_folder_dialog(folder_id: i32, window: &mut Window, cx: &mut App) {
    open_folder_dialog(FolderDialogMode::Edit { folder_id }, window, cx);
}

#[cfg(test)]
mod tests {
    use super::build_folder_submission;

    fn err(result: Result<super::FolderSubmission, String>) -> String {
        match result {
            Ok(_) => panic!("expected folder submission validation to fail"),
            Err(err) => err,
        }
    }

    #[test]
    fn submission_requires_name() {
        let err = err(build_folder_submission("", "folder name required"));

        assert_eq!(err, "folder name required");
    }

    #[test]
    fn submission_rejects_blank_name() {
        let err = err(build_folder_submission(" \t\n ", "folder name required"));

        assert_eq!(err, "folder name required");
    }

    #[test]
    fn submission_trims_name() {
        let submission = build_folder_submission("  QA Folder  ", "folder name required").unwrap();

        assert_eq!(submission.name, "QA Folder");
    }
}

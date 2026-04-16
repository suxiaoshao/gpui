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

use crate::{
    i18n::I18n,
    state::{ChatData, ChatDataEvent},
};

#[derive(Clone, Copy)]
enum FolderDialogMode {
    Add { parent_id: Option<i32> },
    Edit { folder_id: i32 },
}

fn open_folder_dialog(mode: FolderDialogMode, window: &mut Window, cx: &mut App) {
    let span = tracing::info_span!("folder_dialog action");
    let _enter = span.enter();

    let is_edit = matches!(mode, FolderDialogMode::Edit { .. });
    let (name_label, dialog_title, cancel_label, submit_label) = {
        let i18n = cx.global::<I18n>();
        if is_edit {
            (
                i18n.t("field-name"),
                i18n.t("dialog-edit-folder-title"),
                i18n.t("button-cancel"),
                i18n.t("button-submit"),
            )
        } else {
            (
                i18n.t("field-name"),
                i18n.t("dialog-add-folder-title"),
                i18n.t("button-cancel"),
                i18n.t("button-submit"),
            )
        }
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
                    .child(DialogClose::new().child(Button::new("cancel").label(cancel_label.clone())))
                    .child(
                        DialogAction::new().child(
                            Button::new("ok")
                                .primary()
                                .label(submit_label.clone())
                                .on_click({
                                    let folder_input = folder_input.clone();
                                    let mode = mode;
                                    move |_, window, cx| {
                                        let name = folder_input.read(cx).value();
                                        if !name.is_empty() {
                                            let chat_data = cx.global::<ChatData>().deref().clone();
                                            chat_data.update(cx, move |_this, cx| match mode {
                                                FolderDialogMode::Edit { folder_id } => {
                                                    cx.emit(ChatDataEvent::UpdateFolder {
                                                        id: folder_id,
                                                        name,
                                                    });
                                                }
                                                FolderDialogMode::Add { parent_id } => {
                                                    cx.emit(ChatDataEvent::AddFolder {
                                                        name,
                                                        parent_id,
                                                    });
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

pub fn open_add_folder_dialog(parent_id: Option<i32>, window: &mut Window, cx: &mut App) {
    open_folder_dialog(FolderDialogMode::Add { parent_id }, window, cx);
}

pub fn open_edit_folder_dialog(folder_id: i32, window: &mut Window, cx: &mut App) {
    open_folder_dialog(FolderDialogMode::Edit { folder_id }, window, cx);
}

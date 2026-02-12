use gpui::*;
use gpui_component::{
    WindowExt,
    button::{Button, ButtonVariants},
    form::{field, v_form},
    input::{Input, InputState},
};
use std::ops::Deref;
use tracing::{Level, event};

use crate::{
    i18n::I18n,
    store::{ChatData, ChatDataEvent},
};

pub fn add_folder_dialog(parent_id: Option<i32>, window: &mut Window, cx: &mut App) {
    let span = tracing::info_span!("add_folder action");
    let _enter = span.enter();
    event!(Level::INFO, "add_folder action");
    let (name_label, dialog_title, cancel_label, submit_label) = {
        let i18n = cx.global::<I18n>();
        (
            i18n.t("field-name"),
            i18n.t("dialog-add-folder-title"),
            i18n.t("button-cancel"),
            i18n.t("button-submit"),
        )
    };
    let folder_input = cx.new(|cx| InputState::new(window, cx).placeholder(name_label.clone()));
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
            .footer({
                let folder_input = folder_input.clone();
                let cancel_label = cancel_label.clone();
                let submit_label = submit_label.clone();
                move |_this, _state, _window, _cx| {
                    vec![
                        Button::new("cancel")
                            .label(cancel_label.clone())
                            .on_click(|_, window, cx| {
                                window.close_dialog(cx);
                            }),
                        Button::new("ok").primary().label(submit_label.clone()).on_click({
                            let folder_input = folder_input.clone();
                            move |_, window, cx| {
                                let name = folder_input.read(cx).value();
                                if !name.is_empty() {
                                    let chat_data = cx.global::<ChatData>().deref().clone();
                                    chat_data.update(cx, |_this, cx| {
                                        cx.emit(ChatDataEvent::AddFolder { name, parent_id });
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

use gpui::*;
use gpui_component::{
    WindowExt,
    button::{Button, ButtonVariants},
    form::{field, v_form},
    input::{Input, InputState},
};
use std::ops::Deref;
use tracing::{Level, event};

use crate::store::{ChatData, ChatDataEvent};

pub fn add_folder_dialog(parent_id: Option<i32>, window: &mut Window, cx: &mut App) {
    let span = tracing::info_span!("add_folder action");
    let _enter = span.enter();
    event!(Level::INFO, "add_folder action");
    let folder_input = cx.new(|cx| InputState::new(window, cx).placeholder("Name"));
    window.open_dialog(cx, move |dialog, _window, _cx| {
        dialog
            .title("Add Folder")
            .child(
                v_form().child(
                    field()
                        .required(true)
                        .label("Name")
                        .child(Input::new(&folder_input)),
                ),
            )
            .footer({
                let folder_input = folder_input.clone();
                move |_this, _state, _window, _cx| {
                    vec![
                        Button::new("cancel")
                            .label("Cancel")
                            .on_click(|_, window, cx| {
                                window.close_dialog(cx);
                            }),
                        Button::new("ok").primary().label("Submit").on_click({
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

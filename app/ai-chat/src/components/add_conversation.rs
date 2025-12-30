use crate::{
    database::{ConversationTemplate, Db},
    errors::AiChatResult,
    store::{ChatData, ChatDataEvent},
};
use gpui::*;
use gpui_component::{
    IndexPath, WindowExt,
    button::{Button, ButtonVariants},
    form::{field, v_form},
    input::{Input, InputState},
    notification::{Notification, NotificationType},
    select::{Select, SelectState},
};
use std::ops::Deref;
use tracing::{Level, event};

pub fn add_conversation_dialog(parent_id: Option<i32>, window: &mut Window, cx: &mut App) {
    let span = tracing::info_span!("add_conversation action");
    let _enter = span.enter();
    event!(Level::INFO, "add_conversation action");

    fn get_templates(cx: &mut App) -> AiChatResult<Vec<ConversationTemplate>> {
        let conn = &mut cx.global::<Db>().get()?;
        ConversationTemplate::all(conn)
    }

    let templates = match get_templates(cx) {
        Ok(data) => data,
        Err(err) => {
            event!(Level::INFO, "get templates error:{err}");
            window.push_notification(
                Notification::new()
                    .title("Get Templates Failed")
                    .message(SharedString::from(err.to_string()))
                    .with_type(NotificationType::Error),
                cx,
            );
            return;
        }
    };

    let name_input = cx.new(|cx| InputState::new(window, cx).placeholder("Name"));
    let icon_input = cx.new(|cx| InputState::new(window, cx).placeholder("Icon"));
    let info_input = cx.new(|cx| InputState::new(window, cx).placeholder("Info"));
    let template_input =
        cx.new(|cx| SelectState::new(templates, Some(IndexPath::default()), window, cx));
    window.open_dialog(cx, move |dialog, _, _| {
        dialog
            .title("Add Conversation")
            .child(
                v_form()
                    .child(
                        field()
                            .required(true)
                            .label("Name")
                            .child(Input::new(&name_input)),
                    )
                    .child(
                        field()
                            .required(true)
                            .label("Icon")
                            .child(Input::new(&icon_input)),
                    )
                    .child(field().label("Info").child(Input::new(&info_input)))
                    .child(
                        field()
                            .required(true)
                            .label("Template")
                            .child(Select::new(&template_input)),
                    ),
            )
            .footer({
                let name_input = name_input.clone();
                let icon_input = icon_input.clone();
                let info_input = info_input.clone();
                let template_input = template_input.clone();
                move |_this, _state, _window, _cx| {
                    vec![
                        Button::new("cancel")
                            .label("Cancel")
                            .on_click(|_, window, cx| {
                                window.close_dialog(cx);
                            }),
                        Button::new("ok").primary().label("Submit").on_click({
                            let name_input = name_input.clone();
                            let icon_input = icon_input.clone();
                            let info_input = info_input.clone();
                            let template_input = template_input.clone();
                            move |_, window, cx| {
                                let name = name_input.read(cx).value();
                                let icon = icon_input.read(cx).value();
                                let info = info_input.read(cx).value();
                                let template = match template_input.read(cx).selected_value() {
                                    Some(data) => *data,
                                    None => {
                                        window.push_notification(
                                            Notification::new()
                                                .title("Please select a template".to_string())
                                                .with_type(NotificationType::Error),
                                            cx,
                                        );
                                        return;
                                    }
                                };
                                if !name.is_empty() {
                                    let chat_data = cx.global::<ChatData>().deref().clone();
                                    chat_data.update(cx, move |_this, cx| {
                                        cx.emit(ChatDataEvent::AddConversation {
                                            name,
                                            icon,
                                            info: if info.is_empty() { None } else { Some(info) },
                                            template,
                                            parent_id,
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

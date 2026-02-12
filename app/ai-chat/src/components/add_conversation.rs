use crate::{
    database::{ConversationTemplate, Db},
    errors::AiChatResult,
    i18n::I18n,
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

    let (
        get_templates_failed_title,
        dialog_title,
        select_template_title,
        name_label,
        icon_label,
        info_label,
        template_label,
        cancel_label,
        submit_label,
    ) = {
        let i18n = cx.global::<I18n>();
        (
            i18n.t("notify-get-templates-failed"),
            i18n.t("dialog-add-conversation-title"),
            i18n.t("notify-select-template"),
            i18n.t("field-name"),
            i18n.t("field-icon"),
            i18n.t("field-info"),
            i18n.t("field-template"),
            i18n.t("button-cancel"),
            i18n.t("button-submit"),
        )
    };
    let templates = match get_templates(cx) {
        Ok(data) => data,
        Err(err) => {
            event!(Level::INFO, "get templates error:{err}");
            window.push_notification(
                Notification::new()
                    .title(get_templates_failed_title)
                    .message(SharedString::from(err.to_string()))
                    .with_type(NotificationType::Error),
                cx,
            );
            return;
        }
    };

    let name_input = cx.new(|cx| InputState::new(window, cx).placeholder(name_label.clone()));
    let icon_input = cx.new(|cx| InputState::new(window, cx).placeholder(icon_label.clone()));
    let info_input = cx.new(|cx| InputState::new(window, cx).placeholder(info_label.clone()));
    let template_input =
        cx.new(|cx| SelectState::new(templates, Some(IndexPath::default()), window, cx));
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
                    .child(field().label(info_label.clone()).child(Input::new(&info_input)))
                    .child(
                        field()
                            .required(true)
                            .label(template_label.clone())
                            .child(Select::new(&template_input)),
                    ),
            )
            .footer({
                let name_input = name_input.clone();
                let icon_input = icon_input.clone();
                let info_input = info_input.clone();
                let template_input = template_input.clone();
                let cancel_label = cancel_label.clone();
                let submit_label = submit_label.clone();
                let select_template_title = select_template_title.clone();
                move |_this, _state, _window, _cx| {
                    vec![
                        Button::new("cancel")
                            .label(cancel_label.clone())
                            .on_click(|_, window, cx| {
                                window.close_dialog(cx);
                            }),
                        Button::new("ok").primary().label(submit_label.clone()).on_click({
                            let name_input = name_input.clone();
                            let icon_input = icon_input.clone();
                            let info_input = info_input.clone();
                            let template_input = template_input.clone();
                            let select_template_title = select_template_title.clone();
                            move |_, window, cx| {
                                let name = name_input.read(cx).value();
                                let icon = icon_input.read(cx).value();
                                let info = info_input.read(cx).value();
                                let template = match template_input.read(cx).selected_value() {
                                    Some(data) => *data,
                                    None => {
                                        window.push_notification(
                                            Notification::new()
                                                .title(select_template_title.clone())
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

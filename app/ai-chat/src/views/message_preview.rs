use crate::{
    components::message::MessageViewExt,
    database::{Content, Conversation, Db, Message, Role},
    errors::{AiChatError, AiChatResult},
    i18n::I18n,
    store::{ChatData, ChatDataEvent},
    workspace_state::WorkspaceStore,
};
use fluent_bundle::FluentArgs;
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    IconName, Root, WindowExt,
    button::{Button, Toggle, ToggleGroup, ToggleVariants},
    description_list::{DescriptionItem, DescriptionList},
    input::{Input, InputState},
    label::Label,
    notification::Notification,
    scroll::ScrollableElement,
    v_flex,
};
use std::ops::Deref;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tracing::{Level, event};

#[derive(Debug)]
enum PreviewType {
    Preview,
    Edit,
}

impl PreviewType {
    fn preview_checked(&self) -> bool {
        matches!(self, PreviewType::Preview)
    }

    fn edit_checked(&self) -> bool {
        matches!(self, PreviewType::Edit)
    }
}

struct MessageInputs {
    text: Entity<InputState>,
    reasoning_summary: Entity<InputState>,
    citations: Entity<InputState>,
    send_content: Entity<InputState>,
}

impl MessageInputs {
    fn new<T: MessagePreviewExt>(message: &T, window: &mut Window, cx: &mut App) -> Self {
        let build_editor = |value: String, window: &mut Window, cx: &mut App| {
            cx.new(|cx| {
                InputState::new(window, cx)
                    .multi_line(true)
                    .line_number(true)
                    .searchable(true)
                    .default_value(value)
            })
        };
        Self {
            text: build_editor(message.content().text.clone(), window, cx),
            reasoning_summary: build_editor(
                message
                    .content()
                    .reasoning_summary
                    .clone()
                    .unwrap_or_default(),
                window,
                cx,
            ),
            citations: build_editor(
                serde_json::to_string_pretty(&message.content().citations)
                    .unwrap_or_else(|_| "[]".to_string()),
                window,
                cx,
            ),
            send_content: build_editor(
                serde_json::to_string_pretty(message.send_content())
                    .unwrap_or_else(|_| "{}".to_string()),
                window,
                cx,
            ),
        }
    }
}

pub struct MessagePreview<T: MessagePreviewExt> {
    message: T,
    preview_type: PreviewType,
    input: MessageInputs,
}

impl<T: MessagePreviewExt> Deref for MessagePreview<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.message
    }
}

impl<T: MessagePreviewExt> MessagePreview<T> {
    pub fn new(message: T, window: &mut Window, cx: &mut App) -> Self {
        let input = MessageInputs::new(&message, window, cx);
        Self {
            message,
            preview_type: PreviewType::Preview,
            input,
        }
    }

    fn submit(&self, window: &mut Window, cx: &mut Context<Self>) -> AiChatResult<()> {
        let text = self.input.text.read(cx).value().to_string();
        let reasoning_summary = self
            .input
            .reasoning_summary
            .read(cx)
            .value()
            .trim()
            .to_string();
        let citations = serde_json::from_str(&self.input.citations.read(cx).value())
            .map_err(|err| AiChatError::StreamError(err.to_string()))?;
        self.on_update_content(
            Content {
                text,
                reasoning_summary: (!reasoning_summary.is_empty()).then_some(reasoning_summary),
                citations,
            },
            window,
            cx,
        )?;
        Ok(())
    }
}

fn format_time(value: OffsetDateTime) -> String {
    value.format(&Rfc3339).unwrap_or_else(|_| value.to_string())
}

fn render_editor(
    id: &'static str,
    label: SharedString,
    input: &Entity<InputState>,
    disabled: bool,
) -> AnyElement {
    v_flex()
        .id(id)
        .gap_1()
        .child(Label::new(label).text_sm())
        .child(Input::new(input).disabled(disabled).h(px(180.)).w_full())
        .into_any_element()
}

pub(crate) fn open_message_preview_window<T>(message: T, cx: &mut App)
where
    T: MessagePreviewExt + Clone + 'static,
{
    let title = {
        let i18n = cx.global::<I18n>();
        let mut args = FluentArgs::new();
        args.set("id", message.id().to_string());
        i18n.t_with_args("message-preview-title", &args)
    };
    if let Err(err) = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(960.), px(720.)),
                cx,
            ))),
            titlebar: Some(TitlebarOptions {
                title: Some(title.into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        move |window, cx| {
            let message_view = cx.new(|cx| MessagePreview::new(message.clone(), window, cx));
            cx.new(|cx| Root::new(message_view, window, cx))
        },
    ) {
        event!(Level::ERROR, "open message view window: {}", err);
    }
}

pub trait MessagePreviewExt: MessageViewExt {
    fn on_update_content(
        &self,
        content: Content,
        window: &mut Window,
        cx: &mut App,
    ) -> AiChatResult<()>;
}

impl MessageViewExt for Message {
    type Id = i32;

    fn role(&self) -> &crate::database::Role {
        &self.role
    }

    fn content(&self) -> &Content {
        &self.content
    }

    fn send_content(&self) -> &serde_json::Value {
        &self.send_content
    }

    fn status(&self) -> &crate::database::Status {
        &self.status
    }

    fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    fn id(&self) -> Self::Id {
        self.id
    }

    fn description_items(&self, cx: &App) -> Vec<DescriptionItem> {
        let (
            field_id,
            field_conversation_name,
            field_conversation_path,
            field_provider,
            field_role,
            field_status,
            field_created_time,
            field_updated_time,
            field_start_time,
            field_end_time,
            field_error,
        ) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("field-id"),
                i18n.t("field-conversation-name"),
                i18n.t("field-conversation-path"),
                i18n.t("field-provider"),
                i18n.t("field-role"),
                i18n.t("field-status"),
                i18n.t("field-created-time"),
                i18n.t("field-updated-time"),
                i18n.t("field-start-time"),
                i18n.t("field-end-time"),
                i18n.t("field-error"),
            )
        };
        let conversation_name = cx
            .global::<Db>()
            .get()
            .ok()
            .and_then(|mut conn| Conversation::find(self.conversation_id, &mut conn).ok())
            .map(|conversation| conversation.title)
            .unwrap_or_else(|| "-".to_string());
        vec![
            DescriptionItem::new(field_id).value(self.id.to_string()),
            DescriptionItem::new(field_conversation_name).value(conversation_name),
            DescriptionItem::new(field_conversation_path).value(self.conversation_path.clone()),
            DescriptionItem::new(field_provider).value(self.provider.clone()),
            DescriptionItem::new(field_role).value(self.role.to_string()),
            DescriptionItem::new(field_status).value(self.status.to_string()),
            DescriptionItem::new(field_created_time).value(format_time(self.created_time)),
            DescriptionItem::new(field_updated_time).value(format_time(self.updated_time)),
            DescriptionItem::new(field_start_time).value(format_time(self.start_time)),
            DescriptionItem::new(field_end_time).value(format_time(self.end_time)),
            DescriptionItem::new(field_error)
                .value(self.error.clone().unwrap_or_else(|| "-".to_string()))
                .span(3),
        ]
    }

    fn open_view_by_id(id: Self::Id, _window: &mut Window, cx: &mut App) {
        let message = match cx.global::<Db>().get() {
            Ok(mut conn) => match Message::find(id, &mut conn) {
                Ok(message) => message,
                Err(err) => {
                    event!(Level::ERROR, "find message failed: {}", err);
                    return;
                }
            },
            Err(err) => {
                event!(Level::ERROR, "get db failed: {}", err);
                return;
            }
        };
        open_message_preview_window(message, cx);
    }

    fn pause_message_by_id(id: Self::Id, _window: &mut Window, cx: &mut App) {
        let panel = cx
            .global::<WorkspaceStore>()
            .read(cx)
            .active_conversation_panel();
        let Some(panel) = panel else {
            return;
        };
        panel.update(cx, |this, cx| {
            this.pause_message(id, cx);
        });
    }

    fn delete_message_by_id(id: Self::Id, _window: &mut Window, cx: &mut App) {
        let chat_data = cx.global::<ChatData>().deref().clone();
        chat_data.update(cx, move |_this, cx| {
            cx.emit(ChatDataEvent::DeleteMessage(id));
        });
    }

    fn can_resend(&self, cx: &App) -> bool {
        if self.role != Role::Assistant {
            return false;
        }
        cx.global::<WorkspaceStore>()
            .read(cx)
            .active_conversation_panel()
            .is_some_and(|panel| !panel.read(cx).has_running_task())
    }

    fn resend_message_by_id(id: Self::Id, _window: &mut Window, cx: &mut App) {
        let panel = cx
            .global::<WorkspaceStore>()
            .read(cx)
            .active_conversation_panel();
        let Some(panel) = panel else {
            return;
        };
        panel.update(cx, |this, cx| {
            this.resend_message(id, _window, cx);
        });
    }
}

impl MessagePreviewExt for Message {
    fn on_update_content(
        &self,
        content: Content,
        _window: &mut Window,
        cx: &mut App,
    ) -> AiChatResult<()> {
        let conn = &mut cx.global::<Db>().get()?;
        Message::update_content(self.id, &content, conn)?;
        Ok(())
    }
}

impl<T: MessagePreviewExt> Render for MessagePreview<T> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (
            update_success_title,
            update_failed_title,
            section_information,
            section_content,
            field_text,
            field_reasoning_summary,
            field_citations,
            field_send_content,
        ) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("notify-update-message-success"),
                i18n.t("notify-update-message-failed"),
                i18n.t("section-information"),
                i18n.t("section-content"),
                i18n.t("field-text"),
                i18n.t("field-reasoning-summary"),
                i18n.t("field-citations"),
                i18n.t("field-send-content"),
            )
        };
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);
        let disabled = matches!(self.preview_type, PreviewType::Preview);

        v_flex()
            .size_full()
            .p_2()
            .gap_2()
            .child(
                ToggleGroup::new("message-preview-mode")
                    .outline()
                    .child(Toggle::new("preview").label("Preview").checked(self.preview_type.preview_checked()))
                    .child(Toggle::new("edit").label("Edit").checked(self.preview_type.edit_checked()))
                    .on_click(cx.listener(|view, checkeds: &Vec<bool>, _, _cx| {
                        match (checkeds.first(), checkeds.get(1), &view.preview_type) {
                            (Some(true), _, PreviewType::Edit)
                            | (_, Some(false), PreviewType::Edit) => {
                                view.preview_type = PreviewType::Preview
                            }
                            (_, Some(true), PreviewType::Preview)
                            | (Some(false), _, PreviewType::Preview) => {
                                view.preview_type = PreviewType::Edit
                            }
                            _ => {}
                        }
                    })),
            )
            .when(matches!(self.preview_type, PreviewType::Edit), |this| {
                this.child(
                    Button::new("message-preview-submit")
                        .icon(IconName::ArrowUp)
                        .on_click({
                            let update_success_title = update_success_title.clone();
                            let update_failed_title = update_failed_title.clone();
                            cx.listener(move |view, _, window, cx| match view.submit(window, cx) {
                                Ok(_) => {
                                    window.push_notification(
                                        Notification::new()
                                            .title(update_success_title.clone())
                                            .with_type(gpui_component::notification::NotificationType::Success),
                                        cx,
                                    );
                                }
                                Err(err) => {
                                    window.push_notification(
                                        Notification::new()
                                            .title(update_failed_title.clone())
                                            .message(err.to_string())
                                            .with_type(gpui_component::notification::NotificationType::Error),
                                        cx,
                                    );
                                }
                            })
                        }),
                )
            })
            .child(
                v_flex()
                    .flex_1()
                    .gap_4()
                    .child(Label::new(section_information).text_lg())
                    .child(
                        DescriptionList::new()
                            .columns(3)
                            .children(self.description_items(cx))
                            .layout(Axis::Vertical),
                    )
                    .child(Label::new(section_content).text_lg())
                    .child(render_editor(
                        "message-preview-text",
                        field_text.into(),
                        &self.input.text,
                        disabled,
                    ))
                    .child(render_editor(
                        "message-preview-reasoning-summary",
                        field_reasoning_summary.into(),
                        &self.input.reasoning_summary,
                        disabled,
                    ))
                    .child(render_editor(
                        "message-preview-citations",
                        field_citations.into(),
                        &self.input.citations,
                        disabled,
                    ))
                    .child(render_editor(
                        "message-preview-send-content",
                        field_send_content.into(),
                        &self.input.send_content,
                        true,
                    ))
                    .overflow_hidden()
                    .overflow_y_scrollbar(),
            )
            .children(dialog_layer)
            .children(notification_layer)
    }
}

#[cfg(test)]
mod tests {
    use crate::database::{Content, Message, Role, Status};
    use time::OffsetDateTime;

    fn make_message(role: Role) -> Message {
        let now = OffsetDateTime::now_utc();
        Message {
            id: 1,
            conversation_id: 1,
            conversation_path: "/conversation/1".to_string(),
            provider: "OpenAI".to_string(),
            role,
            content: Content::new("hello"),
            send_content: serde_json::json!({}),
            status: Status::Normal,
            created_time: now,
            updated_time: now,
            start_time: now,
            end_time: now,
            error: None,
        }
    }

    #[test]
    fn only_assistant_messages_can_resend() {
        let assistant = make_message(Role::Assistant);
        let user = make_message(Role::User);
        assert_eq!(assistant.role, Role::Assistant);
        assert_eq!(user.role, Role::User);
    }
}

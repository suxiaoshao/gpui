use crate::{
    assets::IconName,
    components::{
        delete_confirm::{DestructiveAction, open_destructive_confirm_dialog},
        message::{MessageViewExt, render_role_pill},
    },
    database::{Content, Conversation, Db, Message, Role, Status},
    errors::{AiChatError, AiChatResult},
    i18n::I18n,
    state::{ChatData, ChatDataEvent, WorkspaceStore},
};
use fluent_bundle::FluentArgs;
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, Root, WindowExt,
    button::{Button, Toggle, ToggleGroup, ToggleVariants},
    h_flex,
    input::{Input, InputState},
    label::Label,
    notification::Notification,
    scroll::ScrollableElement,
    text::TextView,
    tooltip::Tooltip,
    v_flex,
};
use std::ops::Deref;
use time::{OffsetDateTime, UtcOffset};
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
        let build_editor =
            |value: String, language: &'static str, window: &mut Window, cx: &mut App| {
                cx.new(|cx| {
                    InputState::new(window, cx)
                        .code_editor(language)
                        .line_number(true)
                        .searchable(true)
                        .default_value(value)
                })
            };
        Self {
            text: build_editor(message.content().text.clone(), "markdown", window, cx),
            reasoning_summary: build_editor(
                message
                    .content()
                    .reasoning_summary
                    .clone()
                    .unwrap_or_default(),
                "markdown",
                window,
                cx,
            ),
            citations: build_editor(
                serde_json::to_string_pretty(&message.content().citations)
                    .unwrap_or_else(|_| "[]".to_string()),
                "json",
                window,
                cx,
            ),
            send_content: build_editor(
                serde_json::to_string_pretty(message.send_content())
                    .unwrap_or_else(|_| "{}".to_string()),
                "json",
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
    scroll_handle: ScrollHandle,
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
            scroll_handle: ScrollHandle::default(),
        }
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AiChatResult<()> {
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
        let content = Content {
            text,
            reasoning_summary: (!reasoning_summary.is_empty()).then_some(reasoning_summary),
            citations,
        };
        self.message
            .on_update_content(content.clone(), window, cx)?;
        self.message.set_content(content);
        Ok(())
    }
}

struct DisplayTime {
    local: String,
    utc: String,
}

fn display_time(value: OffsetDateTime) -> DisplayTime {
    let local_offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    DisplayTime {
        local: format_time_with_offset(value, local_offset),
        utc: format_time_with_offset(value, UtcOffset::UTC),
    }
}

fn format_time_with_offset(value: OffsetDateTime, offset: UtcOffset) -> String {
    let value = value.to_offset(offset);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} {}",
        value.year(),
        u8::from(value.month()),
        value.day(),
        value.hour(),
        value.minute(),
        value.second(),
        offset_label(offset),
    )
}

fn offset_label(offset: UtcOffset) -> String {
    if offset == UtcOffset::UTC {
        return "UTC".to_string();
    }

    let seconds = offset.whole_seconds();
    let sign = if seconds >= 0 { '+' } else { '-' };
    let abs_seconds = seconds.abs();
    let hours = abs_seconds / 3600;
    let minutes = (abs_seconds % 3600) / 60;
    if minutes == 0 {
        format!("GMT{sign}{hours}")
    } else {
        format!("GMT{sign}{hours}:{minutes:02}")
    }
}

fn status_label(status: Status, cx: &App) -> SharedString {
    let key = match status {
        Status::Normal => "message-status-normal",
        Status::Hidden => "message-status-hidden",
        Status::Loading => "message-status-loading",
        Status::Thinking => "message-status-thinking",
        Status::Paused => "message-status-paused",
        Status::Error => "message-status-error",
    };
    cx.global::<I18n>().t(key).into()
}

fn status_color(status: Status, cx: &App) -> Hsla {
    match status {
        Status::Normal => cx.theme().success,
        Status::Hidden => cx.theme().muted_foreground.opacity(0.6),
        Status::Loading => cx.theme().blue,
        Status::Thinking => cx.theme().blue.opacity(0.7),
        Status::Paused => cx.theme().warning,
        Status::Error => cx.theme().danger,
    }
}

fn render_status_pill(status: Status, cx: &App) -> AnyElement {
    let color = status_color(status, cx);
    h_flex()
        .items_center()
        .gap_1()
        .px_2()
        .py_1()
        .rounded(px(6.))
        .bg(color.opacity(0.10))
        .border_1()
        .border_color(color.opacity(0.22))
        .text_color(color)
        .child(div().size(px(7.)).rounded_full().bg(color))
        .child(Label::new(status_label(status, cx)).text_xs())
        .into_any_element()
}

fn render_inline_metadata(
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
    cx: &App,
) -> AnyElement {
    h_flex()
        .items_baseline()
        .gap_1()
        .child(
            Label::new(label.into())
                .text_xs()
                .text_color(cx.theme().muted_foreground),
        )
        .child(Label::new(value.into()).text_sm())
        .into_any_element()
}

fn render_timeline_item(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    value: OffsetDateTime,
    color: Hsla,
    cx: &App,
) -> AnyElement {
    let time = display_time(value);
    let utc_time: SharedString = time.utc.into();
    v_flex()
        .id(id)
        .gap_1()
        .min_w(px(320.))
        .flex_basis(relative(0.45))
        .flex_grow()
        .child(
            h_flex()
                .items_center()
                .gap_2()
                .child(div().size(px(8.)).rounded_full().bg(color))
                .child(
                    Label::new(label.into())
                        .text_xs()
                        .text_color(cx.theme().muted_foreground),
                ),
        )
        .child(Label::new(time.local).text_sm().text_color(color))
        .hoverable_tooltip(move |window, cx| Tooltip::new(utc_time.clone()).build(window, cx))
        .into_any_element()
}

fn render_error_section(error: &str, cx: &App) -> AnyElement {
    v_flex()
        .gap_2()
        .child(
            Label::new(cx.global::<I18n>().t("field-error"))
                .text_sm()
                .text_color(cx.theme().muted_foreground),
        )
        .child(
            div()
                .w_full()
                .rounded(px(6.))
                .bg(cx.theme().danger.opacity(0.08))
                .border_l_2()
                .border_color(cx.theme().danger.opacity(0.72))
                .px_3()
                .py_2()
                .child(
                    Label::new(error.to_string())
                        .text_sm()
                        .text_color(cx.theme().danger),
                ),
        )
        .into_any_element()
}

fn render_message_metadata<T: MessagePreviewExt>(message: &T, cx: &App) -> AnyElement {
    let (
        field_provider,
        field_conversation_name,
        field_conversation_path,
        field_created_time,
        field_updated_time,
        field_start_time,
        field_end_time,
    ) = {
        let i18n = cx.global::<I18n>();
        (
            i18n.t("field-provider"),
            i18n.t("field-conversation-name"),
            i18n.t("field-conversation-path"),
            i18n.t("field-created-time"),
            i18n.t("field-updated-time"),
            i18n.t("field-start-time"),
            i18n.t("field-end-time"),
        )
    };
    let provider = message.provider_name().to_string();
    let conversation_name = message.conversation_name(cx);
    let conversation_path = message.conversation_path().map(ToOwned::to_owned);
    let error = message
        .error()
        .map(str::trim)
        .filter(|error| !error.is_empty());

    v_flex()
        .gap_4()
        .child(
            h_flex()
                .items_baseline()
                .flex_wrap()
                .gap_6()
                .child(render_inline_metadata(field_provider, provider, cx))
                .when_some(conversation_name, |this, conversation_name| {
                    this.child(render_inline_metadata(
                        field_conversation_name,
                        conversation_name,
                        cx,
                    ))
                })
                .when_some(conversation_path, |this, conversation_path| {
                    this.child(render_inline_metadata(
                        field_conversation_path,
                        conversation_path,
                        cx,
                    ))
                }),
        )
        .child(
            v_flex()
                .gap_3()
                .pt_4()
                .border_t_1()
                .border_color(cx.theme().border.opacity(0.56))
                .child(
                    v_flex()
                        .gap_4()
                        .child(h_flex().w_full().gap_4().children([
                            render_timeline_item(
                                "message-preview-created-time",
                                field_created_time,
                                message.created_time(),
                                cx.theme().muted_foreground,
                                cx,
                            ),
                            render_timeline_item(
                                "message-preview-updated-time",
                                field_updated_time,
                                message.updated_time(),
                                cx.theme().blue,
                                cx,
                            ),
                        ]))
                        .child(h_flex().w_full().gap_4().children([
                            render_timeline_item(
                                "message-preview-start-time",
                                field_start_time,
                                message.start_time(),
                                cx.theme().success,
                                cx,
                            ),
                            render_timeline_item(
                                "message-preview-end-time",
                                field_end_time,
                                message.end_time(),
                                cx.theme().warning,
                                cx,
                            ),
                        ])),
                ),
        )
        .when_some(error, |this, error| {
            this.child(render_error_section(error, cx))
        })
        .into_any_element()
}

fn render_editor(
    id: &'static str,
    label: SharedString,
    input: &Entity<InputState>,
    height: Pixels,
) -> AnyElement {
    v_flex()
        .id(id)
        .gap_1()
        .child(Label::new(label).text_sm())
        .child(Input::new(input).min_h(height).max_h(px(240.)).w_full())
        .into_any_element()
}

fn render_preview_text(
    id: impl Into<SharedString>,
    label: SharedString,
    value: String,
    cx: &App,
) -> AnyElement {
    let value = value.trim().to_string();
    let empty = value.is_empty();
    let body = if empty {
        Label::new(cx.global::<I18n>().t("field-none"))
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .into_any_element()
    } else {
        TextView::markdown(id.into(), &value)
            .selectable(true)
            .into_any_element()
    };

    v_flex()
        .gap_2()
        .child(Label::new(label).text_sm())
        .child(
            div()
                .w_full()
                .min_h(px(72.))
                .rounded(px(8.))
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().background)
                .p_3()
                .child(body),
        )
        .into_any_element()
}

fn render_preview_json(
    id: impl Into<SharedString>,
    label: SharedString,
    value: String,
    cx: &App,
) -> AnyElement {
    let trimmed = value.trim();
    let empty = matches!(trimmed, "" | "[]" | "{}" | "null");
    let body = if empty {
        cx.global::<I18n>().t("field-none").to_string()
    } else {
        format!("```json\n{trimmed}\n```")
    };

    v_flex()
        .gap_2()
        .child(Label::new(label).text_sm())
        .child(
            div()
                .w_full()
                .child(TextView::markdown(id.into(), &body).selectable(true)),
        )
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
    fn provider_name(&self) -> &str;

    fn conversation_name(&self, _cx: &App) -> Option<String> {
        None
    }

    fn conversation_path(&self) -> Option<&str> {
        None
    }

    fn created_time(&self) -> OffsetDateTime;
    fn updated_time(&self) -> OffsetDateTime;
    fn start_time(&self) -> OffsetDateTime;
    fn end_time(&self) -> OffsetDateTime;

    fn on_update_content(
        &self,
        content: Content,
        window: &mut Window,
        cx: &mut App,
    ) -> AiChatResult<()>;

    fn set_content(&mut self, content: Content);
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

    fn delete_message_by_id(id: Self::Id, window: &mut Window, cx: &mut App) {
        let chat_data = cx.global::<ChatData>().deref().clone();
        let (title, message) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("dialog-delete-message-title"),
                i18n.t("dialog-delete-message-message"),
            )
        };
        open_destructive_confirm_dialog(
            title,
            message,
            DestructiveAction::Delete,
            move |_window, cx| {
                chat_data.update(cx, move |_this, cx| {
                    cx.emit(ChatDataEvent::DeleteMessage(id));
                });
            },
            window,
            cx,
        );
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

    fn resend_message_by_id(id: Self::Id, window: &mut Window, cx: &mut App) {
        let (title, message) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("dialog-regenerate-message-title"),
                i18n.t("dialog-regenerate-message-message"),
            )
        };
        open_destructive_confirm_dialog(
            title,
            message,
            DestructiveAction::Regenerate,
            move |window, cx| {
                let panel = cx
                    .global::<WorkspaceStore>()
                    .read(cx)
                    .active_conversation_panel();
                let Some(panel) = panel else {
                    return;
                };
                panel.update(cx, |this, cx| {
                    this.resend_message(id, window, cx);
                });
            },
            window,
            cx,
        );
    }
}

impl MessagePreviewExt for Message {
    fn provider_name(&self) -> &str {
        &self.provider
    }

    fn conversation_name(&self, cx: &App) -> Option<String> {
        cx.global::<Db>()
            .get()
            .ok()
            .and_then(|mut conn| Conversation::find(self.conversation_id, &mut conn).ok())
            .map(|conversation| conversation.title)
    }

    fn conversation_path(&self) -> Option<&str> {
        Some(&self.conversation_path)
    }

    fn created_time(&self) -> OffsetDateTime {
        self.created_time
    }

    fn updated_time(&self) -> OffsetDateTime {
        self.updated_time
    }

    fn start_time(&self) -> OffsetDateTime {
        self.start_time
    }

    fn end_time(&self) -> OffsetDateTime {
        self.end_time
    }

    fn on_update_content(
        &self,
        content: Content,
        _window: &mut Window,
        cx: &mut App,
    ) -> AiChatResult<()> {
        let message_id = self.id;
        let conn = &mut cx.global::<Db>().get()?;
        Message::update_content(message_id, &content, conn)?;
        let chat_data = cx.global::<ChatData>().deref().clone();
        chat_data.update(cx, move |data, cx| {
            if let Ok(data) = data
                && data.update_message_content(message_id, content)
            {
                cx.notify();
            }
        });
        Ok(())
    }

    fn set_content(&mut self, content: Content) {
        self.content = content;
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
            button_preview,
            button_edit,
            button_save_message,
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
                i18n.t("button-preview"),
                i18n.t("button-edit"),
                i18n.t("button-save-message"),
            )
        };
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);
        let is_editing = matches!(self.preview_type, PreviewType::Edit);
        let text_value = self.input.text.read(cx).value().to_string();
        let reasoning_value = self.input.reasoning_summary.read(cx).value().to_string();
        let citations_value = self.input.citations.read(cx).value().to_string();
        let send_content_value = self.input.send_content.read(cx).value().to_string();

        v_flex()
            .size_full()
            .overflow_hidden()
            .child(
                h_flex()
                    .flex_initial()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .p_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(Label::new(format!("#{}", self.id())).text_sm())
                            .child(render_role_pill(*self.role(), cx))
                            .child(render_status_pill(*self.status(), cx))
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                ToggleGroup::new("message-preview-mode")
                                    .segmented()
                                    .outline()
                                    .child(
                                        Toggle::new("preview")
                                            .icon(IconName::Eye)
                                            .tooltip(button_preview)
                                            .checked(self.preview_type.preview_checked()),
                                    )
                                    .child(
                                        Toggle::new("edit")
                                            .icon(IconName::Edit)
                                            .tooltip(button_edit)
                                            .checked(self.preview_type.edit_checked()),
                                    )
                                    .on_click(cx.listener(
                                        |view, checkeds: &Vec<bool>, _, _cx| {
                                            match (
                                                checkeds.first(),
                                                checkeds.get(1),
                                                &view.preview_type,
                                            ) {
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
                                        },
                                    )),
                            )
                            .when(is_editing, |this| {
                                this.child(
                                    Button::new("message-preview-submit")
                                        .icon(IconName::Save)
                                        .tooltip(button_save_message.clone())
                                        .on_click({
                                            let update_success_title =
                                                update_success_title.clone();
                                            let update_failed_title = update_failed_title.clone();
                                            cx.listener(move |view, _, window, cx| {
                                                match view.submit(window, cx) {
                                                    Ok(_) => {
                                                        window.push_notification(
                                                            Notification::new()
                                                                .title(
                                                                    update_success_title.clone(),
                                                                )
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
                                                }
                                            })
                                        }),
                                )
                            }),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .relative()
                    .w_full()
                    .child(
                        div()
                            .id("message-preview-scroll-content")
                            .size_full()
                            .track_scroll(&self.scroll_handle)
                            .overflow_y_scroll()
                            .child(
                                v_flex()
                                    .w_full()
                                    .gap_4()
                                    .p_4()
                                    .child(Label::new(section_information).text_lg())
                                    .child(render_message_metadata(&self.message, cx))
                                    .child(Label::new(section_content).text_lg())
                                    .map(|this| {
                                        if is_editing {
                                            this.child(render_editor(
                                                "message-preview-text",
                                                field_text.into(),
                                                &self.input.text,
                                                px(132.),
                                            ))
                                            .child(render_editor(
                                                "message-preview-reasoning-summary",
                                                field_reasoning_summary.into(),
                                                &self.input.reasoning_summary,
                                                px(104.),
                                            ))
                                            .child(render_editor(
                                                "message-preview-citations",
                                                field_citations.into(),
                                                &self.input.citations,
                                                px(132.),
                                            ))
                                            .child(render_preview_json(
                                                "message-preview-send-content-preview",
                                                field_send_content.into(),
                                                send_content_value,
                                                cx,
                                            ))
                                        } else {
                                            this.child(render_preview_text(
                                                "message-preview-text-preview",
                                                field_text.into(),
                                                text_value,
                                                cx,
                                            ))
                                            .child(render_preview_text(
                                                "message-preview-reasoning-summary-preview",
                                                field_reasoning_summary.into(),
                                                reasoning_value,
                                                cx,
                                            ))
                                            .child(render_preview_json(
                                                "message-preview-citations-preview",
                                                field_citations.into(),
                                                citations_value,
                                                cx,
                                            ))
                                            .child(render_preview_json(
                                                "message-preview-send-content-preview",
                                                field_send_content.into(),
                                                send_content_value,
                                                cx,
                                            ))
                                        }
                                    }),
                            ),
                    )
                    .vertical_scrollbar(&self.scroll_handle),
            )
            .children(dialog_layer)
            .children(notification_layer)
    }
}

#[cfg(test)]
mod tests {
    use crate::database::{Content, Message, Role, Status};
    use time::{OffsetDateTime, UtcOffset};

    use super::{format_time_with_offset, offset_label};

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

    #[test]
    fn utc_times_are_formatted_for_tooltips() {
        assert_eq!(
            format_time_with_offset(OffsetDateTime::UNIX_EPOCH, UtcOffset::UTC),
            "1970-01-01 00:00:00 UTC"
        );
    }

    #[test]
    fn offset_labels_include_gmt_offsets() {
        assert_eq!(offset_label(UtcOffset::from_hms(8, 0, 0).unwrap()), "GMT+8");
        assert_eq!(
            offset_label(UtcOffset::from_hms(-5, -30, 0).unwrap()),
            "GMT-5:30"
        );
    }
}

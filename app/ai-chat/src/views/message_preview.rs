use crate::{
    components::message::MessageViewExt, database::Content, errors::AiChatResult, i18n::I18n,
};
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    IconName, Root, WindowExt,
    button::{Button, Toggle, ToggleGroup, ToggleVariants},
    h_flex,
    input::{Input, InputState},
    notification::Notification,
    v_flex,
};
use std::ops::Deref;
use tracing::event;

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

enum MessageInput {
    Text(Entity<InputState>),
    Extension {
        source: Entity<InputState>,
        content: Entity<InputState>,
        extension_name: String,
    },
}

impl MessageInput {
    fn new<T: MessagePreviewExt>(message: &T, window: &mut Window, cx: &mut App) -> Self {
        match &message.content() {
            crate::database::Content::Text(value) => {
                Self::Text(cx.new(|cx| {
                    InputState::new(window, cx)
                        .multi_line(true) // Language for syntax highlighting
                        .line_number(true) // Show line numbers
                        .searchable(true)
                        .default_value(value)
                }))
            }
            crate::database::Content::Extension {
                source,
                content,
                extension_name,
            } => Self::Extension {
                source: cx.new(|cx| {
                    InputState::new(window, cx)
                        .multi_line(true) // Language for syntax highlighting
                        .line_number(true) // Show line numbers
                        .searchable(true)
                        .default_value(source)
                }),
                content: cx.new(|cx| {
                    InputState::new(window, cx)
                        .multi_line(true) // Language for syntax highlighting
                        .line_number(true) // Show line numbers
                        .searchable(true)
                        .default_value(content)
                }),
                extension_name: extension_name.to_string(),
            },
        }
    }
}

pub struct MessagePreview<T: MessagePreviewExt> {
    message: T,
    preview_type: PreviewType,
    input: MessageInput,
}

impl<T: MessagePreviewExt> Deref for MessagePreview<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.message
    }
}

impl<T: MessagePreviewExt> MessagePreview<T> {
    pub fn new(message: T, window: &mut Window, cx: &mut App) -> Self {
        let input = MessageInput::new(&message, window, cx);
        Self {
            message,
            preview_type: PreviewType::Preview,
            input,
        }
    }
    fn submit(&self, cx: &mut Context<Self>) -> AiChatResult<()> {
        let content = match &self.input {
            MessageInput::Text(entity) => {
                let text = entity.read(cx).value().to_string();
                Content::Text(text)
            }
            MessageInput::Extension {
                source,
                content,
                extension_name,
            } => {
                let source = source.read(cx).value().to_string();
                let content = content.read(cx).value().to_string();
                Content::Extension {
                    source,
                    content,
                    extension_name: extension_name.to_string(),
                }
            }
        };
        self.on_update_content(content, cx)?;
        Ok(())
    }
}

pub trait MessagePreviewExt: MessageViewExt {
    fn on_update_content(&self, content: Content, cx: &mut App) -> AiChatResult<()>;
}

impl<T: MessagePreviewExt> Render for MessagePreview<T> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (update_success_title, update_failed_title) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("notify-update-message-success"),
                i18n.t("notify-update-message-failed"),
            )
        };
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);
        v_flex()
            .p_2()
            .gap_2()
            .size_full()
            .child(
                h_flex()
                    .items_center()
                    .flex_initial()
                    .child(
                        ToggleGroup::new("filter-group")
                            .flex_1()
                            .outline()
                            .child(
                                Toggle::new("preview")
                                    .icon(IconName::Eye)
                                    .checked(self.preview_type.preview_checked()),
                            )
                            .child(
                                Toggle::new("edit")
                                    .icon(IconName::Bot)
                                    .checked(self.preview_type.edit_checked()),
                            )
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
                    .map(|this| {
                        if let PreviewType::Edit = self.preview_type {
                            this.child(Button::new("submit").icon(IconName::ArrowUp).on_click({
                                let update_success_title = update_success_title.clone();
                                let update_failed_title = update_failed_title.clone();
                                cx.listener(move |view, _, window, cx| match view.submit(cx) {
                                    Ok(_) => {
                                        event!(tracing::Level::INFO,"Update Message Content Success");
                                        window.push_notification(Notification::new().title(update_success_title.clone()).with_type(gpui_component::notification::NotificationType::Success), cx);
                                    }
                                    Err(err) => {
                                        event!(
                                            tracing::Level::ERROR,
                                            "Failed to submit message: {}",
                                            err
                                        );
                                        window.push_notification(
                                            Notification::new()
                                                .title(update_failed_title.clone())
                                                .message(err.to_string()).with_type(gpui_component::notification::NotificationType::Error),
                                            cx,
                                        );
                                    }
                                })
                            }))
                        } else {
                            this
                        }
                    }),
            )
            .child({
                let disabled = matches!(self.preview_type, PreviewType::Preview);
                match &self.input {
                    MessageInput::Text(entity) => div()
                        .flex_1()
                        .child(Input::new(entity).disabled(disabled).size_full()),
                    MessageInput::Extension {
                        source, content, ..
                    } => h_flex()
                        .flex_1()
                        .gap_2()
                        .child(Input::new(source).disabled(disabled).size_full())
                        .child(Input::new(content).disabled(disabled).size_full()),
                }
            })
            .children(dialog_layer)
            .children(notification_layer)
    }
}

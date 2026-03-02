use crate::{
    database::{Content, Role, Status},
    i18n::I18n,
};
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    IconName, Sizable, WindowExt,
    alert::Alert,
    avatar::Avatar,
    button::{Button, ButtonVariants},
    divider::Divider,
    h_flex,
    notification::{Notification, NotificationType},
    spinner::Spinner,
    text::TextView,
    v_flex,
};
use std::{
    fmt::Display,
    ops::{Deref, DerefMut},
};

#[derive(IntoElement)]
pub(crate) struct MessageView<T: MessageViewExt>(T);

impl<T: MessageViewExt> MessageView<T> {
    pub fn new(data: T) -> Self {
        Self(data)
    }
}

impl<T: MessageViewExt> DerefMut for MessageView<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: MessageViewExt> Deref for MessageView<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub trait MessageViewExt: 'static {
    type Id: Copy + Into<ElementId> + Display + 'static;

    fn role(&self) -> &Role;
    fn content(&self) -> &Content;
    fn status(&self) -> &Status;
    fn error(&self) -> Option<&str>;
    fn id(&self) -> Self::Id;
    fn open_view_by_id(id: Self::Id, window: &mut Window, cx: &mut App);
    fn pause_message_by_id(id: Self::Id, window: &mut Window, cx: &mut App);
    fn delete_message_by_id(id: Self::Id, window: &mut Window, cx: &mut App);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MessageAccessoryMode {
    Loading,
    Actions,
}

impl From<&Status> for MessageAccessoryMode {
    fn from(value: &Status) -> Self {
        if matches!(value, Status::Loading) {
            MessageAccessoryMode::Loading
        } else {
            MessageAccessoryMode::Actions
        }
    }
}

fn visible_error<'a>(status: &Status, error: Option<&'a str>) -> Option<&'a str> {
    match status {
        Status::Error => error.map(str::trim).filter(|error| !error.is_empty()),
        _ => None,
    }
}

impl<T: MessageViewExt + 'static> RenderOnce for MessageView<T> {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let data = self.0;
        let (
            copy_success_title,
            copy_success_message,
            copy_failed_title,
            copy_failed_message,
            copy_tooltip,
            delete_tooltip,
            view_detail_tooltip,
            pause_tooltip,
            error_title,
        ) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("notify-copy-success-title"),
                i18n.t("notify-copy-success-message"),
                i18n.t("notify-copy-failed-title"),
                i18n.t("notify-copy-failed-message"),
                i18n.t("tooltip-copy"),
                i18n.t("tooltip-delete"),
                i18n.t("tooltip-view-detail"),
                i18n.t("tooltip-pause-message"),
                i18n.t("alert-error-title"),
            )
        };
        let accessory_mode = MessageAccessoryMode::from(data.status());
        let message_error = visible_error(data.status(), data.error()).map(ToOwned::to_owned);
        let copy_text = match data.content() {
            Content::Text(content) => content.to_string(),
            Content::Extension { source, .. } => source.to_string(),
        };
        let id = data.id();
        let button_id = id.to_string();
        let text_id = data.id();
        v_flex()
            .group("message")
            .child(
                h_flex()
                    .items_start()
                    .relative()
                    .pb_4()
                    .child(
                        Avatar::new()
                            .name(data.role().to_string())
                            .src(match data.role() {
                                Role::Developer => "png/system.png",
                                Role::User => "jpg/user.jpg",
                                Role::Assistant => "jpg/assistant.jpg",
                            })
                            .with_size(px(32.))
                            .ml_4()
                            .mt_4(),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .overflow_x_hidden()
                            .child(
                                TextView::markdown(text_id, &copy_text, window, cx)
                                    .selectable(true)
                                    .px_4()
                                    .pt_4(),
                            )
                            .when_some(message_error, |this, error| {
                                this.child(
                                    div().px_4().pb_4().child(
                                        Alert::error(
                                            SharedString::from(format!(
                                                "message-error-{button_id}"
                                            )),
                                            error,
                                        )
                                        .title(error_title.clone()),
                                    ),
                                )
                            }),
                    )
                    .map(|this| match accessory_mode {
                        MessageAccessoryMode::Loading => this.child(
                            div()
                                .absolute()
                                .right_2()
                                .top_0()
                                .w(px(24.))
                                .h(px(24.))
                                .relative()
                                .group("message-loading-control")
                                .child(
                                    div()
                                        .absolute()
                                        .right_1()
                                        .top_2()
                                        .opacity(1.)
                                        .group_hover("message-loading-control", |this| {
                                            this.opacity(0.)
                                        })
                                        .child(Spinner::new()),
                                )
                                .child(
                                    div()
                                        .absolute()
                                        .right_0()
                                        .top_0()
                                        .opacity(0.)
                                        .group_hover("message-loading-control", |this| {
                                            this.opacity(1.)
                                        })
                                        .child(
                                            Button::new(SharedString::from(format!(
                                                "pause-{}",
                                                button_id
                                            )))
                                            .icon(IconName::Close)
                                            .ghost()
                                            .small()
                                            .tooltip(pause_tooltip.clone())
                                            .on_click(move |_, window, cx| {
                                                T::pause_message_by_id(id, window, cx);
                                            }),
                                        ),
                                ),
                        ),
                        MessageAccessoryMode::Actions => this.child(
                            div()
                                .absolute()
                                .right_2()
                                .top_0()
                                .opacity(0.)
                                .group_hover("message", |this| this.opacity(1.))
                                .child(
                                    h_flex()
                                        .gap_1()
                                        .child(
                                            Button::new(SharedString::from(format!(
                                                "copy-{}",
                                                button_id
                                            )))
                                            .icon(IconName::Copy)
                                            .ghost()
                                            .small()
                                            .on_click(move |_, window, cx| {
                                                cx.write_to_clipboard(ClipboardItem::new_string(
                                                    copy_text.clone(),
                                                ));
                                                let copied = cx
                                                    .read_from_clipboard()
                                                    .and_then(|item| item.text())
                                                    .map(|copied| copied == copy_text)
                                                    .unwrap_or(false);
                                                if copied {
                                                    window.push_notification(
                                                        Notification::new()
                                                            .title(copy_success_title.clone())
                                                            .message(copy_success_message.clone())
                                                            .with_type(NotificationType::Success),
                                                        cx,
                                                    );
                                                } else {
                                                    window.push_notification(
                                                        Notification::new()
                                                            .title(copy_failed_title.clone())
                                                            .message(copy_failed_message.clone())
                                                            .with_type(NotificationType::Error),
                                                        cx,
                                                    );
                                                }
                                            })
                                            .tooltip(copy_tooltip.clone()),
                                        )
                                        .child(
                                            Button::new(SharedString::from(format!(
                                                "delete-{}",
                                                button_id
                                            )))
                                            .icon(IconName::Delete)
                                            .ghost()
                                            .small()
                                            .on_click(move |_, window, cx| {
                                                T::delete_message_by_id(id, window, cx);
                                            })
                                            .tooltip(delete_tooltip.clone()),
                                        )
                                        .child(
                                            Button::new(SharedString::from(format!(
                                                "view-{}",
                                                button_id
                                            )))
                                            .icon(IconName::Eye)
                                            .ghost()
                                            .small()
                                            .on_click(move |_, window, cx| {
                                                T::open_view_by_id(id, window, cx);
                                            })
                                            .tooltip(view_detail_tooltip.clone()),
                                        ),
                                ),
                        ),
                    }),
            )
            .child(Divider::horizontal())
    }
}

#[cfg(test)]
mod tests {
    use super::{MessageAccessoryMode, visible_error};
    use crate::database::Status;

    #[test]
    fn accessory_mode_uses_loading_controls_for_loading_status() {
        assert_eq!(
            MessageAccessoryMode::from(&Status::Loading),
            MessageAccessoryMode::Loading
        );
        assert_eq!(
            MessageAccessoryMode::from(&Status::Normal),
            MessageAccessoryMode::Actions
        );
    }

    #[test]
    fn visible_error_only_returns_error_status_message() {
        assert_eq!(
            visible_error(&Status::Error, Some("request failed")),
            Some("request failed")
        );
        assert_eq!(visible_error(&Status::Normal, Some("request failed")), None);
    }

    #[test]
    fn visible_error_skips_missing_or_blank_messages() {
        assert_eq!(visible_error(&Status::Error, None), None);
        assert_eq!(visible_error(&Status::Error, Some("   ")), None);
    }
}

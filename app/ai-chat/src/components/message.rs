use crate::{
    assets::IconName,
    database::{Content, Role, Status},
    i18n::I18n,
};
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, Icon, Sizable, WindowExt,
    alert::Alert,
    badge::Badge,
    button::{Button, ButtonVariants},
    divider::Divider,
    h_flex,
    label::Label,
    notification::{Notification, NotificationType},
    popover::Popover,
    scroll::ScrollableElement,
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
    fn send_content(&self) -> &serde_json::Value;
    fn status(&self) -> &Status;
    fn error(&self) -> Option<&str>;
    fn id(&self) -> Self::Id;
    fn open_view_by_id(id: Self::Id, window: &mut Window, cx: &mut App);
    fn pause_message_by_id(id: Self::Id, window: &mut Window, cx: &mut App);
    fn delete_message_by_id(id: Self::Id, window: &mut Window, cx: &mut App);
    fn can_resend(&self, _cx: &App) -> bool;
    fn resend_message_by_id(id: Self::Id, window: &mut Window, cx: &mut App);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MessageAccessoryMode {
    Thinking,
    Loading,
    Actions,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MessageAction {
    Resend,
    Copy,
    Delete,
    View,
}

impl From<&Status> for MessageAccessoryMode {
    fn from(value: &Status) -> Self {
        match value {
            Status::Thinking => MessageAccessoryMode::Thinking,
            Status::Loading => MessageAccessoryMode::Loading,
            _ => MessageAccessoryMode::Actions,
        }
    }
}

fn visible_error<'a>(status: &Status, error: Option<&'a str>) -> Option<&'a str> {
    match status {
        Status::Error => error.map(str::trim).filter(|error| !error.is_empty()),
        _ => None,
    }
}

fn reasoning_summary_label(status: &Status, i18n: &I18n) -> SharedString {
    match status {
        Status::Thinking => i18n.t("button-reasoning-summary-thinking").into(),
        _ => i18n.t("button-reasoning-summary").into(),
    }
}

fn status_badge_color(status: &Status, cx: &App) -> Hsla {
    match status {
        Status::Normal => cx.theme().success,
        Status::Hidden => cx.theme().muted_foreground.opacity(0.6),
        Status::Loading => cx.theme().blue,
        Status::Thinking => cx.theme().blue.opacity(0.7),
        Status::Paused => cx.theme().warning,
        Status::Error => cx.theme().danger,
    }
}

pub(crate) fn role_label(role: Role, cx: &App) -> SharedString {
    let key = match role {
        Role::Developer => "role-developer",
        Role::User => "role-user",
        Role::Assistant => "role-assistant",
    };
    cx.global::<I18n>().t(key).into()
}

fn role_icon(role: Role) -> IconName {
    match role {
        Role::Developer => IconName::Shield,
        Role::User => IconName::UserRound,
        Role::Assistant => IconName::Bot,
    }
}

fn role_color(role: Role, cx: &App) -> Hsla {
    match role {
        Role::Developer => cx.theme().warning,
        Role::User => cx.theme().primary,
        Role::Assistant => cx.theme().blue,
    }
}

pub(crate) fn render_role_icon(role: Role, cx: &App) -> AnyElement {
    let color = role_color(role, cx);
    div()
        .size(px(32.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(color.opacity(0.12))
        .border_1()
        .border_color(color.opacity(0.32))
        .text_color(color)
        .child(Icon::new(role_icon(role)).with_size(px(16.)))
        .into_any_element()
}

pub(crate) fn render_role_pill(role: Role, cx: &App) -> AnyElement {
    let color = role_color(role, cx);
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
        .child(Icon::new(role_icon(role)).with_size(px(12.)))
        .child(Label::new(role_label(role, cx)).text_xs())
        .into_any_element()
}

fn message_actions(can_resend: bool) -> Vec<MessageAction> {
    let mut actions = vec![
        MessageAction::Copy,
        MessageAction::Delete,
        MessageAction::View,
    ];
    if can_resend {
        actions.insert(0, MessageAction::Resend);
    }
    actions
}

impl<T: MessageViewExt + 'static> RenderOnce for MessageView<T> {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let data = self.0;
        let (
            copy_success_title,
            copy_success_message,
            copy_failed_title,
            copy_failed_message,
            copy_tooltip,
            delete_tooltip,
            resend_tooltip,
            view_detail_tooltip,
            pause_tooltip,
            error_title,
            sources_label,
        ) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("notify-copy-success-title"),
                i18n.t("notify-copy-success-message"),
                i18n.t("notify-copy-failed-title"),
                i18n.t("notify-copy-failed-message"),
                i18n.t("tooltip-copy"),
                i18n.t("tooltip-delete"),
                i18n.t("tooltip-resend-message"),
                i18n.t("tooltip-view-detail"),
                i18n.t("tooltip-pause-message"),
                i18n.t("alert-error-title"),
                i18n.t("field-sources"),
            )
        };
        let accessory_mode = MessageAccessoryMode::from(data.status());
        let reasoning_summary_label = reasoning_summary_label(data.status(), cx.global::<I18n>());
        let message_error = visible_error(data.status(), data.error()).map(ToOwned::to_owned);
        let can_resend = data.can_resend(cx);
        let role = *data.role();
        let avatar = Badge::new()
            .dot()
            .count(1)
            .color(status_badge_color(data.status(), cx))
            .child(render_role_icon(role, cx));
        let role_label = role_label(role, cx);
        let copy_text = data.content().display_markdown(&sources_label);
        let reasoning_summary = data
            .content()
            .reasoning_summary
            .clone()
            .filter(|summary| !summary.trim().is_empty());
        let popover_bg = cx.theme().background;
        let popover_border = cx.theme().border;
        let reasoning_button_fg = cx.theme().muted_foreground;
        let id = data.id();
        let button_id = id.to_string();
        let text_id = data.id();
        let action_buttons = message_actions(can_resend)
            .into_iter()
            .map(|action| match action {
                MessageAction::Resend => {
                    Button::new(SharedString::from(format!("resend-{button_id}")))
                        .icon(IconName::RefreshCcw)
                        .ghost()
                        .small()
                        .on_click(move |_, window, cx| {
                            T::resend_message_by_id(id, window, cx);
                        })
                        .tooltip(resend_tooltip.clone())
                        .into_any_element()
                }
                MessageAction::Copy => Button::new(SharedString::from(format!("copy-{button_id}")))
                    .icon(IconName::Copy)
                    .ghost()
                    .small()
                    .on_click({
                        let copy_text = copy_text.clone();
                        let copy_success_title = copy_success_title.clone();
                        let copy_success_message = copy_success_message.clone();
                        let copy_failed_title = copy_failed_title.clone();
                        let copy_failed_message = copy_failed_message.clone();
                        move |_, window, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(copy_text.clone()));
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
                        }
                    })
                    .tooltip(copy_tooltip.clone())
                    .into_any_element(),
                MessageAction::Delete => {
                    Button::new(SharedString::from(format!("delete-{button_id}")))
                        .icon(IconName::Trash)
                        .ghost()
                        .small()
                        .on_click(move |_, window, cx| {
                            T::delete_message_by_id(id, window, cx);
                        })
                        .tooltip(delete_tooltip.clone())
                        .into_any_element()
                }
                MessageAction::View => Button::new(SharedString::from(format!("view-{button_id}")))
                    .icon(IconName::Eye)
                    .ghost()
                    .small()
                    .on_click(move |_, window, cx| {
                        T::open_view_by_id(id, window, cx);
                    })
                    .tooltip(view_detail_tooltip.clone())
                    .into_any_element(),
            })
            .collect::<Vec<_>>();
        v_flex()
            .group("message")
            .w_full()
            .child(
                h_flex()
                    .items_start()
                    .relative()
                    .pb_4()
                    .child(div().ml_4().mt_4().child(avatar))
                    .child(
                        v_flex()
                            .flex_1()
                            .pt_4()
                            .px_4()
                            .gap_2()
                            .overflow_x_hidden()
                            .child(
                                Label::new(role_label)
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .when_some(reasoning_summary, |this, summary| {
                                let popover_button_id = button_id.clone();
                                this.child(
                                    h_flex().child(
                                        Popover::new(SharedString::from(format!(
                                            "reasoning-summary-popover-{}",
                                            popover_button_id
                                        )))
                                        .anchor(Anchor::TopLeft)
                                        .appearance(false)
                                        .trigger(
                                            Button::new(SharedString::from(format!(
                                                "reasoning-summary-{}",
                                                popover_button_id
                                            )))
                                            .label(reasoning_summary_label.clone())
                                            .ghost()
                                            .small()
                                            .text_color(reasoning_button_fg)
                                            .child(
                                                div().ml_1().child(
                                                    Icon::new(IconName::ChevronRight)
                                                        .with_size(px(12.)),
                                                ),
                                            ),
                                        )
                                        .content(
                                            move |_, _window, _cx| {
                                                div()
                                                    .w(px(520.))
                                                    .occlude()
                                                    .bg(popover_bg)
                                                    .border_1()
                                                    .border_color(popover_border)
                                                    .rounded(px(8.))
                                                    .shadow_md()
                                                    .child(
                                                        v_flex()
                                                            .p_2()
                                                            .h(px(420.))
                                                            .overflow_hidden()
                                                            .overflow_y_scrollbar()
                                                            .child(
                                                                div().child(
                                                                    TextView::markdown(
                                                                        SharedString::from(
                                                                            format!(
                                                                                "reasoning-summary-content-{}",
                                                                                popover_button_id
                                                                            ),
                                                                        ),
                                                                        &summary,
                                                                    )
                                                                    .selectable(true),
                                                                ),
                                                            ),
                                                    )
                                            },
                                        ),
                                    ),
                                )
                            })
                            .child(
                                TextView::markdown(text_id, &copy_text)
                                    .selectable(true),
                            )
                            .when_some(message_error, |this, error| {
                                this.child(
                                    div().child(
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
                        MessageAccessoryMode::Thinking => {
                            this.child(div().absolute().right_2().top_2().child(Spinner::new()))
                        }
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
                                            .icon(IconName::X)
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
                                .child(h_flex().gap_1().children(action_buttons)),
                        ),
                    }),
            )
            .child(Divider::horizontal())
    }
}

#[cfg(test)]
mod tests {
    use super::{MessageAccessoryMode, MessageAction, message_actions, visible_error};
    use crate::database::Status;

    #[test]
    fn accessory_mode_uses_loading_controls_for_loading_status() {
        assert_eq!(
            MessageAccessoryMode::from(&Status::Loading),
            MessageAccessoryMode::Loading
        );
        assert_eq!(
            MessageAccessoryMode::from(&Status::Thinking),
            MessageAccessoryMode::Thinking
        );
        assert_eq!(
            MessageAccessoryMode::from(&Status::Normal),
            MessageAccessoryMode::Actions
        );
        assert_eq!(
            MessageAccessoryMode::from(&Status::Paused),
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

    #[test]
    fn resend_action_is_first_when_available() {
        assert_eq!(
            message_actions(true),
            vec![
                MessageAction::Resend,
                MessageAction::Copy,
                MessageAction::Delete,
                MessageAction::View
            ]
        );
    }

    #[test]
    fn resend_action_is_omitted_when_unavailable() {
        assert_eq!(
            message_actions(false),
            vec![
                MessageAction::Copy,
                MessageAction::Delete,
                MessageAction::View
            ]
        );
    }
}

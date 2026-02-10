use crate::database::{Content, Role, Status};
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    IconName, Sizable, WindowExt,
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
    fn id(&self) -> Self::Id;
    fn open_view_by_id(id: Self::Id, window: &mut Window, cx: &mut App);
    fn delete_message_by_id(id: Self::Id, window: &mut Window, cx: &mut App);
}

impl<T: MessageViewExt + 'static> RenderOnce for MessageView<T> {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let data = self.0;
        let is_loading = matches!(data.status(), Status::Loading);
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
                        TextView::markdown(text_id, &copy_text, window, cx)
                            .selectable(true)
                            .px_4()
                            .pt_4()
                            .flex_1()
                            .overflow_x_hidden(),
                    )
                    .map(|this| {
                        if is_loading {
                            this.child(div().absolute().right_2().top_2().child(Spinner::new()))
                        } else {
                            this.child(
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
                                                    cx.write_to_clipboard(
                                                        ClipboardItem::new_string(
                                                            copy_text.clone(),
                                                        ),
                                                    );
                                                    let copied = cx
                                                        .read_from_clipboard()
                                                        .and_then(|item| item.text())
                                                        .map(|copied| copied == copy_text)
                                                        .unwrap_or(false);
                                                    if copied {
                                                        window.push_notification(
                                                            Notification::new()
                                                                .title("Copy Succeeded")
                                                                .message(
                                                                    "Message copied to clipboard.",
                                                                )
                                                                .with_type(
                                                                    NotificationType::Success,
                                                                ),
                                                            cx,
                                                        );
                                                    } else {
                                                        window.push_notification(
                                                            Notification::new()
                                                                .title("Copy Failed")
                                                                .message(
                                                                    "Could not read clipboard.",
                                                                )
                                                                .with_type(NotificationType::Error),
                                                            cx,
                                                        );
                                                    }
                                                })
                                                .tooltip("Copy"),
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
                                                .tooltip("Delete"),
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
                                                .tooltip("View Detail"),
                                            ),
                                    ),
                            )
                        }
                    }),
            )
            .child(Divider::horizontal())
    }
}

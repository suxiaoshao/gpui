use crate::{
    database::{Content, Db, Message, Role},
    errors::AiChatResult,
    views::message_preview::MessagePreview,
};
use gpui::*;
use gpui_component::{
    IconName, Root, Sizable, WindowExt,
    avatar::Avatar,
    button::{Button, ButtonVariants},
    divider::Divider,
    h_flex,
    notification::{Notification, NotificationType},
    text::TextView,
    v_flex,
};
use tracing::{Level, event};

#[derive(IntoElement)]
pub struct MessageItemView {
    id: i32,
    role: Role,
    content: Content,
}

impl MessageItemView {
    fn open_view_window(message_id: i32, window: &mut Window, cx: &mut App) {
        let message = match Self::get_message(message_id, cx) {
            Ok(data) => data,
            Err(err) => {
                event!(Level::ERROR, "open message view window: {}", err);
                window.push_notification(
                    Notification::new()
                        .title("Get Message Detail Failed")
                        .message(SharedString::from(err.to_string()))
                        .with_type(NotificationType::Error),
                    cx,
                );
                return;
            }
        };
        match cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(800.), px(600.)),
                    cx,
                ))),
                titlebar: Some(TitlebarOptions {
                    title: Some(format!("Message Preview: {}", message.id).into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                let message_view = cx.new(|cx| MessagePreview::new(message, window, cx));
                cx.new(|cx| Root::new(message_view, window, cx))
            },
        ) {
            Ok(_) => {}
            Err(err) => {
                event!(Level::ERROR, "open message view window: {}", err);
            }
        };
    }
    fn get_message(message_id: i32, cx: &mut App) -> AiChatResult<Message> {
        let conn = &mut cx.global::<Db>().get()?;
        let message = Message::find(message_id, conn)?;
        Ok(message)
    }
}

impl From<&Message> for MessageItemView {
    fn from(
        Message {
            id,
            conversation_id,
            conversation_path,
            role,
            content,
            status,
            created_time,
            updated_time,
            start_time,
            end_time,
        }: &Message,
    ) -> Self {
        Self {
            id: *id,
            role: *role,
            content: content.clone(),
        }
    }
}

impl RenderOnce for MessageItemView {
    fn render(self, window: &mut gpui::Window, cx: &mut gpui::App) -> impl gpui::IntoElement {
        v_flex()
            .group("message")
            .child(
                h_flex()
                    .items_start()
                    .relative()
                    .pb_4()
                    .child(
                        Avatar::new()
                            .name(self.role.to_string())
                            .src(match self.role {
                                Role::Developer => "png/system.png",
                                Role::User => "jpg/user.jpg",
                                Role::Assistant => "jpg/assistant.jpg",
                            })
                            .with_size(px(32.))
                            .ml_4()
                            .mt_4(),
                    )
                    .child(
                        TextView::markdown(
                            self.id,
                            match self.content {
                                Content::Text(content) => content,
                                Content::Extension { source, .. } => source,
                            },
                            window,
                            cx,
                        )
                        .selectable(true)
                        .px_4()
                        .pt_4()
                        .flex_1()
                        .overflow_x_hidden(),
                    )
                    .child(
                        div()
                            .absolute()
                            .right_2()
                            .top_0()
                            .opacity(0.)
                            .group_hover("message", |this| this.opacity(1.))
                            .child(
                                Button::new(SharedString::from(format!("view-{}", self.id)))
                                    .icon(IconName::Eye)
                                    .ghost()
                                    .small()
                                    .on_click(move |_, window, cx| {
                                        Self::open_view_window(self.id, window, cx);
                                    })
                                    .tooltip("View Detail"),
                            ),
                    ),
            )
            .child(Divider::horizontal())
    }
}

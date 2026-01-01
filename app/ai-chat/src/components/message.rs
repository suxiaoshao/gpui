use gpui::*;
use gpui_component::{
    ActiveTheme, Sizable, avatar::Avatar, divider::Divider, h_flex, text::TextView, v_flex,
};

use crate::database::{Content, Message, Role};

#[derive(IntoElement)]
pub struct MessageItemView {
    id: i32,
    role: Role,
    content: Content,
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
            .child(
                h_flex()
                    .items_start()
                    .pb_2()
                    .child(
                        Avatar::new()
                            .name(self.role.to_string())
                            .src(match self.role {
                                Role::Developer => "png/system.png",
                                Role::User => "jpg/user.jpg",
                                Role::Assistant => "jpg/assistant.jpg",
                            })
                            .with_size(px(32.))
                            .ml_2()
                            .mt_2(),
                    )
                    .child(
                        TextView::markdown(
                            self.id,
                            match self.content {
                                Content::Text(content) => content,
                                Content::Extension { content, .. } => content,
                            },
                            window,
                            cx,
                        )
                        .selectable(true)
                        .px_2()
                        .pt_2()
                        .flex_1()
                        .overflow_x_hidden(),
                    ),
            )
            .child(Divider::horizontal())
    }
}

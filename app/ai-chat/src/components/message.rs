use crate::database::{Content, Role, Status};
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    IconName, Sizable,
    avatar::Avatar,
    button::{Button, ButtonVariants},
    divider::Divider,
    h_flex,
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
    fn open_view_window(&self, window: &mut Window, cx: &mut App);
    fn role(&self) -> &Role;
    fn content(&self) -> &Content;
    fn status(&self) -> &Status;
    fn id(&self) -> impl Into<ElementId> + Display;
}

impl<T: MessageViewExt + 'static> RenderOnce for MessageView<T> {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let is_loading = matches!(self.status(), Status::Loading);
        v_flex()
            .group("message")
            .child(
                h_flex()
                    .items_start()
                    .relative()
                    .pb_4()
                    .child(
                        Avatar::new()
                            .name(self.role().to_string())
                            .src(match self.role() {
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
                            self.id(),
                            match self.content() {
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
                                        Button::new(SharedString::from(format!(
                                            "view-{}",
                                            &self.id()
                                        )))
                                        .icon(IconName::Eye)
                                        .ghost()
                                        .small()
                                        .on_click(move |_, window, cx| {
                                            self.open_view_window(window, cx);
                                        })
                                        .tooltip("View Detail"),
                                    ),
                            )
                        }
                    }),
            )
            .child(Divider::horizontal())
    }
}

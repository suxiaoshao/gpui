use crate::{
    errors::AiChatResult,
    store::{ChatData, ChatDataInner},
};
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{ActiveTheme, h_flex, v_flex};
use std::ops::Deref;

mod conversation_tab;

pub(crate) use conversation_tab::ConversationTabView;

pub(crate) struct TabsView {
    chat_data: Entity<AiChatResult<ChatDataInner>>,
}

impl TabsView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let chat_data = cx.global::<ChatData>().deref().clone();
        Self { chat_data }
    }
}

impl Render for TabsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().flex_1().map(|this| match self.chat_data.read(cx) {
            Ok(chat_data) => this.child(
                h_flex()
                    .h_7()
                    .bg(cx.theme().accent)
                    .children(chat_data.tabs())
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .border_b_1()
                            .border_color(cx.theme().border),
                    ),
            ),
            Err(_) => this,
        })
    }
}

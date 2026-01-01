use std::ops::Deref;

use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    input::{Input, InputState},
    scroll::ScrollableElement,
    v_flex,
};

use crate::{database::Conversation, store::ChatData};

pub(crate) struct ConversationPanelView {
    conversation_id: i32,
    input_state: Entity<InputState>,
    scroll_handle: ScrollHandle,
}

impl ConversationPanelView {
    pub fn new(conversation: &Conversation, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input_state = cx.new(|cx| InputState::new(window, cx).multi_line(true).auto_grow(3, 8));
        Self {
            conversation_id: conversation.id,
            input_state,
            scroll_handle: ScrollHandle::new(),
        }
    }
}

impl Render for ConversationPanelView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let chat_data = cx.global::<ChatData>().deref().clone();
        let chat_data = chat_data.read(cx).as_ref().ok();
        v_flex()
            .flex_1()
            .w_full()
            .overflow_hidden()
            .pb_2()
            .child(
                div()
                    .flex_1()
                    .relative()
                    .overflow_hidden()
                    .child(
                        div()
                            .id("conversation-panel")
                            .track_scroll(&self.scroll_handle)
                            .size_full()
                            .overflow_scroll()
                            .when_some(chat_data.map(|x| x.panel_messages()), |this, messages| {
                                this.children(messages)
                            }),
                    )
                    .vertical_scrollbar(&self.scroll_handle),
            )
            .child(
                div()
                    .flex_initial()
                    .child(Input::new(&self.input_state))
                    .px_2(),
            )
    }
}

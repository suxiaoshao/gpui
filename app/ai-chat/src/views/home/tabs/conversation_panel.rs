use gpui::{prelude::FluentBuilder, *};
use gpui_component::{input::InputState, scroll::ScrollableElement, select::SelectState, v_flex};
use std::ops::Deref;

use crate::{
    components::chat_input::{ChatInput, input_state},
    database::Conversation,
    extensions::ExtensionContainer,
    store::ChatData,
};

pub(crate) struct ConversationPanelView {
    input_state: Entity<InputState>,
    extension_state: Entity<SelectState<Vec<String>>>,
    _subscriptions: Vec<Subscription>,
}

impl ConversationPanelView {
    pub fn new(conversation: &Conversation, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input_state = input_state(window, cx);
        let _subscriptions = vec![];
        let extension_container = cx.global::<ExtensionContainer>();
        let all_extensions = extension_container.get_all_config();
        Self {
            input_state,
            _subscriptions,
            extension_state: cx.new(|cx| {
                SelectState::new(
                    all_extensions.into_iter().map(|x| x.name).collect(),
                    None,
                    window,
                    cx,
                )
                .searchable(true)
            }),
        }
    }
}

impl Render for ConversationPanelView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let chat_data = cx.global::<ChatData>().deref().clone();
        let chat_data = chat_data.read(cx).as_ref().ok();
        v_flex()
            .flex_1()
            .w_full()
            .overflow_hidden()
            .pb_2()
            .child(
                div()
                    .id("conversation-panel")
                    .flex_1()
                    .overflow_hidden()
                    .when_some(chat_data.map(|x| x.panel_messages()), |this, messages| {
                        this.children(messages)
                    })
                    .child(div().h_2())
                    .overflow_y_scrollbar(),
            )
            .child(
                div()
                    .w_full()
                    .flex_initial()
                    .child(ChatInput::new(&self.input_state, &self.extension_state))
                    .px_2(),
            )
    }
}

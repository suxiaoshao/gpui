use crate::{
    errors::AiChatResult,
    store::{ChatData, ChatDataEvent, ChatDataInner},
};
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{ActiveTheme, h_flex, v_flex};
use std::ops::Deref;

mod conversation_panel;
mod conversation_tab;
mod template_detail;
mod template_list;

pub(crate) use conversation_panel::ConversationPanelView;
pub(crate) use conversation_tab::{ConversationTabView, DragTab};
pub(crate) use template_detail::TemplateDetailView;
pub(crate) use template_list::TemplateListView;

pub fn init(cx: &mut App) {
    template_list::init(cx);
}

pub(crate) struct TabsView {
    chat_data: WeakEntity<AiChatResult<ChatDataInner>>,
}

impl TabsView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let chat_data = cx.global::<ChatData>().deref().downgrade();
        Self { chat_data }
    }
}

impl Render for TabsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let chat_data = self
            .chat_data
            .upgrade()
            .and_then(|x| x.read(cx).as_ref().ok());
        v_flex()
            .flex_1()
            .overflow_hidden()
            .map(|this| match chat_data {
                Some(chat_data) => this
                    .child(
                        h_flex()
                            .flex_initial()
                            .h_7()
                            .bg(cx.theme().accent)
                            .children(chat_data.tabs())
                            .child(
                                div()
                                    .flex_1()
                                    .h_full()
                                    .border_b_1()
                                    .border_color(cx.theme().border)
                                    .drag_over::<DragTab>(|this, _drag, _window, cx| {
                                        this.bg(cx.theme().drop_target)
                                    })
                                    .on_drop(move |drag: &DragTab, _window, cx| {
                                        let chat_data = cx.global::<ChatData>().deref().clone();
                                        chat_data.update(cx, move |_this, cx| {
                                            cx.emit(ChatDataEvent::MoveTab {
                                                from_id: drag.id,
                                                to_id: None,
                                            });
                                        });
                                    }),
                            ),
                    )
                    .when_some(chat_data.panel(), |this, panel| this.child(panel)),
                None => this,
            })
    }
}

use std::ops::Deref;

use gpui::*;
use gpui_component::{ActiveTheme, Icon, IconName, label::Label};

use crate::{
    database::Conversation,
    store::{ChatData, ChatDataEvent},
};

#[derive(IntoElement, Clone)]
pub(crate) struct ConversationTabView {
    pub(crate) id: i32,
    pub(crate) name: SharedString,
}

impl From<&Conversation> for ConversationTabView {
    fn from(conversation: &Conversation) -> Self {
        Self {
            id: conversation.id,
            name: SharedString::from(&conversation.title),
        }
    }
}

impl RenderOnce for ConversationTabView {
    fn render(self, _window: &mut gpui::Window, cx: &mut gpui::App) -> impl gpui::IntoElement {
        div()
            .id(self.id)
            .group("tab")
            .relative()
            .border_b_0()
            .child(
                div()
                    .id(self.id)
                    .child(Icon::new(IconName::Close))
                    .absolute()
                    .top(px(9.))
                    .right_0p5()
                    .opacity(0.)
                    .p_0p5()
                    .rounded_sm()
                    .hover(|style| style.bg(cx.theme().secondary_hover))
                    .group_hover("tab", |style| style.opacity(1.))
                    .on_click(move |_state, _window, cx| {
                        let chat_data = cx.global::<ChatData>().deref().clone();
                        chat_data.update(cx, |_chat_data, cx| {
                            cx.emit(ChatDataEvent::RemoveTab(self.id));
                        });
                    }),
            )
            .child(Label::new(self.name).text_sm())
            .px_6()
            .py_2()
            .bg(cx.theme().tab_active)
            .cursor_pointer()
    }
}

use gpui::{prelude::FluentBuilder, *};
use gpui_component::{ActiveTheme, Icon, IconName, h_flex, label::Label};
use std::ops::Deref;

use crate::{
    database::Conversation,
    store::{ChatData, ChatDataEvent},
};

#[derive(IntoElement, Clone)]
pub(crate) struct ConversationTabView {
    pub(crate) id: i32,
    pub(crate) icon: SharedString,
    pub(crate) name: SharedString,
}

impl From<&Conversation> for ConversationTabView {
    fn from(conversation: &Conversation) -> Self {
        Self {
            id: conversation.id,
            icon: SharedString::from(&conversation.icon),
            name: SharedString::from(&conversation.title),
        }
    }
}

impl RenderOnce for ConversationTabView {
    fn render(self, _window: &mut gpui::Window, cx: &mut gpui::App) -> impl gpui::IntoElement {
        let chat_data = cx.global::<ChatData>().read(cx);
        let active_id = chat_data
            .as_ref()
            .ok()
            .and_then(|data| data.active_tab())
            .map(|tab| tab.id);
        let is_active = active_id == Some(self.id);
        h_flex()
            .id(self.id)
            .group("tab")
            .gap_1()
            .relative()
            .child(
                div()
                    .id(self.id)
                    .child(Icon::new(IconName::Close).size_3())
                    .absolute()
                    .top(px(6.))
                    .right_1()
                    .opacity(0.)
                    .p_0p5()
                    .rounded_sm()
                    .hover(|style| style.bg(cx.theme().secondary_hover))
                    .group_hover("tab", |style| style.opacity(1.))
                    .on_click(move |_state, _window, cx| {
                        cx.stop_propagation();
                        let chat_data = cx.global::<ChatData>().deref().clone();
                        chat_data.update(cx, |_chat_data, cx| {
                            cx.emit(ChatDataEvent::RemoveTab(self.id));
                        });
                    }),
            )
            .child(Label::new(self.icon).text_xs().line_height(rems(0.75)))
            .child(Label::new(self.name).text_xs().line_height(rems(0.75)))
            .px_6()
            .py_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .when(is_active, |this| {
                this.bg(cx.theme().tab_active).border_b_0()
            })
            .on_click(move |_this, _window, cx| {
                let chat_data = cx.global::<ChatData>().deref().clone();
                chat_data.update(cx, move |_this, cx| {
                    cx.emit(ChatDataEvent::AddTab(self.id));
                });
            })
            .cursor_pointer()
    }
}

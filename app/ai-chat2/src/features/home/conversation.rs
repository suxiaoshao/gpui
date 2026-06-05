use crate::{database, foundation::I18n};
use ai_chat_core::ConversationId;
use gpui::*;
use gpui_component::{ActiveTheme, StyledExt, label::Label, v_flex};

#[derive(IntoElement)]
pub(crate) struct ConversationPage {
    conversation_id: ConversationId,
}

impl ConversationPage {
    pub(crate) fn new(conversation_id: ConversationId) -> Self {
        Self { conversation_id }
    }
}

impl RenderOnce for ConversationPage {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let conversation = database::repository(cx)
            .get_conversation(&self.conversation_id)
            .ok()
            .flatten();
        let title = conversation
            .as_ref()
            .map(|conversation| conversation.title.clone())
            .unwrap_or_else(|| cx.global::<I18n>().t("conversation-missing-title"));
        let subtitle = conversation
            .as_ref()
            .map(|conversation| {
                let item_count = conversation.last_item_seq.max(0);
                format!(
                    "{} · {}",
                    cx.global::<I18n>().t("conversation-opened-subtitle"),
                    item_count
                )
            })
            .unwrap_or_else(|| cx.global::<I18n>().t("conversation-missing-subtitle"));

        v_flex()
            .id("ai-chat2-conversation-page")
            .size_full()
            .min_w_0()
            .overflow_hidden()
            .items_center()
            .justify_center()
            .gap_2()
            .px_8()
            .py_12()
            .child(
                Label::new(title)
                    .text_size(px(24.))
                    .font_medium()
                    .text_color(cx.theme().foreground)
                    .truncate(),
            )
            .child(
                Label::new(subtitle)
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            )
    }
}

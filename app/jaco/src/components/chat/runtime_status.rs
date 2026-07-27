use gpui::{App, Entity, IntoElement, ParentElement, RenderOnce, SharedString, Styled};
use gpui_component::{ActiveTheme, Disableable, Sizable, button::Button, h_flex, label::Label};

use crate::{features::conversation, foundation::I18n};

#[derive(IntoElement)]
pub(crate) struct ConversationRuntimeStatus {
    message: SharedString,
    running: bool,
}

impl ConversationRuntimeStatus {
    pub(crate) fn from_runtime(
        runtime: &Entity<conversation::runtime::ConversationRuntimeStore>,
        cx: &App,
    ) -> Option<Self> {
        let runtime = runtime.read(cx);
        let operation = runtime.recovery();
        if matches!(operation, gpui_operation::refresh::Operation::Ready(_)) {
            return None;
        }
        Some(Self {
            message: operation
                .problem()
                .map(ToString::to_string)
                .unwrap_or_else(|| cx.global::<I18n>().t("conversation-runtime-recovering"))
                .into(),
            running: operation.is_running(),
        })
    }
}

impl RenderOnce for ConversationRuntimeStatus {
    fn render(self, _window: &mut gpui::Window, cx: &mut App) -> impl IntoElement {
        h_flex()
            .w_full()
            .items_center()
            .gap_2()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().warning)
            .bg(cx.theme().warning.opacity(0.08))
            .p_3()
            .child(
                Label::new(self.message)
                    .text_sm()
                    .text_color(cx.theme().warning)
                    .flex_1(),
            )
            .child(
                Button::new("conversation-runtime-retry")
                    .label(cx.global::<I18n>().t("resource-status-refresh"))
                    .small()
                    .loading(self.running)
                    .disabled(self.running)
                    .on_click(|_, _window, cx| {
                        if let Some(runtime) = crate::app::session::ready_runtime(cx) {
                            conversation::runtime::retry_recovery_if_needed(&runtime, cx);
                        }
                    }),
            )
    }
}

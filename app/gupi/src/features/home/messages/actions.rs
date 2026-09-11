use super::*;
use gpui_kit::component::{Icon, StyledExt, hover_card::HoverCard, notification::Notification};
use gpui_kit::prelude::FluentBuilder;
use std::time::Duration;

#[derive(IntoElement)]
pub(super) struct MessageActions {
    pub id: String,
    pub message: DisplayMessage,
}
impl RenderOnce for MessageActions {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(format!("copy-state-{}", self.id), cx, |_, _| {
            CopyState { reset: None }
        });
        let group = SharedString::from(format!("message-actions-{}", self.id));
        let user = self.message.role() == "user";
        let mut row = h_flex()
            .id(self.id.clone())
            .group(group.clone())
            .h_6()
            .min_w_0()
            .items_center()
            .gap_1();
        if user && let Some(timestamp) = metadata::timestamp(&self.message) {
            row = row.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(timestamp),
            );
        }
        row = row.child(CopyAction {
            state,
            id: format!("copy-{}", self.id),
            text: self.message.text(),
        });
        if !user {
            let fields = metadata::usage_fields(&self.message, cx);
            if !fields.is_empty() {
                let mut details = v_flex().min_w(px(240.)).gap_2().child(
                    div()
                        .font_semibold()
                        .child(t(cx, "conversation-usage-title")),
                );
                for (label, value) in fields {
                    details = details.child(
                        h_flex()
                            .gap_4()
                            .justify_between()
                            .child(div().text_color(cx.theme().muted_foreground).child(label))
                            .child(value),
                    );
                }
                row = row.child(
                    HoverCard::new(format!("usage-{}", self.id))
                        .anchor(Anchor::BottomLeft)
                        .trigger(
                            div()
                                .id(format!("usage-trigger-{}", self.id))
                                .h_5()
                                .w_5()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_sm()
                                .role(Role::Image)
                                .aria_label(t(cx, "conversation-usage-title"))
                                .focusable()
                                .tab_stop(true)
                                .hover(|s| s.bg(cx.theme().muted))
                                .child(Icon::new(IconName::ChartNoAxesColumn).size_3()),
                        )
                        .child(details),
                );
            }
            if let Some(total) = self.message.value["usage"]["totalTokens"].as_u64() {
                let label = if total >= 1000 {
                    format!("{:.1}k", total as f64 / 1000.)
                } else {
                    total.to_string()
                };
                row = row.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .opacity(0.)
                        .group_hover(group.clone(), |s| s.opacity(1.))
                        .child(label),
                );
            }
            if let Some(timestamp) = metadata::timestamp(&self.message) {
                row = row.child(
                    div()
                        .min_w_0()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .opacity(0.)
                        .group_hover(group, |s| s.opacity(1.))
                        .child(timestamp),
                );
            }
        }
        row
    }
}
struct CopyState {
    reset: Option<Task<()>>,
}
#[derive(IntoElement)]
struct CopyAction {
    state: Entity<CopyState>,
    id: String,
    text: String,
}
impl View for CopyAction {
    fn entity_id(&self) -> Option<EntityId> {
        Some(self.state.entity_id())
    }
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let copied = self.state.read(cx).reset.is_some();
        let label = t(
            cx,
            if copied {
                "conversation-copied"
            } else {
                "conversation-copy"
            },
        );
        Button::new(self.id)
            .ghost()
            .xsmall()
            .disabled(copied)
            .icon(
                Icon::new(if copied {
                    IconName::Check
                } else {
                    IconName::Copy
                })
                .when(copied, |icon| icon.text_color(cx.theme().success)),
            )
            .tooltip(label.clone())
            .accessibility_label(label)
            .on_click(move |_, window, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(self.text.clone()));
                if cx
                    .read_from_clipboard()
                    .and_then(|item| item.text())
                    .as_deref()
                    != Some(self.text.as_str())
                {
                    window.push_notification(
                        Notification::error(t(cx, "conversation-copy-failed")),
                        cx,
                    );
                    return;
                }
                self.state.update(cx, |state, cx| {
                    state.reset = Some(cx.spawn(async move |state, cx| {
                        cx.background_executor().timer(Duration::from_secs(2)).await;
                        let _ = state.update(cx, |state, cx| {
                            state.reset = None;
                            cx.notify();
                        });
                    }));
                    cx.notify();
                });
            })
    }
}

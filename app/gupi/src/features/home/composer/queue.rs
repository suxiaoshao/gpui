use super::*;
use crate::foundation::i18n::t_with_args;
use gpui_kit::component::collapsible::Collapsible;

impl HomeView {
    pub(super) fn render_queue(&self, key: &str, preview: bool, cx: &Context<Self>) -> AnyElement {
        let state = self.state.read(cx);
        let session = &state.sessions[key];
        let open = self.views.get(key).is_some_and(|view| view.queue_open);
        let disabled = preview || !state.can_clear_queue(key, cx);
        let loading = session.command.clearing_queue();
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("count", session.pending_count);
        let title = t_with_args(cx, "conversation-queue-title", &args);
        let target = key.to_owned();
        let mut header = h_flex()
            .items_center()
            .gap_1()
            .child(
                Button::new("toggle-queue")
                    .ghost()
                    .small()
                    .icon(if open {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    })
                    .label(title)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(view) = this.views.get_mut(&target) {
                            view.queue_open = !view.queue_open;
                            cx.notify();
                        }
                    })),
            )
            .child(div().flex_1());
        for (id, icon, label, restore) in [
            (
                "restore-queue",
                IconName::Undo2,
                "conversation-queue-restore",
                true,
            ),
            (
                "clear-queue",
                IconName::Trash,
                "conversation-queue-clear",
                false,
            ),
        ] {
            let target = key.to_owned();
            header = header.child(
                Button::new(id)
                    .ghost()
                    .small()
                    .icon(icon)
                    .tooltip(t(cx, label))
                    .accessibility_label(t(cx, label))
                    .disabled(disabled)
                    .loading(loading)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if restore && this.state.read(cx).can_clear_queue(&target, cx) {
                            this.input.focus_handle(cx).focus(window, cx);
                        }
                        this.state
                            .update(cx, |state, cx| state.clear_queue(&target, restore, cx));
                    })),
            );
        }
        let mut content = v_flex().gap_2().min_w_0().px_2().pb_2();
        if let Some(queue) = &session.queued {
            for (label, messages) in [
                ("conversation-queue-steer", &queue.steering),
                ("conversation-queue-follow-up", &queue.follow_up),
            ] {
                if messages.is_empty() {
                    continue;
                }
                let mut group = v_flex().min_w_0().gap_1().child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(t(cx, label)),
                );
                for text in messages {
                    let summary = if text.trim().is_empty() {
                        t(cx, "conversation-queue-no-text")
                    } else {
                        text.clone()
                    };
                    group =
                        group.child(div().min_w_0().whitespace_normal().text_sm().child(summary));
                }
                content = content.child(group);
            }
        } else {
            content = content.child(
                div()
                    .text_sm()
                    .child(t(cx, "conversation-queue-unavailable")),
            );
        }
        content = content.child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(t(cx, "conversation-queue-text-only")),
        );
        v_flex()
            .w_full()
            .min_w_0()
            .child(header)
            .child(
                Collapsible::new().open(open).content(
                    div()
                        .id("queue-content")
                        .max_h_48()
                        .overflow_y_scroll()
                        .child(content),
                ),
            )
            .into_any_element()
    }
}

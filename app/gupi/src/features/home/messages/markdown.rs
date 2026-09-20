//! Adapt Pi text snapshots to the component's incremental Markdown parser.
use gpui_kit::component::{
    message_scroller::MessageScrollerState,
    text::{TextView, TextViewState, TextViewStyle},
};
use gpui_kit::{
    App, AppContext, Entity, IntoElement, RenderOnce, StyleRefinement, Styled, Subscription,
    WeakEntity, Window, rems, transparent_black,
};

/// No extra layout surface: this renders the managed TextView directly.
#[derive(IntoElement)]
pub(super) struct Markdown {
    id: String,
    text: String,
    scroller: WeakEntity<MessageScrollerState>,
    embedded: bool,
    stream_fade: bool,
}

impl Markdown {
    pub fn new(id: String, text: String, scroller: WeakEntity<MessageScrollerState>) -> Self {
        Self {
            id,
            text,
            scroller,
            embedded: false,
            stream_fade: false,
        }
    }

    pub fn stream_fade(mut self) -> Self {
        self.stream_fade = true;
        self
    }

    pub fn embedded(mut self) -> Self {
        self.embedded = true;
        self
    }
}

struct MarkdownState {
    /// Last submitted snapshot, which may be ahead of the asynchronous parser.
    text: String,
    view: Entity<TextViewState>,
    _subscription: Subscription,
}

impl MarkdownState {
    fn new(text: String, scroller: WeakEntity<MessageScrollerState>, cx: &mut App) -> Self {
        let view = cx.new(|cx| TextViewState::markdown(&text, cx));
        let subscription = cx.observe(&view, move |_, cx| {
            // Parsing finishes after the RPC update's initial row measurement.
            let _ = scroller.update(cx, |scroller, cx| scroller.remeasure(cx));
        });
        Self {
            text,
            view,
            _subscription: subscription,
        }
    }

    fn sync(&mut self, text: String, cx: &mut App) {
        if text == self.text {
            return;
        }
        self.view.update(cx, |state, cx| {
            if let Some(delta) = text.strip_prefix(&self.text) {
                state.push_str(delta, cx);
            } else {
                state.set_text(&text, cx);
            }
        });
        self.text = text;
    }
}

impl RenderOnce for Markdown {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(format!("markdown-{}", self.id), cx, |_, cx| {
            MarkdownState::new(self.text.clone(), self.scroller, cx)
        });
        state.update(cx, |state, cx| state.sync(self.text, cx));
        let view = TextView::new(&state.read(cx).view)
            .selectable(true)
            .stream_fade(self.stream_fade);
        if self.embedded {
            view.style(
                TextViewStyle::default()
                    .paragraph_gap(rems(0.5))
                    .code_block(
                        StyleRefinement::default()
                            .p_0()
                            .border_0()
                            .rounded_none()
                            .bg(transparent_black()),
                    )
                    .inline_code(gpui_kit::HighlightStyle {
                        background_color: Some(transparent_black()),
                        ..Default::default()
                    }),
            )
        } else {
            view
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::TestAppContext;
    use gpui_kit::base::{Text, TextView as BaseTextView};
    use std::{cell::Cell, rc::Rc};

    #[gpui_kit::test]
    fn snapshots_can_overtake_parsing_without_duplicating_or_losing_text(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let scroller = cx.new(|cx| MessageScrollerState::new(1, cx));
        let mut markdown = cx
            .update(|cx| MarkdownState::new("开始\n\n```rust\n".into(), scroller.downgrade(), cx));
        cx.run_until_parked();
        let notifications = Rc::new(Cell::new(0));
        let observed = notifications.clone();
        let _subscription =
            cx.update(|cx| cx.observe(&scroller, move |_, _| observed.set(observed.get() + 1)));

        // Submit several snapshots before the background parser catches up,
        // including a duplicate and a code fence split across updates.
        let complete = "开始\n\n```rust\nfn main() {}\n```\n\n结束";
        cx.update(|cx| {
            for text in [
                "开始\n\n```rust\nfn",
                "开始\n\n```rust\nfn",
                "开始\n\n```rust\nfn main() {}\n``",
                complete,
            ] {
                markdown.sync(text.into(), cx);
            }
        });
        cx.run_until_parked();
        assert_eq!(
            cx.update(|cx| Text::from(BaseTextView::new(&markdown.view)).get_text(cx)),
            complete
        );
        assert!(
            notifications.get() > 0,
            "parsed content must remeasure the scroller"
        );

        // A final-answer projection can replace provisional text. Appends
        // arriving immediately afterward must extend the new baseline.
        cx.update(|cx| {
            markdown.sync("最终".into(), cx);
            markdown.sync("最终回答".into(), cx);
        });
        cx.run_until_parked();
        assert_eq!(
            cx.update(|cx| Text::from(BaseTextView::new(&markdown.view)).get_text(cx)),
            "最终回答"
        );
        notifications.set(0);
        cx.update(|cx| markdown.sync("最终回答".into(), cx));
        cx.run_until_parked();
        assert_eq!(notifications.get(), 0, "unchanged renders must settle");

        cx.update(|cx| markdown.sync(String::new(), cx));
        cx.run_until_parked();
        assert!(
            cx.update(|cx| Text::from(BaseTextView::new(&markdown.view)).get_text(cx))
                .is_empty()
        );
    }
}

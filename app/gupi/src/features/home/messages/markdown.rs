//! Adapt Pi text snapshots to the component's incremental Markdown parser.
use gpui_kit::component::{
    message_scroller::MessageScrollerState,
    text::{RenderedText, TextView, TextViewState},
};
use gpui_kit::{
    App, AppContext, Context, Entity, EventEmitter, IntoElement, RenderOnce, Styled, Subscription,
    WeakEntity, Window,
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    ops::Range,
    rc::Rc,
    time::Instant,
};

pub(in crate::features::home) type RowPositions = Rc<RefCell<HashMap<String, usize>>>;
type RowTarget = Option<(String, RowPositions)>;

/// Weak lookup shares the mounted Markdown entity with find, without retaining a
/// second transcript. Mounted elements and an open find bar own the entities.
#[derive(Clone, Default)]
pub(in crate::features::home) struct Registry(
    Rc<RefCell<HashMap<String, WeakEntity<MarkdownState>>>>,
);
impl Registry {
    pub fn get(
        &self,
        id: &str,
        text: String,
        scroller: WeakEntity<MessageScrollerState>,
        cx: &mut App,
    ) -> Entity<MarkdownState> {
        if let Some(state) = self.0.borrow().get(id).and_then(WeakEntity::upgrade) {
            state.update(cx, |state, cx| state.sync(text, cx));
            return state;
        }
        let state = cx.new(|cx| MarkdownState::new(text, scroller, cx));
        let mut entries = self.0.borrow_mut();
        entries.retain(|_, state| state.is_upgradable());
        entries.insert(id.to_owned(), state.downgrade());
        state
    }
}
pub(in crate::features::home) struct Parsed;
struct Reveal {
    epoch: Rc<Cell<u64>>,
    generation: u64,
    snapshot: RenderedText,
    range: Range<usize>,
    requested: Instant,
}

/// No extra layout surface: this renders the managed TextView directly.
#[derive(IntoElement)]
pub(super) struct Markdown {
    id: String,
    text: String,
    scroller: WeakEntity<MessageScrollerState>,
    row: RowTarget,
    stream_fade: bool,
    registry: Registry,
}

impl Markdown {
    pub fn new(
        id: String,
        text: String,
        scroller: WeakEntity<MessageScrollerState>,
        registry: Registry,
    ) -> Self {
        Self {
            id,
            text,
            scroller,
            row: None,
            stream_fade: false,
            registry,
        }
    }

    pub fn row(mut self, id: String, positions: RowPositions) -> Self {
        self.row = Some((id, positions));
        self
    }
    pub fn stream_fade(mut self) -> Self {
        self.stream_fade = true;
        self
    }
}

pub(in crate::features::home) struct MarkdownState {
    /// Last submitted snapshot, which may be ahead of the asynchronous parser.
    text: String,
    pub view: Entity<TextViewState>,
    _subscription: Subscription,
    row: Rc<RefCell<RowTarget>>,
    pending: Option<Reveal>,
}

impl EventEmitter<Parsed> for MarkdownState {}

impl MarkdownState {
    fn new(
        text: String,
        scroller: WeakEntity<MessageScrollerState>,
        cx: &mut Context<Self>,
    ) -> Self {
        let view = cx.new(|cx| TextViewState::markdown(&text, cx));
        let row: Rc<RefCell<RowTarget>> = Rc::default();
        let target = row.clone();
        let mut snapshot = view.read(cx).rendered_text();
        let subscription = cx.observe(&view, move |_, view, cx| {
            let current = view.read(cx).rendered_text();
            if current == snapshot {
                return; // Highlight/reveal notifications do not change row geometry.
            }
            snapshot = current;
            cx.emit(Parsed);
            // Parsing finishes after the RPC update's initial row measurement.
            let index = target
                .borrow()
                .as_ref()
                .and_then(|(id, positions)| positions.borrow().get(id).copied());
            if let Some(index) = index {
                let _ = scroller.update(cx, |scroller, cx| {
                    scroller.remeasure_items(index..index + 1, cx);
                });
            }
        });
        Self {
            text,
            view,
            _subscription: subscription,
            row,
            pending: None,
        }
    }

    pub fn sync(&mut self, text: String, cx: &mut App) {
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
    pub fn reveal(&mut self, range: Range<usize>, epoch: Rc<Cell<u64>>, cx: &App) {
        self.pending = Some(Reveal {
            generation: epoch.get(),
            epoch,
            range,
            snapshot: self.view.read(cx).rendered_text(),
            requested: Instant::now(),
        });
    }
    fn render_view(&mut self, row: RowTarget, cx: &mut App) -> Entity<TextViewState> {
        *self.row.borrow_mut() = row;
        if let Some(reveal) = self.pending.take()
            && reveal.epoch.get() == reveal.generation
            && reveal.requested.elapsed().as_secs_f32() < 1.
            && reveal.snapshot == self.view.read(cx).rendered_text()
        {
            self.view.update(cx, |view, cx| {
                let _ = view.reveal_range(reveal.range, cx);
            });
        }
        self.view.clone()
    }
}

impl RenderOnce for Markdown {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let retained = window.use_keyed_state(format!("markdown-{}", self.id), cx, |_, cx| {
            self.registry
                .get(&self.id, self.text.clone(), self.scroller.clone(), cx)
        });
        let state = retained.read(cx).clone();
        let view = state.update(cx, |state, cx| {
            state.sync(self.text, cx);
            state.render_view(self.row, cx)
        });
        // MessageContent aligns children instead of stretching them. Give the
        // Markdown root the available width before its list items are measured.
        TextView::new(&view)
            .plugin(super::resources::ResourceLinks)
            .w_full()
            .min_w_0()
            .selectable(true)
            .stream_fade(self.stream_fade)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::{
        ParentElement, Render, TestAppContext,
        component::{
            Root,
            bubble::{Bubble, BubbleContent},
            message::{Message, MessageAlignment, MessageContent},
            text::RangeHighlight,
        },
        div, px, size,
        test::TestWindowExt,
    };

    struct WidthFixture {
        registry: Registry,
        markdown: Entity<MarkdownState>,
        scroller: Entity<MessageScrollerState>,
        width: f32,
        user: bool,
    }

    impl Render for WidthFixture {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let text = Markdown::new(
                "width-fixture".into(),
                self.markdown.read(cx).text.clone(),
                self.scroller.downgrade(),
                self.registry.clone(),
            );
            let message = if self.user {
                Message::new().alignment(MessageAlignment::End).content(
                    MessageContent::new().bubble(
                        Bubble::new().content(BubbleContent::new().child(div().child(text))),
                    ),
                )
            } else {
                Message::new().content(MessageContent::new().child(text))
            };
            div().w(px(self.width)).child(message)
        }
    }

    #[gpui_kit::test]
    fn file_link_plugin_preserves_offscreen_find_coordinates(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let text = "前文 [report.txt](file:///tmp/report.txt) 后文";
        let registry = Registry::default();
        let scroller = cx.new(|cx| MessageScrollerState::new(1, cx));
        let state =
            cx.update(|cx| registry.get("width-fixture", text.into(), scroller.downgrade(), cx));
        cx.run_until_parked();
        let before = cx.read(|cx| {
            state
                .read(cx)
                .view
                .read(cx)
                .rendered_text()
                .as_str()
                .to_owned()
        });
        let start = before.find("report.txt").unwrap();
        state.update(cx, |state, cx| {
            state.view.update(cx, |view, cx| {
                view.set_range_highlights(
                    [RangeHighlight::new(
                        start..start + "report.txt".len(),
                        gpui_kit::rgb(0xffff00),
                    )],
                    cx,
                )
                .unwrap()
            });
        });
        let (_, visual) = cx.add_window_view(|window, cx| {
            let view = cx.new(|_| WidthFixture {
                registry,
                markdown: state.clone(),
                scroller,
                width: 600.,
                user: true,
            });
            Root::new(view, window, cx)
        });
        visual.run_until_parked();
        visual.update(|window, cx| {
            window.render_frame(cx);
            assert_eq!(
                state.read(cx).view.read(cx).rendered_text().as_str(),
                before
            );
            state.read(cx).view.clone().update(cx, |view, cx| {
                view.reveal_range(start..start + "report.txt".len(), cx)
                    .unwrap();
            });
            window.remove_window();
        });
    }

    #[gpui_kit::test]
    fn markdown_wraps_in_message_content_without_widening_short_bubbles(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let sentence = "这里是用于检查布局的普通文字，包含多个段落和正常的中文标点。".repeat(12);
        for width in [420., 820.] {
            for (user, text) in [
                (false, sentence.clone()),
                (false, format!("- {sentence}")),
                (false, format!("- `ITEM-001` {sentence}")),
                (true, "你好".into()),
                (true, sentence.clone()),
                (true, format!("- `ITEM-001` {sentence}")),
            ] {
                let registry = Registry::default();
                let scroller = cx.new(|cx| MessageScrollerState::new(1, cx));
                let markdown = cx.update(|cx| {
                    registry.get("width-fixture", text.clone(), scroller.downgrade(), cx)
                });
                let (_, visual) = cx.add_window_view(|window, cx| {
                    let view = cx.new(|_| WidthFixture {
                        registry,
                        markdown: markdown.clone(),
                        scroller,
                        width,
                        user,
                    });
                    Root::new(view, window, cx)
                });
                visual.simulate_resize(size(px(1000.), px(1200.)));
                visual.run_until_parked();
                visual.update(|window, cx| {
                    window.render_frame(cx);
                    let bounds = markdown.read(cx).view.read(cx).bounds();
                    assert!(bounds.left() >= px(0.) && bounds.right() <= px(width));
                    if user && text == "你好" {
                        assert!(bounds.size.width < px(100.), "short bubble: {bounds:?}");
                        assert!(bounds.left() > px(width / 2.));
                    } else {
                        // Catch both intrinsic-width overflow and list items collapsing
                        // to marker width and wrapping into thousands of pixels tall.
                        assert!(bounds.size.width > px(width / 2.), "{bounds:?}");
                        assert!(bounds.size.height > px(30.) && bounds.size.height < px(600.));
                    }
                    window.remove_window();
                });
                visual.run_until_parked();
            }
        }
    }

    #[gpui_kit::test]
    fn snapshots_can_overtake_parsing_without_duplicating_or_losing_text(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let scroller = cx.new(|cx| MessageScrollerState::new(1, cx));
        let markdown =
            cx.new(|cx| MarkdownState::new("开始\n\n```rust\n".into(), scroller.downgrade(), cx));
        markdown.update(cx, |state, _| {
            *state.row.borrow_mut() = Some((
                "row".into(),
                Rc::new(RefCell::new(HashMap::from([("row".into(), 0)]))),
            ))
        });
        cx.run_until_parked();
        let notifications = Rc::new(Cell::new(0));
        let observed = notifications.clone();
        let _subscription =
            cx.update(|cx| cx.observe(&scroller, move |_, _| observed.set(observed.get() + 1)));
        let complete = "开始\n\n```rust\nfn main() {}\n```\n\n结束";
        markdown.update(cx, |state, cx| {
            for text in [
                "开始\n\n```rust\nfn",
                "开始\n\n```rust\nfn",
                "开始\n\n```rust\nfn main() {}\n``",
                complete,
            ] {
                state.sync(text.into(), cx);
            }
        });
        cx.run_until_parked();
        let text = |cx: &mut App| {
            markdown
                .read(cx)
                .view
                .read(cx)
                .rendered_text()
                .as_str()
                .to_owned()
        };
        assert!(cx.update(text).contains("fn main() {}"));
        assert!(notifications.get() > 0);
        markdown.update(cx, |state, cx| {
            state.sync("最终".into(), cx);
            state.sync("最终回答".into(), cx);
        });
        cx.run_until_parked();
        assert_eq!(cx.update(text).trim(), "最终回答");
        notifications.set(0);
        markdown.update(cx, |state, cx| state.sync("最终回答".into(), cx));
        cx.run_until_parked();
        assert_eq!(notifications.get(), 0);
        // Painting a result and navigating it must not trigger another row measurement.
        markdown.update(cx, |state, cx| {
            state.view.update(cx, |view, cx| {
                view.set_range_highlights([RangeHighlight::new(0..6, gpui_kit::rgb(0xffff00))], cx)
                    .unwrap();
                view.reveal_range(0..6, cx).unwrap();
            })
        });
        cx.run_until_parked();
        assert_eq!(
            notifications.get(),
            0,
            "highlight/reveal is not a content change"
        );
        markdown.update(cx, |state, cx| state.sync(String::new(), cx));
        cx.run_until_parked();
        assert!(cx.update(text).is_empty());
    }
}

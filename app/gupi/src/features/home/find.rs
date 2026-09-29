//! Current-branch find over the same Markdown entities used by the transcript.
use super::messages::markdown::{MarkdownState, Parsed, Registry};
use super::*;
use crate::foundation::i18n::t_with_args;
use fluent_bundle::FluentArgs;
use gpui_kit::component::{
    input::{Escape, Input, InputState},
    text::{RangeHighlight, RenderedText},
    toolbar::Toolbar,
};
use regex::{Regex, RegexBuilder};
use std::{cell::Cell, ops::Range};

pub(super) struct Source {
    pub id: String,
    pub row: String,
    pub process: Option<String>,
    pub text: String,
}
struct Document {
    id: String,
    row: String,
    process: Option<String>,
    markdown: Entity<MarkdownState>,
    snapshot: Option<RenderedText>,
    ranges: Vec<Range<usize>>,
    painted: Option<(Option<Range<usize>>, Hsla, Hsla)>,
    _subscription: Subscription,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Hit {
    id: String,
    range: Range<usize>,
}
pub(super) enum FindEvent {
    Close,
    Reveal {
        row: String,
        process: Option<String>,
    },
}
pub(super) struct FindBar {
    query: Entity<InputState>,
    key: String,
    registry: Registry,
    scroller: WeakEntity<MessageScrollerState>,
    documents: Vec<Document>,
    matcher: Option<Regex>,
    hits: Vec<Hit>,
    current: Option<Hit>,
    reveal_first: bool,
    epoch: Rc<Cell<u64>>,
    previous_focus: Option<FocusHandle>,
    _subscription: Subscription,
}
impl EventEmitter<FindEvent> for FindBar {}
impl FindBar {
    fn new(
        key: String,
        registry: Registry,
        scroller: WeakEntity<MessageScrollerState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let query = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t(cx, "conversation-find-placeholder"))
        });
        let subscription = cx.subscribe(&query, |this, query, event, cx| {
            if matches!(event, InputEvent::Change) {
                let value = query.read(cx).value();
                this.matcher = matcher(&value);
                this.current = None;
                this.reveal_first = !value.is_empty();
                this.cancel_reveal();
                this.refresh(true, cx);
            }
        });
        Self {
            query,
            key,
            registry,
            scroller,
            documents: vec![],
            matcher: None,
            hits: vec![],
            current: None,
            reveal_first: false,
            epoch: Rc::new(Cell::new(0)),
            previous_focus: window.focused(cx),
            _subscription: subscription,
        }
    }
    pub fn sync(&mut self, sources: Vec<Source>, cx: &mut Context<Self>) {
        // New RPC content must never consume a previous query's deferred jump.
        self.reveal_first = false;
        let mut previous: HashMap<_, _> = self
            .documents
            .drain(..)
            .map(|d| (d.id.clone(), d))
            .collect();
        self.documents = sources
            .into_iter()
            .map(|source| {
                let id = format!("{}-{}", self.key, source.id);
                let markdown = self
                    .registry
                    .get(&id, source.text, self.scroller.clone(), cx);
                if let Some(mut document) = previous.remove(&id) {
                    document.row = source.row;
                    document.process = source.process;
                    return document;
                }
                let subscription =
                    cx.subscribe(&markdown, |this, _, _: &Parsed, cx| this.refresh(false, cx));
                Document {
                    id,
                    row: source.row,
                    process: source.process,
                    markdown,
                    snapshot: None,
                    ranges: vec![],
                    painted: None,
                    _subscription: subscription,
                }
            })
            .collect();
        for document in previous.into_values() {
            document
                .markdown
                .read(cx)
                .view
                .clone()
                .update(cx, |view, cx| view.clear_range_highlights(cx));
        }
        self.refresh(false, cx);
    }
    fn cancel_reveal(&self) {
        self.epoch.set(self.epoch.get().wrapping_add(1));
    }
    fn refresh(&mut self, query_changed: bool, cx: &mut Context<Self>) {
        for document in &mut self.documents {
            let snapshot = document.markdown.read(cx).view.read(cx).rendered_text();
            if query_changed || document.snapshot.as_ref() != Some(&snapshot) {
                document.ranges = matches(self.matcher.as_ref(), snapshot.as_str());
                document.snapshot = Some(snapshot);
                // The parser may have remapped or discarded its old highlights.
                document.painted = None;
            }
        }
        self.hits = self
            .documents
            .iter()
            .flat_map(|d| {
                d.ranges.iter().map(|range| Hit {
                    id: d.id.clone(),
                    range: range.clone(),
                })
            })
            .collect();
        if self
            .current
            .as_ref()
            .is_some_and(|hit| !self.hits.contains(hit))
        {
            self.current = None;
            self.cancel_reveal();
        }
        if self.current.is_none() {
            self.current = self.hits.first().cloned();
        }
        self.paint(cx);
        if self.reveal_first && self.current.is_some() {
            self.reveal_first = false;
            self.reveal(cx);
        }
        cx.notify();
    }
    fn paint(&mut self, cx: &mut App) {
        let normal = cx.theme().primary.opacity(0.18);
        let active = cx.theme().selection;
        for document in &mut self.documents {
            let current = self
                .current
                .as_ref()
                .filter(|hit| hit.id == document.id)
                .map(|hit| hit.range.clone());
            let appearance = (current.clone(), normal, active);
            if document.painted.as_ref() == Some(&appearance) {
                continue;
            }
            let view = document.markdown.read(cx).view.clone();
            view.update(cx, |view, cx| {
                if document.ranges.is_empty() {
                    view.clear_range_highlights(cx);
                } else {
                    let highlights = document.ranges.iter().map(|range| {
                        RangeHighlight::new(
                            range.clone(),
                            if current.as_ref() == Some(range) {
                                active
                            } else {
                                normal
                            },
                        )
                    });
                    if let Err(error) = view.set_range_highlights(highlights, cx) {
                        tracing::warn!(%error, "failed to highlight conversation find matches");
                    }
                }
            });
            document.painted = Some(appearance);
        }
    }

    fn navigate(&mut self, backwards: bool, cx: &mut Context<Self>) {
        if self.hits.is_empty() {
            return;
        }
        let current = self
            .current
            .as_ref()
            .and_then(|hit| self.hits.iter().position(|h| h == hit))
            .unwrap_or(0);
        let ix = if backwards {
            (current + self.hits.len() - 1) % self.hits.len()
        } else {
            (current + 1) % self.hits.len()
        };
        self.current = Some(self.hits[ix].clone());
        self.paint(cx);
        self.reveal(cx);
        cx.notify();
    }
    fn reveal(&mut self, cx: &mut Context<Self>) {
        self.cancel_reveal();
        let Some(hit) = &self.current else { return };
        let Some(document) = self.documents.iter().find(|d| d.id == hit.id) else {
            return;
        };
        document.markdown.update(cx, |state, cx| {
            state.reveal(hit.range.clone(), self.epoch.clone(), cx)
        });
        cx.emit(FindEvent::Reveal {
            row: document.row.clone(),
            process: document.process.clone(),
        });
    }
    fn clear(&mut self, cx: &mut App) {
        self.cancel_reveal();
        for document in &self.documents {
            document
                .markdown
                .read(cx)
                .view
                .clone()
                .update(cx, |view, cx| view.clear_range_highlights(cx));
        }
        self.documents.clear();
    }
}
impl Drop for FindBar {
    fn drop(&mut self) {
        self.cancel_reveal();
    }
}
fn matcher(query: &str) -> Option<Regex> {
    if query.is_empty() {
        return None;
    }
    RegexBuilder::new(&regex::escape(query))
        .case_insensitive(true)
        .build()
        .ok()
}
fn matches(matcher: Option<&Regex>, text: &str) -> Vec<Range<usize>> {
    matcher
        .map(|m| m.find_iter(text).map(|hit| hit.range()).collect())
        .unwrap_or_default()
}
impl Render for FindBar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // A theme refresh can change colors without changing the query or text.
        self.paint(cx);
        let mut args = FluentArgs::new();
        let current = self
            .current
            .as_ref()
            .and_then(|hit| self.hits.iter().position(|h| h == hit))
            .map_or(0, |ix| ix + 1);
        args.set("current", current);
        args.set("total", self.hits.len());
        let count = t_with_args(cx, "conversation-find-count", &args);
        div()
            .id("conversation-find-scope")
            .debug_selector(|| "conversation-find-scope".into())
            .w_full()
            .key_context("GupiFind")
            .capture_action(cx.listener(|this, action: &Enter, window, cx| {
                if this.query.update(cx, |input, cx| {
                    input.marked_text_range(window, cx).is_some()
                }) {
                    cx.propagate();
                    return;
                }
                this.navigate(action.shift, cx);
                cx.stop_propagation();
            }))
            .capture_action(cx.listener(|_, _: &Escape, _, cx| {
                cx.emit(FindEvent::Close);
                cx.stop_propagation();
            }))
            .child(
                Toolbar::new("conversation-find-bar")
                    .small()
                    .w_full()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .content(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Input::new(&self.query).small().cleanable(true)),
                    )
                    .content(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(count),
                    )
                    .child(
                        Button::new("find-previous")
                            .icon(IconName::ChevronUp)
                            .disabled(self.hits.is_empty())
                            .tooltip(t(cx, "conversation-find-previous"))
                            .accessibility_label(t(cx, "conversation-find-previous"))
                            .on_click(cx.listener(|this, _, _, cx| this.navigate(true, cx))),
                    )
                    .child(
                        Button::new("find-next")
                            .icon(IconName::ChevronDown)
                            .disabled(self.hits.is_empty())
                            .tooltip(t(cx, "conversation-find-next"))
                            .accessibility_label(t(cx, "conversation-find-next"))
                            .on_click(cx.listener(|this, _, _, cx| this.navigate(false, cx))),
                    )
                    .child(
                        Button::new("find-close")
                            .icon(IconName::X)
                            .tooltip(t(cx, "conversation-find-close"))
                            .accessibility_label(t(cx, "conversation-find-close"))
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(FindEvent::Close))),
                    ),
            )
    }
}
impl HomeView {
    pub(super) fn open_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette.is_some()
            || self.command_panel.is_some()
            || window.has_active_dialog(cx)
            || self.has_image_preview(cx)
        {
            return;
        }
        if let Some(find) = &self.find {
            find.read(cx)
                .query
                .clone()
                .update(cx, |query, cx| query.focus(window, cx));
            return;
        }
        let Some(key) = self.shown_key.clone() else {
            return;
        };
        let Some(view) = self.views.get(&key) else {
            return;
        };
        let sources = view
            .rows
            .iter()
            .flat_map(messages::ChatRow::find_sources)
            .collect();
        let find = cx.new(|cx| {
            FindBar::new(
                key,
                view.markdown.clone(),
                view.scroller.downgrade(),
                window,
                cx,
            )
        });
        self.find_subscription =
            Some(
                cx.subscribe_in(&find, window, |this, _, event, window, cx| match event {
                    FindEvent::Close => this.close_find(true, window, cx),
                    FindEvent::Reveal { row, process } => {
                        let Some(view) = this
                            .shown_key
                            .as_ref()
                            .and_then(|key| this.views.get_mut(key))
                        else {
                            return;
                        };
                        let Some(ix) = view.row_positions.borrow().get(row).copied() else {
                            return;
                        };
                        if let Some(process) = process {
                            view.process_open.insert(process.clone(), true);
                        }
                        view.scroller.update(cx, |scroller, cx| {
                            scroller.remeasure_items(ix..ix + 1, cx);
                            scroller.scroll_to_item(ix, cx);
                        });
                        cx.notify();
                    }
                }),
            );
        find.update(cx, |find, cx| {
            find.sync(sources, cx);
            find.query.update(cx, |input, cx| input.focus(window, cx));
        });
        self.find = Some(find);
        cx.notify();
    }
    pub(super) fn close_find(
        &mut self,
        restore: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(find) = self.find.take() {
            find.update(cx, |find, cx| {
                find.clear(cx);
                if restore && let Some(focus) = find.previous_focus.take() {
                    focus.focus(window, cx);
                }
            });
            self.find_subscription = None;
            cx.notify();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FindBar, FindEvent, Registry, Source, matcher, matches};
    use gpui_kit::component::message_scroller::MessageScrollerState;
    use gpui_kit::{AppContext, TestAppContext};
    use std::{cell::Cell, rc::Rc};
    #[test]
    fn literal_matching_preserves_original_unicode_byte_ranges() {
        let text = "é HELLO 中 hello 👋 [.*]";
        assert_eq!(matches(matcher("hello").as_ref(), text), vec![3..8, 13..18]);
        assert_eq!(matches(matcher("É").as_ref(), text), vec![0..2]);
        assert_eq!(matches(matcher("[.*]").as_ref(), text), vec![24..28]);
        assert!(matches(matcher("").as_ref(), text).is_empty());
    }
    #[gpui_kit::test]
    fn rendered_matches_navigation_streaming_and_cleanup(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::foundation::i18n::apply(crate::state::config::AppLanguage::English, cx);
        });
        let registry = Registry::default();
        let scroller = cx.new(|cx| MessageScrollerState::new(2, cx));
        let mut find = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view =
                cx.new(|cx| FindBar::new("key".into(), registry, scroller.downgrade(), window, cx));
            find = Some(view.clone());
            gpui_kit::component::Root::new(view, window, cx)
        });
        let find = find.unwrap();
        let source = |id: &str, text: &str| Source {
            id: id.into(),
            row: id.into(),
            process: None,
            text: text.into(),
        };
        find.update(cx, |find, cx| {
            find.sync(
                vec![
                    source("one", "# He**llo**\n\n```rust\nHELLO\n```"),
                    source("two", "| heading |\n|---|\n| hello |"),
                ],
                cx,
            )
        });
        cx.run_until_parked();
        find.update_in(cx, |find, window, cx| {
            find.query.update(cx, |query, cx| query.focus(window, cx))
        });
        cx.simulate_input("hello");
        cx.run_until_parked();
        find.update(cx, |find, cx| {
            assert_eq!(find.hits.len(), 3);
            assert_eq!(find.current, find.hits.first().cloned());
            find.navigate(true, cx);
            assert_eq!(find.current, find.hits.last().cloned());
            find.navigate(false, cx);
            assert_eq!(find.current, find.hits.first().cloned());
        });
        cx.simulate_keystrokes("shift-enter");
        find.read_with(cx, |find, _| {
            assert_eq!(find.current, find.hits.last().cloned())
        });
        cx.simulate_keystrokes("enter");
        find.read_with(cx, |find, _| {
            assert_eq!(find.current, find.hits.first().cloned())
        });
        let reveals = Rc::new(Cell::new(0));
        let seen = reveals.clone();
        let _subscription = cx.update(|_, cx| {
            cx.subscribe(&find, move |_, event: &FindEvent, _| {
                if matches!(event, FindEvent::Reveal { .. }) {
                    seen.set(seen.get() + 1);
                }
            })
        });
        find.update(cx, |find, cx| {
            find.sync(
                vec![
                    source("one", "# He**llo**\n\n```rust\nHELLO\n```\nhello"),
                    source("two", "| heading |\n|---|\n| hello |"),
                ],
                cx,
            )
        });
        cx.run_until_parked();
        find.update(cx, |find, cx| {
            assert_eq!(find.hits.len(), 4);
            assert_eq!(reveals.get(), 0, "streaming updates must not scroll");
            find.sync(vec![source("final", "No match")], cx);
        });
        cx.run_until_parked();
        find.update(cx, |find, cx| {
            assert!(find.hits.is_empty());
            assert!(find.current.is_none());
            let epoch = find.epoch.get();
            find.clear(cx);
            assert_ne!(epoch, find.epoch.get());
            assert!(find.documents.is_empty());
        });
    }
}

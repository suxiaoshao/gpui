use super::*;
use crate::state::{conversation::Session, history::DisplayMessage};
use gpui_kit::component::{
    bubble::{Bubble, BubbleContent, BubbleVariant},
    menu::{ContextMenuExt, PopupMenuItem},
    message::{Message, MessageAlignment, MessageContent, MessageFooter},
    message_scroller::MessageScroller,
    text::TextView,
};
use gpui_kit::prelude::FluentBuilder;

mod actions;
mod activity;
mod metadata;
mod presentation;
mod viewport;
use activity::{Activity, ActivityBlock, RunContent, ToolStatus};
use presentation::Disclosure;

#[derive(Clone)]
pub(super) struct ChatRow {
    pub id: String,
    pub entries: Vec<String>,
    kind: RowKind,
}
impl ChatRow {
    fn spacing_before(&self, previous: Option<&Self>) -> Rems {
        let Some(previous) = previous else {
            return rems(0.5);
        };
        match (&previous.kind, &self.kind) {
            (RowKind::Run { .. } | RowKind::Compaction(_), RowKind::Compaction(_))
            | (RowKind::Compaction(_), RowKind::Run { .. }) => rems(0.75),
            _ => rems(3.),
        }
    }

    pub(super) fn reveal(&self, entry: &str, open: &mut HashMap<String, bool>) {
        if !self.entries.iter().any(|id| id == entry) {
            return;
        }
        match &self.kind {
            RowKind::Compaction(_) => {
                open.insert(self.id.clone(), true);
            }
            RowKind::Run { messages, .. } => {
                open.insert(self.id.clone(), true);
                let content = RunContent::project(messages, &[], false);
                for block in content.blocks() {
                    if let ActivityBlock::Group { id, items } = block {
                        for item in items {
                            if let Activity::Tool(tool) = item
                                && tool.entries.iter().any(|id| id == entry)
                            {
                                open.insert(id.clone(), true);
                                open.insert(tool.id.clone(), true);
                            }
                        }
                    }
                }
            }
            RowKind::User(_) | RowKind::BranchSummary(_) => {}
        }
    }
}
#[derive(Clone)]
enum RowKind {
    User(DisplayMessage),
    Run {
        messages: Vec<DisplayMessage>,
        active: bool,
    },
    Compaction(DisplayMessage),
    BranchSummary(DisplayMessage),
}
pub(super) fn project(session: &Session, preview: Option<&str>) -> Vec<ChatRow> {
    let active = session
        .active_messages()
        .filter(|_| preview.is_none_or(|id| session.history().on_current_path(id)));
    project_rows(session.messages(preview), active)
}
fn project_rows(
    messages: Vec<DisplayMessage>,
    active_messages: Option<&HashSet<String>>,
) -> Vec<ChatRow> {
    let mut rows: Vec<ChatRow> = vec![];
    for m in messages {
        let entries = m.entry.iter().cloned().collect();
        if m.role() == "user" {
            rows.push(ChatRow {
                id: m.id.clone(),
                entries,
                kind: RowKind::User(m),
            });
        } else if matches!(m.role(), "compaction" | "branch_summary") {
            // A compaction is a chronological assistant activity, not a container
            // for earlier messages. Keep both adjacent runs' answers intact.
            rows.push(ChatRow {
                id: m.id.clone(),
                entries,
                kind: if m.role() == "compaction" {
                    RowKind::Compaction(m)
                } else {
                    RowKind::BranchSummary(m)
                },
            });
        } else if let Some(ChatRow {
            entries,
            kind: RowKind::Run { messages, .. },
            ..
        }) = rows.last_mut()
        {
            entries.extend(m.entry.clone());
            messages.push(m);
        } else {
            rows.push(ChatRow {
                id: format!("run-{}", m.id),
                entries,
                kind: RowKind::Run {
                    messages: vec![m],
                    active: false,
                },
            });
        }
    }
    if let Some(active_messages) = active_messages {
        for row in &mut rows {
            if let RowKind::Run { messages, active } = &mut row.kind {
                *active = messages
                    .iter()
                    .any(|m| active_messages.contains(&m.signature()));
            }
        }
        // A reconnected streaming session may precede our first delivered event.
        if active_messages.is_empty()
            && let Some(ChatRow {
                kind: RowKind::Run { active, .. },
                ..
            }) = rows.last_mut()
        {
            *active = true;
        }
    }
    rows
}
fn text_view(id: String, text: String) -> TextView {
    TextView::markdown(id, text).selectable(true)
}
impl HomeView {
    pub(super) fn render_messages(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(key) = self.shown_key.clone() else {
            return div().flex_1().into_any_element();
        };
        let Some(view) = self.views.get(&key) else {
            return div().flex_1().into_any_element();
        };
        let owner = cx.entity().downgrade();
        let rows = view.rows.clone();
        let mut body = v_flex().flex_1().min_h_0().w_full();
        // Restored selection arrives before the asynchronous catalog scan has
        // materialized its session. A history-list notification can render here.
        let Some(session) = self.state.read(cx).sessions.get(&key) else {
            return body.into_any_element();
        };
        use crate::state::conversation::content::BodyState;
        match session.body_state() {
            BodyState::New => {
                return body
                    .child(
                        v_flex()
                            .flex_1()
                            .justify_center()
                            .items_center()
                            .gap_2()
                            .child(div().text_xl().child(t(cx, "conversation-welcome")))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(t(cx, "conversation-welcome-hint")),
                            ),
                    )
                    .into_any_element();
            }
            BodyState::Loading(stage) => {
                return body.child(content::skeleton(stage, cx)).into_any_element();
            }
            BodyState::Failed(error) => {
                return body
                    .child(self.render_content_error("retry-messages", error, cx))
                    .into_any_element();
            }
            BodyState::Ready => {}
            BodyState::Refreshing(stage) => body = body.child(content::refreshing(stage, cx)),
            BodyState::RefreshFailed(error) => {
                body = body.child(self.render_content_error("retry-messages", error, cx))
            }
        }
        if view
            .preview
            .as_deref()
            .is_some_and(|id| !session.history().on_current_path(id))
        {
            body = body.child(
                h_flex()
                    .px_4()
                    .py_2()
                    .gap_2()
                    .bg(cx.theme().muted)
                    .child(div().flex_1().child(t(cx, "conversation-preview")))
                    .child(
                        Button::new("return-current")
                            .small()
                            .label(t(cx, "conversation-return-current"))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.return_current(window, cx)),
                            ),
                    ),
            );
        }
        if rows.is_empty() {
            body = body.child(
                v_flex()
                    .flex_1()
                    .justify_center()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(t(cx, "conversation-empty-history")),
                    ),
            );
        } else {
            body = body.child(
                MessageScroller::new(
                    format!("messages-{key}"),
                    view.scroller.clone(),
                    move |index, _window, cx| {
                        let Some(row) = rows.get(index) else {
                            return div().into_any_element();
                        };
                        owner
                            .read_with(cx, |this, cx| {
                                div()
                                    .w_full()
                                    .flex()
                                    .justify_center()
                                    .px_5()
                                    .pt(row.spacing_before(
                                        index.checked_sub(1).and_then(|i| rows.get(i)),
                                    ))
                                    .when(index + 1 == rows.len(), |row| row.pb_2())
                                    .child(
                                        div()
                                            .w_full()
                                            .max_w(px(820.))
                                            .child(this.render_row(&key, row, cx)),
                                    )
                                    .into_any_element()
                            })
                            .unwrap_or_else(|_| div().into_any_element())
                    },
                )
                // Gupi owns spacing between row kinds; the scroller's default
                // bottom padding would separate compaction from its run.
                .with_row_style(StyleRefinement::default().pb_0())
                .with_jump_button_label(t(cx, "conversation-bottom"))
                .size_full(),
            );
        }
        body.into_any_element()
    }
    fn toggle_process(&mut self, key: &str, id: &str, current: bool, cx: &mut Context<Self>) {
        if let Some(view) = self.views.get_mut(key) {
            view.process_open.insert(id.to_owned(), !current);
            view.scroller.update(cx, |s, cx| s.remeasure(cx));
            cx.notify();
        }
    }
    fn render_row(&self, key: &str, row: &ChatRow, cx: &App) -> AnyElement {
        match &row.kind {
            RowKind::User(m) => {
                let state = self.state.clone();
                let target = key.to_owned();
                let entry = m.entry.clone();
                let can_fork = self.state.read(cx).sessions.get(key).is_some_and(|s| {
                    !s.settings_busy()
                        && !s.model_change.unconfirmed()
                        && entry
                            .as_ref()
                            .is_some_and(|id| s.fork_options().iter().any(|m| &m.entry_id == id))
                });
                let label = t(cx, "conversation-fork");
                div()
                    .id(row.id.clone())
                    .child(
                        Message::new()
                            .alignment(MessageAlignment::End)
                            .content(
                                MessageContent::new().bubble(
                                    Bubble::new().with_variant(BubbleVariant::Muted).content(
                                        BubbleContent::new()
                                            .child(text_view(format!("text-{}", m.id), m.text())),
                                    ),
                                ),
                            )
                            .footer(MessageFooter::new().child(actions::MessageActions {
                                id: format!("{key}-{}", m.id),
                                message: m.clone(),
                            })),
                    )
                    .context_menu(move |menu, _, _| {
                        let state = state.clone();
                        let key = target.clone();
                        let entry = entry.clone();
                        menu.item(
                            PopupMenuItem::new(label.clone())
                                .disabled(!can_fork)
                                .on_click(move |_, _, cx| {
                                    if let Some(entry) = entry.clone() {
                                        state.update(cx, |s, cx| s.fork(&key, entry, cx));
                                    }
                                }),
                        )
                    })
                    .into_any_element()
            }
            RowKind::Compaction(m) => Message::new()
                .content(
                    MessageContent::new().child(
                        self.fold(
                            key,
                            &row.id,
                            Disclosure::compaction(t(cx, "conversation-compaction")),
                            false,
                            div()
                                .pl_6()
                                .child(text_view(format!("text-{}", m.id), m.text()))
                                .into_any_element(),
                            cx,
                        ),
                    ),
                )
                .into_any_element(),
            RowKind::BranchSummary(m) => v_flex()
                .gap_2()
                .text_sm()
                .child(
                    div()
                        .text_color(cx.theme().muted_foreground)
                        .child(t(cx, "conversation-branch-summary")),
                )
                .child(text_view(format!("text-{}", m.id), m.text()))
                .into_any_element(),
            RowKind::Run { messages, active } => {
                let live = self
                    .state
                    .read(cx)
                    .sessions
                    .get(key)
                    .filter(|_| *active)
                    .map(|session| session.tools.as_slice())
                    .unwrap_or_default();
                let content = RunContent::project(messages, live, *active);
                let mut result = v_flex().w_full().min_w_0().gap_3();
                let has_process = !content.activities.is_empty();
                if has_process {
                    let process = v_flex()
                        .w_full()
                        .min_w_0()
                        .gap_3()
                        .children(
                            content
                                .blocks()
                                .into_iter()
                                .map(|block| self.render_block(key, block, cx)),
                        )
                        .into_any_element();
                    result = result.child(
                        self.fold(
                            key,
                            &row.id,
                            Disclosure::run(metadata::process_title(messages, *active, cx))
                                .loading(*active),
                            *active || content.interrupted || content.answer.is_none(),
                            process,
                            cx,
                        ),
                    );
                } else if *active && content.answer.is_none() {
                    result = result.child(
                        gpui_kit::component::marker::Marker::new()
                            .loading(true)
                            .content(
                                gpui_kit::component::marker::MarkerContent::new()
                                    .text(t(cx, "conversation-working")),
                            ),
                    );
                }
                if let Some(index) = content.answer {
                    let m = &messages[index];
                    let text = m.text();
                    result = result.child(
                        Message::new()
                            .content(
                                MessageContent::new()
                                    .child(text_view(format!("text-{}", m.id), text)),
                            )
                            .footer(MessageFooter::new().content_inset(false).child(
                                actions::MessageActions {
                                    id: format!("{key}-{}", m.id),
                                    message: m.clone(),
                                },
                            )),
                    );
                }
                if messages.iter().any(|m| m.value["stopReason"] == "aborted") {
                    result = result.child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(t(cx, "conversation-interrupted")),
                    );
                }
                for m in messages {
                    if let Some(error) = m.value["errorMessage"].as_str().filter(|s| !s.is_empty())
                    {
                        result = result.child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().danger)
                                .child(error.to_owned()),
                        );
                    }
                }
                result.into_any_element()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{RowKind, RunContent, project_rows};
    use crate::state::history::DisplayMessage;
    use std::collections::{HashMap, HashSet};
    fn message(id: &str, role: &str) -> DisplayMessage {
        DisplayMessage {
            id: id.into(),
            entry: Some(id.into()),
            value: serde_json::json!({"role":role,"content":id,"timestamp":id}),
            completed_at: None,
        }
    }
    #[test]
    fn steering_does_not_collapse_an_earlier_part_of_the_active_run() {
        let first = message("a", "assistant");
        let second = message("b", "assistant");
        let active = HashSet::from([first.signature(), second.signature()]);
        let rows = project_rows(
            vec![
                message("u", "user"),
                first,
                message("steer", "user"),
                second,
            ],
            Some(&active),
        );
        assert!(matches!(rows[1].kind, RowKind::Run { active: true, .. }));
        assert!(matches!(rows[3].kind, RowKind::Run { active: true, .. }));
    }
    #[test]
    fn compactions_preserve_chronological_messages_and_adjacent_answers() {
        let rows = project_rows(
            vec![
                message("u", "user"),
                message("a", "assistant"),
                message("c", "compaction"),
                message("after", "assistant"),
                message("c2", "compaction"),
                message("next", "user"),
            ],
            None,
        );
        assert_eq!(
            rows.iter()
                .flat_map(|row| row.entries.iter().map(String::as_str))
                .collect::<Vec<_>>(),
            ["u", "a", "c", "after", "c2", "next"]
        );
        assert_eq!(rows.len(), 6);
        assert!(matches!(rows[0].kind, RowKind::User(_)));
        assert!(matches!(rows[2].kind, RowKind::Compaction(_)));
        assert!(matches!(rows[4].kind, RowKind::Compaction(_)));
        for index in [1, 3] {
            let RowKind::Run { messages, .. } = &rows[index].kind else {
                panic!("assistant message must remain visible")
            };
            assert_eq!(RunContent::project(messages, &[], false).answer, Some(0));
        }
    }
    #[test]
    fn locating_a_compaction_expands_only_its_summary() {
        let rows = project_rows(
            vec![
                message("u", "user"),
                message("a", "assistant"),
                message("c", "compaction"),
            ],
            None,
        );
        let mut open = HashMap::new();
        for row in &rows {
            row.reveal("c", &mut open);
        }
        assert_eq!(open, HashMap::from([("c".to_owned(), true)]));
        open.clear();
        for row in &rows {
            row.reveal("u", &mut open);
        }
        assert!(open.is_empty());
    }
}

use super::*;
use crate::state::{conversation::Session, history::DisplayMessage};
use gpui_kit::component::{
    bubble::{Bubble, BubbleContent, BubbleVariant},
    menu::{ContextMenuExt, PopupMenuItem},
    message::{Message, MessageAlignment, MessageContent, MessageFooter},
    message_scroller::MessageScroller,
    text::TextView,
};

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
    pub(super) fn reveal(&self, entry: &str, open: &mut HashMap<String, bool>) {
        if !self.entries.iter().any(|id| id == entry) {
            return;
        }
        match &self.kind {
            RowKind::Archive(rows) => {
                open.insert(self.id.clone(), true);
                for row in rows {
                    row.reveal(entry, open);
                }
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
            _ => {}
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
    Summary(DisplayMessage),
    Archive(Vec<ChatRow>),
}
pub(super) fn project(session: &Session, preview: Option<&str>) -> Vec<ChatRow> {
    let active = (preview.is_none_or(|id| session.history.on_current_path(id)) && session.running)
        .then_some(&session.active_messages);
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
            if m.role() == "compaction" && !rows.is_empty() {
                let archived = std::mem::take(&mut rows);
                rows.push(ChatRow {
                    id: format!("archive-{}", m.id),
                    entries: archived.iter().flat_map(|r| r.entries.clone()).collect(),
                    kind: RowKind::Archive(archived),
                });
            }
            rows.push(ChatRow {
                id: m.id.clone(),
                entries,
                kind: RowKind::Summary(m),
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
        if view
            .preview
            .as_deref()
            .is_some_and(|id| !session.history.on_current_path(id))
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
                    .child(div().text_xl().child(t(cx, "conversation-welcome")))
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(t(cx, "conversation-welcome-hint")),
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
                                    .py_2()
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
                    !s.busy()
                        && s.operation.is_none()
                        && s.pending_ui.is_empty()
                        && entry
                            .as_ref()
                            .is_some_and(|id| s.fork_messages.iter().any(|m| &m.entry_id == id))
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
            RowKind::Summary(m) => v_flex()
                .gap_2()
                .text_sm()
                .child(div().text_color(cx.theme().muted_foreground).child(t(
                    cx,
                    if m.role() == "compaction" {
                        "conversation-compaction"
                    } else {
                        "conversation-branch-summary"
                    },
                )))
                .child(text_view(format!("text-{}", m.id), m.text()))
                .into_any_element(),
            RowKind::Archive(rows) => self.fold(
                key,
                &row.id,
                Disclosure::run(t(cx, "conversation-archive")),
                false,
                v_flex()
                    .gap_4()
                    .children(rows.iter().map(|r| self.render_row(key, r, cx)))
                    .into_any_element(),
                cx,
            ),
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
    use super::{RowKind, project_rows};
    use crate::state::history::DisplayMessage;
    use std::collections::HashSet;
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
    fn compaction_keeps_original_entries_available_for_location() {
        let rows = project_rows(
            vec![
                message("u", "user"),
                message("a", "assistant"),
                message("c", "compaction"),
                message("next", "user"),
            ],
            None,
        );
        assert_eq!(rows[0].entries, ["u", "a"]);
        assert!(matches!(&rows[0].kind, RowKind::Archive(messages) if messages.len()==2));
    }
}

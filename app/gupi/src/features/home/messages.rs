use super::*;
use crate::state::{conversation::Session, history::DisplayMessage};
use gpui_kit::component::{
    bubble::{Bubble, BubbleContent, BubbleVariant},
    marker::{Marker, MarkerContent, MarkerLoadingStyle},
    message::{
        Message, MessageAlignment, MessageContent, MessageFooter, MessageGroup, MessageHeader,
    },
    message_scroller::MessageScroller,
};

mod actions;
mod activity;
mod markdown;
mod metadata;
mod presentation;
mod tool_details;
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
            return rems(0.);
        };
        match (&previous.kind, &self.kind) {
            (RowKind::Run { .. } | RowKind::Compaction(_), RowKind::Compaction(_))
            | (RowKind::Compaction(_), RowKind::Run { .. }) => rems(0.75),
            _ => rems(2.),
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
        started_at: Option<i64>,
    },
    Compaction(DisplayMessage),
    BranchSummary(DisplayMessage),
}
pub(super) fn project(session: &Session, preview: Option<&str>) -> Vec<ChatRow> {
    let active = session
        .active_messages()
        .filter(|_| preview.is_none_or(|id| session.history().on_current_path(id)));
    let mut rows = project_rows(session.messages(preview), active);
    for row in &mut rows {
        if let RowKind::Run {
            active: true,
            started_at,
            ..
        } = &mut row.kind
        {
            *started_at = started_at.or(session.run_started_at());
        }
    }
    rows
}
fn project_rows(
    messages: Vec<DisplayMessage>,
    active_messages: Option<&HashSet<String>>,
) -> Vec<ChatRow> {
    let mut rows: Vec<ChatRow> = vec![];
    let mut started_at = None;
    for m in messages {
        let entries = m.entry.iter().cloned().collect();
        if m.role() == "user" {
            started_at = m.value["timestamp"].as_i64();
            rows.push(ChatRow {
                id: m.id.clone(),
                entries,
                kind: RowKind::User(m),
            });
        } else if matches!(m.role(), "compaction" | "branch_summary") {
            started_at = None;
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
                id: rows
                    .last()
                    .map(|r| format!("run-after-{}", r.id))
                    .unwrap_or_else(|| format!("run-{}", m.id)),
                entries,
                kind: RowKind::Run {
                    started_at: started_at.or_else(|| m.value["timestamp"].as_i64()),
                    messages: vec![m],
                    active: false,
                },
            });
        }
    }
    if let Some(active_messages) = active_messages {
        if rows
            .last()
            .is_none_or(|row| !matches!(row.kind, RowKind::Run { .. }))
        {
            rows.push(ChatRow {
                id: rows
                    .last()
                    .map(|row| format!("run-after-{}", row.id))
                    .unwrap_or_else(|| "run-pending".into()),
                entries: vec![],
                kind: RowKind::Run {
                    messages: vec![],
                    active: true,
                    started_at,
                },
            });
        }
        for row in &mut rows {
            if let RowKind::Run {
                messages, active, ..
            } = &mut row.kind
            {
                *active = messages.is_empty()
                    || messages
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
impl HomeView {
    fn text_view(&self, key: &str, id: String, text: String) -> markdown::Markdown {
        markdown::Markdown::new(
            format!("{key}-{id}"),
            text,
            self.views[key].scroller.downgrade(),
        )
    }
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
            BodyState::Ready
                if session.empty_conversation() && session.info.path.as_os_str().is_empty() =>
            {
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
                                    .pt(row.spacing_before(
                                        index.checked_sub(1).and_then(|i| rows.get(i)),
                                    ))
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
                    !self.state.read(cx).temporary
                        && !s.settings_busy()
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
                                            .child(self.text_view(
                                                key,
                                                format!("text-{}", m.id),
                                                m.text(),
                                            ))
                                            .children(
                                                m.value
                                                    .get("content")
                                                    .and_then(|v| v.as_array())
                                                    .into_iter()
                                                    .flatten()
                                                    .enumerate()
                                                    .filter(|(_, block)| {
                                                        block.get("type").and_then(|v| v.as_str())
                                                            == Some("image")
                                                    })
                                                    .map(|(index, block)| {
                                                        tool_details::ToolImage {
                                                            id: format!(
                                                                "user-image-{key}-{}-{index}",
                                                                m.id
                                                            ),
                                                            mime: block
                                                                .get("mimeType")
                                                                .and_then(|v| v.as_str())
                                                                .unwrap_or_default()
                                                                .into(),
                                                            data: block
                                                                .get("data")
                                                                .and_then(|v| v.as_str())
                                                                .unwrap_or_default()
                                                                .into(),
                                                        }
                                                    }),
                                            ),
                                    ),
                                ),
                            )
                            .footer(
                                MessageFooter::new().child(actions::MessageActions {
                                    id: format!("{key}-{}", m.id),
                                    message: m.clone(),
                                    text: m.text(),
                                    before_copy: Some(
                                        Button::new(format!("fork-{key}-{}", m.id))
                                            .ghost()
                                            .xsmall()
                                            .icon(IconName::GitBranch)
                                            .disabled(!can_fork)
                                            .tooltip(label.clone())
                                            .accessibility_label(label)
                                            .on_click(move |_, _, cx| {
                                                if let Some(entry) = entry.clone() {
                                                    state.update(cx, |s, cx| {
                                                        s.fork(&target, entry, cx)
                                                    });
                                                }
                                            }),
                                    ),
                                }),
                            ),
                    )
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
                                .child(self.text_view(key, format!("text-{}", m.id), m.text()))
                                .into_any_element(),
                            cx,
                        ),
                    ),
                )
                .into_any_element(),
            RowKind::BranchSummary(m) => Message::new()
                .header(
                    MessageHeader::new()
                        .content_inset(false)
                        .child(t(cx, "conversation-branch-summary")),
                )
                .content(MessageContent::new().child(self.text_view(
                    key,
                    format!("text-{}", m.id),
                    m.text(),
                )))
                .into_any_element(),
            RowKind::Run {
                messages,
                active,
                started_at,
            } => {
                let live = self
                    .state
                    .read(cx)
                    .sessions
                    .get(key)
                    .filter(|_| *active)
                    .map(|session| session.tools.as_slice())
                    .unwrap_or_default();
                let content = RunContent::project(messages, live, *active);
                let mut result = MessageGroup::new().w_full();
                if content.has_process() {
                    let title = metadata::process_title(messages, *active, *started_at, cx);
                    let blocks = content.blocks();
                    let last = blocks.len().saturating_sub(1);
                    let process = MessageGroup::new()
                        .w_full()
                        .children(blocks.into_iter().enumerate().map(|(i, block)| {
                            self.render_block(
                                key,
                                block,
                                *active && !content.final_started && i == last,
                                cx,
                            )
                        }))
                        .into_any_element();
                    result = result.child(
                        self.fold(
                            key,
                            &row.id,
                            Disclosure::run(title)
                                .locked((*active && !content.final_started) || content.interrupted),
                            *active || content.interrupted || content.answer.is_none(),
                            process,
                            cx,
                        ),
                    );
                } else if *active
                    && !content.interrupted
                    && !content.final_started
                    && content.answer_text.is_empty()
                {
                    result = result.child(
                        Marker::new()
                            .loading(true)
                            .with_loading_style(MarkerLoadingStyle::Shimmer)
                            .content(
                                MarkerContent::new().text(t(cx, "conversation-thinking-running")),
                            ),
                    );
                }
                if let Some(index) = content.answer {
                    let m = &messages[index];
                    let text = content.answer_text.clone();
                    result = result.child(
                        Message::new()
                            .content(
                                MessageContent::new().child(
                                    self.text_view(key, format!("text-{}", m.id), text)
                                        .stream_fade(),
                                ),
                            )
                            .footer(MessageFooter::new().content_inset(false).child(
                                actions::MessageActions {
                                    id: format!("{key}-{}", m.id),
                                    message: m.clone(),
                                    text: content.answer_text.clone(),
                                    before_copy: None,
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
    #[test]
    fn installed_pi_recording_updates_the_projection_during_each_message() {
        use super::{Activity, project};
        use crate::state::conversation::Session;
        let events: Vec<serde_json::Value> =
            include_str!("../../../tests/fixtures/rpc-message-stream.jsonl")
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect();
        let mut thinking_updates = 0;
        let mut text_updates = 0;
        let mut call_starts = 0;
        for (index, event) in events.iter().enumerate() {
            if event["type"] != "message_update" {
                continue;
            }
            assert!(event.get("message").is_none());
            let session = Session::from_rpc_messages(&events[..=index]);
            let rows = project(&session, None);
            let RowKind::Run {
                messages, active, ..
            } = &rows.last().unwrap().kind
            else {
                panic!("missing run")
            };
            assert!(messages.last().unwrap().completed_at.is_none());
            let content = RunContent::project(messages, &session.tools, *active);
            let delta = &event["assistantMessageEvent"];
            match delta["type"].as_str() {
                Some("thinking_delta") => {
                    thinking_updates += 1;
                    assert!(
                        matches!(content.activities.last(), Some(Activity::Text { text, running: true, .. })
                        if text.ends_with(delta["delta"].as_str().unwrap()))
                    );
                }
                Some("text_delta") => {
                    text_updates += 1;
                    assert!(
                        content
                            .answer_text
                            .ends_with(delta["delta"].as_str().unwrap())
                    );
                }
                Some("toolcall_start") => {
                    call_starts += 1;
                    assert!(
                        matches!(content.activities.last(), Some(Activity::Tool(tool))
                        if tool.name == delta["toolName"].as_str().unwrap())
                    );
                }
                _ => {}
            }
        }
        assert_eq!((thinking_updates, text_updates, call_starts), (18, 6, 2));
        let session = Session::from_rpc_messages(&events);
        assert_eq!(
            session
                .messages(None)
                .iter()
                .filter(|m| m.role() == "assistant")
                .count(),
            3
        );
        assert!(session.interrupted);
    }

    #[test]
    fn rpc_deltas_reach_visible_rows_before_message_end() {
        use super::{Activity, ActivityBlock, ToolStatus, project};
        use crate::state::conversation::Session;
        use serde_json::json;

        let mut wire = vec![
            json!({"type":"message_start", "message":{"role":"user", "timestamp":1, "content":[{"type":"text", "text":"检查文件"}]}}),
            json!({"type":"message_start", "message":{"role":"assistant", "timestamp":2, "stopReason":"pending", "content":[]}}),
            json!({"type":"message_update", "assistantMessageEvent":{"type":"thinking_start", "contentIndex":0}}),
        ];
        let project_run = |wire: &[serde_json::Value]| {
            let session = Session::from_rpc_messages(wire);
            let rows = project(&session, None);
            let RowKind::Run {
                messages, active, ..
            } = &rows.last().unwrap().kind
            else {
                panic!("missing run")
            };
            assert!(*active);
            RunContent::project(messages, &session.tools, *active)
        };
        assert!(
            matches!(project_run(&wire).activities.last(), Some(Activity::Text {
            text, thinking: true, running: true, ..
        }) if text.is_empty())
        );

        wire.extend([
            json!({"type":"message_update", "assistantMessageEvent":{"type":"thinking_delta", "contentIndex":0, "delta":"先检查"}}),
            json!({"type":"message_update", "assistantMessageEvent":{"type":"thinking_delta", "contentIndex":0, "delta":"目录"}}),
        ]);
        assert!(
            matches!(project_run(&wire).activities.last(), Some(Activity::Text {
            text, thinking: true, running: true, ..
        }) if text == "先检查目录")
        );

        wire.extend([
            json!({"type":"message_update", "assistantMessageEvent":{"type":"thinking_end", "contentIndex":0, "content":"先检查目录。"}}),
            json!({"type":"message_update", "assistantMessageEvent":{"type":"text_start", "contentIndex":1}}),
            json!({"type":"message_update", "usage":{"output":8}, "assistantMessageEvent":{"type":"text_delta", "contentIndex":1, "delta":"我来读取"}}),
        ]);
        let content = project_run(&wire);
        assert_eq!(content.answer_text, "我来读取");
        assert!(!content.final_started);
        assert!(
            matches!(&content.activities[0], Activity::Text { text, running: false, .. } if text == "先检查目录。")
        );
        assert_eq!(
            Session::from_rpc_messages(&wire).live[1].value["usage"]["output"],
            8
        );

        wire.extend([
            json!({"type":"message_update", "assistantMessageEvent":{"type":"text_end", "contentIndex":1, "content":"我来读取技能"}}),
            json!({"type":"message_update", "assistantMessageEvent":{"type":"toolcall_start", "contentIndex":2, "id":"read-1", "toolName":"read"}}),
            json!({"type":"message_update", "assistantMessageEvent":{"type":"toolcall_delta", "contentIndex":2, "delta":"{\"path\":\"skills/"}}),
        ]);
        assert!(
            matches!(project_run(&wire).activities.last(), Some(Activity::Tool(tool)) if tool.name == "read" && tool.status == ToolStatus::Running)
        );
        wire.push(json!({"type":"message_update", "assistantMessageEvent":{
            "type":"toolcall_delta", "contentIndex":2, "delta":"review/SKILL.md\"}"
        }}));
        assert!(
            matches!(project_run(&wire).activities.last(), Some(Activity::Tool(tool))
            if tool.summary() == Some("review"))
        );
        wire.push(json!({"type":"message_update", "assistantMessageEvent":{
            "type":"toolcall_end", "contentIndex":2,
            "toolCall":{"type":"toolCall", "id":"read-1", "name":"read", "arguments":{"path":"skills/final/SKILL.md"}}
        }}));
        let content = project_run(&wire);
        assert!(content.answer.is_none());
        assert!(
            matches!(content.blocks()[1], ActivityBlock::Message(Activity::Text { text, .. }) if text == "我来读取技能")
        );
        assert!(
            matches!(content.activities.last(), Some(Activity::Tool(tool)) if tool.summary() == Some("final"))
        );
        assert!(wire.iter().all(|e| e["type"] != "message_end"));
    }
    fn message(id: &str, role: &str) -> DisplayMessage {
        DisplayMessage {
            id: id.into(),
            entry: Some(id.into()),
            value: serde_json::json!({"role":role,"content":id,"timestamp":id}),
            final_answer_part: None,
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
    #[test]
    fn first_message_has_a_stable_working_row_before_assistant_output() {
        let mut user = message("user", "user");
        user.value["timestamp"] = serde_json::json!(1000);
        let before = project_rows(vec![user.clone()], Some(&HashSet::new()));
        assert_eq!(before.len(), 2);
        assert!(
            matches!(&before[1].kind, RowKind::Run { active: true, messages, started_at: Some(1000) } if messages.is_empty())
        );
        let reply = message("reply", "assistant");
        let after = project_rows(
            vec![user, reply.clone()],
            Some(&HashSet::from([reply.signature()])),
        );
        assert_eq!(before[1].id, after[1].id);
        assert!(matches!(
            after[1].kind,
            RowKind::Run {
                active: true,
                started_at: Some(1000),
                ..
            }
        ));
    }
}

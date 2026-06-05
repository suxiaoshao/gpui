use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};

use ai_chat_core::AgentRunId;
use ai_chat_db::{AgentRunRecord, ConversationItemRecord};
use gpui::*;
use gpui_component::{
    ActiveTheme, IndexPath, h_flex,
    label::Label,
    list::{ListDelegate, ListState},
};

use crate::{foundation::I18n, state::conversations::ConversationLoadSnapshot};

use super::{
    format,
    message::{AgentTurnRow, OnCopy, OnToggleAgent, TimelineListItem, TimelineRow, UserMessageRow},
};

pub(super) struct ConversationTimelineDelegate {
    rows: Vec<TimelineRow>,
}

impl ConversationTimelineDelegate {
    pub(super) fn new(rows: Vec<TimelineRow>) -> Self {
        Self { rows }
    }

    pub(super) fn set_rows(&mut self, rows: Vec<TimelineRow>) {
        self.rows = rows;
    }
}

impl ListDelegate for ConversationTimelineDelegate {
    type Item = TimelineListItem;

    fn items_count(&self, _section: usize, _cx: &App) -> usize {
        self.rows.len()
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        self.rows.get(ix.row).cloned().map(TimelineListItem::new)
    }

    fn set_selected_index(
        &mut self,
        _ix: Option<IndexPath>,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) {
    }

    fn render_empty(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        h_flex()
            .w_full()
            .justify_center()
            .py_8()
            .text_color(cx.theme().muted_foreground)
            .child(Label::new(cx.global::<I18n>().t("conversation-empty")).text_sm())
            .into_any_element()
    }
}

pub(super) fn build_rows(
    snapshot: &ConversationLoadSnapshot,
    expanded_agent_runs: &HashMap<AgentRunId, bool>,
    on_toggle: OnToggleAgent,
    on_copy: OnCopy,
) -> Vec<TimelineRow> {
    let run_by_id = snapshot
        .runs
        .iter()
        .cloned()
        .map(|run| (run.id.clone(), run))
        .collect::<HashMap<_, _>>();
    let mut run_items: HashMap<AgentRunId, Vec<ConversationItemRecord>> = HashMap::new();
    let mut pending_rows = Vec::new();
    let mut seen_runs = HashSet::new();

    for item in &snapshot.items {
        if format::is_user_message(item) {
            pending_rows.push(PendingTimelineRow::User(item.clone()));
            continue;
        }

        if let Some(agent_run_id) = item.agent_run_id.clone() {
            if seen_runs.insert(agent_run_id.clone()) {
                pending_rows.push(PendingTimelineRow::Agent(agent_run_id.clone()));
            }
            run_items
                .entry(agent_run_id)
                .or_default()
                .push(item.clone());
        } else {
            pending_rows.push(PendingTimelineRow::LooseAgent(item.clone()));
        }
    }

    for run in &snapshot.runs {
        if seen_runs.insert(run.id.clone()) {
            pending_rows.push(PendingTimelineRow::Agent(run.id.clone()));
        }
    }

    pending_rows
        .into_iter()
        .map(|row| match row {
            PendingTimelineRow::User(item) => TimelineRow::User(Box::new(UserMessageRow {
                item,
                on_copy: on_copy.clone(),
            })),
            PendingTimelineRow::Agent(run_id) => {
                let items = run_items.remove(&run_id).unwrap_or_default();
                let run = run_by_id.get(&run_id).cloned();
                TimelineRow::Agent(Box::new(agent_turn_row(
                    Some(run_id),
                    run,
                    items,
                    expanded_agent_runs,
                    on_toggle.clone(),
                    on_copy.clone(),
                )))
            }
            PendingTimelineRow::LooseAgent(item) => TimelineRow::Agent(Box::new(agent_turn_row(
                None,
                None,
                vec![item],
                expanded_agent_runs,
                on_toggle.clone(),
                on_copy.clone(),
            ))),
        })
        .collect()
}

enum PendingTimelineRow {
    User(ConversationItemRecord),
    Agent(AgentRunId),
    LooseAgent(ConversationItemRecord),
}

fn agent_turn_row(
    run_id: Option<AgentRunId>,
    run: Option<AgentRunRecord>,
    items: Vec<ConversationItemRecord>,
    expanded_agent_runs: &HashMap<AgentRunId, bool>,
    on_toggle: OnToggleAgent,
    on_copy: OnCopy,
) -> AgentTurnRow {
    let final_item = final_item_for_run(run.as_ref(), &items);
    let default_expanded = !run
        .as_ref()
        .is_some_and(|run| format::is_terminal_run(run) && final_item.is_some());
    let expanded = run_id
        .as_ref()
        .and_then(|run_id| expanded_agent_runs.get(run_id).copied())
        .unwrap_or(default_expanded);

    AgentTurnRow {
        run_id,
        run,
        items,
        final_item,
        expanded,
        on_toggle,
        on_copy,
    }
}

fn final_item_for_run(
    run: Option<&AgentRunRecord>,
    items: &[ConversationItemRecord],
) -> Option<ConversationItemRecord> {
    run.and_then(|run| {
        run.output
            .as_ref()
            .and_then(|output| output.final_item_id.as_ref())
    })
    .and_then(|final_item_id| items.iter().find(|item| &item.id == final_item_id).cloned())
    .or_else(|| {
        items
            .iter()
            .rev()
            .find(|item| format::is_assistant_message(item))
            .cloned()
    })
}

#[allow(clippy::type_complexity)]
pub(super) fn callback_pair(
    on_toggle: impl Fn(AgentRunId, &mut Window, &mut App) + 'static,
    on_copy: impl Fn(String, &mut Window, &mut App) -> bool + 'static,
) -> (OnToggleAgent, OnCopy) {
    (Rc::new(on_toggle), Rc::new(on_copy))
}

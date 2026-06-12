use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};

use ai_chat_core::{AgentRunId, ConversationItemId};
use ai_chat_db::{AgentRunRecord, ConversationItemRecord};
use gpui::{App, Entity, Window};
use gpui_component::text::TextViewState;

use crate::{
    foundation::conversation_format as format, state::conversations::ConversationLoadSnapshot,
};

use super::message::{
    AgentTurnRow, OnCopy, OnToggleAgent, TimelineRow, TimelineRowKey, UserMessageRow,
};

pub(super) struct ConversationTimelineRows {
    rows: Vec<TimelineRow>,
    keys: Vec<TimelineRowKey>,
}

impl ConversationTimelineRows {
    pub(super) fn new(rows: Vec<TimelineRow>) -> Self {
        let keys = row_keys(&rows);
        Self { rows, keys }
    }

    pub(super) fn set_rows(&mut self, rows: Vec<TimelineRow>) -> Vec<TimelineRowKey> {
        let previous_keys = std::mem::take(&mut self.keys);
        self.keys = row_keys(&rows);
        self.rows = rows;
        previous_keys
    }

    pub(super) fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub(super) fn row(&self, ix: usize) -> Option<TimelineRow> {
        self.rows.get(ix).cloned()
    }

    pub(super) fn keys(&self) -> &[TimelineRowKey] {
        &self.keys
    }

    pub(super) fn row_index_for_item(&self, item_id: &ConversationItemId) -> Option<usize> {
        self.rows.iter().position(|row| row.contains_item(item_id))
    }
}

pub(super) fn build_rows(
    snapshot: &ConversationLoadSnapshot,
    expanded_agent_runs: &HashMap<AgentRunId, bool>,
    text_states: &HashMap<ConversationItemId, Entity<TextViewState>>,
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
                text_state: text_states.get(&item.id).cloned(),
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
                    text_states,
                    on_toggle.clone(),
                    on_copy.clone(),
                )))
            }
            PendingTimelineRow::LooseAgent(item) => TimelineRow::Agent(Box::new(agent_turn_row(
                None,
                None,
                vec![item],
                expanded_agent_runs,
                text_states,
                on_toggle.clone(),
                on_copy.clone(),
            ))),
        })
        .collect()
}

fn row_keys(rows: &[TimelineRow]) -> Vec<TimelineRowKey> {
    rows.iter().map(TimelineRow::key).collect()
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
    text_states: &HashMap<ConversationItemId, Entity<TextViewState>>,
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
        text_states: text_states.clone(),
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

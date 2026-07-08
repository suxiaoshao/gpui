use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};

use gpui::{App, Entity, Window};
use gpui_component::text::TextViewState;
use jaco_core::{
    AgentRunId, ApprovalDecisionItem, ApprovalRequestItem, ApprovalStatus, ConversationItemId,
    ConversationItemKind, ConversationItemPayload, ConversationItemStatus, ToolInvocationId,
    ToolInvocationStatus,
};
use jaco_db::{
    AgentRunRecord, ConversationItemRecord, ToolInvocationApproval, ToolInvocationRecord,
};

use crate::{
    foundation::conversation_format as format, state::conversations::ConversationLoadSnapshot,
};

use super::attachments;
use super::message::{
    AgentTurnRow, OnApprovalDecision, OnCopy, OnToggleAgent, TimelineRow, TimelineRowKey,
    UserMessageRow,
};

#[derive(Clone)]
pub(super) struct TimelineCallbacks {
    on_toggle: OnToggleAgent,
    on_copy: OnCopy,
    on_approval_decision: OnApprovalDecision,
}

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
    callbacks: TimelineCallbacks,
) -> Vec<TimelineRow> {
    let run_by_id = snapshot
        .runs
        .iter()
        .cloned()
        .map(|run| (run.id.clone(), run))
        .collect::<HashMap<_, _>>();
    let attachments_by_id = attachments::attachments_by_id(&snapshot.attachments);
    let mut run_items: HashMap<AgentRunId, Vec<ConversationItemRecord>> = HashMap::new();
    let mut run_tool_invocations: HashMap<AgentRunId, Vec<ToolInvocationRecord>> = HashMap::new();
    let mut pending_rows = Vec::new();
    let mut seen_runs = HashSet::new();
    for invocation in &snapshot.tool_invocations {
        run_tool_invocations
            .entry(invocation.agent_run_id.clone())
            .or_default()
            .push(invocation.clone());
    }

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
                image_attachments: attachments::user_image_attachments(&item, &attachments_by_id),
                item,
                on_copy: callbacks.on_copy.clone(),
            })),
            PendingTimelineRow::Agent(run_id) => {
                let items = run_items.remove(&run_id).unwrap_or_default();
                let tool_invocations = run_tool_invocations.remove(&run_id).unwrap_or_default();
                let run = run_by_id.get(&run_id).cloned();
                TimelineRow::Agent(Box::new(agent_turn_row(
                    Some(run_id),
                    run,
                    items,
                    tool_invocations,
                    expanded_agent_runs,
                    text_states,
                    callbacks.clone(),
                )))
            }
            PendingTimelineRow::LooseAgent(item) => TimelineRow::Agent(Box::new(agent_turn_row(
                None,
                None,
                vec![item],
                Vec::new(),
                expanded_agent_runs,
                text_states,
                callbacks.clone(),
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
    tool_invocations: Vec<ToolInvocationRecord>,
    expanded_agent_runs: &HashMap<AgentRunId, bool>,
    text_states: &HashMap<ConversationItemId, Entity<TextViewState>>,
    callbacks: TimelineCallbacks,
) -> AgentTurnRow {
    let items = items_with_derived_approvals(items, &tool_invocations);
    let final_item = final_item_for_run(run.as_ref(), &items);
    let default_expanded = !run.as_ref().is_some_and(format::is_terminal_run);
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
        on_toggle: callbacks.on_toggle,
        on_copy: callbacks.on_copy,
        on_approval_decision: callbacks.on_approval_decision,
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
pub(super) fn callbacks(
    on_toggle: impl Fn(AgentRunId, &mut Window, &mut App) + 'static,
    on_copy: impl Fn(String, &mut Window, &mut App) -> bool + 'static,
    on_approval_decision: impl Fn(ToolInvocationId, bool, &mut Window, &mut App) + 'static,
) -> TimelineCallbacks {
    TimelineCallbacks {
        on_toggle: Rc::new(on_toggle),
        on_copy: Rc::new(on_copy),
        on_approval_decision: Rc::new(on_approval_decision),
    }
}

fn items_with_derived_approvals(
    items: Vec<ConversationItemRecord>,
    tool_invocations: &[ToolInvocationRecord],
) -> Vec<ConversationItemRecord> {
    let approvals_by_invocation = tool_invocations
        .iter()
        .filter_map(|invocation| {
            invocation
                .approval
                .as_ref()
                .map(|approval| (invocation.id.clone(), (invocation, approval)))
        })
        .collect::<HashMap<_, _>>();
    if approvals_by_invocation.is_empty() {
        return items;
    }

    let mut rows = Vec::with_capacity(items.len() + approvals_by_invocation.len() * 2);
    let mut inserted = HashSet::new();
    for item in items {
        let tool_invocation_id = item.tool_invocation_id.clone();
        rows.push(item.clone());
        let Some(tool_invocation_id) = tool_invocation_id else {
            continue;
        };
        if !matches!(item.payload, ConversationItemPayload::ToolCall(_)) {
            continue;
        }
        let Some((invocation, approval)) = approvals_by_invocation.get(&tool_invocation_id) else {
            continue;
        };
        inserted.insert(tool_invocation_id);
        push_derived_approval_rows(&mut rows, &item, invocation, approval);
    }

    for (tool_invocation_id, (invocation, approval)) in approvals_by_invocation {
        if inserted.contains(&tool_invocation_id) {
            continue;
        }
        if let Some(base) = rows
            .iter()
            .find(|item| item.agent_run_id.as_ref() == Some(&invocation.agent_run_id))
            .cloned()
        {
            push_derived_approval_rows(&mut rows, &base, invocation, approval);
        }
    }

    rows
}

fn push_derived_approval_rows(
    rows: &mut Vec<ConversationItemRecord>,
    base: &ConversationItemRecord,
    invocation: &ToolInvocationRecord,
    approval: &ToolInvocationApproval,
) {
    rows.push(derived_approval_request(base, invocation, approval));
    if approval.decision.is_some() {
        rows.push(derived_approval_decision(base, invocation, approval));
    }
}

fn derived_approval_request(
    base: &ConversationItemRecord,
    invocation: &ToolInvocationRecord,
    approval: &ToolInvocationApproval,
) -> ConversationItemRecord {
    let status = if invocation.status == ToolInvocationStatus::AwaitingApproval
        && approval.status == ApprovalStatus::Pending
    {
        ConversationItemStatus::WaitingForApproval
    } else {
        ConversationItemStatus::Completed
    };
    derived_approval_item(
        base,
        invocation,
        format!("{}:approval-request", invocation.id),
        ConversationItemKind::ApprovalRequest,
        status,
        ConversationItemPayload::ApprovalRequest(ApprovalRequestItem {
            tool_invocation_id: invocation.id.clone(),
            request: approval.request.clone(),
        }),
        approval.requested_at,
    )
}

fn derived_approval_decision(
    base: &ConversationItemRecord,
    invocation: &ToolInvocationRecord,
    approval: &ToolInvocationApproval,
) -> ConversationItemRecord {
    let decision = approval
        .decision
        .clone()
        .expect("approval decision row requires decision payload");
    derived_approval_item(
        base,
        invocation,
        format!("{}:approval-decision", invocation.id),
        ConversationItemKind::ApprovalDecision,
        ConversationItemStatus::Completed,
        ConversationItemPayload::ApprovalDecision(ApprovalDecisionItem {
            tool_invocation_id: invocation.id.clone(),
            decision,
        }),
        approval.decided_at.unwrap_or(approval.requested_at),
    )
}

fn derived_approval_item(
    base: &ConversationItemRecord,
    invocation: &ToolInvocationRecord,
    id: ConversationItemId,
    kind: ConversationItemKind,
    status: ConversationItemStatus,
    payload: ConversationItemPayload,
    timestamp: time::OffsetDateTime,
) -> ConversationItemRecord {
    ConversationItemRecord {
        id,
        conversation_id: base.conversation_id.clone(),
        seq: base.seq,
        kind,
        status,
        agent_run_id: Some(invocation.agent_run_id.clone()),
        provider_step_id: invocation.provider_step_id.clone(),
        tool_invocation_id: Some(invocation.id.clone()),
        provider_item_id: None,
        search_text: payload.search_text(),
        payload,
        created_at: timestamp,
        updated_at: timestamp,
    }
}

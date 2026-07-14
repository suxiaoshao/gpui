use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};

use gpui::{App, Entity, Window};
use gpui_component::text::TextViewState;
use jaco_core::{AgentRunId, ConversationEntryId, ToolInvocationId};
use jaco_db::{AgentRunRecord, ConversationEntryRecord};

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

    pub(super) fn row_index_for_item(&self, item_id: &ConversationEntryId) -> Option<usize> {
        self.rows.iter().position(|row| row.contains_item(item_id))
    }
}

pub(super) fn build_rows(
    snapshot: &ConversationLoadSnapshot,
    active_agent_run_id: Option<&AgentRunId>,
    expanded_agent_runs: &HashMap<AgentRunId, bool>,
    text_states: &HashMap<ConversationEntryId, Entity<TextViewState>>,
    callbacks: TimelineCallbacks,
) -> Vec<TimelineRow> {
    let attachments_by_id = attachments::attachments_by_id(&snapshot.attachments);
    let (pending_rows, mut run_items) =
        collect_pending_rows(&snapshot.items, &snapshot.runs, active_agent_run_id);
    let run_by_id = snapshot
        .runs
        .iter()
        .cloned()
        .map(|run| (run.id.clone(), run))
        .collect::<HashMap<_, _>>();

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
                let run = run_by_id.get(&run_id).cloned();
                TimelineRow::Agent(Box::new(agent_turn_row(
                    Some(run_id),
                    run,
                    items,
                    expanded_agent_runs,
                    text_states,
                    callbacks.clone(),
                )))
            }
            PendingTimelineRow::LooseAgent(item) => TimelineRow::Agent(Box::new(agent_turn_row(
                None,
                None,
                vec![item],
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
    User(ConversationEntryRecord),
    Agent(AgentRunId),
    LooseAgent(ConversationEntryRecord),
}

fn collect_pending_rows(
    items: &[ConversationEntryRecord],
    runs: &[AgentRunRecord],
    active_agent_run_id: Option<&AgentRunId>,
) -> (
    Vec<PendingTimelineRow>,
    HashMap<AgentRunId, Vec<ConversationEntryRecord>>,
) {
    let run_by_id = runs
        .iter()
        .map(|run| (run.id.clone(), run))
        .collect::<HashMap<_, _>>();
    let mut run_items: HashMap<AgentRunId, Vec<ConversationEntryRecord>> = HashMap::new();
    let mut pending_rows = Vec::new();
    let mut seen_runs = HashSet::new();

    for item in items {
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

    if let Some(active_agent_run_id) = active_agent_run_id
        && !seen_runs.contains(active_agent_run_id)
        && run_by_id
            .get(active_agent_run_id)
            .is_some_and(|run| !format::is_terminal_run(run))
    {
        pending_rows.push(PendingTimelineRow::Agent(active_agent_run_id.clone()));
    }

    (pending_rows, run_items)
}

fn agent_turn_row(
    run_id: Option<AgentRunId>,
    run: Option<AgentRunRecord>,
    items: Vec<ConversationEntryRecord>,
    expanded_agent_runs: &HashMap<AgentRunId, bool>,
    text_states: &HashMap<ConversationEntryId, Entity<TextViewState>>,
    callbacks: TimelineCallbacks,
) -> AgentTurnRow {
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
    items: &[ConversationEntryRecord],
) -> Option<ConversationEntryRecord> {
    run.and_then(|run| run.output.as_ref().map(|output| &output.final_entry_id))
        .and_then(|final_entry_id| {
            items
                .iter()
                .find(|item| &item.id == final_entry_id)
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

#[cfg(test)]
mod tests {
    use super::*;
    use jaco_core::{
        AgentEngineKind, AgentRunInput, AgentRunStatus, AgentRunTriggerKind, AgentRuntimeSnapshot,
        ContentPart, ConversationEntryPayload, ConversationEntryStatus, ProviderSettingsPayload,
        RunErrorPayload, RunSettingsSnapshot, ToolApprovalMode, ToolApprovalPolicy,
        ToolNameStrategy, ToolPolicySnapshot, TranscriptRole, conservative_model_capabilities,
    };
    use time::OffsetDateTime;

    #[test]
    fn persisted_error_entry_keeps_position_when_later_entries_exist() {
        let run_id = AgentRunId::from("run-1");
        let items = vec![
            entry(
                "entry-user-before",
                1,
                None,
                ConversationEntryPayload::Message {
                    role: TranscriptRole::User,
                    content: vec![ContentPart::Text {
                        text: "before".to_string(),
                    }],
                },
            ),
            entry(
                "entry-error",
                2,
                Some(run_id.clone()),
                ConversationEntryPayload::Error(RunErrorPayload {
                    code: "prompt_error".to_string(),
                    message: "forced provider-open failure".to_string(),
                    retryable: true,
                    provider: None,
                    raw: None,
                }),
            ),
            entry(
                "entry-user-after",
                3,
                None,
                ConversationEntryPayload::Message {
                    role: TranscriptRole::User,
                    content: vec![ContentPart::Text {
                        text: "after".to_string(),
                    }],
                },
            ),
        ];

        let (pending_rows, run_items) = collect_pending_rows(&items, &[], None);
        let keys = pending_rows
            .iter()
            .map(|row| match row {
                PendingTimelineRow::User(item) => format!("user:{}", item.id),
                PendingTimelineRow::Agent(run_id) => format!("agent:{run_id}"),
                PendingTimelineRow::LooseAgent(item) => format!("loose:{}", item.id),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            keys,
            vec![
                "user:entry-user-before",
                "agent:run-1",
                "user:entry-user-after",
            ]
        );
        assert_eq!(
            run_items
                .get(&run_id)
                .unwrap()
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec!["entry-error"]
        );
    }

    #[test]
    fn active_zero_entry_run_gets_ephemeral_tail_row_only_while_non_terminal() {
        let run_id = AgentRunId::from("run-active");
        let active_run = active_run(run_id.clone());

        let (pending_rows, run_items) =
            collect_pending_rows(&[], std::slice::from_ref(&active_run), Some(&run_id));
        assert_eq!(pending_rows.len(), 1);
        assert!(matches!(&pending_rows[0], PendingTimelineRow::Agent(id) if id == &run_id));
        assert!(run_items.is_empty());

        let mut terminal_run = active_run.clone();
        terminal_run.status = AgentRunStatus::Completed;
        let (pending_rows, _) = collect_pending_rows(&[], &[terminal_run], Some(&run_id));
        assert!(pending_rows.is_empty());

        let entry = entry(
            "entry-active",
            1,
            Some(run_id.clone()),
            ConversationEntryPayload::Reasoning {
                text: "working".to_string(),
                summary: None,
            },
        );
        let (pending_rows, run_items) =
            collect_pending_rows(&[entry], &[active_run], Some(&run_id));
        assert_eq!(pending_rows.len(), 1);
        assert!(matches!(&pending_rows[0], PendingTimelineRow::Agent(id) if id == &run_id));
        assert_eq!(run_items.get(&run_id).unwrap().len(), 1);
    }

    fn active_run(id: AgentRunId) -> AgentRunRecord {
        AgentRunRecord {
            id,
            conversation_id: "conversation-1".to_string(),
            trigger_entry_id: "trigger-entry".to_string(),
            trigger_kind: AgentRunTriggerKind::User,
            status: AgentRunStatus::Running,
            input: AgentRunInput {
                prompt_snapshot: None,
                provider_id: "provider".to_string(),
                model_id: "model".to_string(),
                settings_snapshot: RunSettingsSnapshot {
                    prompt: None,
                    provider_id: "provider".to_string(),
                    model_id: "model".to_string(),
                    model_capabilities: conservative_model_capabilities("openai"),
                    provider_settings: ProviderSettingsPayload {
                        provider_kind: "openai".to_string(),
                        fields: Vec::new(),
                    },
                    reasoning_selection: None,
                    tool_policy: ToolPolicySnapshot {
                        approval_policy: ToolApprovalPolicy::Never,
                        enabled_sources: Vec::new(),
                        max_steps: 1,
                        approval_mode: ToolApprovalMode::RequestApproval,
                        permission_scope: None,
                    },
                },
                runtime_snapshot: AgentRuntimeSnapshot {
                    engine: AgentEngineKind::Rig,
                    engine_version: "test".to_string(),
                    skill_catalog_hash: None,
                    tool_name_strategy: ToolNameStrategy::Direct,
                },
                max_steps: 1,
            },
            output: None,
            error: None,
            created_at: OffsetDateTime::UNIX_EPOCH,
            started_at: Some(OffsetDateTime::UNIX_EPOCH),
            completed_at: None,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    fn entry(
        id: &str,
        seq: i32,
        agent_run_id: Option<AgentRunId>,
        payload: ConversationEntryPayload,
    ) -> ConversationEntryRecord {
        ConversationEntryRecord {
            id: id.to_string(),
            conversation_id: "conversation-1".to_string(),
            seq,
            kind: payload.kind(),
            status: if matches!(&payload, ConversationEntryPayload::Error(_)) {
                ConversationEntryStatus::Failed
            } else {
                ConversationEntryStatus::Completed
            },
            agent_run_id,
            provider_step_id: None,
            tool_invocation_id: None,
            provider_item_id: None,
            search_text: payload.search_text(),
            payload,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        }
    }
}

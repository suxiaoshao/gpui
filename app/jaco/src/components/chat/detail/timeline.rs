use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};

use gpui::{App, Entity, Window};
use gpui_component::text::TextViewState;
use jaco_core::{
    AgentRun, AgentRunId, Conversation, ConversationAttachment, ConversationEntry,
    ConversationEntryId, ToolInvocation, ToolInvocationId,
};

use crate::foundation::conversation_format as format;

use super::attachments;
use super::copy_button::OnCopy;
use super::message::{
    AgentTurnRow, OnApprovalDecision, OnToggleAgent, TimelineRow, TimelineRowKey, UserMessageRow,
};
use super::tool_invocation::{
    AgentDetailItem, OnToggleToolInvocation, ToolInvocationDetail, ToolInvocationPreviewCacheEntry,
    is_tool_lifecycle_entry, project_agent_details,
};

#[derive(Clone)]
pub(super) struct TimelineCallbacks {
    on_toggle: OnToggleAgent,
    on_toggle_tool_invocation: OnToggleToolInvocation,
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

    pub(super) fn update_entry(
        &mut self,
        entry: ConversationEntry,
        attachments: &[ConversationAttachment],
        text_state: Option<Entity<TextViewState>>,
    ) -> Option<TimelineRowKey> {
        if is_tool_lifecycle_entry(&entry) {
            return None;
        }
        let attachments_by_id = attachments::attachments_by_id(attachments);
        for row in &mut self.rows {
            match row {
                TimelineRow::User(user) if user.item.id == entry.id => {
                    user.image_attachments =
                        attachments::user_image_attachments(&entry, &attachments_by_id);
                    user.text_state = text_state;
                    user.item = entry;
                    return Some(row.key());
                }
                TimelineRow::Agent(agent)
                    if agent
                        .items
                        .iter()
                        .any(|current| current.contains_entry_id(&entry.id)) =>
                {
                    if let Some(current) = agent
                        .items
                        .iter_mut()
                        .find(|current| current.contains_entry_id(&entry.id))
                    {
                        *current = AgentDetailItem::Entry(entry.clone());
                    }
                    if let Some(text_state) = text_state {
                        agent.text_states.insert(entry.id.clone(), text_state);
                    } else {
                        agent.text_states.remove(&entry.id);
                    }
                    return Some(row.key());
                }
                TimelineRow::User(_) | TimelineRow::Agent(_) => {}
            }
        }
        None
    }

    pub(super) fn update_run(&mut self, run: AgentRun) -> Option<TimelineRowKey> {
        let row = self.rows.iter_mut().find(|row| {
            matches!(
                row,
                TimelineRow::Agent(agent) if agent.run_id.as_ref() == Some(&run.id)
            )
        })?;
        let TimelineRow::Agent(agent) = row else {
            unreachable!("run rows are always agent rows")
        };
        agent.run = Some(run);
        Some(row.key())
    }

    pub(super) fn update_tool_invocation(
        &mut self,
        detail: ToolInvocationDetail,
    ) -> Option<TimelineRowKey> {
        let row = self.rows.iter_mut().find(|row| {
            matches!(
                row,
                TimelineRow::Agent(agent)
                    if agent.items.iter().any(|item| {
                        matches!(item, AgentDetailItem::ToolInvocation(current) if current.id == detail.id)
                    })
            )
        })?;
        let TimelineRow::Agent(agent) = row else {
            unreachable!("tool invocation rows are always agent rows")
        };
        let current = agent.items.iter_mut().find(|item| {
            matches!(item, AgentDetailItem::ToolInvocation(current) if current.id == detail.id)
        })?;
        if !matches!(current, AgentDetailItem::ToolInvocation(current) if current.agent_run_id == detail.agent_run_id)
        {
            return None;
        }
        *current = AgentDetailItem::ToolInvocation(detail);
        Some(row.key())
    }

    pub(super) fn row_key_for_tool_invocation(
        &self,
        id: &ToolInvocationId,
    ) -> Option<TimelineRowKey> {
        self.rows.iter().find_map(|row| {
            match row {
            TimelineRow::Agent(agent)
                if agent.items.iter().any(|item| {
                    matches!(item, AgentDetailItem::ToolInvocation(detail) if &detail.id == id)
                }) =>
            {
                Some(row.key())
            }
            TimelineRow::User(_) | TimelineRow::Agent(_) => None,
        }
        })
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_rows(
    snapshot: &Conversation,
    active_agent_run_id: Option<&AgentRunId>,
    expanded_agent_runs: &HashMap<AgentRunId, bool>,
    expanded_tool_invocations: &HashMap<ToolInvocationId, bool>,
    previews: &HashMap<ToolInvocationId, ToolInvocationPreviewCacheEntry>,
    approval_decidable: &HashSet<ToolInvocationId>,
    text_states: &HashMap<ConversationEntryId, Entity<TextViewState>>,
    callbacks: TimelineCallbacks,
) -> Vec<TimelineRow> {
    let attachments_by_id = attachments::attachments_by_id(&snapshot.attachments);
    let (pending_rows, mut run_items) = collect_pending_rows(
        &snapshot.entries,
        &snapshot.runs,
        &snapshot.tool_invocations,
        active_agent_run_id,
    );
    let run_by_id = snapshot
        .runs
        .iter()
        .cloned()
        .map(|run| (run.id.clone(), run))
        .collect::<HashMap<_, _>>();
    let mut invocations_by_run = group_invocations_by_run(&snapshot.tool_invocations);

    pending_rows
        .into_iter()
        .map(|row| match row {
            PendingTimelineRow::User(item) => TimelineRow::User(Box::new(UserMessageRow {
                text_state: text_states.get(&item.id).cloned(),
                image_attachments: attachments::user_image_attachments(item, &attachments_by_id),
                item: item.clone(),
                on_copy: callbacks.on_copy.clone(),
            })),
            PendingTimelineRow::Agent(run_id) => {
                let items = run_items.remove(&run_id).unwrap_or_default();
                let invocations = invocations_by_run.remove(&run_id).unwrap_or_default();
                let run = run_by_id.get(&run_id).cloned();
                TimelineRow::Agent(Box::new(agent_turn_row(
                    Some(run_id),
                    run,
                    items,
                    invocations,
                    expanded_agent_runs,
                    expanded_tool_invocations,
                    previews,
                    approval_decidable,
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
                expanded_tool_invocations,
                previews,
                approval_decidable,
                text_states,
                callbacks.clone(),
            ))),
        })
        .collect()
}

fn group_invocations_by_run(
    invocations: &[ToolInvocation],
) -> HashMap<AgentRunId, Vec<&ToolInvocation>> {
    invocations.iter().fold(
        HashMap::<AgentRunId, Vec<&ToolInvocation>>::new(),
        |mut by_run, invocation| {
            by_run
                .entry(invocation.agent_run_id.clone())
                .or_default()
                .push(invocation);
            by_run
        },
    )
}

fn row_keys(rows: &[TimelineRow]) -> Vec<TimelineRowKey> {
    rows.iter().map(TimelineRow::key).collect()
}

enum PendingTimelineRow<'a> {
    User(&'a ConversationEntry),
    Agent(AgentRunId),
    LooseAgent(&'a ConversationEntry),
}

fn collect_pending_rows<'a>(
    items: &'a [ConversationEntry],
    runs: &[AgentRun],
    invocations: &[ToolInvocation],
    active_agent_run_id: Option<&AgentRunId>,
) -> (
    Vec<PendingTimelineRow<'a>>,
    HashMap<AgentRunId, Vec<&'a ConversationEntry>>,
) {
    let run_by_id = runs
        .iter()
        .map(|run| (run.id.clone(), run))
        .collect::<HashMap<_, _>>();
    let mut run_items: HashMap<AgentRunId, Vec<&ConversationEntry>> = HashMap::new();
    let mut pending_rows = Vec::new();
    let mut seen_runs = HashSet::new();
    let runs_with_entries = items
        .iter()
        .filter_map(|item| item.agent_run_id.clone())
        .collect::<HashSet<_>>();
    let runs_with_invocations = invocations
        .iter()
        .map(|invocation| invocation.agent_run_id.clone())
        .collect::<HashSet<_>>();
    let invocation_only_runs_by_trigger = runs
        .iter()
        .filter(|run| {
            runs_with_invocations.contains(&run.id) && !runs_with_entries.contains(&run.id)
        })
        .fold(
            HashMap::<ConversationEntryId, Vec<AgentRunId>>::new(),
            |mut by_trigger, run| {
                by_trigger
                    .entry(run.trigger_entry_id.clone())
                    .or_default()
                    .push(run.id.clone());
                by_trigger
            },
        );

    for item in items {
        if format::is_user_message(item) {
            pending_rows.push(PendingTimelineRow::User(item));
            if let Some(run_ids) = invocation_only_runs_by_trigger.get(&item.id) {
                for run_id in run_ids {
                    if seen_runs.insert(run_id.clone()) {
                        pending_rows.push(PendingTimelineRow::Agent(run_id.clone()));
                    }
                }
            }
            continue;
        }

        if let Some(agent_run_id) = item.agent_run_id.clone() {
            if seen_runs.insert(agent_run_id.clone()) {
                pending_rows.push(PendingTimelineRow::Agent(agent_run_id.clone()));
            }
            run_items.entry(agent_run_id).or_default().push(item);
        } else {
            pending_rows.push(PendingTimelineRow::LooseAgent(item));
        }
    }

    for run in runs {
        if runs_with_invocations.contains(&run.id)
            && !runs_with_entries.contains(&run.id)
            && seen_runs.insert(run.id.clone())
        {
            pending_rows.push(PendingTimelineRow::Agent(run.id.clone()));
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

#[allow(clippy::too_many_arguments)]
fn agent_turn_row(
    run_id: Option<AgentRunId>,
    run: Option<AgentRun>,
    items: Vec<&ConversationEntry>,
    invocations: Vec<&ToolInvocation>,
    expanded_agent_runs: &HashMap<AgentRunId, bool>,
    expanded_tool_invocations: &HashMap<ToolInvocationId, bool>,
    previews: &HashMap<ToolInvocationId, ToolInvocationPreviewCacheEntry>,
    approval_decidable: &HashSet<ToolInvocationId>,
    text_states: &HashMap<ConversationEntryId, Entity<TextViewState>>,
    callbacks: TimelineCallbacks,
) -> AgentTurnRow {
    let default_expanded = !run.as_ref().is_some_and(format::is_terminal_run);
    let expanded = run_id
        .as_ref()
        .and_then(|run_id| expanded_agent_runs.get(run_id).copied())
        .unwrap_or(default_expanded);

    let items = project_agent_details(
        items,
        invocations,
        expanded_tool_invocations,
        previews,
        approval_decidable,
    );

    AgentTurnRow {
        run_id,
        run,
        items,
        text_states: text_states.clone(),
        expanded,
        on_toggle: callbacks.on_toggle,
        on_toggle_tool_invocation: callbacks.on_toggle_tool_invocation,
        on_copy: callbacks.on_copy,
        on_approval_decision: callbacks.on_approval_decision,
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn callbacks(
    on_toggle: impl Fn(AgentRunId, &mut Window, &mut App) + 'static,
    on_toggle_tool_invocation: impl Fn(ToolInvocationId, &mut Window, &mut App) + 'static,
    on_copy: impl Fn(String, &mut Window, &mut App) -> bool + 'static,
    on_approval_decision: impl Fn(ToolInvocationId, bool, &mut Window, &mut App) + 'static,
) -> TimelineCallbacks {
    TimelineCallbacks {
        on_toggle: Rc::new(on_toggle),
        on_toggle_tool_invocation: Rc::new(on_toggle_tool_invocation),
        on_copy: Rc::new(on_copy),
        on_approval_decision: Rc::new(on_approval_decision),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jaco_core::{
        AgentEngineKind, AgentRunInput, AgentRunOutput, AgentRunStatus, AgentRunTriggerKind,
        AgentRuntimeSnapshot, AgentStoppedReason, ContentPart, ConversationEntryPayload,
        ConversationEntryStatus, ProviderRawPayload, ProviderSettingsPayload, RunErrorPayload,
        RunSettingsSnapshot, ToolApprovalMode, ToolApprovalPolicy, ToolArguments,
        ToolExecutionPolicy, ToolInvocationInput, ToolInvocationOutput, ToolInvocationStatus,
        ToolNameStrategy, ToolPolicySnapshot, ToolResultEntry, ToolSource, TranscriptRole,
        conservative_model_capabilities,
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

        let (pending_rows, run_items) = collect_pending_rows(&items, &[], &[], None);
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
            collect_pending_rows(&[], std::slice::from_ref(&active_run), &[], Some(&run_id));
        assert_eq!(pending_rows.len(), 1);
        assert!(matches!(&pending_rows[0], PendingTimelineRow::Agent(id) if id == &run_id));
        assert!(run_items.is_empty());

        let mut terminal_run = active_run.clone();
        terminal_run.status = AgentRunStatus::Completed;
        let (pending_rows, _) = collect_pending_rows(&[], &[terminal_run], &[], Some(&run_id));
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
        let entries = [entry];
        let (pending_rows, run_items) =
            collect_pending_rows(&entries, &[active_run], &[], Some(&run_id));
        assert_eq!(pending_rows.len(), 1);
        assert!(matches!(&pending_rows[0], PendingTimelineRow::Agent(id) if id == &run_id));
        assert_eq!(run_items.get(&run_id).unwrap().len(), 1);
    }

    #[test]
    fn invocation_only_run_is_inserted_after_its_trigger_user_row() {
        let run_id = AgentRunId::from("run-with-orphan-invocation");
        let run = active_run(run_id.clone());
        let trigger = entry(
            "trigger-entry",
            1,
            None,
            ConversationEntryPayload::Message {
                role: TranscriptRole::User,
                content: vec![ContentPart::Text {
                    text: "run a tool".to_string(),
                }],
            },
        );
        let invocation = tool_invocation("invocation-orphan", run_id.clone());

        let entries = [trigger];
        let (pending_rows, run_items) = collect_pending_rows(&entries, &[run], &[invocation], None);
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
                "user:trigger-entry".to_string(),
                "agent:run-with-orphan-invocation".to_string(),
            ]
        );
        assert!(run_items.is_empty());
    }

    #[test]
    fn snapshot_first_orphan_is_reanchored_to_one_lifecycle_block() {
        let run_id = AgentRunId::from("run-reanchored");
        let invocation = tool_invocation("invocation-reanchored", run_id.clone());
        let no_expansion = HashMap::new();
        let no_previews = HashMap::new();
        let no_actions = HashSet::new();

        let orphan = project_agent_details(
            &[],
            std::slice::from_ref(&invocation),
            &no_expansion,
            &no_previews,
            &no_actions,
        );
        assert!(matches!(
            orphan.as_slice(),
            [AgentDetailItem::ToolInvocation(detail)] if detail.id == "invocation-reanchored"
        ));

        let mut lifecycle = entry(
            "entry-reanchored-call",
            1,
            Some(run_id),
            ConversationEntryPayload::ToolCall(jaco_core::ToolCallEntry {
                tool_invocation_id: Some("invocation-reanchored".to_string()),
                call_id: "call-invocation-reanchored".to_string(),
                source: ToolSource::Local,
                name: "read_file".to_string(),
                runtime_tool_name: "read_file".to_string(),
                arguments: ToolArguments {
                    value: serde_json::json!({"path": "src/main.rs"}),
                },
            }),
        );
        lifecycle.tool_invocation_id = Some("invocation-reanchored".to_string());
        let anchored = project_agent_details(
            std::slice::from_ref(&lifecycle),
            std::slice::from_ref(&invocation),
            &no_expansion,
            &no_previews,
            &no_actions,
        );
        assert!(matches!(
            anchored.as_slice(),
            [AgentDetailItem::ToolInvocation(detail)] if detail.id == "invocation-reanchored"
        ));
        assert_eq!(anchored[0].stable_id_suffix(), orphan[0].stable_id_suffix());
    }

    #[test]
    fn terminal_entry_batch_then_snapshot_stays_one_invocation_block() {
        let run_id = AgentRunId::from("run-terminal-batch");
        let invocation_id = "invocation-terminal-batch";
        let invocation = tool_invocation(invocation_id, run_id.clone());
        let mut call_entry = entry(
            "entry-terminal-call",
            1,
            Some(run_id.clone()),
            ConversationEntryPayload::ToolCall(jaco_core::ToolCallEntry {
                tool_invocation_id: Some(invocation_id.to_string()),
                call_id: format!("call-{invocation_id}"),
                source: ToolSource::Local,
                name: "read_file".to_string(),
                runtime_tool_name: "read_file".to_string(),
                arguments: ToolArguments {
                    value: serde_json::json!({"path": "src/main.rs"}),
                },
            }),
        );
        call_entry.tool_invocation_id = Some(invocation_id.to_string());
        let mut result_entry = entry(
            "entry-terminal-result",
            2,
            Some(run_id),
            ConversationEntryPayload::ToolResult(ToolResultEntry {
                tool_invocation_id: Some(invocation_id.to_string()),
                call_id: format!("call-{invocation_id}"),
                content: vec![ContentPart::Text {
                    text: "terminal output".to_string(),
                }],
                is_error: false,
                structured_output: None,
                raw_output: None,
            }),
        );
        result_entry.tool_invocation_id = Some(invocation_id.to_string());
        let entries = vec![call_entry, result_entry];
        let items = project_agent_details(
            &entries,
            std::slice::from_ref(&invocation),
            &HashMap::new(),
            &HashMap::new(),
            &HashSet::new(),
        );
        assert_eq!(
            items
                .iter()
                .filter(|item| matches!(item, AgentDetailItem::ToolInvocation(detail) if detail.id == invocation_id))
                .count(),
            1
        );
        assert!(
            !items
                .iter()
                .any(|item| matches!(item, AgentDetailItem::UnresolvedToolLifecycle(_)))
        );
    }

    #[test]
    fn availability_update_replaces_same_invocation_and_preserves_sibling_action() {
        let run_a = AgentRunId::from("run-action-a");
        let run_b = AgentRunId::from("run-action-b");
        let invocation_a = tool_invocation("invocation-action-a", run_a.clone());
        let invocation_b = tool_invocation("invocation-action-b", run_b);
        let mut detail_a = super::super::tool_invocation::project_tool_invocation_detail(
            &invocation_a,
            true,
            None,
            false,
        );
        let mut detail_b = super::super::tool_invocation::project_tool_invocation_detail(
            &invocation_b,
            true,
            None,
            true,
        );
        detail_a.approval_decidable = true;
        detail_b.approval_decidable = true;
        let callbacks = callbacks(|_, _, _| {}, |_, _, _| {}, |_, _, _| true, |_, _, _, _| {});
        let row = agent_turn_row(
            Some(run_a.clone()),
            None,
            Vec::new(),
            Vec::new(),
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            &HashSet::new(),
            &HashMap::new(),
            callbacks,
        );
        let mut row = row;
        row.items = vec![
            AgentDetailItem::ToolInvocation(detail_a.clone()),
            AgentDetailItem::ToolInvocation(detail_b.clone()),
        ];
        let mut rows = ConversationTimelineRows::new(vec![TimelineRow::Agent(Box::new(row))]);

        detail_a.approval_decidable = false;
        let key = rows
            .update_tool_invocation(detail_a.clone())
            .expect("availability update should find invocation block");
        assert_eq!(key, TimelineRowKey::Agent(run_a.to_string()));
        let TimelineRow::Agent(agent) = rows.row(0).unwrap() else {
            panic!("expected agent row");
        };
        assert!(matches!(
            &agent.items[0],
            AgentDetailItem::ToolInvocation(detail) if !detail.approval_decidable
        ));
        assert!(matches!(
            &agent.items[1],
            AgentDetailItem::ToolInvocation(detail) if detail.id == "invocation-action-b" && detail.approval_decidable
        ));
    }

    #[test]
    fn collapsed_projection_borrows_large_tool_payloads_and_retains_only_metadata() {
        let run_id = AgentRunId::from("run-large-tool");
        let invocation_id = "invocation-large-tool".to_string();
        let mut invocation = tool_invocation(&invocation_id, run_id.clone());
        invocation.input.arguments.value =
            serde_json::json!({ "large": "argument-marker".repeat(64 * 1_024) });
        invocation.output = Some(ToolInvocationOutput {
            content: vec![ContentPart::Text {
                text: "large-result-marker".repeat(64 * 1_024),
            }],
            structured_output: None,
            raw_output: Some(ProviderRawPayload {
                provider_kind: "synthetic".to_string(),
                value: serde_json::json!({ "raw": "provider-raw-marker".repeat(64 * 1_024) }),
            }),
            is_error: false,
        });
        let mut lifecycle = entry(
            "entry-large-tool-result",
            1,
            Some(run_id.clone()),
            ConversationEntryPayload::ToolResult(ToolResultEntry {
                tool_invocation_id: Some(invocation_id.clone()),
                call_id: "call-invocation-large-tool".to_string(),
                content: vec![ContentPart::Text {
                    text: "entry-result-marker".repeat(64 * 1_024),
                }],
                is_error: false,
                structured_output: None,
                raw_output: Some(ProviderRawPayload {
                    provider_kind: "synthetic".to_string(),
                    value: serde_json::json!({ "raw": "entry-raw-marker".repeat(64 * 1_024) }),
                }),
            }),
        );
        lifecycle.tool_invocation_id = Some(invocation_id.clone());
        let entries = vec![lifecycle];
        let invocations = vec![invocation];
        let run = active_run(run_id.clone());

        let (_, mut entries_by_run) =
            collect_pending_rows(&entries, std::slice::from_ref(&run), &invocations, None);
        let mut invocations_by_run = group_invocations_by_run(&invocations);
        assert!(std::ptr::eq(
            entries_by_run.get(&run_id).unwrap()[0],
            &entries[0]
        ));
        assert!(std::ptr::eq(
            invocations_by_run.get(&run_id).unwrap()[0],
            &invocations[0]
        ));

        let projected = project_agent_details(
            entries_by_run.remove(&run_id).unwrap(),
            invocations_by_run.remove(&run_id).unwrap(),
            &HashMap::new(),
            &HashMap::new(),
            &HashSet::new(),
        );

        assert_eq!(projected.len(), 1);
        let AgentDetailItem::ToolInvocation(detail) = &projected[0] else {
            panic!("collapsed lifecycle must project to one lightweight invocation item");
        };
        assert_eq!(detail.id, invocation_id);
        assert!(!detail.expanded);
        assert!(detail.preview.is_none());
        assert_eq!(detail.runtime_tool_name.text, "read_file");
    }

    #[test]
    fn updating_one_entry_preserves_other_timeline_rows() {
        let first = entry(
            "entry-1",
            1,
            None,
            ConversationEntryPayload::Message {
                role: TranscriptRole::User,
                content: vec![ContentPart::Text {
                    text: "before".to_string(),
                }],
            },
        );
        let second = entry(
            "entry-2",
            2,
            None,
            ConversationEntryPayload::Message {
                role: TranscriptRole::User,
                content: vec![ContentPart::Text {
                    text: "untouched".to_string(),
                }],
            },
        );
        let on_copy: OnCopy = Rc::new(|_, _, _| true);
        let mut rows = ConversationTimelineRows::new(vec![
            TimelineRow::User(Box::new(UserMessageRow {
                item: first,
                image_attachments: Vec::new(),
                text_state: None,
                on_copy: on_copy.clone(),
            })),
            TimelineRow::User(Box::new(UserMessageRow {
                item: second.clone(),
                image_attachments: Vec::new(),
                text_state: None,
                on_copy,
            })),
        ]);
        let updated = entry(
            "entry-1",
            1,
            None,
            ConversationEntryPayload::Message {
                role: TranscriptRole::User,
                content: vec![ContentPart::Text {
                    text: "after".to_string(),
                }],
            },
        );

        let key = rows.update_entry(updated.clone(), &[], None);

        assert_eq!(key, Some(TimelineRowKey::User("entry-1".to_string())));
        let TimelineRow::User(first) = &rows.rows[0] else {
            panic!("first row must remain a user row");
        };
        let TimelineRow::User(second_row) = &rows.rows[1] else {
            panic!("second row must remain a user row");
        };
        assert_eq!(first.item, updated);
        assert_eq!(second_row.item, second);
    }

    #[test]
    fn final_item_is_derived_after_run_completion() {
        let run_id = AgentRunId::from("run-1");
        let final_entry = entry(
            "entry-final",
            1,
            Some(run_id.clone()),
            ConversationEntryPayload::Message {
                role: TranscriptRole::Assistant,
                content: vec![ContentPart::Text {
                    text: "final response".to_string(),
                }],
            },
        );
        let callbacks = callbacks(|_, _, _| {}, |_, _, _| {}, |_, _, _| true, |_, _, _, _| {});
        let row = agent_turn_row(
            Some(run_id.clone()),
            Some(active_run(run_id.clone())),
            vec![&final_entry],
            Vec::new(),
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            &HashSet::new(),
            &HashMap::new(),
            callbacks,
        );
        assert!(row.final_item().is_none());
        let mut rows = ConversationTimelineRows::new(vec![TimelineRow::Agent(Box::new(row))]);
        let mut completed = active_run(run_id);
        completed.status = AgentRunStatus::Completed;
        completed.output = Some(AgentRunOutput {
            final_entry_id: final_entry.id.clone(),
            stopped_reason: AgentStoppedReason::Completed,
        });
        completed.completed_at = Some(OffsetDateTime::UNIX_EPOCH);

        rows.update_run(completed);

        let TimelineRow::Agent(row) = &rows.rows[0] else {
            panic!("run row must remain an agent row");
        };
        assert_eq!(row.final_item(), Some(&final_entry));
    }

    fn active_run(id: AgentRunId) -> AgentRun {
        AgentRun {
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

    fn tool_invocation(id: &str, agent_run_id: AgentRunId) -> ToolInvocation {
        ToolInvocation {
            id: id.to_string(),
            agent_run_id,
            provider_step_id: None,
            call_id: format!("call-{id}"),
            source: ToolSource::Local,
            namespace: None,
            server_id: None,
            tool_name: "read_file".to_string(),
            runtime_tool_name: "read_file".to_string(),
            status: ToolInvocationStatus::Running,
            input: ToolInvocationInput {
                source: ToolSource::Local,
                namespace: None,
                tool_name: "read_file".to_string(),
                runtime_tool_name: "read_file".to_string(),
                call_id: format!("call-{id}"),
                arguments: ToolArguments {
                    value: serde_json::json!({"path": "src/main.rs"}),
                },
                approval_policy: ToolApprovalPolicy::Never,
                execution_policy: ToolExecutionPolicy::Foreground,
            },
            output: None,
            error: None,
            approval: None,
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
    ) -> ConversationEntry {
        ConversationEntry {
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

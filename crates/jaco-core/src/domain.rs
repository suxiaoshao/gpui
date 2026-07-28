use time::OffsetDateTime;

use crate::*;
use gpui_operation::Transition;

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectSummary {
    pub id: ProjectId,
    pub path: String,
    pub display_name: String,
    pub kind: ProjectKind,
    pub pinned: bool,
    pub removed: bool,
    pub metadata: ProjectMetadata,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub last_opened_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConversationSummary {
    pub id: ConversationId,
    pub project_id: ProjectId,
    pub title: String,
    pub status: ConversationStatus,
    pub pinned: bool,
    pub prompt_id: Option<PromptId>,
    pub default_provider_id: Option<ProviderId>,
    pub default_model_id: Option<ProviderModelId>,
    pub last_entry_seq: i32,
    pub metadata: ConversationMetadata,
    pub settings_snapshot: ConversationSettingsSnapshot,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub archived_at: Option<OffsetDateTime>,
    pub deleted_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Conversation {
    pub summary: ConversationSummary,
    pub project: ProjectSummary,
    pub entries: Vec<ConversationEntry>,
    pub attachments: Vec<ConversationAttachment>,
    pub runs: Vec<AgentRun>,
    pub provider_steps: Vec<ProviderStep>,
    pub tool_invocations: Vec<ToolInvocation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConversationEntry {
    pub id: ConversationEntryId,
    pub conversation_id: ConversationId,
    pub seq: i32,
    pub kind: ConversationEntryKind,
    pub status: ConversationEntryStatus,
    pub agent_run_id: Option<AgentRunId>,
    pub provider_step_id: Option<ProviderStepId>,
    pub tool_invocation_id: Option<ToolInvocationId>,
    pub provider_item_id: Option<String>,
    pub payload: ConversationEntryPayload,
    pub search_text: String,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConversationAttachment {
    pub id: AttachmentId,
    pub conversation_id: ConversationId,
    pub kind: AttachmentKind,
    pub storage_kind: AttachmentStorageKind,
    pub mime_type: Option<String>,
    pub name: Option<String>,
    pub path: Option<String>,
    pub external_uri: Option<String>,
    pub provider_id: Option<ProviderId>,
    pub provider_file_id: Option<String>,
    pub sha256: Option<String>,
    pub size_bytes: Option<i64>,
    pub metadata: AttachmentMetadata,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AgentRun {
    pub id: AgentRunId,
    pub conversation_id: ConversationId,
    pub trigger_entry_id: ConversationEntryId,
    pub trigger_kind: AgentRunTriggerKind,
    pub status: AgentRunStatus,
    pub input: AgentRunInput,
    pub output: Option<AgentRunOutput>,
    pub error: Option<RunErrorPayload>,
    pub created_at: OffsetDateTime,
    pub started_at: Option<OffsetDateTime>,
    pub completed_at: Option<OffsetDateTime>,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderStep {
    pub id: ProviderStepId,
    pub agent_run_id: AgentRunId,
    pub seq: i32,
    pub provider_id: ProviderId,
    pub model_id: ProviderModelId,
    pub status: ProviderStepStatus,
    pub request_snapshot: ProviderStepRequestSnapshot,
    pub response_snapshot: Option<ProviderStepResponseSnapshot>,
    pub state_snapshot: Option<ProviderRunStateSnapshot>,
    pub settings_snapshot: RunSettingsSnapshot,
    pub error: Option<RunErrorPayload>,
    pub created_at: OffsetDateTime,
    pub started_at: Option<OffsetDateTime>,
    pub completed_at: Option<OffsetDateTime>,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolInvocation {
    pub id: ToolInvocationId,
    pub agent_run_id: AgentRunId,
    pub provider_step_id: Option<ProviderStepId>,
    pub call_id: String,
    pub source: ToolSource,
    pub namespace: Option<String>,
    pub server_id: Option<String>,
    pub tool_name: String,
    pub runtime_tool_name: String,
    pub status: ToolInvocationStatus,
    pub input: ToolInvocationInput,
    pub output: Option<ToolInvocationOutput>,
    pub error: Option<RunErrorPayload>,
    pub approval: Option<ToolInvocationApproval>,
    pub created_at: OffsetDateTime,
    pub started_at: Option<OffsetDateTime>,
    pub completed_at: Option<OffsetDateTime>,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolInvocationApproval {
    pub status: ApprovalStatus,
    pub request: ApprovalRequestPayload,
    pub decision: Option<ApprovalDecisionPayload>,
    pub requested_at: OffsetDateTime,
    pub decided_at: Option<OffsetDateTime>,
    pub expires_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryChangeKind {
    TextAppended,
    Replaced,
    StatusChanged,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConversationChange {
    SummaryChanged {
        summary: Box<ConversationSummary>,
    },
    EntryAppended {
        entry: Box<ConversationEntry>,
    },
    EntryUpdated {
        entry: Box<ConversationEntry>,
        kind: EntryChangeKind,
    },
    AttachmentUpserted {
        attachment: Box<ConversationAttachment>,
    },
    ProviderStepChanged {
        step: Box<ProviderStep>,
    },
    ToolInvocationChanged {
        invocation: Box<ToolInvocation>,
    },
    RunStatusChanged {
        run: Box<AgentRun>,
    },
    Deleted,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConversationChanges(pub Vec<ConversationChange>);

impl From<Vec<ConversationChange>> for ConversationChanges {
    fn from(changes: Vec<ConversationChange>) -> Self {
        Self(changes)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConversationEffect {
    SummaryChanged,
    EntryInserted {
        entry_id: ConversationEntryId,
    },
    EntryChanged {
        entry_id: ConversationEntryId,
        kind: EntryChangeKind,
    },
    AttachmentChanged {
        attachment_id: AttachmentId,
    },
    RunChanged {
        run_id: AgentRunId,
    },
    ProviderStepChanged {
        provider_step_id: ProviderStepId,
    },
    ToolInvocationChanged {
        tool_invocation_id: ToolInvocationId,
    },
    Deleted,
}

impl Transition<ConversationChange> for &mut Conversation {
    type Output = ConversationEffect;

    fn transition(self, change: ConversationChange) -> Self::Output {
        match change {
            ConversationChange::SummaryChanged { summary } => {
                self.summary = *summary;
                ConversationEffect::SummaryChanged
            }
            ConversationChange::EntryAppended { entry } => {
                let entry_id = entry.id.clone();
                if let Some(current) = self
                    .entries
                    .iter_mut()
                    .find(|current| current.id == entry_id)
                {
                    *current = *entry;
                } else {
                    self.entries.push(*entry);
                    self.entries.sort_by(|left, right| {
                        left.seq
                            .cmp(&right.seq)
                            .then_with(|| left.id.cmp(&right.id))
                    });
                }
                ConversationEffect::EntryInserted { entry_id }
            }
            ConversationChange::EntryUpdated { entry, kind } => {
                let entry_id = entry.id.clone();
                if let Some(current) = self
                    .entries
                    .iter_mut()
                    .find(|current| current.id == entry_id)
                {
                    *current = *entry;
                    ConversationEffect::EntryChanged { entry_id, kind }
                } else {
                    self.entries.push(*entry);
                    self.entries.sort_by(|left, right| {
                        left.seq
                            .cmp(&right.seq)
                            .then_with(|| left.id.cmp(&right.id))
                    });
                    ConversationEffect::EntryInserted { entry_id }
                }
            }
            ConversationChange::AttachmentUpserted { attachment } => {
                let attachment_id = attachment.id.clone();
                if let Some(current) = self
                    .attachments
                    .iter_mut()
                    .find(|current| current.id == attachment_id)
                {
                    *current = *attachment;
                } else {
                    self.attachments.push(*attachment);
                }
                ConversationEffect::AttachmentChanged { attachment_id }
            }
            ConversationChange::RunStatusChanged { run } => {
                let run_id = run.id.clone();
                if let Some(current) = self.runs.iter_mut().find(|current| current.id == run_id) {
                    *current = *run;
                } else {
                    self.runs.push(*run);
                }
                ConversationEffect::RunChanged { run_id }
            }
            ConversationChange::ProviderStepChanged { step } => {
                let provider_step_id = step.id.clone();
                if let Some(current) = self
                    .provider_steps
                    .iter_mut()
                    .find(|current| current.id == provider_step_id)
                {
                    *current = *step;
                } else {
                    self.provider_steps.push(*step);
                    self.provider_steps.sort_by(|left, right| {
                        left.agent_run_id
                            .cmp(&right.agent_run_id)
                            .then_with(|| left.seq.cmp(&right.seq))
                            .then_with(|| left.id.cmp(&right.id))
                    });
                }
                ConversationEffect::ProviderStepChanged { provider_step_id }
            }
            ConversationChange::ToolInvocationChanged { invocation } => {
                let tool_invocation_id = invocation.id.clone();
                if let Some(current) = self
                    .tool_invocations
                    .iter_mut()
                    .find(|current| current.id == tool_invocation_id)
                {
                    *current = *invocation;
                } else {
                    self.tool_invocations.push(*invocation);
                }
                ConversationEffect::ToolInvocationChanged { tool_invocation_id }
            }
            ConversationChange::Deleted => ConversationEffect::Deleted,
        }
    }
}

impl Transition<ConversationChanges> for &mut Conversation {
    type Output = Vec<ConversationEffect>;

    fn transition(self, changes: ConversationChanges) -> Self::Output {
        changes
            .0
            .into_iter()
            .map(|change| (&mut *self).transition(change))
            .collect()
    }
}

impl Transition<ConversationChanges> for &mut Option<Conversation> {
    type Output = Vec<ConversationEffect>;

    fn transition(self, changes: ConversationChanges) -> Self::Output {
        let Some(conversation) = self.as_mut() else {
            return Vec::new();
        };
        let mut effects = Vec::with_capacity(changes.0.len());
        for change in changes.0 {
            if matches!(change, ConversationChange::Deleted) {
                *self = None;
                effects.push(ConversationEffect::Deleted);
                break;
            }
            effects.push((&mut *conversation).transition(change));
        }
        effects
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversation_changes_only_update_the_target_entry() {
        let mut conversation = conversation(vec![
            entry("entry-1", 1, "before"),
            entry("entry-2", 2, "untouched"),
        ]);
        let untouched = conversation.entries[1].clone();
        let updated = entry("entry-1", 1, "before and after");
        let changes = ConversationChanges(vec![ConversationChange::EntryUpdated {
            entry: Box::new(updated.clone()),
            kind: EntryChangeKind::TextAppended,
        }]);

        let effects = (&mut conversation).transition(changes);

        assert_eq!(conversation.entries[0], updated);
        assert_eq!(conversation.entries[1], untouched);
        assert_eq!(
            effects,
            vec![ConversationEffect::EntryChanged {
                entry_id: "entry-1".to_string(),
                kind: EntryChangeKind::TextAppended,
            }]
        );
    }

    #[test]
    fn updating_a_missing_entry_reports_the_actual_insert_effect() {
        let mut conversation = Some(conversation(Vec::new()));
        let inserted = entry("entry-1", 1, "inserted");

        let effects = (&mut conversation).transition(ConversationChanges(vec![
            ConversationChange::EntryUpdated {
                entry: Box::new(inserted.clone()),
                kind: EntryChangeKind::Replaced,
            },
        ]));

        assert_eq!(
            effects,
            vec![ConversationEffect::EntryInserted {
                entry_id: inserted.id.clone(),
            }]
        );
        assert_eq!(conversation.unwrap().entries, vec![inserted]);
    }

    #[test]
    fn deleting_a_conversation_clears_the_optional_data() {
        let mut conversation = Some(conversation(Vec::new()));

        let effects =
            (&mut conversation).transition(ConversationChanges(vec![ConversationChange::Deleted]));

        assert!(conversation.is_none());
        assert_eq!(effects, vec![ConversationEffect::Deleted]);
    }

    fn conversation(entries: Vec<ConversationEntry>) -> Conversation {
        Conversation {
            summary: ConversationSummary {
                id: "conversation-1".to_string(),
                project_id: "project-1".to_string(),
                title: "Conversation".to_string(),
                status: ConversationStatus::Active,
                pinned: false,
                prompt_id: None,
                default_provider_id: None,
                default_model_id: None,
                last_entry_seq: entries.last().map_or(0, |entry| entry.seq),
                metadata: ConversationMetadata {
                    summary: None,
                    tags: Vec::new(),
                },
                settings_snapshot: ConversationSettingsSnapshot {
                    prompt: None,
                    provider_id: None,
                    model_id: None,
                    model_capabilities: None,
                    tool_policy: ToolPolicySnapshot {
                        approval_policy: ToolApprovalPolicy::Never,
                        enabled_sources: Vec::new(),
                        max_steps: 1,
                        approval_mode: ToolApprovalMode::RequestApproval,
                        permission_scope: None,
                    },
                },
                created_at: OffsetDateTime::UNIX_EPOCH,
                updated_at: OffsetDateTime::UNIX_EPOCH,
                archived_at: None,
                deleted_at: None,
            },
            project: ProjectSummary {
                id: "project-1".to_string(),
                path: "/tmp/project".to_string(),
                display_name: "Project".to_string(),
                kind: ProjectKind::Normal,
                pinned: false,
                removed: false,
                metadata: ProjectMetadata {
                    scratch_reason: None,
                    git_root: None,
                    last_active_conversation_id: None,
                },
                created_at: OffsetDateTime::UNIX_EPOCH,
                updated_at: OffsetDateTime::UNIX_EPOCH,
                last_opened_at: None,
            },
            entries,
            attachments: Vec::new(),
            runs: Vec::new(),
            provider_steps: Vec::new(),
            tool_invocations: Vec::new(),
        }
    }

    fn entry(id: &str, seq: i32, text: &str) -> ConversationEntry {
        ConversationEntry {
            id: id.to_string(),
            conversation_id: "conversation-1".to_string(),
            seq,
            kind: ConversationEntryKind::Message,
            status: ConversationEntryStatus::Completed,
            agent_run_id: None,
            provider_step_id: None,
            tool_invocation_id: None,
            provider_item_id: None,
            payload: ConversationEntryPayload::Message {
                role: TranscriptRole::Assistant,
                content: vec![ContentPart::Text {
                    text: text.to_string(),
                }],
            },
            search_text: text.to_string(),
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }
    }
}

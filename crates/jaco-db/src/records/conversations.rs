use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct NewConversationTransaction {
    pub new_project: Option<(ProjectId, NewProject)>,
    pub conversation_id: ConversationId,
    pub conversation: NewConversationWithUserItem,
    pub attachments: Vec<NewAttachment>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreatedConversationTransaction {
    pub project: ProjectRecord,
    pub record: ConversationWithUserItemRecord,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SendConversationTransaction {
    pub entry: NewConversationEntry,
    pub attachments: Vec<NewAttachment>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SentConversationTransaction {
    pub project: ProjectRecord,
    pub commit: ConversationCommit<ConversationEntryRecord>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConversationRecord {
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
pub struct ConversationCommit<T> {
    pub value: T,
    pub conversation: ConversationRecord,
    pub index_delta: ConversationIndexDelta,
}

impl<T> ConversationCommit<T> {
    pub fn into_value(self) -> T {
        self.value
    }
}

impl<T> std::ops::Deref for ConversationCommit<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConversationIndexDelta {
    InsertIfMissing(Box<ConversationRecord>),
    EntryAdvanced {
        id: ConversationId,
        last_entry_seq: i32,
        updated_at: OffsetDateTime,
    },
    PresentationChanged {
        id: ConversationId,
        title: Option<String>,
        pinned: Option<bool>,
        status: Option<ConversationStatus>,
        updated_at: OffsetDateTime,
    },
    Remove {
        id: ConversationId,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConversationChange {
    EntryAppended { entry: ConversationEntryRecord },
    EntryUpdated { entry: ConversationEntryRecord },
    ProviderStepChanged { step: Box<ProviderStepRecord> },
    ToolInvocationChanged { invocation: ToolInvocationRecord },
    RunStatusChanged { run: AgentRunRecord },
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewConversation {
    pub project_id: ProjectId,
    pub title: String,
    pub pinned: bool,
    pub prompt_id: Option<PromptId>,
    pub default_provider_id: Option<ProviderId>,
    pub default_model_id: Option<ProviderModelId>,
    pub metadata: ConversationMetadata,
    pub settings_snapshot: ConversationSettingsSnapshot,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewConversationWithUserItem {
    pub conversation: NewConversation,
    pub user_item: NewConversationEntry,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConversationWithUserItemRecord {
    pub conversation: ConversationRecord,
    pub user_item: ConversationEntryRecord,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConversationTimelineRecords {
    pub conversation: ConversationRecord,
    pub project: ProjectRecord,
    pub items: Vec<ConversationEntryRecord>,
    pub attachments: Vec<AttachmentRecord>,
    pub runs: Vec<AgentRunRecord>,
    pub tool_invocations: Vec<ToolInvocationRecord>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConversationEntryRecord {
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
pub struct NewConversationEntry {
    pub conversation_id: ConversationId,
    pub status: ConversationEntryStatus,
    pub agent_run_id: Option<AgentRunId>,
    pub provider_step_id: Option<ProviderStepId>,
    pub tool_invocation_id: Option<ToolInvocationId>,
    pub provider_item_id: Option<String>,
    pub payload: ConversationEntryPayload,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttachmentRecord {
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
pub struct NewAttachment {
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
}

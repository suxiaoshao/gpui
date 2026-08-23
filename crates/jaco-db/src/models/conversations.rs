use super::*;

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = conversations)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub(crate) struct SqlConversationRow {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) title: String,
    pub(crate) status: String,
    pub(crate) pinned: bool,
    pub(crate) prompt_id: Option<String>,
    pub(crate) default_provider_id: Option<String>,
    pub(crate) default_model_id: Option<String>,
    pub(crate) last_entry_seq: i32,
    pub(crate) metadata_json: Value,
    pub(crate) settings_snapshot_json: Value,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) updated_at: OffsetDateTime,
    pub(crate) recency_at: OffsetDateTime,
    pub(crate) archived_at: Option<OffsetDateTime>,
    pub(crate) deleted_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = conversations)]
pub(crate) struct SqlNewConversationRow {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) title: String,
    pub(crate) status: String,
    pub(crate) pinned: bool,
    pub(crate) prompt_id: Option<String>,
    pub(crate) default_provider_id: Option<String>,
    pub(crate) default_model_id: Option<String>,
    pub(crate) last_entry_seq: i32,
    pub(crate) metadata_json: Value,
    pub(crate) settings_snapshot_json: Value,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) updated_at: OffsetDateTime,
    pub(crate) recency_at: OffsetDateTime,
    pub(crate) archived_at: Option<OffsetDateTime>,
    pub(crate) deleted_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = conversation_entries)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub(crate) struct SqlConversationEntryRow {
    pub(crate) id: String,
    pub(crate) conversation_id: String,
    pub(crate) seq: i32,
    pub(crate) kind: String,
    pub(crate) status: String,
    pub(crate) agent_run_id: Option<String>,
    pub(crate) provider_step_id: Option<String>,
    pub(crate) tool_invocation_id: Option<String>,
    pub(crate) provider_item_id: Option<String>,
    pub(crate) payload_json: Value,
    pub(crate) search_text: String,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = conversation_entries)]
pub(crate) struct SqlNewConversationEntryRow {
    pub(crate) id: String,
    pub(crate) conversation_id: String,
    pub(crate) seq: i32,
    pub(crate) kind: String,
    pub(crate) status: String,
    pub(crate) agent_run_id: Option<String>,
    pub(crate) provider_step_id: Option<String>,
    pub(crate) tool_invocation_id: Option<String>,
    pub(crate) provider_item_id: Option<String>,
    pub(crate) payload_json: Value,
    pub(crate) search_text: String,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, AsChangeset)]
#[diesel(table_name = conversation_entries)]
pub(crate) struct SqlConversationEntryPayloadChanges {
    pub(crate) kind: String,
    pub(crate) status: String,
    pub(crate) payload_json: Value,
    pub(crate) search_text: String,
    pub(crate) updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = attachments)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub(crate) struct SqlAttachmentRow {
    pub(crate) id: String,
    pub(crate) conversation_id: String,
    pub(crate) kind: String,
    pub(crate) storage_kind: String,
    pub(crate) mime_type: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) path: Option<String>,
    pub(crate) external_uri: Option<String>,
    pub(crate) provider_id: Option<String>,
    pub(crate) provider_file_id: Option<String>,
    pub(crate) sha256: Option<String>,
    pub(crate) size_bytes: Option<i64>,
    pub(crate) metadata_json: Value,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = attachments)]
pub(crate) struct SqlNewAttachmentRow {
    pub(crate) id: String,
    pub(crate) conversation_id: String,
    pub(crate) kind: String,
    pub(crate) storage_kind: String,
    pub(crate) mime_type: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) path: Option<String>,
    pub(crate) external_uri: Option<String>,
    pub(crate) provider_id: Option<String>,
    pub(crate) provider_file_id: Option<String>,
    pub(crate) sha256: Option<String>,
    pub(crate) size_bytes: Option<i64>,
    pub(crate) metadata_json: Value,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) updated_at: OffsetDateTime,
}

impl TryFrom<SqlConversationRow> for ConversationRecord {
    type Error = DbError;

    fn try_from(row: SqlConversationRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            project_id: row.project_id,
            title: row.title,
            status: db_label_parse(row.status)?,
            pinned: row.pinned,
            prompt_id: row.prompt_id,
            default_provider_id: row.default_provider_id,
            default_model_id: row.default_model_id,
            last_entry_seq: row.last_entry_seq,
            metadata: from_json(row.metadata_json)?,
            settings_snapshot: from_json(row.settings_snapshot_json)?,
            created_at: row.created_at,
            updated_at: row.updated_at,
            recency_at: row.recency_at,
            archived_at: row.archived_at,
            deleted_at: row.deleted_at,
        })
    }
}

impl TryFrom<SqlConversationEntryRow> for ConversationEntryRecord {
    type Error = DbError;

    fn try_from(row: SqlConversationEntryRow) -> Result<Self> {
        let payload: ConversationEntryPayload = from_json(row.payload_json)?;
        let kind = db_label_parse(row.kind)?;
        if payload.kind() != kind {
            return Err(DbError::Invariant(
                "conversation item kind does not match payload".to_string(),
            ));
        }
        Ok(Self {
            id: row.id,
            conversation_id: row.conversation_id,
            seq: row.seq,
            kind,
            status: db_label_parse(row.status)?,
            agent_run_id: row.agent_run_id,
            provider_step_id: row.provider_step_id,
            tool_invocation_id: row.tool_invocation_id,
            provider_item_id: row.provider_item_id,
            payload,
            search_text: row.search_text,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

impl TryFrom<SqlAttachmentRow> for AttachmentRecord {
    type Error = DbError;

    fn try_from(row: SqlAttachmentRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            conversation_id: row.conversation_id,
            kind: db_label_parse(row.kind)?,
            storage_kind: db_label_parse(row.storage_kind)?,
            mime_type: row.mime_type,
            name: row.name,
            path: row.path,
            external_uri: row.external_uri,
            provider_id: row.provider_id,
            provider_file_id: row.provider_file_id,
            sha256: row.sha256,
            size_bytes: row.size_bytes,
            metadata: from_json(row.metadata_json)?,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

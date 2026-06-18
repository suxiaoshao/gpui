use crate::{Result, error::DbError, records::*, schema::*};
use ai_chat_core::*;
use diesel::prelude::*;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use time::OffsetDateTime;

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = schema_migrations)]
pub(crate) struct SqlNewSchemaMigrationRow {
    pub(crate) name: String,
    pub(crate) executed_at: OffsetDateTime,
}

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = schema_metadata)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub(crate) struct SqlSchemaMetadataRow {
    pub(crate) id: String,
    pub(crate) schema_version: i32,
    pub(crate) created_app_version: Option<String>,
    pub(crate) last_opened_app_version: Option<String>,
    pub(crate) payload_json: Value,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = schema_metadata)]
pub(crate) struct SqlNewSchemaMetadataRow {
    pub(crate) id: String,
    pub(crate) schema_version: i32,
    pub(crate) created_app_version: Option<String>,
    pub(crate) last_opened_app_version: Option<String>,
    pub(crate) payload_json: Value,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = projects)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub(crate) struct SqlProjectRow {
    pub(crate) id: String,
    pub(crate) path: String,
    pub(crate) display_name: String,
    pub(crate) kind: String,
    pub(crate) pinned: bool,
    pub(crate) removed: bool,
    pub(crate) metadata_json: Value,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) updated_at: OffsetDateTime,
    pub(crate) last_opened_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = projects)]
pub(crate) struct SqlNewProjectRow {
    pub(crate) id: String,
    pub(crate) path: String,
    pub(crate) display_name: String,
    pub(crate) kind: String,
    pub(crate) pinned: bool,
    pub(crate) removed: bool,
    pub(crate) metadata_json: Value,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) updated_at: OffsetDateTime,
    pub(crate) last_opened_at: Option<OffsetDateTime>,
}

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
    pub(crate) last_item_seq: i32,
    pub(crate) metadata_json: Value,
    pub(crate) settings_snapshot_json: Value,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) updated_at: OffsetDateTime,
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
    pub(crate) last_item_seq: i32,
    pub(crate) metadata_json: Value,
    pub(crate) settings_snapshot_json: Value,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) updated_at: OffsetDateTime,
    pub(crate) archived_at: Option<OffsetDateTime>,
    pub(crate) deleted_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = conversation_items)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub(crate) struct SqlConversationItemRow {
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
#[diesel(table_name = conversation_items)]
pub(crate) struct SqlNewConversationItemRow {
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
#[diesel(table_name = conversation_items)]
pub(crate) struct SqlConversationItemPayloadChanges {
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

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = agent_runs)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub(crate) struct SqlAgentRunRow {
    pub(crate) id: String,
    pub(crate) conversation_id: String,
    pub(crate) trigger_kind: String,
    pub(crate) status: String,
    pub(crate) input_json: Value,
    pub(crate) output_json: Option<Value>,
    pub(crate) error_json: Option<Value>,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) started_at: Option<OffsetDateTime>,
    pub(crate) completed_at: Option<OffsetDateTime>,
    pub(crate) updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = agent_runs)]
pub(crate) struct SqlNewAgentRunRow {
    pub(crate) id: String,
    pub(crate) conversation_id: String,
    pub(crate) trigger_kind: String,
    pub(crate) status: String,
    pub(crate) input_json: Value,
    pub(crate) output_json: Option<Value>,
    pub(crate) error_json: Option<Value>,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) started_at: Option<OffsetDateTime>,
    pub(crate) completed_at: Option<OffsetDateTime>,
    pub(crate) updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, AsChangeset)]
#[diesel(table_name = agent_runs)]
#[diesel(treat_none_as_null = true)]
pub(crate) struct SqlAgentRunStatusChanges {
    pub(crate) status: String,
    pub(crate) output_json: Option<Value>,
    pub(crate) error_json: Option<Value>,
    pub(crate) started_at: Option<OffsetDateTime>,
    pub(crate) completed_at: Option<OffsetDateTime>,
    pub(crate) updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = provider_steps)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub(crate) struct SqlProviderStepRow {
    pub(crate) id: String,
    pub(crate) agent_run_id: String,
    pub(crate) seq: i32,
    pub(crate) provider_id: String,
    pub(crate) model_id: String,
    pub(crate) status: String,
    pub(crate) request_snapshot_json: Value,
    pub(crate) response_snapshot_json: Option<Value>,
    pub(crate) state_snapshot_json: Option<Value>,
    pub(crate) settings_snapshot_json: Value,
    pub(crate) error_json: Option<Value>,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) started_at: Option<OffsetDateTime>,
    pub(crate) completed_at: Option<OffsetDateTime>,
    pub(crate) updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = provider_steps)]
pub(crate) struct SqlNewProviderStepRow {
    pub(crate) id: String,
    pub(crate) agent_run_id: String,
    pub(crate) seq: i32,
    pub(crate) provider_id: String,
    pub(crate) model_id: String,
    pub(crate) status: String,
    pub(crate) request_snapshot_json: Value,
    pub(crate) response_snapshot_json: Option<Value>,
    pub(crate) state_snapshot_json: Option<Value>,
    pub(crate) settings_snapshot_json: Value,
    pub(crate) error_json: Option<Value>,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) started_at: Option<OffsetDateTime>,
    pub(crate) completed_at: Option<OffsetDateTime>,
    pub(crate) updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, AsChangeset)]
#[diesel(table_name = provider_steps)]
#[diesel(treat_none_as_null = true)]
pub(crate) struct SqlProviderStepStatusChanges {
    pub(crate) status: String,
    pub(crate) response_snapshot_json: Option<Value>,
    pub(crate) state_snapshot_json: Option<Value>,
    pub(crate) error_json: Option<Value>,
    pub(crate) started_at: Option<OffsetDateTime>,
    pub(crate) completed_at: Option<OffsetDateTime>,
    pub(crate) updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = tool_invocations)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub(crate) struct SqlToolInvocationRow {
    pub(crate) id: String,
    pub(crate) agent_run_id: String,
    pub(crate) provider_step_id: Option<String>,
    pub(crate) call_id: String,
    pub(crate) source: String,
    pub(crate) namespace: Option<String>,
    pub(crate) server_id: Option<String>,
    pub(crate) tool_name: String,
    pub(crate) runtime_tool_name: String,
    pub(crate) status: String,
    pub(crate) input_json: Value,
    pub(crate) output_json: Option<Value>,
    pub(crate) error_json: Option<Value>,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) started_at: Option<OffsetDateTime>,
    pub(crate) completed_at: Option<OffsetDateTime>,
    pub(crate) updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = tool_invocations)]
pub(crate) struct SqlNewToolInvocationRow {
    pub(crate) id: String,
    pub(crate) agent_run_id: String,
    pub(crate) provider_step_id: Option<String>,
    pub(crate) call_id: String,
    pub(crate) source: String,
    pub(crate) namespace: Option<String>,
    pub(crate) server_id: Option<String>,
    pub(crate) tool_name: String,
    pub(crate) runtime_tool_name: String,
    pub(crate) status: String,
    pub(crate) input_json: Value,
    pub(crate) output_json: Option<Value>,
    pub(crate) error_json: Option<Value>,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) started_at: Option<OffsetDateTime>,
    pub(crate) completed_at: Option<OffsetDateTime>,
    pub(crate) updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, AsChangeset)]
#[diesel(table_name = tool_invocations)]
#[diesel(treat_none_as_null = true)]
pub(crate) struct SqlToolInvocationStatusChanges {
    pub(crate) status: String,
    pub(crate) output_json: Option<Value>,
    pub(crate) error_json: Option<Value>,
    pub(crate) started_at: Option<OffsetDateTime>,
    pub(crate) completed_at: Option<OffsetDateTime>,
    pub(crate) updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = approval_decisions)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub(crate) struct SqlApprovalDecisionRow {
    pub(crate) id: String,
    pub(crate) tool_invocation_id: String,
    pub(crate) status: String,
    pub(crate) request_json: Value,
    pub(crate) decision_json: Option<Value>,
    pub(crate) requested_at: OffsetDateTime,
    pub(crate) decided_at: Option<OffsetDateTime>,
    pub(crate) expires_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = approval_decisions)]
pub(crate) struct SqlNewApprovalDecisionRow {
    pub(crate) id: String,
    pub(crate) tool_invocation_id: String,
    pub(crate) status: String,
    pub(crate) request_json: Value,
    pub(crate) decision_json: Option<Value>,
    pub(crate) requested_at: OffsetDateTime,
    pub(crate) decided_at: Option<OffsetDateTime>,
    pub(crate) expires_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, AsChangeset)]
#[diesel(table_name = approval_decisions)]
#[diesel(treat_none_as_null = true)]
pub(crate) struct SqlApprovalDecisionChanges {
    pub(crate) status: String,
    pub(crate) decision_json: Option<Value>,
    pub(crate) decided_at: Option<OffsetDateTime>,
    pub(crate) expires_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = usage_events)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub(crate) struct SqlUsageEventRow {
    pub(crate) id: String,
    pub(crate) provider_step_id: String,
    pub(crate) conversation_id: String,
    pub(crate) provider_id: String,
    pub(crate) model_id: String,
    pub(crate) date_key: String,
    pub(crate) input_tokens: i64,
    pub(crate) output_tokens: i64,
    pub(crate) cached_input_tokens: i64,
    pub(crate) cache_write_input_tokens: i64,
    pub(crate) reasoning_tokens: i64,
    pub(crate) total_tokens: i64,
    pub(crate) usage_json: Value,
    pub(crate) created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = usage_events)]
pub(crate) struct SqlNewUsageEventRow {
    pub(crate) id: String,
    pub(crate) provider_step_id: String,
    pub(crate) conversation_id: String,
    pub(crate) provider_id: String,
    pub(crate) model_id: String,
    pub(crate) date_key: String,
    pub(crate) input_tokens: i64,
    pub(crate) output_tokens: i64,
    pub(crate) cached_input_tokens: i64,
    pub(crate) cache_write_input_tokens: i64,
    pub(crate) reasoning_tokens: i64,
    pub(crate) total_tokens: i64,
    pub(crate) usage_json: Value,
    pub(crate) created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = prompts)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub(crate) struct SqlPromptRow {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) content: String,
    pub(crate) enabled: bool,
    pub(crate) sort_order: i32,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = prompts)]
pub(crate) struct SqlNewPromptRow {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) content: String,
    pub(crate) enabled: bool,
    pub(crate) sort_order: i32,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = shortcuts)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub(crate) struct SqlShortcutRow {
    pub(crate) id: String,
    pub(crate) hotkey: String,
    pub(crate) enabled: bool,
    pub(crate) prompt_id: Option<String>,
    pub(crate) provider_id: Option<String>,
    pub(crate) model_id: Option<String>,
    pub(crate) input_source: String,
    pub(crate) action_json: Value,
    pub(crate) settings_snapshot_json: Value,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = shortcuts)]
pub(crate) struct SqlNewShortcutRow {
    pub(crate) id: String,
    pub(crate) hotkey: String,
    pub(crate) enabled: bool,
    pub(crate) prompt_id: Option<String>,
    pub(crate) provider_id: Option<String>,
    pub(crate) model_id: Option<String>,
    pub(crate) input_source: String,
    pub(crate) action_json: Value,
    pub(crate) settings_snapshot_json: Value,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = providers)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub(crate) struct SqlProviderRow {
    pub(crate) id: String,
    pub(crate) kind: String,
    pub(crate) display_name: String,
    pub(crate) enabled: bool,
    pub(crate) settings_json: Value,
    pub(crate) secret_refs_json: Value,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = providers)]
pub(crate) struct SqlNewProviderRow {
    pub(crate) id: String,
    pub(crate) kind: String,
    pub(crate) display_name: String,
    pub(crate) enabled: bool,
    pub(crate) settings_json: Value,
    pub(crate) secret_refs_json: Value,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = provider_models)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub(crate) struct SqlProviderModelRow {
    pub(crate) id: String,
    pub(crate) provider_id: String,
    pub(crate) model_id: String,
    pub(crate) display_name: Option<String>,
    pub(crate) enabled: bool,
    pub(crate) capabilities_json: Value,
    pub(crate) metadata_json: Value,
    pub(crate) fetched_at: OffsetDateTime,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = provider_models)]
pub(crate) struct SqlNewProviderModelRow {
    pub(crate) id: String,
    pub(crate) provider_id: String,
    pub(crate) model_id: String,
    pub(crate) display_name: Option<String>,
    pub(crate) enabled: bool,
    pub(crate) capabilities_json: Value,
    pub(crate) metadata_json: Value,
    pub(crate) fetched_at: OffsetDateTime,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) updated_at: OffsetDateTime,
}

impl TryFrom<SqlSchemaMetadataRow> for SchemaMetadataRecord {
    type Error = DbError;

    fn try_from(row: SqlSchemaMetadataRow) -> Result<Self> {
        if row.id != "default" {
            return Err(DbError::Invariant(format!(
                "unexpected schema metadata id {}",
                row.id
            )));
        }
        Ok(Self {
            schema_version: row.schema_version,
            created_app_version: row.created_app_version,
            last_opened_app_version: row.last_opened_app_version,
            payload: from_json(row.payload_json)?,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

impl TryFrom<SqlProjectRow> for ProjectRecord {
    type Error = DbError;

    fn try_from(row: SqlProjectRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            path: row.path,
            display_name: row.display_name,
            kind: db_label_parse(row.kind)?,
            pinned: row.pinned,
            removed: row.removed,
            metadata: from_json(row.metadata_json)?,
            created_at: row.created_at,
            updated_at: row.updated_at,
            last_opened_at: row.last_opened_at,
        })
    }
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
            last_item_seq: row.last_item_seq,
            metadata: from_json(row.metadata_json)?,
            settings_snapshot: from_json(row.settings_snapshot_json)?,
            created_at: row.created_at,
            updated_at: row.updated_at,
            archived_at: row.archived_at,
            deleted_at: row.deleted_at,
        })
    }
}

impl TryFrom<SqlConversationItemRow> for ConversationItemRecord {
    type Error = DbError;

    fn try_from(row: SqlConversationItemRow) -> Result<Self> {
        let payload: ConversationItemPayload = from_json(row.payload_json)?;
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

impl TryFrom<SqlAgentRunRow> for AgentRunRecord {
    type Error = DbError;

    fn try_from(row: SqlAgentRunRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            conversation_id: row.conversation_id,
            trigger_kind: db_label_parse(row.trigger_kind)?,
            status: db_label_parse(row.status)?,
            input: from_json(row.input_json)?,
            output: from_json_opt(row.output_json)?,
            error: from_json_opt(row.error_json)?,
            created_at: row.created_at,
            started_at: row.started_at,
            completed_at: row.completed_at,
            updated_at: row.updated_at,
        })
    }
}

impl TryFrom<SqlProviderStepRow> for ProviderStepRecord {
    type Error = DbError;

    fn try_from(row: SqlProviderStepRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            agent_run_id: row.agent_run_id,
            seq: row.seq,
            provider_id: row.provider_id,
            model_id: row.model_id,
            status: db_label_parse(row.status)?,
            request_snapshot: from_json(row.request_snapshot_json)?,
            response_snapshot: from_json_opt(row.response_snapshot_json)?,
            state_snapshot: from_json_opt(row.state_snapshot_json)?,
            settings_snapshot: from_json(row.settings_snapshot_json)?,
            error: from_json_opt(row.error_json)?,
            created_at: row.created_at,
            started_at: row.started_at,
            completed_at: row.completed_at,
            updated_at: row.updated_at,
        })
    }
}

impl TryFrom<SqlToolInvocationRow> for ToolInvocationRecord {
    type Error = DbError;

    fn try_from(row: SqlToolInvocationRow) -> Result<Self> {
        let input: ToolInvocationInput = from_json(row.input_json)?;
        if input.call_id != row.call_id
            || input.tool_name != row.tool_name
            || input.runtime_tool_name != row.runtime_tool_name
            || tool_source_label(&input.source) != row.source
        {
            return Err(DbError::Invariant(
                "tool invocation indexes do not match input payload".to_string(),
            ));
        }
        Ok(Self {
            id: row.id,
            agent_run_id: row.agent_run_id,
            provider_step_id: row.provider_step_id,
            call_id: row.call_id,
            source: input.source.clone(),
            namespace: row.namespace,
            server_id: row.server_id,
            tool_name: row.tool_name,
            runtime_tool_name: row.runtime_tool_name,
            status: db_label_parse(row.status)?,
            input,
            output: from_json_opt(row.output_json)?,
            error: from_json_opt(row.error_json)?,
            created_at: row.created_at,
            started_at: row.started_at,
            completed_at: row.completed_at,
            updated_at: row.updated_at,
        })
    }
}

impl TryFrom<SqlApprovalDecisionRow> for ApprovalDecisionRecord {
    type Error = DbError;

    fn try_from(row: SqlApprovalDecisionRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            tool_invocation_id: row.tool_invocation_id,
            status: db_label_parse(row.status)?,
            request: from_json(row.request_json)?,
            decision: from_json_opt(row.decision_json)?,
            requested_at: row.requested_at,
            decided_at: row.decided_at,
            expires_at: row.expires_at,
        })
    }
}

impl TryFrom<SqlUsageEventRow> for UsageEventRecord {
    type Error = DbError;

    fn try_from(row: SqlUsageEventRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            provider_step_id: row.provider_step_id,
            conversation_id: row.conversation_id,
            provider_id: row.provider_id,
            model_id: row.model_id,
            date_key: row.date_key,
            input_tokens: row.input_tokens,
            output_tokens: row.output_tokens,
            cached_input_tokens: row.cached_input_tokens,
            cache_write_input_tokens: row.cache_write_input_tokens,
            reasoning_tokens: row.reasoning_tokens,
            total_tokens: row.total_tokens,
            usage: from_json(row.usage_json)?,
            created_at: row.created_at,
        })
    }
}

impl TryFrom<SqlPromptRow> for PromptRecord {
    type Error = DbError;

    fn try_from(row: SqlPromptRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            name: row.name,
            content: PromptContent { text: row.content },
            enabled: row.enabled,
            sort_order: row.sort_order,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

impl TryFrom<SqlShortcutRow> for ShortcutRecord {
    type Error = DbError;

    fn try_from(row: SqlShortcutRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            hotkey: row.hotkey,
            enabled: row.enabled,
            prompt_id: row.prompt_id,
            provider_id: row.provider_id,
            model_id: row.model_id,
            input_source: db_label_parse(row.input_source)?,
            action: from_json(row.action_json)?,
            settings_snapshot: from_json(row.settings_snapshot_json)?,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

impl TryFrom<SqlProviderRow> for ProviderRecord {
    type Error = DbError;

    fn try_from(row: SqlProviderRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            kind: row.kind,
            display_name: row.display_name,
            enabled: row.enabled,
            settings: from_json(row.settings_json)?,
            secret_refs: from_json(row.secret_refs_json)?,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

impl TryFrom<SqlProviderModelRow> for ProviderModelRecord {
    type Error = DbError;

    fn try_from(row: SqlProviderModelRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            provider_id: row.provider_id,
            model_id: row.model_id,
            display_name: row.display_name,
            enabled: row.enabled,
            capabilities: from_json(row.capabilities_json)?,
            metadata: from_json(row.metadata_json)?,
            fetched_at: row.fetched_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

pub(crate) fn db_label<T: Serialize>(value: &T) -> Result<String> {
    match serde_json::to_value(value)? {
        Value::String(value) => Ok(value),
        _ => Err(DbError::Invariant(
            "database enum labels must serialize to strings".to_string(),
        )),
    }
}

pub(crate) fn db_label_parse<T: DeserializeOwned>(value: String) -> Result<T> {
    Ok(serde_json::from_value(Value::String(value))?)
}

pub(crate) fn to_json<T: Serialize>(value: &T) -> Result<Value> {
    Ok(serde_json::to_value(value)?)
}

pub(crate) fn to_json_opt<T: Serialize>(value: &Option<T>) -> Result<Option<Value>> {
    value.as_ref().map(to_json).transpose()
}

pub(crate) fn from_json<T: DeserializeOwned>(value: Value) -> Result<T> {
    Ok(serde_json::from_value(value)?)
}

pub(crate) fn from_json_opt<T: DeserializeOwned>(value: Option<Value>) -> Result<Option<T>> {
    value.map(from_json).transpose()
}

pub(crate) fn tool_source_label(source: &ToolSource) -> String {
    match source {
        ToolSource::Local => "local".to_string(),
        ToolSource::Mcp { .. } => "mcp".to_string(),
        ToolSource::ProviderHosted { .. } => "provider_hosted".to_string(),
    }
}

pub(crate) fn tool_source_server_id(source: &ToolSource) -> Option<String> {
    match source {
        ToolSource::Mcp { server_id } => Some(server_id.clone()),
        ToolSource::Local | ToolSource::ProviderHosted { .. } => None,
    }
}

pub(crate) fn u64_to_i64(value: u64) -> Result<i64> {
    i64::try_from(value)
        .map_err(|_| DbError::Invariant("usage token count exceeds i64".to_string()))
}

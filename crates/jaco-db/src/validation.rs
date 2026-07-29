use std::collections::HashMap;

use diesel::{
    connection::SimpleConnection,
    prelude::*,
    sql_query,
    sql_types::{BigInt, Text},
};

use crate::{
    DatabaseValidationError,
    migrations::SCHEMA_VERSION,
    models::{
        SqlAgentRunRow, SqlAttachmentRow, SqlConversationEntryRow, SqlConversationRow,
        SqlProjectRow, SqlPromptRow, SqlProviderModelRow, SqlProviderRow, SqlProviderStepRow,
        SqlSchemaMetadataRow, SqlShortcutRow, SqlToolInvocationRow, SqlUsageEventRow,
    },
    records::{
        AgentRunRecord, AttachmentRecord, ConversationEntryRecord, ConversationRecord,
        ProjectRecord, PromptRecord, ProviderModelRecord, ProviderRecord, ProviderStepRecord,
        SchemaMetadataRecord, ShortcutRecord, ToolInvocationRecord, UsageEventRecord,
    },
    schema::{
        agent_runs, attachments, conversation_entries, conversations, projects, prompts,
        provider_models, provider_steps, providers, schema_metadata, shortcuts, tool_invocations,
        usage_events,
    },
};

#[derive(QueryableByName)]
struct TextRow {
    #[diesel(sql_type = Text)]
    value: String,
}

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

pub(crate) fn validate_connection(
    conn: &mut SqliteConnection,
) -> Result<(), DatabaseValidationError> {
    conn.batch_execute("PRAGMA query_only = ON; BEGIN;")
        .map_err(query)?;
    let result = validate_snapshot(conn);
    let rollback = conn.batch_execute("ROLLBACK;");
    match (result, rollback) {
        (result, Ok(())) => result,
        (Ok(()), Err(error)) => Err(query(error)),
        (Err(error), _) => Err(error),
    }
}

fn validate_snapshot(conn: &mut SqliteConnection) -> Result<(), DatabaseValidationError> {
    validate_integrity(conn)?;
    validate_foreign_keys(conn)?;
    validate_schema(conn)?;

    let metadata = schema_metadata::table
        .select(SqlSchemaMetadataRow::as_select())
        .load::<SqlSchemaMetadataRow>(conn)
        .map_err(query)?;
    if metadata.len() != 1 {
        return Err(DatabaseValidationError::Schema(format!(
            "expected one schema_metadata row, found {}",
            metadata.len()
        )));
    }
    let metadata: SchemaMetadataRecord = metadata
        .into_iter()
        .next()
        .expect("length checked")
        .try_into()
        .map_err(stored("schema_metadata"))?;
    if metadata.schema_version != SCHEMA_VERSION {
        return Err(DatabaseValidationError::Schema(format!(
            "expected schema version {SCHEMA_VERSION}, found {}",
            metadata.schema_version
        )));
    }

    convert_all::<SqlProjectRow, ProjectRecord>(
        projects::table
            .select(SqlProjectRow::as_select())
            .load(conn),
        "projects",
    )?;
    convert_all::<SqlProviderRow, ProviderRecord>(
        providers::table
            .select(SqlProviderRow::as_select())
            .load(conn),
        "providers",
    )?;
    convert_all::<SqlProviderModelRow, ProviderModelRecord>(
        provider_models::table
            .select(SqlProviderModelRow::as_select())
            .load(conn),
        "provider_models",
    )?;
    convert_all::<SqlPromptRow, PromptRecord>(
        prompts::table.select(SqlPromptRow::as_select()).load(conn),
        "prompts",
    )?;
    convert_all::<SqlShortcutRow, ShortcutRecord>(
        shortcuts::table
            .select(SqlShortcutRow::as_select())
            .load(conn),
        "shortcuts",
    )?;
    convert_all::<SqlAttachmentRow, AttachmentRecord>(
        attachments::table
            .select(SqlAttachmentRow::as_select())
            .load(conn),
        "attachments",
    )?;
    convert_all::<SqlAgentRunRow, AgentRunRecord>(
        agent_runs::table
            .select(SqlAgentRunRow::as_select())
            .load(conn),
        "agent_runs",
    )?;
    convert_all::<SqlProviderStepRow, ProviderStepRecord>(
        provider_steps::table
            .select(SqlProviderStepRow::as_select())
            .load(conn),
        "provider_steps",
    )?;
    convert_all::<SqlToolInvocationRow, ToolInvocationRecord>(
        tool_invocations::table
            .select(SqlToolInvocationRow::as_select())
            .load(conn),
        "tool_invocations",
    )?;
    convert_all::<SqlUsageEventRow, UsageEventRecord>(
        usage_events::table
            .select(SqlUsageEventRow::as_select())
            .load(conn),
        "usage_events",
    )?;

    let conversations = convert_all::<SqlConversationRow, ConversationRecord>(
        conversations::table
            .select(SqlConversationRow::as_select())
            .load(conn),
        "conversations",
    )?;
    let entries = convert_all::<SqlConversationEntryRow, ConversationEntryRecord>(
        conversation_entries::table
            .select(SqlConversationEntryRow::as_select())
            .load(conn),
        "conversation_entries",
    )?;
    validate_last_entry_seq(&conversations, &entries)
}

fn validate_integrity(conn: &mut SqliteConnection) -> Result<(), DatabaseValidationError> {
    let rows = sql_query("SELECT quick_check AS value FROM pragma_quick_check")
        .load::<TextRow>(conn)
        .map_err(query)?;
    if rows.len() == 1 && rows[0].value == "ok" {
        Ok(())
    } else {
        Err(DatabaseValidationError::Integrity(
            rows.into_iter()
                .map(|row| row.value)
                .collect::<Vec<_>>()
                .join("; "),
        ))
    }
}

fn validate_foreign_keys(conn: &mut SqliteConnection) -> Result<(), DatabaseValidationError> {
    let count = sql_query("SELECT COUNT(*) AS value FROM pragma_foreign_key_check")
        .get_result::<CountRow>(conn)
        .map_err(query)?
        .value;
    if count == 0 {
        Ok(())
    } else {
        Err(DatabaseValidationError::ForeignKey(format!(
            "{count} violation(s)"
        )))
    }
}

fn validate_schema(conn: &mut SqliteConnection) -> Result<(), DatabaseValidationError> {
    const REQUIRED: &[&str] = &[
        "schema_migrations",
        "schema_metadata",
        "projects",
        "providers",
        "provider_models",
        "prompts",
        "conversations",
        "conversation_entries",
        "attachments",
        "agent_runs",
        "provider_steps",
        "tool_invocations",
        "usage_events",
        "shortcuts",
    ];
    let rows =
        sql_query("SELECT name AS value FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .load::<TextRow>(conn)
            .map_err(query)?;
    let names = rows.into_iter().map(|row| row.value).collect::<Vec<_>>();
    let missing = REQUIRED
        .iter()
        .filter(|name| !names.iter().any(|existing| existing == **name))
        .copied()
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(DatabaseValidationError::Schema(format!(
            "missing tables: {}",
            missing.join(", ")
        )))
    }
}

fn convert_all<Row, Record>(
    rows: diesel::QueryResult<Vec<Row>>,
    table: &'static str,
) -> Result<Vec<Record>, DatabaseValidationError>
where
    Row: TryInto<Record, Error = crate::DbError>,
{
    rows.map_err(query)?
        .into_iter()
        .enumerate()
        .map(|(index, row)| {
            row.try_into().map_err(|error| {
                DatabaseValidationError::StoredData(format!("{table} row {index}: {error}"))
            })
        })
        .collect()
}

fn validate_last_entry_seq(
    conversations: &[ConversationRecord],
    entries: &[ConversationEntryRecord],
) -> Result<(), DatabaseValidationError> {
    let mut maximum = HashMap::<&str, i32>::new();
    for entry in entries {
        maximum
            .entry(entry.conversation_id.as_str())
            .and_modify(|current| *current = (*current).max(entry.seq))
            .or_insert(entry.seq);
    }
    for conversation in conversations {
        let expected = maximum.get(conversation.id.as_str()).copied().unwrap_or(0);
        if conversation.last_entry_seq != expected {
            return Err(DatabaseValidationError::Invariant(format!(
                "conversation {} has last_entry_seq {}, expected {expected}",
                conversation.id, conversation.last_entry_seq
            )));
        }
    }
    Ok(())
}

fn query(error: impl std::fmt::Display) -> DatabaseValidationError {
    DatabaseValidationError::Query(error.to_string())
}

fn stored(table: &'static str) -> impl FnOnce(crate::DbError) -> DatabaseValidationError {
    move |error| DatabaseValidationError::StoredData(format!("{table}: {error}"))
}

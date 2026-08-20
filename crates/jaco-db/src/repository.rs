mod agent;
mod analytics;
#[cfg(test)]
pub(crate) use analytics::{DAILY_FINITE_SQL, PROVIDER_MODELS_FINITE_SQL, SUMMARY_FINITE_SQL};
#[path = "repository/conversations.rs"]
mod conversation_repository;
#[path = "repository/projects.rs"]
mod project_repository;
#[path = "repository/prompts.rs"]
mod prompt_repository;
#[path = "repository/providers.rs"]
mod provider_repository;
#[path = "repository/shortcuts.rs"]
mod shortcut_repository;

use crate::{
    DbPool, Result,
    error::DbError,
    models::*,
    records::*,
    schema::{
        agent_runs, attachments, conversation_entries, conversations, projects, prompts,
        provider_models, provider_steps, providers, shortcuts, tool_invocations, usage_events,
    },
};
use diesel::{
    connection::SimpleConnection,
    prelude::*,
    r2d2::{ConnectionManager, PooledConnection},
    sql_query,
    sql_types::Text,
    upsert::excluded,
};
use jaco_core::*;
use std::collections::HashMap;
use time::OffsetDateTime;

#[derive(Clone)]
pub struct FreshRepository {
    pool: DbPool,
}

impl FreshRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn metadata(&self) -> Result<SchemaMetadataRecord> {
        let mut conn = self.conn()?;
        schema_metadata_row(&mut conn)?
            .ok_or_else(|| DbError::Invariant("schema metadata row is missing".to_string()))?
            .try_into()
    }

    pub fn table_names(&self) -> Result<Vec<String>> {
        let mut conn = self.conn()?;
        let rows = sql_query(
            "SELECT name AS value FROM sqlite_master WHERE type IN ('table', 'view') ORDER BY name",
        )
        .load::<TextValueRow>(&mut conn)?;
        Ok(rows.into_iter().map(|row| row.value).collect())
    }

    pub fn has_table(&self, table_name: &str) -> Result<bool> {
        Ok(self.table_names()?.iter().any(|name| name == table_name))
    }

    fn conn(&self) -> Result<PooledConnection<ConnectionManager<SqliteConnection>>> {
        let mut conn = self.pool.get()?;
        conn.batch_execute("PRAGMA foreign_keys = ON;")?;
        Ok(conn)
    }
}
fn schema_metadata_row(conn: &mut SqliteConnection) -> Result<Option<SqlSchemaMetadataRow>> {
    Ok(crate::schema::schema_metadata::table
        .find("default")
        .select(SqlSchemaMetadataRow::as_select())
        .first(conn)
        .optional()?)
}

fn project_row(conn: &mut SqliteConnection, id: &str) -> Result<Option<SqlProjectRow>> {
    Ok(projects::table
        .find(id)
        .select(SqlProjectRow::as_select())
        .first(conn)
        .optional()?)
}

fn provider_row(conn: &mut SqliteConnection, id: &str) -> Result<Option<SqlProviderRow>> {
    Ok(providers::table
        .find(id)
        .select(SqlProviderRow::as_select())
        .first(conn)
        .optional()?)
}

fn provider_model_row(
    conn: &mut SqliteConnection,
    provider_id: &str,
    model_id: &str,
) -> Result<Option<SqlProviderModelRow>> {
    Ok(provider_models::table
        .filter(provider_models::provider_id.eq(provider_id))
        .filter(provider_models::model_id.eq(model_id))
        .select(SqlProviderModelRow::as_select())
        .first(conn)
        .optional()?)
}

fn prompt_row(conn: &mut SqliteConnection, id: &str) -> Result<Option<SqlPromptRow>> {
    Ok(prompts::table
        .find(id)
        .select(SqlPromptRow::as_select())
        .first(conn)
        .optional()?)
}

fn shortcut_row(conn: &mut SqliteConnection, id: &str) -> Result<Option<SqlShortcutRow>> {
    Ok(shortcuts::table
        .find(id)
        .select(SqlShortcutRow::as_select())
        .first(conn)
        .optional()?)
}

fn conversation_row(conn: &mut SqliteConnection, id: &str) -> Result<Option<SqlConversationRow>> {
    Ok(conversations::table
        .find(id)
        .select(SqlConversationRow::as_select())
        .first(conn)
        .optional()?)
}

fn conversation_entry_row(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<Option<SqlConversationEntryRow>> {
    Ok(conversation_entries::table
        .find(id)
        .select(SqlConversationEntryRow::as_select())
        .first(conn)
        .optional()?)
}

fn agent_run_row(conn: &mut SqliteConnection, id: &str) -> Result<Option<SqlAgentRunRow>> {
    Ok(agent_runs::table
        .find(id)
        .select(SqlAgentRunRow::as_select())
        .first(conn)
        .optional()?)
}

fn provider_step_row(conn: &mut SqliteConnection, id: &str) -> Result<Option<SqlProviderStepRow>> {
    Ok(provider_steps::table
        .find(id)
        .select(SqlProviderStepRow::as_select())
        .first(conn)
        .optional()?)
}

fn usage_event_row(
    conn: &mut SqliteConnection,
    provider_step_id: &str,
) -> Result<Option<SqlUsageEventRow>> {
    Ok(usage_events::table
        .filter(usage_events::provider_step_id.eq(provider_step_id))
        .select(SqlUsageEventRow::as_select())
        .first(conn)
        .optional()?)
}

fn usage_events_for_conversation_with_conn(
    conn: &mut SqliteConnection,
    conversation_id: &str,
) -> Result<Vec<UsageEventRecord>> {
    usage_events::table
        .inner_join(provider_steps::table.on(provider_steps::id.eq(usage_events::provider_step_id)))
        .inner_join(agent_runs::table.on(agent_runs::id.eq(provider_steps::agent_run_id)))
        .filter(agent_runs::conversation_id.eq(conversation_id))
        .order((usage_events::created_at.asc(), usage_events::id.asc()))
        .select(SqlUsageEventRow::as_select())
        .load::<SqlUsageEventRow>(conn)?
        .into_iter()
        .map(TryInto::try_into)
        .collect()
}

fn tool_invocation_row(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<Option<SqlToolInvocationRow>> {
    Ok(tool_invocations::table
        .find(id)
        .select(SqlToolInvocationRow::as_select())
        .first(conn)
        .optional()?)
}

fn load_agent_run_row(conn: &mut SqliteConnection, id: &str) -> Result<SqlAgentRunRow> {
    agent_runs::table
        .find(id)
        .select(SqlAgentRunRow::as_select())
        .first(conn)
        .optional()?
        .ok_or_else(|| DbError::Invariant(format!("agent run {id} is missing")))
}

fn load_provider_step_row(conn: &mut SqliteConnection, id: &str) -> Result<SqlProviderStepRow> {
    provider_steps::table
        .find(id)
        .select(SqlProviderStepRow::as_select())
        .first(conn)
        .optional()?
        .ok_or_else(|| DbError::Invariant(format!("provider step {id} is missing")))
}

fn load_tool_invocation_row(conn: &mut SqliteConnection, id: &str) -> Result<SqlToolInvocationRow> {
    tool_invocations::table
        .find(id)
        .select(SqlToolInvocationRow::as_select())
        .first(conn)
        .optional()?
        .ok_or_else(|| DbError::Invariant(format!("tool invocation {id} is missing")))
}

fn append_conversation_entry_with_conn(
    conn: &mut SqliteConnection,
    input: NewConversationEntry,
) -> Result<ConversationEntryRecord> {
    let conversation = conversation_row(conn, &input.conversation_id)?
        .ok_or_else(|| DbError::Invariant("conversation is missing".to_string()))?;
    validate_execution_links(conn, &input.conversation_id, &input)?;

    let seq = conversation.last_entry_seq + 1;
    let now = now_string()?;
    let row = SqlNewConversationEntryRow {
        id: new_id(),
        conversation_id: input.conversation_id.clone(),
        seq,
        kind: db_label(&input.payload.kind())?,
        status: db_label(&input.status)?,
        agent_run_id: input.agent_run_id,
        provider_step_id: input.provider_step_id,
        tool_invocation_id: input.tool_invocation_id,
        provider_item_id: input.provider_item_id,
        payload_json: to_json(&input.payload)?,
        search_text: input.payload.search_text(),
        created_at: now,
        updated_at: now,
    };
    let item = diesel::insert_into(conversation_entries::table)
        .values(&row)
        .returning(SqlConversationEntryRow::as_returning())
        .get_result::<SqlConversationEntryRow>(conn)?;
    diesel::update(conversations::table.find(&row.conversation_id))
        .set((
            conversations::last_entry_seq.eq(seq),
            conversations::updated_at.eq(now),
        ))
        .execute(conn)?;
    item.try_into()
}

fn conversation_commit_with_conn<T>(
    conn: &mut SqliteConnection,
    conversation_id: ConversationId,
    value: T,
) -> Result<ConversationCommit<T>> {
    let conversation: ConversationRecord = conversation_row(conn, &conversation_id)?
        .ok_or_else(|| {
            DbError::Invariant(format!(
                "conversation {conversation_id} is missing after commit"
            ))
        })?
        .try_into()?;
    let index_delta = ConversationIndexDelta::EntryAdvanced {
        id: conversation.id.clone(),
        last_entry_seq: conversation.last_entry_seq,
        updated_at: conversation.updated_at,
    };
    Ok(ConversationCommit {
        value,
        conversation,
        index_delta,
    })
}

fn insert_attachments_into_message_item_with_conn(
    conn: &mut SqliteConnection,
    item: &mut NewConversationEntry,
    attachments: Vec<NewAttachment>,
) -> Result<Vec<AttachmentRecord>> {
    if attachments.is_empty() {
        return Ok(Vec::new());
    }
    let ConversationEntryPayload::Message { content, .. } = &mut item.payload else {
        return Err(DbError::Invariant(
            "attachments can only be added to message items".to_string(),
        ));
    };

    let mut records = Vec::with_capacity(attachments.len());
    for attachment in attachments {
        if attachment.conversation_id != item.conversation_id {
            return Err(DbError::Invariant(
                "attachment conversation does not match message conversation".to_string(),
            ));
        }
        let record = insert_attachment_with_conn(conn, attachment)?;
        content.push(content_part_for_attachment(&record));
        records.push(record);
    }
    Ok(records)
}

fn content_part_for_attachment(attachment: &AttachmentRecord) -> ContentPart {
    match attachment.kind {
        AttachmentKind::Image => ContentPart::Image {
            attachment_id: attachment.id.clone(),
        },
        AttachmentKind::File => ContentPart::File {
            attachment_id: attachment.id.clone(),
        },
        AttachmentKind::Audio | AttachmentKind::Attachment => ContentPart::Attachment {
            attachment_id: attachment.id.clone(),
        },
    }
}

fn insert_attachment_with_conn(
    conn: &mut SqliteConnection,
    input: NewAttachment,
) -> Result<AttachmentRecord> {
    let now = now_string()?;
    let row = SqlNewAttachmentRow {
        id: new_id(),
        conversation_id: input.conversation_id,
        kind: db_label(&input.kind)?,
        storage_kind: db_label(&input.storage_kind)?,
        mime_type: input.mime_type,
        name: input.name,
        path: input.path,
        external_uri: input.external_uri,
        provider_id: input.provider_id,
        provider_file_id: input.provider_file_id,
        sha256: input.sha256,
        size_bytes: input.size_bytes,
        metadata_json: to_json(&input.metadata)?,
        created_at: now,
        updated_at: now,
    };
    diesel::insert_into(attachments::table)
        .values(&row)
        .returning(SqlAttachmentRow::as_returning())
        .get_result::<SqlAttachmentRow>(conn)?
        .try_into()
}

fn validate_timeline_run_entries(
    runs: &[AgentRunRecord],
    entries: &[ConversationEntryRecord],
) -> Result<()> {
    let entries_by_id = entries
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect::<HashMap<_, _>>();

    for run in runs {
        if !is_terminal_agent_run_status(run.status) {
            continue;
        }
        let output = run.output.as_ref().ok_or_else(|| {
            DbError::Invariant(format!("terminal run {} has no final entry", run.id))
        })?;
        let final_entry = entries_by_id
            .get(output.final_entry_id.as_str())
            .ok_or_else(|| {
                DbError::Invariant(format!(
                    "final entry {} for run {} is missing",
                    output.final_entry_id, run.id
                ))
            })?;
        if final_entry.agent_run_id.as_deref() != Some(run.id.as_str()) {
            return Err(DbError::Invariant(format!(
                "final entry {} for run {} belongs to a different run",
                output.final_entry_id, run.id
            )));
        }
        if run.status == AgentRunStatus::Failed
            && !matches!(
                &final_entry.payload,
                ConversationEntryPayload::Error(error)
                    if run.error.as_ref() == Some(error)
            )
        {
            return Err(DbError::Invariant(format!(
                "failed run {} final entry does not match its error",
                run.id
            )));
        }
    }
    Ok(())
}

fn agent_message_request_usage_from_parts(
    run: &AgentRunRecord,
    final_entry: &ConversationEntryRecord,
    provider_step: &ProviderStepRecord,
    usage: Option<&UsageEventRecord>,
) -> Result<Option<AgentMessageRequestUsage>> {
    let output = run.output.as_ref().ok_or_else(|| {
        DbError::Invariant(format!(
            "agent run {} has no final entry for request usage",
            run.id
        ))
    })?;
    ensure_equal(
        "agent run final entry",
        &output.final_entry_id,
        &final_entry.id,
    )?;
    ensure_conversation_owner(
        "final conversation entry",
        &final_entry.id,
        &final_entry.conversation_id,
        &run.conversation_id,
    )?;
    ensure_agent_link(
        "final conversation entry",
        &final_entry.id,
        final_entry.agent_run_id.as_deref().ok_or_else(|| {
            DbError::Invariant(format!(
                "final conversation entry {} has no agent run",
                final_entry.id
            ))
        })?,
        Some(&run.id),
    )?;

    if !is_completed_assistant_message(final_entry) {
        return Ok(None);
    }

    let Some(final_provider_step_id) = final_entry.provider_step_id.as_deref() else {
        return Ok(None);
    };
    ensure_equal(
        "final conversation entry provider step",
        final_provider_step_id,
        &provider_step.id,
    )?;
    ensure_agent_link(
        "final provider step",
        &provider_step.id,
        &provider_step.agent_run_id,
        Some(&run.id),
    )?;
    ensure_equal(
        "provider step request provider",
        &provider_step.request_snapshot.provider_id,
        &provider_step.provider_id,
    )?;
    ensure_equal(
        "provider step request model",
        &provider_step.request_snapshot.model_id,
        &provider_step.model_id,
    )?;
    ensure_equal(
        "provider step settings provider",
        &provider_step.settings_snapshot.provider_id,
        &provider_step.provider_id,
    )?;
    ensure_equal(
        "provider step settings model",
        &provider_step.settings_snapshot.model_id,
        &provider_step.model_id,
    )?;

    if provider_step.status != ProviderStepStatus::Completed {
        return Ok(None);
    }
    let Some(completed_at) = provider_step.completed_at else {
        return Ok(None);
    };

    let usage = usage
        .map(|usage| -> Result<ProviderUsageSnapshot> {
            ensure_equal(
                "usage event provider step",
                &usage.provider_step_id,
                &provider_step.id,
            )?;
            ensure_equal(
                "usage event conversation",
                &usage.conversation_id,
                &run.conversation_id,
            )?;
            ensure_equal(
                "usage event provider",
                &usage.provider_id,
                &provider_step.provider_id,
            )?;
            ensure_equal(
                "usage event model",
                &usage.model_id,
                &provider_step.model_id,
            )?;
            Ok(usage.usage.clone())
        })
        .transpose()?;

    Ok(Some(AgentMessageRequestUsage {
        conversation_entry_id: final_entry.id.clone(),
        agent_run_id: run.id.clone(),
        provider_step_id: provider_step.id.clone(),
        provider_id: provider_step.provider_id.clone(),
        model_id: provider_step.model_id.clone(),
        provider_kind: provider_step
            .settings_snapshot
            .provider_settings
            .provider_kind
            .clone(),
        completed_at,
        usage,
    }))
}

fn agent_message_request_usage_for_final_entry_with_conn(
    conn: &mut SqliteConnection,
    run: &AgentRunRecord,
    final_entry: &ConversationEntryRecord,
) -> Result<Option<AgentMessageRequestUsage>> {
    if !is_completed_assistant_message(final_entry) {
        return Ok(None);
    }
    let Some(provider_step_id) = final_entry.provider_step_id.as_deref() else {
        return Ok(None);
    };
    let provider_step: ProviderStepRecord =
        load_provider_step_row(conn, provider_step_id)?.try_into()?;
    let usage = usage_event_row(conn, provider_step_id)?
        .map(TryInto::try_into)
        .transpose()?;
    agent_message_request_usage_from_parts(run, final_entry, &provider_step, usage.as_ref())
}

fn conversation_context_request_usage_from_parts(
    conversation_id: &str,
    run: &AgentRunRecord,
    provider_step: &ProviderStepRecord,
    usage: Option<&UsageEventRecord>,
) -> Result<ConversationContextRequestUsage> {
    ensure_conversation_owner("agent run", &run.id, &run.conversation_id, conversation_id)?;
    if run.status != AgentRunStatus::Completed {
        return Err(DbError::Invariant(format!(
            "agent run {} is not completed for context request usage",
            run.id
        )));
    }
    let agent_run_completed_at = run.completed_at.ok_or_else(|| {
        DbError::Invariant(format!(
            "completed agent run {} has no completion timestamp",
            run.id
        ))
    })?;
    ensure_agent_link(
        "context provider step",
        &provider_step.id,
        &provider_step.agent_run_id,
        Some(&run.id),
    )?;
    if provider_step.status != ProviderStepStatus::Completed {
        return Err(DbError::Invariant(format!(
            "provider step {} is not completed for context request usage",
            provider_step.id
        )));
    }
    let provider_step_completed_at = provider_step.completed_at.ok_or_else(|| {
        DbError::Invariant(format!(
            "completed provider step {} has no completion timestamp",
            provider_step.id
        ))
    })?;
    ensure_equal(
        "provider step request provider",
        &provider_step.request_snapshot.provider_id,
        &provider_step.provider_id,
    )?;
    ensure_equal(
        "provider step request model",
        &provider_step.request_snapshot.model_id,
        &provider_step.model_id,
    )?;
    ensure_equal(
        "provider step settings provider",
        &provider_step.settings_snapshot.provider_id,
        &provider_step.provider_id,
    )?;
    ensure_equal(
        "provider step settings model",
        &provider_step.settings_snapshot.model_id,
        &provider_step.model_id,
    )?;
    ensure_equal(
        "agent run input provider",
        &run.input.provider_id,
        &provider_step.provider_id,
    )?;
    ensure_equal(
        "agent run input model",
        &run.input.model_id,
        &provider_step.model_id,
    )?;
    ensure_equal(
        "agent run settings provider",
        &run.input.settings_snapshot.provider_id,
        &provider_step.provider_id,
    )?;
    ensure_equal(
        "agent run settings model",
        &run.input.settings_snapshot.model_id,
        &provider_step.model_id,
    )?;

    let usage = usage
        .map(|usage| -> Result<ProviderUsageSnapshot> {
            ensure_equal(
                "usage event provider step",
                &usage.provider_step_id,
                &provider_step.id,
            )?;
            ensure_equal(
                "usage event conversation",
                &usage.conversation_id,
                conversation_id,
            )?;
            ensure_equal(
                "usage event provider",
                &usage.provider_id,
                &provider_step.provider_id,
            )?;
            ensure_equal(
                "usage event model",
                &usage.model_id,
                &provider_step.model_id,
            )?;
            Ok(usage.usage.clone())
        })
        .transpose()?;

    Ok(ConversationContextRequestUsage {
        agent_run_id: run.id.clone(),
        provider_step_id: provider_step.id.clone(),
        provider_step_seq: provider_step.seq,
        provider_id: provider_step.provider_id.clone(),
        model_id: provider_step.model_id.clone(),
        provider_step_completed_at,
        agent_run_completed_at,
        usage,
    })
}

fn latest_conversation_context_request_usage_from_parts(
    conversation_id: &str,
    runs: &[AgentRunRecord],
    provider_steps: &[ProviderStepRecord],
    usage_events: &[UsageEventRecord],
) -> Result<Option<ConversationContextRequestUsage>> {
    let runs_by_id = runs
        .iter()
        .map(|run| (run.id.as_str(), run))
        .collect::<HashMap<_, _>>();
    let usage_events_by_step_id = usage_events
        .iter()
        .map(|usage| (usage.provider_step_id.as_str(), usage))
        .collect::<HashMap<_, _>>();

    let mut candidates = Vec::new();
    for step in provider_steps {
        let run = runs_by_id
            .get(step.agent_run_id.as_str())
            .copied()
            .ok_or_else(|| {
                DbError::Invariant(format!(
                    "provider step {} references missing agent run {}",
                    step.id, step.agent_run_id
                ))
            })?;
        ensure_conversation_owner("agent run", &run.id, &run.conversation_id, conversation_id)?;
        if run.status != AgentRunStatus::Completed || step.status != ProviderStepStatus::Completed {
            continue;
        }
        let provider_step_completed_at = step.completed_at.ok_or_else(|| {
            DbError::Invariant(format!(
                "completed provider step {} has no completion timestamp",
                step.id
            ))
        })?;
        let agent_run_completed_at = run.completed_at.ok_or_else(|| {
            DbError::Invariant(format!(
                "completed agent run {} has no completion timestamp",
                run.id
            ))
        })?;
        candidates.push((
            run,
            step,
            (
                provider_step_completed_at,
                agent_run_completed_at,
                step.seq,
                step.id.as_str(),
            ),
        ));
    }
    let candidate = candidates
        .into_iter()
        .max_by(|left, right| left.2.cmp(&right.2));

    candidate
        .map(|(run, step, _)| {
            conversation_context_request_usage_from_parts(
                conversation_id,
                run,
                step,
                usage_events_by_step_id.get(step.id.as_str()).copied(),
            )
        })
        .transpose()
}

fn latest_conversation_context_request_usage_with_conn(
    conn: &mut SqliteConnection,
    conversation_id: &str,
) -> Result<Option<ConversationContextRequestUsage>> {
    let runs = agent_runs::table
        .filter(agent_runs::conversation_id.eq(conversation_id))
        .select(SqlAgentRunRow::as_select())
        .load::<SqlAgentRunRow>(conn)?
        .into_iter()
        .map(TryInto::try_into)
        .collect::<Result<Vec<AgentRunRecord>>>()?;
    let provider_steps = provider_steps::table
        .inner_join(agent_runs::table.on(agent_runs::id.eq(provider_steps::agent_run_id)))
        .filter(agent_runs::conversation_id.eq(conversation_id))
        .select(SqlProviderStepRow::as_select())
        .load::<SqlProviderStepRow>(conn)?
        .into_iter()
        .map(TryInto::try_into)
        .collect::<Result<Vec<ProviderStepRecord>>>()?;
    let usage_events = usage_events_for_conversation_with_conn(conn, conversation_id)?;
    latest_conversation_context_request_usage_from_parts(
        conversation_id,
        &runs,
        &provider_steps,
        &usage_events,
    )
}

fn context_request_usage_delta_for_run_with_conn(
    conn: &mut SqliteConnection,
    run: &AgentRunRecord,
) -> Result<Option<ConversationContextRequestUsage>> {
    if run.status != AgentRunStatus::Completed {
        return Ok(None);
    }
    Ok(
        latest_conversation_context_request_usage_with_conn(conn, &run.conversation_id)?
            .filter(|request_usage| request_usage.agent_run_id == run.id),
    )
}

fn is_completed_assistant_message(entry: &ConversationEntryRecord) -> bool {
    entry.status == ConversationEntryStatus::Completed
        && matches!(
            &entry.payload,
            ConversationEntryPayload::Message {
                role: TranscriptRole::Assistant,
                ..
            }
        )
}

fn conversation_matches_query(
    conversation: &ConversationRecord,
    project: Option<&ProjectRecord>,
    item_search_text: Option<&String>,
    query: &str,
) -> bool {
    contains_query(&conversation.title, query)
        || project.is_some_and(|project| {
            contains_query(&project.display_name, query) || contains_query(&project.path, query)
        })
        || item_search_text.is_some_and(|text| contains_query(text, query))
}

fn contains_query(value: &str, query: &str) -> bool {
    value.to_lowercase().contains(query)
}

fn finish_agent_run_with_conn(
    conn: &mut SqliteConnection,
    id: &str,
    finish: FinishAgentRun,
) -> Result<FinishedAgentRun> {
    let existing = load_agent_run_row(conn, id)?;
    let existing_status: AgentRunStatus = db_label_parse(existing.status.clone())?;
    if is_terminal_agent_run_status(existing_status) {
        let run: AgentRunRecord = existing.try_into()?;
        let final_entry_id = run
            .output
            .as_ref()
            .map(|output| output.final_entry_id.as_str())
            .ok_or_else(|| DbError::Invariant(format!("terminal run {id} has no final entry")))?;
        let final_entry = conversation_entry_row(conn, final_entry_id)?
            .ok_or_else(|| DbError::Invariant(format!("final entry {final_entry_id} is missing")))?
            .try_into()?;
        let request_usage =
            agent_message_request_usage_for_final_entry_with_conn(conn, &run, &final_entry)?;
        let context_request_usage = context_request_usage_delta_for_run_with_conn(conn, &run)?;
        return Ok(FinishedAgentRun {
            run,
            final_entry,
            appended_final_entry: false,
            request_usage,
            context_request_usage,
        });
    }

    if !is_terminal_agent_run_status(finish.status) {
        return Err(DbError::Invariant(
            "finish_agent_run requires a terminal status".to_string(),
        ));
    }
    let expected_stopped_reason = match finish.status {
        AgentRunStatus::Completed => {
            if !matches!(
                finish.stopped_reason,
                AgentStoppedReason::Completed | AgentStoppedReason::MaxSteps
            ) {
                return Err(DbError::Invariant(
                    "completed run has an invalid stopped reason".to_string(),
                ));
            }
            finish.stopped_reason
        }
        AgentRunStatus::Failed => AgentStoppedReason::Failed,
        AgentRunStatus::Canceled => AgentStoppedReason::Canceled,
        AgentRunStatus::Running => unreachable!(),
    };
    if (finish.status == AgentRunStatus::Failed) != finish.error.is_some() {
        return Err(DbError::Invariant(
            "failed run must have an error and non-failed run must not have one".to_string(),
        ));
    }

    let conversation_id = existing.conversation_id.clone();
    let (final_entry, appended_final_entry) = match finish.final_entry {
        AgentRunFinalEntry::Existing(final_entry_id) => {
            let row = conversation_entry_row(conn, &final_entry_id)?.ok_or_else(|| {
                DbError::Invariant(format!(
                    "final conversation entry {final_entry_id} is missing"
                ))
            })?;
            let entry: ConversationEntryRecord = row.try_into()?;
            validate_final_entry(&entry, id, &conversation_id, finish.error.as_ref())?;
            (entry, false)
        }
        AgentRunFinalEntry::Append(entry) => {
            let mut entry = *entry;
            entry.conversation_id = conversation_id.clone();
            entry.agent_run_id = Some(id.to_string());
            if finish.status == AgentRunStatus::Failed
                && (entry.status != ConversationEntryStatus::Failed
                    || entry.payload
                        != ConversationEntryPayload::Error(finish.error.clone().unwrap()))
            {
                return Err(DbError::Invariant(
                    "failed run final entry must be a failed entry with the same error payload"
                        .to_string(),
                ));
            }
            (append_conversation_entry_with_conn(conn, entry)?, true)
        }
    };

    let now = now_string()?;
    let changes = SqlAgentRunFinalChanges {
        status: db_label(&finish.status)?,
        final_entry_id: final_entry.id.clone(),
        stopped_reason: db_label(&expected_stopped_reason)?,
        error_json: to_json_opt(&finish.error)?,
        started_at: next_started_at(existing.started_at, finish.status, now),
        completed_at: Some(now),
        updated_at: now,
    };
    let run = diesel::update(agent_runs::table.find(id))
        .set(&changes)
        .returning(SqlAgentRunRow::as_returning())
        .get_result::<SqlAgentRunRow>(conn)?
        .try_into()?;
    let request_usage =
        agent_message_request_usage_for_final_entry_with_conn(conn, &run, &final_entry)?;
    let context_request_usage = context_request_usage_delta_for_run_with_conn(conn, &run)?;
    Ok(FinishedAgentRun {
        run,
        final_entry,
        appended_final_entry,
        request_usage,
        context_request_usage,
    })
}

fn validate_final_entry(
    entry: &ConversationEntryRecord,
    agent_run_id: &str,
    conversation_id: &str,
    error: Option<&RunErrorPayload>,
) -> Result<()> {
    ensure_conversation_owner(
        "final conversation entry",
        &entry.id,
        &entry.conversation_id,
        conversation_id,
    )?;
    ensure_agent_link(
        "final conversation entry",
        &entry.id,
        entry.agent_run_id.as_deref().ok_or_else(|| {
            DbError::Invariant(format!(
                "final conversation entry {} has no agent run",
                entry.id
            ))
        })?,
        Some(agent_run_id),
    )?;
    if let Some(error) = error
        && (entry.status != ConversationEntryStatus::Failed
            || entry.payload != ConversationEntryPayload::Error(error.clone()))
    {
        return Err(DbError::Invariant(
            "failed run final entry must be a failed entry with the same error payload".to_string(),
        ));
    }
    Ok(())
}

fn update_provider_step_status_with_conn(
    conn: &mut SqliteConnection,
    id: &str,
    update: UpdateProviderStepStatus,
) -> Result<ProviderStepRecord> {
    let existing = load_provider_step_row(conn, id)?;
    if is_terminal_provider_step_status(db_label_parse(existing.status.clone())?) {
        return existing.try_into();
    }
    if let Some(state_snapshot) = update.state_snapshot.as_ref() {
        ensure_equal(
            "provider step state provider",
            &state_snapshot.provider_id,
            &existing.provider_id,
        )?;
    }
    let now = now_string()?;
    let changes = SqlProviderStepStatusChanges {
        status: db_label(&update.status)?,
        response_snapshot_json: to_json_opt(&update.response_snapshot)?,
        state_snapshot_json: to_json_opt(&update.state_snapshot)?,
        continuation_kind: None,
        provider_response_id: None,
        reasoning_context: None,
        continuation_expires_at: None,
        continuation_invalidated_at: None,
        continuation_error_json: None,
        error_json: to_json_opt(&update.error)?,
        started_at: next_started_at(existing.started_at, update.status, now),
        completed_at: next_provider_step_completed_at(existing.completed_at, update.status, now),
        updated_at: now,
    };
    diesel::update(provider_steps::table.find(id))
        .set(&changes)
        .returning(SqlProviderStepRow::as_returning())
        .get_result::<SqlProviderStepRow>(conn)?
        .try_into()
}

fn update_tool_invocation_status_with_conn(
    conn: &mut SqliteConnection,
    id: &str,
    update: UpdateToolInvocationStatus,
) -> Result<ToolInvocationRecord> {
    let existing = load_tool_invocation_row(conn, id)?;
    if is_terminal_tool_invocation_status(db_label_parse(existing.status.clone())?) {
        return existing.try_into();
    }
    let now = now_string()?;
    let changes = SqlToolInvocationStatusChanges {
        status: db_label(&update.status)?,
        output_json: to_json_opt(&update.output)?,
        error_json: to_json_opt(&update.error)?,
        started_at: next_started_at(existing.started_at, update.status, now),
        completed_at: next_tool_invocation_completed_at(existing.completed_at, update.status, now),
        updated_at: now,
    };
    diesel::update(tool_invocations::table.find(id))
        .set(&changes)
        .returning(SqlToolInvocationRow::as_returning())
        .get_result::<SqlToolInvocationRow>(conn)?
        .try_into()
}

fn update_tool_invocation_full_with_conn(
    conn: &mut SqliteConnection,
    id: &str,
    update: UpdateToolInvocationStatus,
    approval: Option<ToolInvocationApproval>,
) -> Result<ToolInvocationRecord> {
    let existing = load_tool_invocation_row(conn, id)?;
    if is_terminal_tool_invocation_status(db_label_parse(existing.status.clone())?) {
        return existing.try_into();
    }
    let now = now_string()?;
    let approval_json = match approval {
        Some(approval) => to_json_opt(&Some(approval))?,
        None => existing.approval_json.clone(),
    };
    let changes = SqlToolInvocationFullChanges {
        status: db_label(&update.status)?,
        output_json: to_json_opt(&update.output)?,
        error_json: to_json_opt(&update.error)?,
        approval_json,
        started_at: next_started_at(existing.started_at, update.status, now),
        completed_at: next_tool_invocation_completed_at(existing.completed_at, update.status, now),
        updated_at: now,
    };
    diesel::update(tool_invocations::table.find(id))
        .set(&changes)
        .returning(SqlToolInvocationRow::as_returning())
        .get_result::<SqlToolInvocationRow>(conn)?
        .try_into()
}

fn ensure_tool_invocation_not_terminal(conn: &mut SqliteConnection, id: &str) -> Result<()> {
    let existing = load_tool_invocation_row(conn, id)?;
    if is_terminal_tool_invocation_status(db_label_parse(existing.status.clone())?) {
        return Err(DbError::Invariant(format!(
            "tool invocation {id} is already terminal"
        )));
    }
    Ok(())
}

fn update_tool_invocation_approval_with_conn(
    conn: &mut SqliteConnection,
    id: &str,
    status: ToolInvocationStatus,
    approval: Option<ToolInvocationApproval>,
) -> Result<ToolInvocationRecord> {
    let existing = load_tool_invocation_row(conn, id)?;
    let now = now_string()?;
    let changes = SqlToolInvocationApprovalChanges {
        status: db_label(&status)?,
        approval_json: to_json_opt(&approval)?,
        started_at: next_started_at(existing.started_at, status, now),
        completed_at: next_tool_invocation_completed_at(existing.completed_at, status, now),
        updated_at: now,
    };
    diesel::update(tool_invocations::table.find(id))
        .set(&changes)
        .returning(SqlToolInvocationRow::as_returning())
        .get_result::<SqlToolInvocationRow>(conn)?
        .try_into()
}

fn validate_execution_links(
    conn: &mut SqliteConnection,
    conversation_id: &str,
    item: &NewConversationEntry,
) -> Result<()> {
    let mut expected_agent_run_id = match item.agent_run_id.as_deref() {
        Some(agent_run_id) => {
            let agent_run = load_agent_run_row(conn, agent_run_id)?;
            ensure_conversation_owner(
                "agent run",
                agent_run_id,
                &agent_run.conversation_id,
                conversation_id,
            )?;
            Some(agent_run.id)
        }
        None => None,
    };

    if let Some(provider_step_id) = item.provider_step_id.as_deref() {
        let provider_step = load_provider_step_row(conn, provider_step_id)?;
        let agent_run = load_agent_run_row(conn, &provider_step.agent_run_id)?;
        ensure_conversation_owner(
            "provider step",
            provider_step_id,
            &agent_run.conversation_id,
            conversation_id,
        )?;
        ensure_agent_link(
            "provider step",
            provider_step_id,
            &provider_step.agent_run_id,
            expected_agent_run_id.as_deref(),
        )?;
        expected_agent_run_id.get_or_insert(provider_step.agent_run_id);
    }

    if let Some(tool_invocation_id) = item.tool_invocation_id.as_deref() {
        let tool_invocation = load_tool_invocation_row(conn, tool_invocation_id)?;
        let agent_run = load_agent_run_row(conn, &tool_invocation.agent_run_id)?;
        ensure_conversation_owner(
            "tool invocation",
            tool_invocation_id,
            &agent_run.conversation_id,
            conversation_id,
        )?;
        ensure_agent_link(
            "tool invocation",
            tool_invocation_id,
            &tool_invocation.agent_run_id,
            expected_agent_run_id.as_deref(),
        )?;

        if let Some(tool_provider_step_id) = tool_invocation.provider_step_id.as_deref() {
            let provider_step = load_provider_step_row(conn, tool_provider_step_id)?;
            ensure_agent_link(
                "tool invocation provider step",
                tool_provider_step_id,
                &provider_step.agent_run_id,
                Some(&tool_invocation.agent_run_id),
            )?;
            let provider_step_agent_run = load_agent_run_row(conn, &provider_step.agent_run_id)?;
            ensure_conversation_owner(
                "tool invocation provider step",
                tool_provider_step_id,
                &provider_step_agent_run.conversation_id,
                conversation_id,
            )?;
        }

        if item.provider_step_id.as_deref() != tool_invocation.provider_step_id.as_deref()
            && item.provider_step_id.is_some()
        {
            return Err(DbError::Invariant(
                "tool invocation does not belong to the linked provider step".to_string(),
            ));
        }
    }

    Ok(())
}

fn validate_agent_run_trigger(
    conn: &mut SqliteConnection,
    conversation_id: &str,
    trigger_entry_id: &str,
) -> Result<()> {
    let item = conversation_entry_row(conn, trigger_entry_id)?.ok_or_else(|| {
        DbError::Invariant(format!("trigger entry {trigger_entry_id} is missing"))
    })?;
    let item: ConversationEntryRecord = item.try_into()?;
    ensure_conversation_owner(
        "trigger entry",
        trigger_entry_id,
        &item.conversation_id,
        conversation_id,
    )?;
    match item.payload {
        ConversationEntryPayload::Message {
            role: TranscriptRole::User,
            ..
        } => Ok(()),
        _ => Err(DbError::Invariant(format!(
            "trigger entry {trigger_entry_id} must be a user message"
        ))),
    }
}

fn validate_provider_step_snapshots(input: &NewProviderStep) -> Result<()> {
    ensure_equal(
        "provider step settings provider",
        &input.settings_snapshot.provider_id,
        &input.request_snapshot.provider_id,
    )?;
    ensure_equal(
        "provider step settings model",
        &input.settings_snapshot.model_id,
        &input.request_snapshot.model_id,
    )?;
    if let Some(state_snapshot) = input.state_snapshot.as_ref() {
        ensure_equal(
            "provider step state provider",
            &state_snapshot.provider_id,
            &input.request_snapshot.provider_id,
        )?;
    }
    Ok(())
}

fn validate_provider_step_input_items(
    conn: &mut SqliteConnection,
    conversation_id: &str,
    input: &NewProviderStep,
) -> Result<()> {
    for item_id in &input.request_snapshot.input_item_ids {
        let item = conversation_entry_row(conn, item_id)?.ok_or_else(|| {
            DbError::Invariant(format!("provider step input item {item_id} is missing"))
        })?;
        ensure_conversation_owner(
            "provider step input item",
            item_id,
            &item.conversation_id,
            conversation_id,
        )?;
    }
    Ok(())
}

fn apply_approval_outcome(
    approval: &mut ToolInvocationApproval,
    outcome: ToolInvocationApprovalOutcome,
    now: OffsetDateTime,
) {
    match outcome {
        ToolInvocationApprovalOutcome::Approved { decided_by, reason } => {
            approval.status = ApprovalStatus::Approved;
            approval.decision = Some(ApprovalDecisionPayload {
                approved: true,
                decided_by,
                reason,
            });
        }
        ToolInvocationApprovalOutcome::Denied { decided_by, reason } => {
            approval.status = ApprovalStatus::Denied;
            approval.decision = Some(ApprovalDecisionPayload {
                approved: false,
                decided_by,
                reason,
            });
        }
        ToolInvocationApprovalOutcome::Expired => {
            approval.status = ApprovalStatus::Expired;
            approval.decision = None;
        }
        ToolInvocationApprovalOutcome::Canceled => {
            approval.status = ApprovalStatus::Canceled;
            approval.decision = None;
        }
    }
    approval.decided_at = Some(now);
    approval.expires_at = None;
}

trait ExecutionStatusTiming {
    fn starts_clock(self) -> bool;
}

impl ExecutionStatusTiming for AgentRunStatus {
    fn starts_clock(self) -> bool {
        true
    }
}

impl ExecutionStatusTiming for ProviderStepStatus {
    fn starts_clock(self) -> bool {
        !matches!(self, ProviderStepStatus::Queued)
    }
}

impl ExecutionStatusTiming for ToolInvocationStatus {
    fn starts_clock(self) -> bool {
        !matches!(self, ToolInvocationStatus::Requested)
    }
}

fn next_started_at<T>(
    existing: Option<OffsetDateTime>,
    status: T,
    now: OffsetDateTime,
) -> Option<OffsetDateTime>
where
    T: ExecutionStatusTiming,
{
    existing.or_else(|| status.starts_clock().then_some(now))
}

fn next_provider_step_completed_at(
    existing: Option<OffsetDateTime>,
    status: ProviderStepStatus,
    now: OffsetDateTime,
) -> Option<OffsetDateTime> {
    existing.or_else(|| is_terminal_provider_step_status(status).then_some(now))
}

fn next_tool_invocation_completed_at(
    existing: Option<OffsetDateTime>,
    status: ToolInvocationStatus,
    now: OffsetDateTime,
) -> Option<OffsetDateTime> {
    existing.or_else(|| is_terminal_tool_invocation_status(status).then_some(now))
}

fn is_terminal_agent_run_status(status: AgentRunStatus) -> bool {
    matches!(
        status,
        AgentRunStatus::Completed | AgentRunStatus::Failed | AgentRunStatus::Canceled
    )
}

fn is_terminal_provider_step_status(status: ProviderStepStatus) -> bool {
    matches!(
        status,
        ProviderStepStatus::Completed | ProviderStepStatus::Failed | ProviderStepStatus::Canceled
    )
}

fn is_terminal_tool_invocation_status(status: ToolInvocationStatus) -> bool {
    matches!(
        status,
        ToolInvocationStatus::Succeeded
            | ToolInvocationStatus::Failed
            | ToolInvocationStatus::Denied
            | ToolInvocationStatus::Canceled
    )
}

fn ensure_equal(entity: &str, actual: &str, expected: &str) -> Result<()> {
    if actual == expected {
        return Ok(());
    }
    Err(DbError::Invariant(format!(
        "{entity} is {actual}, not {expected}"
    )))
}

fn ensure_conversation_owner(
    entity: &str,
    entity_id: &str,
    actual_conversation_id: &str,
    expected_conversation_id: &str,
) -> Result<()> {
    if actual_conversation_id == expected_conversation_id {
        return Ok(());
    }
    Err(DbError::Invariant(format!(
        "{entity} {entity_id} belongs to conversation {actual_conversation_id}, not {expected_conversation_id}"
    )))
}

fn ensure_agent_link(
    entity: &str,
    entity_id: &str,
    actual_agent_run_id: &str,
    expected_agent_run_id: Option<&str>,
) -> Result<()> {
    match expected_agent_run_id {
        Some(expected_agent_run_id) if actual_agent_run_id != expected_agent_run_id => {
            Err(DbError::Invariant(format!(
                "{entity} {entity_id} belongs to agent run {actual_agent_run_id}, not {expected_agent_run_id}"
            )))
        }
        _ => Ok(()),
    }
}

fn now_string() -> Result<OffsetDateTime> {
    Ok(OffsetDateTime::now_utc())
}

#[derive(diesel::QueryableByName)]
struct TextValueRow {
    #[diesel(sql_type = Text)]
    value: String,
}

#[cfg(test)]
pub(crate) fn schema_version() -> i32 {
    crate::migrations::SCHEMA_VERSION
}

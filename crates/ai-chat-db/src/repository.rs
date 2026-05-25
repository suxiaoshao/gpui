use crate::{
    DbPool, Result,
    error::DbError,
    models::*,
    records::*,
    schema::{
        agent_runs, app_settings, approval_decisions, attachments, conversation_items,
        conversations, projects, prompts, provider_models, provider_steps, providers, shortcuts,
        tool_invocations, usage_events,
    },
};
use ai_chat_core::*;
use diesel::{
    connection::SimpleConnection,
    prelude::*,
    r2d2::{ConnectionManager, PooledConnection},
    sql_query,
    sql_types::Text,
    upsert::excluded,
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

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

    pub fn insert_project(&self, input: NewProject) -> Result<ProjectRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let now = now_string()?;
            let row = SqlNewProjectRow {
                id: new_id(),
                path: input.path,
                display_name: input.display_name,
                kind: db_label(&input.kind)?,
                metadata_json: to_json(&input.metadata)?,
                created_at: now.clone(),
                updated_at: now,
                last_opened_at: None,
            };
            diesel::insert_into(projects::table)
                .values(&row)
                .returning(SqlProjectRow::as_returning())
                .get_result::<SqlProjectRow>(conn)?
                .try_into()
        })
    }

    pub fn get_project(&self, id: &str) -> Result<Option<ProjectRecord>> {
        let mut conn = self.conn()?;
        project_row(&mut conn, id)?
            .map(TryInto::try_into)
            .transpose()
    }

    pub fn insert_provider(&self, input: NewProvider) -> Result<ProviderRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let now = now_string()?;
            let row = SqlNewProviderRow {
                id: new_id(),
                kind: input.kind,
                display_name: input.display_name,
                enabled: input.enabled,
                settings_json: to_json(&input.settings)?,
                secret_refs_json: to_json(&input.secret_refs)?,
                created_at: now.clone(),
                updated_at: now,
            };
            diesel::insert_into(providers::table)
                .values(&row)
                .returning(SqlProviderRow::as_returning())
                .get_result::<SqlProviderRow>(conn)?
                .try_into()
        })
    }

    pub fn get_provider(&self, id: &str) -> Result<Option<ProviderRecord>> {
        let mut conn = self.conn()?;
        provider_row(&mut conn, id)?
            .map(TryInto::try_into)
            .transpose()
    }

    pub fn upsert_provider_model(&self, input: NewProviderModel) -> Result<ProviderModelRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let now = now_string()?;
            let row = SqlNewProviderModelRow {
                id: new_id(),
                provider_id: input.provider_id,
                model_id: input.model_id,
                display_name: input.display_name,
                capabilities_json: to_json(&input.capabilities)?,
                metadata_json: to_json(&input.metadata)?,
                fetched_at: now.clone(),
                created_at: now.clone(),
                updated_at: now,
            };
            diesel::insert_into(provider_models::table)
                .values(&row)
                .on_conflict((provider_models::provider_id, provider_models::model_id))
                .do_update()
                .set((
                    provider_models::display_name.eq(excluded(provider_models::display_name)),
                    provider_models::capabilities_json
                        .eq(excluded(provider_models::capabilities_json)),
                    provider_models::metadata_json.eq(excluded(provider_models::metadata_json)),
                    provider_models::fetched_at.eq(excluded(provider_models::fetched_at)),
                    provider_models::updated_at.eq(excluded(provider_models::updated_at)),
                ))
                .returning(SqlProviderModelRow::as_returning())
                .get_result::<SqlProviderModelRow>(conn)?
                .try_into()
        })
    }

    pub fn get_provider_model(
        &self,
        provider_id: &str,
        model_id: &str,
    ) -> Result<Option<ProviderModelRecord>> {
        let mut conn = self.conn()?;
        provider_model_row(&mut conn, provider_id, model_id)?
            .map(TryInto::try_into)
            .transpose()
    }

    pub fn insert_prompt(&self, input: NewPrompt) -> Result<PromptRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let now = now_string()?;
            let row = SqlNewPromptRow {
                id: new_id(),
                name: input.name,
                content_json: to_json(&input.content)?,
                enabled: input.enabled,
                sort_order: input.sort_order,
                created_at: now.clone(),
                updated_at: now,
            };
            diesel::insert_into(prompts::table)
                .values(&row)
                .returning(SqlPromptRow::as_returning())
                .get_result::<SqlPromptRow>(conn)?
                .try_into()
        })
    }

    pub fn get_prompt(&self, id: &str) -> Result<Option<PromptRecord>> {
        let mut conn = self.conn()?;
        prompt_row(&mut conn, id)?
            .map(TryInto::try_into)
            .transpose()
    }

    pub fn insert_conversation(&self, input: NewConversation) -> Result<ConversationRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let now = now_string()?;
            let row = SqlNewConversationRow {
                id: new_id(),
                project_id: input.project_id,
                title: input.title,
                status: db_label(&ConversationStatus::Active)?,
                prompt_id: input.prompt_id,
                default_provider_id: input.default_provider_id,
                default_model_id: input.default_model_id,
                last_item_seq: 0,
                metadata_json: to_json(&input.metadata)?,
                settings_snapshot_json: to_json(&input.settings_snapshot)?,
                created_at: now.clone(),
                updated_at: now,
                archived_at: None,
                deleted_at: None,
            };
            diesel::insert_into(conversations::table)
                .values(&row)
                .returning(SqlConversationRow::as_returning())
                .get_result::<SqlConversationRow>(conn)?
                .try_into()
        })
    }

    pub fn get_conversation(&self, id: &str) -> Result<Option<ConversationRecord>> {
        let mut conn = self.conn()?;
        conversation_row(&mut conn, id)?
            .map(TryInto::try_into)
            .transpose()
    }

    pub fn append_conversation_item(
        &self,
        input: NewConversationItem,
    ) -> Result<ConversationItemRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let conversation = conversation_row(conn, &input.conversation_id)?
                .ok_or_else(|| DbError::Invariant("conversation is missing".to_string()))?;
            validate_execution_links(conn, &input.conversation_id, &input)?;

            let seq = conversation.last_item_seq + 1;
            let now = now_string()?;
            let row = SqlNewConversationItemRow {
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
                created_at: now.clone(),
                updated_at: now.clone(),
            };
            let item = diesel::insert_into(conversation_items::table)
                .values(&row)
                .returning(SqlConversationItemRow::as_returning())
                .get_result::<SqlConversationItemRow>(conn)?;
            diesel::update(conversations::table.find(&row.conversation_id))
                .set((
                    conversations::last_item_seq.eq(seq),
                    conversations::updated_at.eq(now),
                ))
                .execute(conn)?;
            item.try_into()
        })
    }

    pub fn conversation_items(&self, conversation_id: &str) -> Result<Vec<ConversationItemRecord>> {
        let mut conn = self.conn()?;
        conversation_items::table
            .filter(conversation_items::conversation_id.eq(conversation_id))
            .order(conversation_items::seq.asc())
            .select(SqlConversationItemRow::as_select())
            .load::<SqlConversationItemRow>(&mut conn)?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }

    pub fn update_conversation_item_payload(
        &self,
        item_id: &str,
        status: ConversationItemStatus,
        payload: ConversationItemPayload,
    ) -> Result<ConversationItemRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let now = now_string()?;
            let changes = SqlConversationItemPayloadChanges {
                kind: db_label(&payload.kind())?,
                status: db_label(&status)?,
                payload_json: to_json(&payload)?,
                search_text: payload.search_text(),
                updated_at: now.clone(),
            };
            let item = diesel::update(conversation_items::table.find(item_id))
                .set(&changes)
                .returning(SqlConversationItemRow::as_returning())
                .get_result::<SqlConversationItemRow>(conn)?;
            diesel::update(conversations::table.find(&item.conversation_id))
                .set(conversations::updated_at.eq(now))
                .execute(conn)?;
            item.try_into()
        })
    }

    pub fn insert_attachment(&self, input: NewAttachment) -> Result<AttachmentRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
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
                created_at: now.clone(),
                updated_at: now,
            };
            diesel::insert_into(attachments::table)
                .values(&row)
                .returning(SqlAttachmentRow::as_returning())
                .get_result::<SqlAttachmentRow>(conn)?
                .try_into()
        })
    }

    pub fn insert_agent_run(&self, input: NewAgentRun) -> Result<AgentRunRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let conversation_id =
                derive_agent_run_conversation_id(conn, &input.input.user_item_id)?;
            let now = now_string()?;
            let row = SqlNewAgentRunRow {
                id: new_id(),
                conversation_id,
                trigger_kind: db_label(&input.trigger_kind)?,
                status: db_label(&input.status)?,
                input_json: to_json(&input.input)?,
                output_json: None,
                error_json: None,
                created_at: now.clone(),
                started_at: None,
                completed_at: None,
                updated_at: now,
            };
            diesel::insert_into(agent_runs::table)
                .values(&row)
                .returning(SqlAgentRunRow::as_returning())
                .get_result::<SqlAgentRunRow>(conn)?
                .try_into()
        })
    }

    pub fn insert_provider_step(&self, input: NewProviderStep) -> Result<ProviderStepRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            validate_provider_step_snapshots(&input)?;
            let now = now_string()?;
            let provider_id = input.request_snapshot.provider_id.clone();
            let model_id = input.request_snapshot.model_id.clone();
            let row = SqlNewProviderStepRow {
                id: new_id(),
                agent_run_id: input.agent_run_id,
                seq: input.seq,
                provider_id,
                model_id,
                status: db_label(&input.status)?,
                request_snapshot_json: to_json(&input.request_snapshot)?,
                response_snapshot_json: to_json_opt(&input.response_snapshot)?,
                state_snapshot_json: to_json_opt(&input.state_snapshot)?,
                settings_snapshot_json: to_json(&input.settings_snapshot)?,
                error_json: to_json_opt(&input.error)?,
                created_at: now.clone(),
                started_at: None,
                completed_at: None,
                updated_at: now,
            };
            diesel::insert_into(provider_steps::table)
                .values(&row)
                .returning(SqlProviderStepRow::as_returning())
                .get_result::<SqlProviderStepRow>(conn)?
                .try_into()
        })
    }

    pub fn insert_tool_invocation(&self, input: NewToolInvocation) -> Result<ToolInvocationRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            if let Some(provider_step_id) = input.provider_step_id.as_deref() {
                let provider_step = load_provider_step_row(conn, provider_step_id)?;
                ensure_agent_link(
                    "tool invocation provider step",
                    provider_step_id,
                    &provider_step.agent_run_id,
                    Some(&input.agent_run_id),
                )?;
            }
            let now = now_string()?;
            let row = SqlNewToolInvocationRow {
                id: new_id(),
                agent_run_id: input.agent_run_id,
                provider_step_id: input.provider_step_id,
                call_id: input.input.call_id.clone(),
                source: tool_source_label(&input.input.source),
                namespace: input.input.namespace.clone(),
                server_id: tool_source_server_id(&input.input.source),
                tool_name: input.input.tool_name.clone(),
                runtime_tool_name: input.input.runtime_tool_name.clone(),
                status: db_label(&input.status)?,
                input_json: to_json(&input.input)?,
                output_json: to_json_opt(&input.output)?,
                error_json: to_json_opt(&input.error)?,
                created_at: now.clone(),
                started_at: None,
                completed_at: None,
                updated_at: now,
            };
            diesel::insert_into(tool_invocations::table)
                .values(&row)
                .returning(SqlToolInvocationRow::as_returning())
                .get_result::<SqlToolInvocationRow>(conn)?
                .try_into()
        })
    }

    pub fn insert_approval_decision(
        &self,
        input: NewApprovalDecision,
    ) -> Result<ApprovalDecisionRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let now = now_string()?;
            let outcome = approval_outcome_columns(input.outcome, &now)?;
            let row = SqlNewApprovalDecisionRow {
                id: new_id(),
                tool_invocation_id: input.tool_invocation_id,
                status: db_label(&outcome.status)?,
                request_json: to_json(&input.request)?,
                decision_json: to_json_opt(&outcome.decision)?,
                requested_at: now.clone(),
                decided_at: outcome.decided_at,
                expires_at: outcome.expires_at,
            };
            diesel::insert_into(approval_decisions::table)
                .values(&row)
                .returning(SqlApprovalDecisionRow::as_returning())
                .get_result::<SqlApprovalDecisionRow>(conn)?
                .try_into()
        })
    }

    pub fn insert_usage_event(&self, input: NewUsageEvent) -> Result<UsageEventRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let provider_step = load_provider_step_row(conn, &input.provider_step_id)?;
            let agent_run = load_agent_run_row(conn, &provider_step.agent_run_id)?;
            let now = now_string()?;
            let row = SqlNewUsageEventRow {
                id: new_id(),
                provider_step_id: provider_step.id,
                conversation_id: agent_run.conversation_id,
                provider_id: provider_step.provider_id,
                model_id: provider_step.model_id,
                date_key: input.date_key,
                input_tokens: u64_to_i64(input.usage.input_tokens)?,
                output_tokens: u64_to_i64(input.usage.output_tokens)?,
                cached_input_tokens: u64_to_i64(input.usage.cached_input_tokens)?,
                cache_write_input_tokens: u64_to_i64(input.usage.cache_write_input_tokens)?,
                reasoning_tokens: u64_to_i64(input.usage.reasoning_tokens)?,
                total_tokens: u64_to_i64(input.usage.total_tokens)?,
                usage_json: to_json(&input.usage)?,
                created_at: now,
            };
            diesel::insert_into(usage_events::table)
                .values(&row)
                .returning(SqlUsageEventRow::as_returning())
                .get_result::<SqlUsageEventRow>(conn)?
                .try_into()
        })
    }

    pub fn insert_shortcut(&self, input: NewShortcut) -> Result<ShortcutRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let now = now_string()?;
            let row = SqlNewShortcutRow {
                id: new_id(),
                hotkey: input.hotkey,
                enabled: input.enabled,
                prompt_id: input.prompt_id,
                provider_id: input.provider_id,
                model_id: input.model_id,
                input_source: db_label(&input.input_source)?,
                action_json: to_json(&input.action)?,
                settings_snapshot_json: to_json(&input.settings_snapshot)?,
                created_at: now.clone(),
                updated_at: now,
            };
            diesel::insert_into(shortcuts::table)
                .values(&row)
                .returning(SqlShortcutRow::as_returning())
                .get_result::<SqlShortcutRow>(conn)?
                .try_into()
        })
    }

    pub fn set_app_settings(&self, settings: AppSettingsPayload) -> Result<AppSettingsRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let now = now_string()?;
            let row = SqlNewAppSettingsRow {
                id: "default".to_string(),
                settings_json: to_json(&settings)?,
                created_at: now.clone(),
                updated_at: now,
            };
            diesel::insert_into(app_settings::table)
                .values(&row)
                .on_conflict(app_settings::id)
                .do_update()
                .set((
                    app_settings::settings_json.eq(excluded(app_settings::settings_json)),
                    app_settings::updated_at.eq(excluded(app_settings::updated_at)),
                ))
                .returning(SqlAppSettingsRow::as_returning())
                .get_result::<SqlAppSettingsRow>(conn)?
                .try_into()
        })
    }

    pub fn get_app_settings(&self) -> Result<Option<AppSettingsRecord>> {
        let mut conn = self.conn()?;
        app_settings_row(&mut conn)?
            .map(TryInto::try_into)
            .transpose()
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

fn conversation_row(conn: &mut SqliteConnection, id: &str) -> Result<Option<SqlConversationRow>> {
    Ok(conversations::table
        .find(id)
        .select(SqlConversationRow::as_select())
        .first(conn)
        .optional()?)
}

fn conversation_item_row(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<Option<SqlConversationItemRow>> {
    Ok(conversation_items::table
        .find(id)
        .select(SqlConversationItemRow::as_select())
        .first(conn)
        .optional()?)
}

fn app_settings_row(conn: &mut SqliteConnection) -> Result<Option<SqlAppSettingsRow>> {
    Ok(app_settings::table
        .find("default")
        .select(SqlAppSettingsRow::as_select())
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

fn validate_execution_links(
    conn: &mut SqliteConnection,
    conversation_id: &str,
    item: &NewConversationItem,
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

fn derive_agent_run_conversation_id(
    conn: &mut SqliteConnection,
    user_item_id: &str,
) -> Result<ConversationId> {
    let item = conversation_item_row(conn, user_item_id)?
        .ok_or_else(|| DbError::Invariant(format!("user item {user_item_id} is missing")))?;
    let item: ConversationItemRecord = item.try_into()?;
    match item.payload {
        ConversationItemPayload::Message {
            role: TranscriptRole::User,
            ..
        } => Ok(item.conversation_id),
        _ => Err(DbError::Invariant(format!(
            "user item {user_item_id} must be a user message"
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

fn approval_outcome_columns(
    outcome: NewApprovalDecisionOutcome,
    now: &str,
) -> Result<ApprovalOutcomeColumns> {
    Ok(match outcome {
        NewApprovalDecisionOutcome::Pending { expires_at } => ApprovalOutcomeColumns {
            status: ApprovalStatus::Pending,
            decision: None,
            decided_at: None,
            expires_at: format_time_opt(expires_at.as_ref())?,
        },
        NewApprovalDecisionOutcome::Approved { decided_by, reason } => ApprovalOutcomeColumns {
            status: ApprovalStatus::Approved,
            decision: Some(ApprovalDecisionPayload {
                approved: true,
                decided_by,
                reason,
            }),
            decided_at: Some(now.to_string()),
            expires_at: None,
        },
        NewApprovalDecisionOutcome::Denied { decided_by, reason } => ApprovalOutcomeColumns {
            status: ApprovalStatus::Denied,
            decision: Some(ApprovalDecisionPayload {
                approved: false,
                decided_by,
                reason,
            }),
            decided_at: Some(now.to_string()),
            expires_at: None,
        },
        NewApprovalDecisionOutcome::Expired => ApprovalOutcomeColumns {
            status: ApprovalStatus::Expired,
            decision: None,
            decided_at: Some(now.to_string()),
            expires_at: None,
        },
        NewApprovalDecisionOutcome::Canceled => ApprovalOutcomeColumns {
            status: ApprovalStatus::Canceled,
            decision: None,
            decided_at: Some(now.to_string()),
            expires_at: None,
        },
    })
}

struct ApprovalOutcomeColumns {
    status: ApprovalStatus,
    decision: Option<ApprovalDecisionPayload>,
    decided_at: Option<String>,
    expires_at: Option<String>,
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

fn now_string() -> Result<String> {
    Ok(OffsetDateTime::now_utc().format(&Rfc3339)?)
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

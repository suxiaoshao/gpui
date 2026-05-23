use crate::{DbPool, Result, error::DbError, records::*};
use ai_chat_core::*;
use diesel::{
    RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    r2d2::{ConnectionManager, PooledConnection},
    sql_query,
    sql_types::{BigInt, Bool, Integer, Json, Nullable, Text},
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
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
        let row = sql_query(
            "SELECT schema_version, created_app_version, last_opened_app_version, payload_json, created_at, updated_at
             FROM schema_metadata WHERE id = 'default'",
        )
        .load::<SqlSchemaMetadataRow>(&mut conn)?
        .into_iter()
        .next()
        .ok_or_else(|| DbError::Invariant("schema metadata row is missing".to_string()))?;
        row.try_into()
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
        let id = new_id();
        let now = now_string()?;
        sql_query(
            "INSERT INTO projects (id, path, display_name, kind, metadata_json, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(&id)
        .bind::<Text, _>(&input.path)
        .bind::<Text, _>(&input.display_name)
        .bind::<Text, _>(&db_label(&input.kind)?)
        .bind::<Json, _>(&to_json(&input.metadata)?)
        .bind::<Text, _>(&now)
        .bind::<Text, _>(&now)
        .execute(&mut conn)?;
        self.get_project(&id)?
            .ok_or_else(|| DbError::Invariant("inserted project is missing".to_string()))
    }

    pub fn get_project(&self, id: &str) -> Result<Option<ProjectRecord>> {
        let mut conn = self.conn()?;
        let rows = sql_query("SELECT * FROM projects WHERE id = ?")
            .bind::<Text, _>(id)
            .load::<SqlProjectRow>(&mut conn)?;
        rows.into_iter().map(TryInto::try_into).next().transpose()
    }

    pub fn insert_provider(&self, input: NewProvider) -> Result<ProviderRecord> {
        let mut conn = self.conn()?;
        let id = new_id();
        let now = now_string()?;
        sql_query(
            "INSERT INTO providers (id, kind, display_name, enabled, settings_json, secret_refs_json, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(&id)
        .bind::<Text, _>(&input.kind)
        .bind::<Text, _>(&input.display_name)
        .bind::<Bool, _>(input.enabled)
        .bind::<Json, _>(&to_json(&input.settings)?)
        .bind::<Json, _>(&to_json(&input.secret_refs)?)
        .bind::<Text, _>(&now)
        .bind::<Text, _>(&now)
        .execute(&mut conn)?;
        self.get_provider(&id)?
            .ok_or_else(|| DbError::Invariant("inserted provider is missing".to_string()))
    }

    pub fn get_provider(&self, id: &str) -> Result<Option<ProviderRecord>> {
        let mut conn = self.conn()?;
        let rows = sql_query("SELECT * FROM providers WHERE id = ?")
            .bind::<Text, _>(id)
            .load::<SqlProviderRow>(&mut conn)?;
        rows.into_iter().map(TryInto::try_into).next().transpose()
    }

    pub fn upsert_provider_model(&self, input: NewProviderModel) -> Result<ProviderModelRecord> {
        let mut conn = self.conn()?;
        let id = new_id();
        let now = now_string()?;
        sql_query(
            "INSERT INTO provider_models (
                id, provider_id, model_id, display_name, capabilities_json, metadata_json, fetched_at, created_at, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(provider_id, model_id) DO UPDATE SET
                display_name = excluded.display_name,
                capabilities_json = excluded.capabilities_json,
                metadata_json = excluded.metadata_json,
                fetched_at = excluded.fetched_at,
                updated_at = excluded.updated_at",
        )
        .bind::<Text, _>(&id)
        .bind::<Text, _>(&input.provider_id)
        .bind::<Text, _>(&input.model_id)
        .bind::<Nullable<Text>, _>(&input.display_name)
        .bind::<Json, _>(&to_json(&input.capabilities)?)
        .bind::<Json, _>(&to_json(&input.metadata)?)
        .bind::<Text, _>(&now)
        .bind::<Text, _>(&now)
        .bind::<Text, _>(&now)
        .execute(&mut conn)?;
        self.get_provider_model(&input.provider_id, &input.model_id)?
            .ok_or_else(|| DbError::Invariant("upserted provider model is missing".to_string()))
    }

    pub fn get_provider_model(
        &self,
        provider_id: &str,
        model_id: &str,
    ) -> Result<Option<ProviderModelRecord>> {
        let mut conn = self.conn()?;
        let rows =
            sql_query("SELECT * FROM provider_models WHERE provider_id = ? AND model_id = ?")
                .bind::<Text, _>(provider_id)
                .bind::<Text, _>(model_id)
                .load::<SqlProviderModelRow>(&mut conn)?;
        rows.into_iter().map(TryInto::try_into).next().transpose()
    }

    pub fn insert_prompt(&self, input: NewPrompt) -> Result<PromptRecord> {
        let mut conn = self.conn()?;
        let id = new_id();
        let now = now_string()?;
        sql_query(
            "INSERT INTO prompts (id, name, content_json, enabled, sort_order, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(&id)
        .bind::<Text, _>(&input.name)
        .bind::<Json, _>(&to_json(&input.content)?)
        .bind::<Bool, _>(input.enabled)
        .bind::<Integer, _>(input.sort_order)
        .bind::<Text, _>(&now)
        .bind::<Text, _>(&now)
        .execute(&mut conn)?;
        self.get_prompt(&id)?
            .ok_or_else(|| DbError::Invariant("inserted prompt is missing".to_string()))
    }

    pub fn get_prompt(&self, id: &str) -> Result<Option<PromptRecord>> {
        let mut conn = self.conn()?;
        let rows = sql_query("SELECT * FROM prompts WHERE id = ?")
            .bind::<Text, _>(id)
            .load::<SqlPromptRow>(&mut conn)?;
        rows.into_iter().map(TryInto::try_into).next().transpose()
    }

    pub fn insert_conversation(&self, input: NewConversation) -> Result<ConversationRecord> {
        let mut conn = self.conn()?;
        let id = new_id();
        let now = now_string()?;
        sql_query(
            "INSERT INTO conversations (
                id, project_id, title, status, prompt_id, default_provider_id, default_model_id,
                metadata_json, settings_snapshot_json, created_at, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(&id)
        .bind::<Text, _>(&input.project_id)
        .bind::<Text, _>(&input.title)
        .bind::<Text, _>(&db_label(&ConversationStatus::Active)?)
        .bind::<Nullable<Text>, _>(&input.prompt_id)
        .bind::<Nullable<Text>, _>(&input.default_provider_id)
        .bind::<Nullable<Text>, _>(&input.default_model_id)
        .bind::<Json, _>(&to_json(&input.metadata)?)
        .bind::<Json, _>(&to_json(&input.settings_snapshot)?)
        .bind::<Text, _>(&now)
        .bind::<Text, _>(&now)
        .execute(&mut conn)?;
        self.get_conversation(&id)?
            .ok_or_else(|| DbError::Invariant("inserted conversation is missing".to_string()))
    }

    pub fn get_conversation(&self, id: &str) -> Result<Option<ConversationRecord>> {
        let mut conn = self.conn()?;
        let rows = sql_query("SELECT * FROM conversations WHERE id = ?")
            .bind::<Text, _>(id)
            .load::<SqlConversationRow>(&mut conn)?;
        rows.into_iter().map(TryInto::try_into).next().transpose()
    }

    pub fn append_conversation_item(
        &self,
        input: NewConversationItem,
    ) -> Result<ConversationItemRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let current_seq = sql_query("SELECT last_item_seq AS value FROM conversations WHERE id = ?")
                .bind::<Text, _>(&input.conversation_id)
                .load::<IntValueRow>(conn)?
                .into_iter()
                .next()
                .ok_or_else(|| DbError::Invariant("conversation is missing".to_string()))?
                .value;
            let seq = current_seq + 1;
            let id = new_id();
            let now = now_string()?;
            let payload_json = to_json(&input.payload)?;
            let search_text = input.payload.search_text();
            sql_query(
                "INSERT INTO conversation_items (
                    id, conversation_id, seq, kind, status, agent_run_id, provider_step_id,
                    tool_invocation_id, provider_item_id, payload_json, search_text, created_at, updated_at
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind::<Text, _>(&id)
            .bind::<Text, _>(&input.conversation_id)
            .bind::<Integer, _>(seq)
            .bind::<Text, _>(&db_label(&input.payload.kind())?)
            .bind::<Text, _>(&db_label(&input.status)?)
            .bind::<Nullable<Text>, _>(&input.agent_run_id)
            .bind::<Nullable<Text>, _>(&input.provider_step_id)
            .bind::<Nullable<Text>, _>(&input.tool_invocation_id)
            .bind::<Nullable<Text>, _>(&input.provider_item_id)
            .bind::<Json, _>(&payload_json)
            .bind::<Text, _>(&search_text)
            .bind::<Text, _>(&now)
            .bind::<Text, _>(&now)
            .execute(conn)?;
            sql_query("UPDATE conversations SET last_item_seq = ?, updated_at = ? WHERE id = ?")
                .bind::<Integer, _>(seq)
                .bind::<Text, _>(&now)
                .bind::<Text, _>(&input.conversation_id)
                .execute(conn)?;
            insert_fts(conn, &id, &input.conversation_id, &search_text)?;
            load_conversation_item(conn, &id)
        })
    }

    pub fn conversation_items(&self, conversation_id: &str) -> Result<Vec<ConversationItemRecord>> {
        let mut conn = self.conn()?;
        let rows =
            sql_query("SELECT * FROM conversation_items WHERE conversation_id = ? ORDER BY seq")
                .bind::<Text, _>(conversation_id)
                .load::<SqlConversationItemRow>(&mut conn)?;
        rows.into_iter().map(TryInto::try_into).collect()
    }

    pub fn search_conversation_items(&self, query: &str) -> Result<Vec<ConversationItemRecord>> {
        let mut conn = self.conn()?;
        let rows = sql_query(
            "SELECT conversation_items.*
             FROM conversation_item_fts
             JOIN conversation_items ON conversation_items.id = conversation_item_fts.item_id
             WHERE conversation_item_fts.content MATCH ?
             ORDER BY conversation_items.seq",
        )
        .bind::<Text, _>(query)
        .load::<SqlConversationItemRow>(&mut conn)?;
        rows.into_iter().map(TryInto::try_into).collect()
    }

    pub fn update_conversation_item_payload(
        &self,
        item_id: &str,
        status: ConversationItemStatus,
        payload: ConversationItemPayload,
    ) -> Result<ConversationItemRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let existing = load_conversation_item(conn, item_id)?;
            let now = now_string()?;
            let search_text = payload.search_text();
            sql_query(
                "UPDATE conversation_items
                 SET kind = ?, status = ?, payload_json = ?, search_text = ?, updated_at = ?
                 WHERE id = ?",
            )
            .bind::<Text, _>(&db_label(&payload.kind())?)
            .bind::<Text, _>(&db_label(&status)?)
            .bind::<Json, _>(&to_json(&payload)?)
            .bind::<Text, _>(&search_text)
            .bind::<Text, _>(&now)
            .bind::<Text, _>(item_id)
            .execute(conn)?;
            delete_fts(conn, item_id)?;
            insert_fts(conn, item_id, &existing.conversation_id, &search_text)?;
            load_conversation_item(conn, item_id)
        })
    }

    pub fn delete_conversation_item(&self, item_id: &str) -> Result<usize> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            delete_fts(conn, item_id)?;
            let deleted = sql_query("DELETE FROM conversation_items WHERE id = ?")
                .bind::<Text, _>(item_id)
                .execute(conn)?;
            Ok::<_, DbError>(deleted)
        })
    }

    pub fn insert_attachment(&self, input: NewAttachment) -> Result<AttachmentRecord> {
        let mut conn = self.conn()?;
        let id = new_id();
        let now = now_string()?;
        sql_query(
            "INSERT INTO attachments (
                id, conversation_id, kind, storage_kind, mime_type, name, path, external_uri,
                provider_id, provider_file_id, sha256, size_bytes, metadata_json, created_at, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(&id)
        .bind::<Text, _>(&input.conversation_id)
        .bind::<Text, _>(&db_label(&input.kind)?)
        .bind::<Text, _>(&db_label(&input.storage_kind)?)
        .bind::<Nullable<Text>, _>(&input.mime_type)
        .bind::<Nullable<Text>, _>(&input.name)
        .bind::<Nullable<Text>, _>(&input.path)
        .bind::<Nullable<Text>, _>(&input.external_uri)
        .bind::<Nullable<Text>, _>(&input.provider_id)
        .bind::<Nullable<Text>, _>(&input.provider_file_id)
        .bind::<Nullable<Text>, _>(&input.sha256)
        .bind::<Nullable<BigInt>, _>(&input.size_bytes)
        .bind::<Json, _>(&to_json(&input.metadata)?)
        .bind::<Text, _>(&now)
        .bind::<Text, _>(&now)
        .execute(&mut conn)?;
        load_attachment(&mut conn, &id)
    }

    pub fn insert_agent_run(&self, input: NewAgentRun) -> Result<AgentRunRecord> {
        let mut conn = self.conn()?;
        let id = new_id();
        let now = now_string()?;
        sql_query(
            "INSERT INTO agent_runs (
                id, conversation_id, trigger_kind, status, input_json, created_at, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(&id)
        .bind::<Text, _>(&input.conversation_id)
        .bind::<Text, _>(&db_label(&input.trigger_kind)?)
        .bind::<Text, _>(&db_label(&input.status)?)
        .bind::<Json, _>(&to_json(&input.input)?)
        .bind::<Text, _>(&now)
        .bind::<Text, _>(&now)
        .execute(&mut conn)?;
        load_agent_run(&mut conn, &id)
    }

    pub fn insert_provider_step(&self, input: NewProviderStep) -> Result<ProviderStepRecord> {
        let mut conn = self.conn()?;
        let id = new_id();
        let now = now_string()?;
        sql_query(
            "INSERT INTO provider_steps (
                id, agent_run_id, seq, provider_id, model_id, status, request_snapshot_json,
                response_snapshot_json, state_snapshot_json, settings_snapshot_json, error_json,
                created_at, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(&id)
        .bind::<Text, _>(&input.agent_run_id)
        .bind::<Integer, _>(input.seq)
        .bind::<Text, _>(&input.provider_id)
        .bind::<Text, _>(&input.model_id)
        .bind::<Text, _>(&db_label(&input.status)?)
        .bind::<Json, _>(&to_json(&input.request_snapshot)?)
        .bind::<Nullable<Json>, _>(&to_json_opt(&input.response_snapshot)?)
        .bind::<Nullable<Json>, _>(&to_json_opt(&input.state_snapshot)?)
        .bind::<Json, _>(&to_json(&input.settings_snapshot)?)
        .bind::<Nullable<Json>, _>(&to_json_opt(&input.error)?)
        .bind::<Text, _>(&now)
        .bind::<Text, _>(&now)
        .execute(&mut conn)?;
        load_provider_step(&mut conn, &id)
    }

    pub fn insert_tool_invocation(&self, input: NewToolInvocation) -> Result<ToolInvocationRecord> {
        let mut conn = self.conn()?;
        let id = new_id();
        let now = now_string()?;
        sql_query(
            "INSERT INTO tool_invocations (
                id, agent_run_id, provider_step_id, call_id, source, namespace, server_id,
                tool_name, runtime_tool_name, status, input_json, output_json, error_json,
                created_at, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(&id)
        .bind::<Text, _>(&input.agent_run_id)
        .bind::<Nullable<Text>, _>(&input.provider_step_id)
        .bind::<Text, _>(&input.input.call_id)
        .bind::<Text, _>(&tool_source_label(&input.input.source))
        .bind::<Nullable<Text>, _>(&input.input.namespace)
        .bind::<Nullable<Text>, _>(&tool_source_server_id(&input.input.source))
        .bind::<Text, _>(&input.input.tool_name)
        .bind::<Text, _>(&input.input.runtime_tool_name)
        .bind::<Text, _>(&db_label(&input.status)?)
        .bind::<Json, _>(&to_json(&input.input)?)
        .bind::<Nullable<Json>, _>(&to_json_opt(&input.output)?)
        .bind::<Nullable<Json>, _>(&to_json_opt(&input.error)?)
        .bind::<Text, _>(&now)
        .bind::<Text, _>(&now)
        .execute(&mut conn)?;
        load_tool_invocation(&mut conn, &id)
    }

    pub fn insert_approval_decision(
        &self,
        input: NewApprovalDecision,
    ) -> Result<ApprovalDecisionRecord> {
        let mut conn = self.conn()?;
        let id = new_id();
        let now = now_string()?;
        let decided_at = input.decision.as_ref().map(|_| now.clone());
        let expires_at = format_time_opt(input.expires_at.as_ref())?;
        sql_query(
            "INSERT INTO approval_decisions (
                id, tool_invocation_id, status, request_json, decision_json,
                requested_at, decided_at, expires_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(&id)
        .bind::<Text, _>(&input.tool_invocation_id)
        .bind::<Text, _>(&db_label(&input.status)?)
        .bind::<Json, _>(&to_json(&input.request)?)
        .bind::<Nullable<Json>, _>(&to_json_opt(&input.decision)?)
        .bind::<Text, _>(&now)
        .bind::<Nullable<Text>, _>(&decided_at)
        .bind::<Nullable<Text>, _>(&expires_at)
        .execute(&mut conn)?;
        load_approval_decision(&mut conn, &id)
    }

    pub fn insert_usage_event(&self, input: NewUsageEvent) -> Result<UsageEventRecord> {
        let mut conn = self.conn()?;
        let id = new_id();
        let now = now_string()?;
        sql_query(
            "INSERT INTO usage_events (
                id, provider_step_id, conversation_id, provider_id, model_id, date_key,
                input_tokens, output_tokens, cached_input_tokens, cache_write_input_tokens,
                reasoning_tokens, total_tokens, usage_json, created_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(&id)
        .bind::<Text, _>(&input.provider_step_id)
        .bind::<Text, _>(&input.conversation_id)
        .bind::<Text, _>(&input.provider_id)
        .bind::<Text, _>(&input.model_id)
        .bind::<Text, _>(&input.date_key)
        .bind::<BigInt, _>(u64_to_i64(input.usage.input_tokens)?)
        .bind::<BigInt, _>(u64_to_i64(input.usage.output_tokens)?)
        .bind::<BigInt, _>(u64_to_i64(input.usage.cached_input_tokens)?)
        .bind::<BigInt, _>(u64_to_i64(input.usage.cache_write_input_tokens)?)
        .bind::<BigInt, _>(u64_to_i64(input.usage.reasoning_tokens)?)
        .bind::<BigInt, _>(u64_to_i64(input.usage.total_tokens)?)
        .bind::<Json, _>(&to_json(&input.usage)?)
        .bind::<Text, _>(&now)
        .execute(&mut conn)?;
        load_usage_event(&mut conn, &id)
    }

    pub fn insert_shortcut(&self, input: NewShortcut) -> Result<ShortcutRecord> {
        let mut conn = self.conn()?;
        let id = new_id();
        let now = now_string()?;
        sql_query(
            "INSERT INTO shortcuts (
                id, hotkey, enabled, prompt_id, provider_id, model_id, input_source,
                action_json, settings_snapshot_json, created_at, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(&id)
        .bind::<Text, _>(&input.hotkey)
        .bind::<Bool, _>(input.enabled)
        .bind::<Nullable<Text>, _>(&input.prompt_id)
        .bind::<Nullable<Text>, _>(&input.provider_id)
        .bind::<Nullable<Text>, _>(&input.model_id)
        .bind::<Text, _>(&db_label(&input.input_source)?)
        .bind::<Json, _>(&to_json(&input.action)?)
        .bind::<Json, _>(&to_json(&input.settings_snapshot)?)
        .bind::<Text, _>(&now)
        .bind::<Text, _>(&now)
        .execute(&mut conn)?;
        load_shortcut(&mut conn, &id)
    }

    pub fn set_app_settings(&self, settings: AppSettingsPayload) -> Result<AppSettingsRecord> {
        let mut conn = self.conn()?;
        let now = now_string()?;
        sql_query(
            "INSERT INTO app_settings (id, settings_json, created_at, updated_at)
             VALUES ('default', ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                settings_json = excluded.settings_json,
                updated_at = excluded.updated_at",
        )
        .bind::<Json, _>(&to_json(&settings)?)
        .bind::<Text, _>(&now)
        .bind::<Text, _>(&now)
        .execute(&mut conn)?;
        self.get_app_settings()?
            .ok_or_else(|| DbError::Invariant("app settings row is missing".to_string()))
    }

    pub fn get_app_settings(&self) -> Result<Option<AppSettingsRecord>> {
        let mut conn = self.conn()?;
        let rows = sql_query("SELECT * FROM app_settings WHERE id = 'default'")
            .load::<SqlAppSettingsRow>(&mut conn)?;
        rows.into_iter().map(TryInto::try_into).next().transpose()
    }

    fn conn(&self) -> Result<PooledConnection<ConnectionManager<SqliteConnection>>> {
        let mut conn = self.pool.get()?;
        conn.batch_execute("PRAGMA foreign_keys = ON;")?;
        Ok(conn)
    }
}

fn load_conversation_item(
    conn: &mut SqliteConnection,
    item_id: &str,
) -> Result<ConversationItemRecord> {
    sql_query("SELECT * FROM conversation_items WHERE id = ?")
        .bind::<Text, _>(item_id)
        .load::<SqlConversationItemRow>(conn)?
        .into_iter()
        .next()
        .ok_or_else(|| DbError::Invariant("conversation item is missing".to_string()))?
        .try_into()
}

fn load_attachment(conn: &mut SqliteConnection, id: &str) -> Result<AttachmentRecord> {
    sql_query("SELECT * FROM attachments WHERE id = ?")
        .bind::<Text, _>(id)
        .load::<SqlAttachmentRow>(conn)?
        .into_iter()
        .next()
        .ok_or_else(|| DbError::Invariant("attachment is missing".to_string()))?
        .try_into()
}

fn load_agent_run(conn: &mut SqliteConnection, id: &str) -> Result<AgentRunRecord> {
    sql_query("SELECT * FROM agent_runs WHERE id = ?")
        .bind::<Text, _>(id)
        .load::<SqlAgentRunRow>(conn)?
        .into_iter()
        .next()
        .ok_or_else(|| DbError::Invariant("agent run is missing".to_string()))?
        .try_into()
}

fn load_provider_step(conn: &mut SqliteConnection, id: &str) -> Result<ProviderStepRecord> {
    sql_query("SELECT * FROM provider_steps WHERE id = ?")
        .bind::<Text, _>(id)
        .load::<SqlProviderStepRow>(conn)?
        .into_iter()
        .next()
        .ok_or_else(|| DbError::Invariant("provider step is missing".to_string()))?
        .try_into()
}

fn load_tool_invocation(conn: &mut SqliteConnection, id: &str) -> Result<ToolInvocationRecord> {
    sql_query("SELECT * FROM tool_invocations WHERE id = ?")
        .bind::<Text, _>(id)
        .load::<SqlToolInvocationRow>(conn)?
        .into_iter()
        .next()
        .ok_or_else(|| DbError::Invariant("tool invocation is missing".to_string()))?
        .try_into()
}

fn load_approval_decision(conn: &mut SqliteConnection, id: &str) -> Result<ApprovalDecisionRecord> {
    sql_query("SELECT * FROM approval_decisions WHERE id = ?")
        .bind::<Text, _>(id)
        .load::<SqlApprovalDecisionRow>(conn)?
        .into_iter()
        .next()
        .ok_or_else(|| DbError::Invariant("approval decision is missing".to_string()))?
        .try_into()
}

fn load_usage_event(conn: &mut SqliteConnection, id: &str) -> Result<UsageEventRecord> {
    sql_query("SELECT * FROM usage_events WHERE id = ?")
        .bind::<Text, _>(id)
        .load::<SqlUsageEventRow>(conn)?
        .into_iter()
        .next()
        .ok_or_else(|| DbError::Invariant("usage event is missing".to_string()))?
        .try_into()
}

fn load_shortcut(conn: &mut SqliteConnection, id: &str) -> Result<ShortcutRecord> {
    sql_query("SELECT * FROM shortcuts WHERE id = ?")
        .bind::<Text, _>(id)
        .load::<SqlShortcutRow>(conn)?
        .into_iter()
        .next()
        .ok_or_else(|| DbError::Invariant("shortcut is missing".to_string()))?
        .try_into()
}

fn insert_fts(
    conn: &mut SqliteConnection,
    item_id: &str,
    conversation_id: &str,
    content: &str,
) -> Result<()> {
    sql_query(
        "INSERT INTO conversation_item_fts (item_id, conversation_id, content) VALUES (?, ?, ?)",
    )
    .bind::<Text, _>(item_id)
    .bind::<Text, _>(conversation_id)
    .bind::<Text, _>(content)
    .execute(conn)?;
    Ok(())
}

fn delete_fts(conn: &mut SqliteConnection, item_id: &str) -> Result<()> {
    sql_query("DELETE FROM conversation_item_fts WHERE item_id = ?")
        .bind::<Text, _>(item_id)
        .execute(conn)?;
    Ok(())
}

fn now_string() -> Result<String> {
    Ok(OffsetDateTime::now_utc().format(&Rfc3339)?)
}

fn parse_time(value: String) -> Result<OffsetDateTime> {
    Ok(OffsetDateTime::parse(&value, &Rfc3339)?)
}

fn parse_time_opt(value: Option<String>) -> Result<Option<OffsetDateTime>> {
    value.map(parse_time).transpose()
}

fn format_time_opt(value: Option<&OffsetDateTime>) -> Result<Option<String>> {
    value.map(|time| Ok(time.format(&Rfc3339)?)).transpose()
}

fn db_label<T: Serialize>(value: &T) -> Result<String> {
    match serde_json::to_value(value)? {
        Value::String(value) => Ok(value),
        _ => Err(DbError::Invariant(
            "database enum labels must serialize to strings".to_string(),
        )),
    }
}

fn db_label_parse<T: DeserializeOwned>(value: String) -> Result<T> {
    Ok(serde_json::from_value(Value::String(value))?)
}

fn to_json<T: Serialize>(value: &T) -> Result<Value> {
    Ok(serde_json::to_value(value)?)
}

fn to_json_opt<T: Serialize>(value: &Option<T>) -> Result<Option<Value>> {
    value.as_ref().map(to_json).transpose()
}

fn from_json<T: DeserializeOwned>(value: Value) -> Result<T> {
    Ok(serde_json::from_value(value)?)
}

fn from_json_opt<T: DeserializeOwned>(value: Option<Value>) -> Result<Option<T>> {
    value.map(from_json).transpose()
}

fn tool_source_label(source: &ToolSource) -> String {
    match source {
        ToolSource::Local => "local".to_string(),
        ToolSource::Mcp { .. } => "mcp".to_string(),
        ToolSource::ProviderHosted { .. } => "provider_hosted".to_string(),
    }
}

fn tool_source_server_id(source: &ToolSource) -> Option<String> {
    match source {
        ToolSource::Mcp { server_id } => Some(server_id.clone()),
        ToolSource::Local | ToolSource::ProviderHosted { .. } => None,
    }
}

fn u64_to_i64(value: u64) -> Result<i64> {
    i64::try_from(value)
        .map_err(|_| DbError::Invariant("usage token count exceeds i64".to_string()))
}

#[derive(diesel::QueryableByName)]
struct TextValueRow {
    #[diesel(sql_type = Text)]
    value: String,
}

#[derive(diesel::QueryableByName)]
struct IntValueRow {
    #[diesel(sql_type = Integer)]
    value: i32,
}

#[derive(diesel::QueryableByName)]
struct SqlSchemaMetadataRow {
    #[diesel(sql_type = Integer)]
    schema_version: i32,
    #[diesel(sql_type = Nullable<Text>)]
    created_app_version: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    last_opened_app_version: Option<String>,
    #[diesel(sql_type = Json)]
    payload_json: Value,
    #[diesel(sql_type = Text)]
    created_at: String,
    #[diesel(sql_type = Text)]
    updated_at: String,
}

impl TryFrom<SqlSchemaMetadataRow> for SchemaMetadataRecord {
    type Error = DbError;

    fn try_from(row: SqlSchemaMetadataRow) -> Result<Self> {
        Ok(Self {
            schema_version: row.schema_version,
            created_app_version: row.created_app_version,
            last_opened_app_version: row.last_opened_app_version,
            payload: from_json(row.payload_json)?,
            created_at: parse_time(row.created_at)?,
            updated_at: parse_time(row.updated_at)?,
        })
    }
}

#[derive(diesel::QueryableByName)]
struct SqlProjectRow {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Text)]
    path: String,
    #[diesel(sql_type = Text)]
    display_name: String,
    #[diesel(sql_type = Text)]
    kind: String,
    #[diesel(sql_type = Json)]
    metadata_json: Value,
    #[diesel(sql_type = Text)]
    created_at: String,
    #[diesel(sql_type = Text)]
    updated_at: String,
    #[diesel(sql_type = Nullable<Text>)]
    last_opened_at: Option<String>,
}

impl TryFrom<SqlProjectRow> for ProjectRecord {
    type Error = DbError;

    fn try_from(row: SqlProjectRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            path: row.path,
            display_name: row.display_name,
            kind: db_label_parse(row.kind)?,
            metadata: from_json(row.metadata_json)?,
            created_at: parse_time(row.created_at)?,
            updated_at: parse_time(row.updated_at)?,
            last_opened_at: parse_time_opt(row.last_opened_at)?,
        })
    }
}

#[derive(diesel::QueryableByName)]
struct SqlConversationRow {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Text)]
    project_id: String,
    #[diesel(sql_type = Text)]
    title: String,
    #[diesel(sql_type = Text)]
    status: String,
    #[diesel(sql_type = Nullable<Text>)]
    prompt_id: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    default_provider_id: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    default_model_id: Option<String>,
    #[diesel(sql_type = Integer)]
    last_item_seq: i32,
    #[diesel(sql_type = Json)]
    metadata_json: Value,
    #[diesel(sql_type = Json)]
    settings_snapshot_json: Value,
    #[diesel(sql_type = Text)]
    created_at: String,
    #[diesel(sql_type = Text)]
    updated_at: String,
    #[diesel(sql_type = Nullable<Text>)]
    archived_at: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    deleted_at: Option<String>,
}

impl TryFrom<SqlConversationRow> for ConversationRecord {
    type Error = DbError;

    fn try_from(row: SqlConversationRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            project_id: row.project_id,
            title: row.title,
            status: db_label_parse(row.status)?,
            prompt_id: row.prompt_id,
            default_provider_id: row.default_provider_id,
            default_model_id: row.default_model_id,
            last_item_seq: row.last_item_seq,
            metadata: from_json(row.metadata_json)?,
            settings_snapshot: from_json(row.settings_snapshot_json)?,
            created_at: parse_time(row.created_at)?,
            updated_at: parse_time(row.updated_at)?,
            archived_at: parse_time_opt(row.archived_at)?,
            deleted_at: parse_time_opt(row.deleted_at)?,
        })
    }
}

#[derive(diesel::QueryableByName)]
struct SqlConversationItemRow {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Text)]
    conversation_id: String,
    #[diesel(sql_type = Integer)]
    seq: i32,
    #[diesel(sql_type = Text)]
    kind: String,
    #[diesel(sql_type = Text)]
    status: String,
    #[diesel(sql_type = Nullable<Text>)]
    agent_run_id: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    provider_step_id: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    tool_invocation_id: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    provider_item_id: Option<String>,
    #[diesel(sql_type = Json)]
    payload_json: Value,
    #[diesel(sql_type = Text)]
    search_text: String,
    #[diesel(sql_type = Text)]
    created_at: String,
    #[diesel(sql_type = Text)]
    updated_at: String,
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
            created_at: parse_time(row.created_at)?,
            updated_at: parse_time(row.updated_at)?,
        })
    }
}

#[derive(diesel::QueryableByName)]
struct SqlAttachmentRow {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Text)]
    conversation_id: String,
    #[diesel(sql_type = Text)]
    kind: String,
    #[diesel(sql_type = Text)]
    storage_kind: String,
    #[diesel(sql_type = Nullable<Text>)]
    mime_type: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    path: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    external_uri: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    provider_id: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    provider_file_id: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    sha256: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    size_bytes: Option<i64>,
    #[diesel(sql_type = Json)]
    metadata_json: Value,
    #[diesel(sql_type = Text)]
    created_at: String,
    #[diesel(sql_type = Text)]
    updated_at: String,
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
            created_at: parse_time(row.created_at)?,
            updated_at: parse_time(row.updated_at)?,
        })
    }
}

#[derive(diesel::QueryableByName)]
struct SqlAgentRunRow {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Text)]
    conversation_id: String,
    #[diesel(sql_type = Text)]
    trigger_kind: String,
    #[diesel(sql_type = Text)]
    status: String,
    #[diesel(sql_type = Json)]
    input_json: Value,
    #[diesel(sql_type = Nullable<Json>)]
    output_json: Option<Value>,
    #[diesel(sql_type = Nullable<Json>)]
    error_json: Option<Value>,
    #[diesel(sql_type = Text)]
    created_at: String,
    #[diesel(sql_type = Nullable<Text>)]
    started_at: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    completed_at: Option<String>,
    #[diesel(sql_type = Text)]
    updated_at: String,
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
            created_at: parse_time(row.created_at)?,
            started_at: parse_time_opt(row.started_at)?,
            completed_at: parse_time_opt(row.completed_at)?,
            updated_at: parse_time(row.updated_at)?,
        })
    }
}

#[derive(diesel::QueryableByName)]
struct SqlProviderStepRow {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Text)]
    agent_run_id: String,
    #[diesel(sql_type = Integer)]
    seq: i32,
    #[diesel(sql_type = Text)]
    provider_id: String,
    #[diesel(sql_type = Text)]
    model_id: String,
    #[diesel(sql_type = Text)]
    status: String,
    #[diesel(sql_type = Json)]
    request_snapshot_json: Value,
    #[diesel(sql_type = Nullable<Json>)]
    response_snapshot_json: Option<Value>,
    #[diesel(sql_type = Nullable<Json>)]
    state_snapshot_json: Option<Value>,
    #[diesel(sql_type = Json)]
    settings_snapshot_json: Value,
    #[diesel(sql_type = Nullable<Json>)]
    error_json: Option<Value>,
    #[diesel(sql_type = Text)]
    created_at: String,
    #[diesel(sql_type = Nullable<Text>)]
    started_at: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    completed_at: Option<String>,
    #[diesel(sql_type = Text)]
    updated_at: String,
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
            created_at: parse_time(row.created_at)?,
            started_at: parse_time_opt(row.started_at)?,
            completed_at: parse_time_opt(row.completed_at)?,
            updated_at: parse_time(row.updated_at)?,
        })
    }
}

#[derive(diesel::QueryableByName)]
struct SqlToolInvocationRow {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Text)]
    agent_run_id: String,
    #[diesel(sql_type = Nullable<Text>)]
    provider_step_id: Option<String>,
    #[diesel(sql_type = Text)]
    call_id: String,
    #[diesel(sql_type = Text)]
    source: String,
    #[diesel(sql_type = Nullable<Text>)]
    namespace: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    server_id: Option<String>,
    #[diesel(sql_type = Text)]
    tool_name: String,
    #[diesel(sql_type = Text)]
    runtime_tool_name: String,
    #[diesel(sql_type = Text)]
    status: String,
    #[diesel(sql_type = Json)]
    input_json: Value,
    #[diesel(sql_type = Nullable<Json>)]
    output_json: Option<Value>,
    #[diesel(sql_type = Nullable<Json>)]
    error_json: Option<Value>,
    #[diesel(sql_type = Text)]
    created_at: String,
    #[diesel(sql_type = Nullable<Text>)]
    started_at: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    completed_at: Option<String>,
    #[diesel(sql_type = Text)]
    updated_at: String,
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
            created_at: parse_time(row.created_at)?,
            started_at: parse_time_opt(row.started_at)?,
            completed_at: parse_time_opt(row.completed_at)?,
            updated_at: parse_time(row.updated_at)?,
        })
    }
}

#[derive(diesel::QueryableByName)]
struct SqlApprovalDecisionRow {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Text)]
    tool_invocation_id: String,
    #[diesel(sql_type = Text)]
    status: String,
    #[diesel(sql_type = Json)]
    request_json: Value,
    #[diesel(sql_type = Nullable<Json>)]
    decision_json: Option<Value>,
    #[diesel(sql_type = Text)]
    requested_at: String,
    #[diesel(sql_type = Nullable<Text>)]
    decided_at: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    expires_at: Option<String>,
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
            requested_at: parse_time(row.requested_at)?,
            decided_at: parse_time_opt(row.decided_at)?,
            expires_at: parse_time_opt(row.expires_at)?,
        })
    }
}

#[derive(diesel::QueryableByName)]
struct SqlUsageEventRow {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Text)]
    provider_step_id: String,
    #[diesel(sql_type = Text)]
    conversation_id: String,
    #[diesel(sql_type = Text)]
    provider_id: String,
    #[diesel(sql_type = Text)]
    model_id: String,
    #[diesel(sql_type = Text)]
    date_key: String,
    #[diesel(sql_type = BigInt)]
    input_tokens: i64,
    #[diesel(sql_type = BigInt)]
    output_tokens: i64,
    #[diesel(sql_type = BigInt)]
    cached_input_tokens: i64,
    #[diesel(sql_type = BigInt)]
    cache_write_input_tokens: i64,
    #[diesel(sql_type = BigInt)]
    reasoning_tokens: i64,
    #[diesel(sql_type = BigInt)]
    total_tokens: i64,
    #[diesel(sql_type = Json)]
    usage_json: Value,
    #[diesel(sql_type = Text)]
    created_at: String,
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
            created_at: parse_time(row.created_at)?,
        })
    }
}

#[derive(diesel::QueryableByName)]
struct SqlPromptRow {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Json)]
    content_json: Value,
    #[diesel(sql_type = Bool)]
    enabled: bool,
    #[diesel(sql_type = Integer)]
    sort_order: i32,
    #[diesel(sql_type = Text)]
    created_at: String,
    #[diesel(sql_type = Text)]
    updated_at: String,
}

impl TryFrom<SqlPromptRow> for PromptRecord {
    type Error = DbError;

    fn try_from(row: SqlPromptRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            name: row.name,
            content: from_json(row.content_json)?,
            enabled: row.enabled,
            sort_order: row.sort_order,
            created_at: parse_time(row.created_at)?,
            updated_at: parse_time(row.updated_at)?,
        })
    }
}

#[derive(diesel::QueryableByName)]
struct SqlShortcutRow {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Text)]
    hotkey: String,
    #[diesel(sql_type = Bool)]
    enabled: bool,
    #[diesel(sql_type = Nullable<Text>)]
    prompt_id: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    provider_id: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    model_id: Option<String>,
    #[diesel(sql_type = Text)]
    input_source: String,
    #[diesel(sql_type = Json)]
    action_json: Value,
    #[diesel(sql_type = Json)]
    settings_snapshot_json: Value,
    #[diesel(sql_type = Text)]
    created_at: String,
    #[diesel(sql_type = Text)]
    updated_at: String,
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
            created_at: parse_time(row.created_at)?,
            updated_at: parse_time(row.updated_at)?,
        })
    }
}

#[derive(diesel::QueryableByName)]
struct SqlProviderRow {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Text)]
    kind: String,
    #[diesel(sql_type = Text)]
    display_name: String,
    #[diesel(sql_type = Bool)]
    enabled: bool,
    #[diesel(sql_type = Json)]
    settings_json: Value,
    #[diesel(sql_type = Json)]
    secret_refs_json: Value,
    #[diesel(sql_type = Text)]
    created_at: String,
    #[diesel(sql_type = Text)]
    updated_at: String,
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
            created_at: parse_time(row.created_at)?,
            updated_at: parse_time(row.updated_at)?,
        })
    }
}

#[derive(diesel::QueryableByName)]
struct SqlProviderModelRow {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Text)]
    provider_id: String,
    #[diesel(sql_type = Text)]
    model_id: String,
    #[diesel(sql_type = Nullable<Text>)]
    display_name: Option<String>,
    #[diesel(sql_type = Json)]
    capabilities_json: Value,
    #[diesel(sql_type = Json)]
    metadata_json: Value,
    #[diesel(sql_type = Text)]
    fetched_at: String,
    #[diesel(sql_type = Text)]
    created_at: String,
    #[diesel(sql_type = Text)]
    updated_at: String,
}

impl TryFrom<SqlProviderModelRow> for ProviderModelRecord {
    type Error = DbError;

    fn try_from(row: SqlProviderModelRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            provider_id: row.provider_id,
            model_id: row.model_id,
            display_name: row.display_name,
            capabilities: from_json(row.capabilities_json)?,
            metadata: from_json(row.metadata_json)?,
            fetched_at: parse_time(row.fetched_at)?,
            created_at: parse_time(row.created_at)?,
            updated_at: parse_time(row.updated_at)?,
        })
    }
}

#[derive(diesel::QueryableByName)]
struct SqlAppSettingsRow {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Json)]
    settings_json: Value,
    #[diesel(sql_type = Text)]
    created_at: String,
    #[diesel(sql_type = Text)]
    updated_at: String,
}

impl TryFrom<SqlAppSettingsRow> for AppSettingsRecord {
    type Error = DbError;

    fn try_from(row: SqlAppSettingsRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            settings: from_json(row.settings_json)?,
            created_at: parse_time(row.created_at)?,
            updated_at: parse_time(row.updated_at)?,
        })
    }
}

#[cfg(test)]
pub(crate) fn schema_version() -> i32 {
    crate::migrations::SCHEMA_VERSION
}

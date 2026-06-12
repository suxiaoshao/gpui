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

    pub fn insert_project(&self, input: NewProject) -> Result<ProjectRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let now = now_string()?;
            let row = SqlNewProjectRow {
                id: new_id(),
                path: input.path,
                display_name: input.display_name,
                kind: db_label(&input.kind)?,
                pinned: input.pinned,
                removed: input.removed,
                metadata_json: to_json(&input.metadata)?,
                created_at: now,
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

    pub fn get_project_by_path(&self, path: &str) -> Result<Option<ProjectRecord>> {
        let mut conn = self.conn()?;
        projects::table
            .filter(projects::path.eq(path))
            .select(SqlProjectRow::as_select())
            .first::<SqlProjectRow>(&mut conn)
            .optional()?
            .map(TryInto::try_into)
            .transpose()
    }

    pub fn list_projects(&self) -> Result<Vec<ProjectRecord>> {
        let mut conn = self.conn()?;
        projects::table
            .order((projects::display_name.asc(), projects::path.asc()))
            .select(SqlProjectRow::as_select())
            .load::<SqlProjectRow>(&mut conn)?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }

    pub fn list_visible_projects(&self) -> Result<Vec<ProjectRecord>> {
        let mut conn = self.conn()?;
        projects::table
            .filter(projects::removed.eq(false))
            .order((projects::display_name.asc(), projects::path.asc()))
            .select(SqlProjectRow::as_select())
            .load::<SqlProjectRow>(&mut conn)?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }

    pub fn list_sidebar_projects(&self) -> Result<Vec<ProjectRecord>> {
        let mut conn = self.conn()?;
        let normal = db_label(&ProjectKind::Normal)?;
        projects::table
            .filter(projects::kind.eq(normal))
            .filter(projects::removed.eq(false))
            .order((projects::display_name.asc(), projects::path.asc()))
            .select(SqlProjectRow::as_select())
            .load::<SqlProjectRow>(&mut conn)?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }

    pub fn update_project_metadata(
        &self,
        id: &str,
        metadata: ProjectMetadata,
    ) -> Result<ProjectRecord> {
        let mut conn = self.conn()?;
        diesel::update(projects::table.find(id))
            .set((
                projects::metadata_json.eq(to_json(&metadata)?),
                projects::updated_at.eq(now_string()?),
            ))
            .returning(SqlProjectRow::as_returning())
            .get_result::<SqlProjectRow>(&mut conn)?
            .try_into()
    }

    pub fn rename_project(&self, id: &str, display_name: String) -> Result<ProjectRecord> {
        let mut conn = self.conn()?;
        diesel::update(projects::table.find(id))
            .set((
                projects::display_name.eq(display_name),
                projects::updated_at.eq(now_string()?),
            ))
            .returning(SqlProjectRow::as_returning())
            .get_result::<SqlProjectRow>(&mut conn)?
            .try_into()
    }

    pub fn set_project_removed(&self, id: &str, removed: bool) -> Result<ProjectRecord> {
        let mut conn = self.conn()?;
        diesel::update(projects::table.find(id))
            .set((
                projects::removed.eq(removed),
                projects::updated_at.eq(now_string()?),
            ))
            .returning(SqlProjectRow::as_returning())
            .get_result::<SqlProjectRow>(&mut conn)?
            .try_into()
    }

    pub fn set_project_pinned(&self, id: &str, pinned: bool) -> Result<ProjectRecord> {
        let mut conn = self.conn()?;
        diesel::update(projects::table.find(id))
            .set((
                projects::pinned.eq(pinned),
                projects::updated_at.eq(now_string()?),
            ))
            .returning(SqlProjectRow::as_returning())
            .get_result::<SqlProjectRow>(&mut conn)?
            .try_into()
    }

    pub fn insert_provider(&self, input: NewProvider) -> Result<ProviderRecord> {
        self.insert_provider_with_id(new_id(), input)
    }

    pub fn insert_provider_with_id(
        &self,
        id: ProviderId,
        input: NewProvider,
    ) -> Result<ProviderRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let now = now_string()?;
            let row = SqlNewProviderRow {
                id,
                kind: input.kind,
                display_name: input.display_name,
                enabled: input.enabled,
                settings_json: to_json(&input.settings)?,
                secret_refs_json: to_json(&input.secret_refs)?,
                created_at: now,
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

    pub fn list_providers(&self) -> Result<Vec<ProviderRecord>> {
        let mut conn = self.conn()?;
        providers::table
            .order((providers::display_name.asc(), providers::kind.asc()))
            .select(SqlProviderRow::as_select())
            .load::<SqlProviderRow>(&mut conn)?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }

    pub fn update_provider(&self, id: &str, input: UpdateProvider) -> Result<ProviderRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            diesel::update(providers::table.find(id))
                .set((
                    providers::display_name.eq(input.display_name),
                    providers::enabled.eq(input.enabled),
                    providers::settings_json.eq(to_json(&input.settings)?),
                    providers::secret_refs_json.eq(to_json(&input.secret_refs)?),
                    providers::updated_at.eq(now_string()?),
                ))
                .returning(SqlProviderRow::as_returning())
                .get_result::<SqlProviderRow>(conn)?
                .try_into()
        })
    }

    pub fn delete_provider(&self, id: &str) -> Result<usize> {
        let mut conn = self.conn()?;
        Ok(diesel::delete(providers::table.find(id)).execute(&mut conn)?)
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
                enabled: input.enabled,
                capabilities_json: to_json(&input.capabilities)?,
                metadata_json: to_json(&input.metadata)?,
                fetched_at: now,
                created_at: now,
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

    pub fn replace_fetched_provider_models(
        &self,
        provider_id: &str,
        models: Vec<NewProviderModel>,
    ) -> Result<Vec<ProviderModelRecord>> {
        let model_ids = models
            .iter()
            .map(|model| {
                if model.provider_id != provider_id {
                    return Err(DbError::Invariant(format!(
                        "provider model {} belongs to provider {}, expected {}",
                        model.model_id, model.provider_id, provider_id
                    )));
                }
                Ok(model.model_id.clone())
            })
            .collect::<Result<Vec<_>>>()?;
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let delete_query = diesel::delete(
                provider_models::table.filter(provider_models::provider_id.eq(provider_id)),
            );
            if model_ids.is_empty() {
                delete_query.execute(conn)?;
            } else {
                delete_query
                    .filter(provider_models::model_id.ne_all(&model_ids))
                    .execute(conn)?;
            }
            let mut records = Vec::with_capacity(models.len());
            for input in models {
                let now = now_string()?;
                let row = SqlNewProviderModelRow {
                    id: new_id(),
                    provider_id: input.provider_id,
                    model_id: input.model_id,
                    display_name: input.display_name,
                    enabled: input.enabled,
                    capabilities_json: to_json(&input.capabilities)?,
                    metadata_json: to_json(&input.metadata)?,
                    fetched_at: now,
                    created_at: now,
                    updated_at: now,
                };
                let record = diesel::insert_into(provider_models::table)
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
                    .try_into()?;
                records.push(record);
            }
            Ok(records)
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

    pub fn list_provider_models(&self, provider_id: &str) -> Result<Vec<ProviderModelRecord>> {
        let mut conn = self.conn()?;
        provider_models::table
            .filter(provider_models::provider_id.eq(provider_id))
            .order((
                provider_models::display_name.asc(),
                provider_models::model_id.asc(),
            ))
            .select(SqlProviderModelRow::as_select())
            .load::<SqlProviderModelRow>(&mut conn)?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }

    pub fn set_provider_model_enabled(
        &self,
        provider_id: &str,
        model_id: &str,
        enabled: bool,
    ) -> Result<ProviderModelRecord> {
        let mut conn = self.conn()?;
        diesel::update(
            provider_models::table
                .filter(provider_models::provider_id.eq(provider_id))
                .filter(provider_models::model_id.eq(model_id)),
        )
        .set((
            provider_models::enabled.eq(enabled),
            provider_models::updated_at.eq(now_string()?),
        ))
        .returning(SqlProviderModelRow::as_returning())
        .get_result::<SqlProviderModelRow>(&mut conn)?
        .try_into()
    }

    pub fn delete_provider_model(&self, provider_id: &str, model_id: &str) -> Result<usize> {
        let mut conn = self.conn()?;
        Ok(diesel::delete(
            provider_models::table
                .filter(provider_models::provider_id.eq(provider_id))
                .filter(provider_models::model_id.eq(model_id)),
        )
        .execute(&mut conn)?)
    }

    pub fn insert_prompt(&self, input: NewPrompt) -> Result<PromptRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let now = now_string()?;
            let row = SqlNewPromptRow {
                id: new_id(),
                name: input.name,
                content: input.content.text,
                enabled: input.enabled,
                sort_order: input.sort_order,
                created_at: now,
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

    pub fn list_prompts(&self) -> Result<Vec<PromptRecord>> {
        let mut conn = self.conn()?;
        prompts::table
            .order((
                prompts::sort_order.asc(),
                prompts::name.asc(),
                prompts::created_at.asc(),
            ))
            .select(SqlPromptRow::as_select())
            .load::<SqlPromptRow>(&mut conn)?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }

    pub fn update_prompt(&self, id: &str, input: UpdatePrompt) -> Result<PromptRecord> {
        let mut conn = self.conn()?;
        diesel::update(prompts::table.find(id))
            .set((
                prompts::name.eq(input.name),
                prompts::content.eq(input.content.text),
                prompts::enabled.eq(input.enabled),
                prompts::sort_order.eq(input.sort_order),
                prompts::updated_at.eq(now_string()?),
            ))
            .returning(SqlPromptRow::as_returning())
            .get_result::<SqlPromptRow>(&mut conn)?
            .try_into()
    }

    pub fn delete_prompt(&self, id: &str) -> Result<usize> {
        let mut conn = self.conn()?;
        Ok(diesel::delete(prompts::table.find(id)).execute(&mut conn)?)
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
                pinned: input.pinned,
                prompt_id: input.prompt_id,
                default_provider_id: input.default_provider_id,
                default_model_id: input.default_model_id,
                last_item_seq: 0,
                metadata_json: to_json(&input.metadata)?,
                settings_snapshot_json: to_json(&input.settings_snapshot)?,
                created_at: now,
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

    pub fn insert_conversation_with_user_item(
        &self,
        input: NewConversationWithUserItem,
    ) -> Result<ConversationWithUserItemRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let now = now_string()?;
            let new_conversation_row = SqlNewConversationRow {
                id: new_id(),
                project_id: input.conversation.project_id,
                title: input.conversation.title,
                status: db_label(&ConversationStatus::Active)?,
                pinned: input.conversation.pinned,
                prompt_id: input.conversation.prompt_id,
                default_provider_id: input.conversation.default_provider_id,
                default_model_id: input.conversation.default_model_id,
                last_item_seq: 0,
                metadata_json: to_json(&input.conversation.metadata)?,
                settings_snapshot_json: to_json(&input.conversation.settings_snapshot)?,
                created_at: now,
                updated_at: now,
                archived_at: None,
                deleted_at: None,
            };
            let conversation: ConversationRecord = diesel::insert_into(conversations::table)
                .values(&new_conversation_row)
                .returning(SqlConversationRow::as_returning())
                .get_result::<SqlConversationRow>(conn)?
                .try_into()?;
            let mut user_item = input.user_item;
            user_item.conversation_id = new_conversation_row.id;
            let user_item = append_conversation_item_with_conn(conn, user_item)?;
            let conversation = conversation_row(conn, &conversation.id)?
                .ok_or_else(|| DbError::Invariant("conversation is missing".to_string()))?
                .try_into()?;

            Ok(ConversationWithUserItemRecord {
                conversation,
                user_item,
            })
        })
    }

    pub fn get_conversation(&self, id: &str) -> Result<Option<ConversationRecord>> {
        let mut conn = self.conn()?;
        conversation_row(&mut conn, id)?
            .map(TryInto::try_into)
            .transpose()
    }

    pub fn list_sidebar_conversations(&self) -> Result<Vec<ConversationRecord>> {
        let active = db_label(&ConversationStatus::Active)?;
        let mut conn = self.conn()?;
        conversations::table
            .filter(conversations::status.eq(active))
            .filter(
                conversations::project_id.eq_any(
                    projects::table
                        .filter(projects::removed.eq(false))
                        .select(projects::id),
                ),
            )
            .order(conversations::updated_at.desc())
            .select(SqlConversationRow::as_select())
            .load::<SqlConversationRow>(&mut conn)?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }

    pub fn list_no_project_conversations(&self, query: &str) -> Result<Vec<ConversationRecord>> {
        let active = db_label(&ConversationStatus::Active)?;
        let scratch = db_label(&ProjectKind::Scratch)?;
        let mut conn = self.conn()?;
        let conversations = conversations::table
            .filter(conversations::status.eq(active))
            .filter(
                conversations::project_id.eq_any(
                    projects::table
                        .filter(projects::removed.eq(false))
                        .filter(projects::kind.eq(scratch))
                        .select(projects::id),
                ),
            )
            .order(conversations::updated_at.desc())
            .select(SqlConversationRow::as_select())
            .load::<SqlConversationRow>(&mut conn)?
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<ConversationRecord>>>()?;

        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return Ok(conversations);
        }

        let item_text_by_conversation = self.conversation_search_texts(
            conversations
                .iter()
                .map(|conversation| conversation.id.clone())
                .collect(),
        )?;

        Ok(conversations
            .into_iter()
            .filter(|conversation| {
                conversation_matches_query(
                    conversation,
                    None,
                    item_text_by_conversation.get(&conversation.id),
                    &query,
                )
            })
            .collect())
    }

    pub fn update_conversation_metadata(
        &self,
        id: &str,
        metadata: ConversationMetadata,
    ) -> Result<ConversationRecord> {
        let mut conn = self.conn()?;
        diesel::update(conversations::table.find(id))
            .set((
                conversations::metadata_json.eq(to_json(&metadata)?),
                conversations::updated_at.eq(now_string()?),
            ))
            .returning(SqlConversationRow::as_returning())
            .get_result::<SqlConversationRow>(&mut conn)?
            .try_into()
    }

    pub fn set_conversation_pinned(&self, id: &str, pinned: bool) -> Result<ConversationRecord> {
        let mut conn = self.conn()?;
        diesel::update(conversations::table.find(id))
            .set((
                conversations::pinned.eq(pinned),
                conversations::updated_at.eq(now_string()?),
            ))
            .returning(SqlConversationRow::as_returning())
            .get_result::<SqlConversationRow>(&mut conn)?
            .try_into()
    }

    pub fn soft_delete_conversation(&self, id: &str) -> Result<ConversationRecord> {
        let mut conn = self.conn()?;
        let now = now_string()?;
        diesel::update(conversations::table.find(id))
            .set((
                conversations::status.eq(db_label(&ConversationStatus::Deleted)?),
                conversations::deleted_at.eq(Some(now)),
                conversations::updated_at.eq(now),
            ))
            .returning(SqlConversationRow::as_returning())
            .get_result::<SqlConversationRow>(&mut conn)?
            .try_into()
    }

    pub fn search_sidebar_conversations(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ConversationRecord>> {
        let conversations = self.list_sidebar_conversations()?;
        if limit == 0 {
            return Ok(Vec::new());
        }

        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return Ok(conversations.into_iter().take(limit).collect());
        }

        let projects = self.visible_sidebar_project_map()?;
        let item_text_by_conversation = self.conversation_search_texts(
            conversations
                .iter()
                .map(|conversation| conversation.id.clone())
                .collect(),
        )?;

        Ok(conversations
            .into_iter()
            .filter(|conversation| {
                conversation_matches_query(
                    conversation,
                    projects.get(&conversation.project_id),
                    item_text_by_conversation.get(&conversation.id),
                    &query,
                )
            })
            .take(limit)
            .collect())
    }

    pub fn append_conversation_item(
        &self,
        input: NewConversationItem,
    ) -> Result<ConversationItemRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| append_conversation_item_with_conn(conn, input))
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

    pub fn conversation_timeline_records(
        &self,
        conversation_id: &str,
    ) -> Result<Option<ConversationTimelineRecords>> {
        let Some(conversation) = self.get_conversation(conversation_id)? else {
            return Ok(None);
        };
        let project = self.get_project(&conversation.project_id)?.ok_or_else(|| {
            DbError::Invariant(format!("project {} is missing", conversation.project_id))
        })?;
        let items = self.conversation_items(conversation_id)?;
        let runs = self.agent_runs_for_conversation(conversation_id)?;

        Ok(Some(ConversationTimelineRecords {
            conversation,
            project,
            items,
            runs,
        }))
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
                updated_at: now,
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

    fn visible_sidebar_project_map(&self) -> Result<HashMap<ProjectId, ProjectRecord>> {
        Ok(self
            .list_visible_projects()?
            .into_iter()
            .filter(|project| matches!(project.kind, ProjectKind::Normal | ProjectKind::Scratch))
            .map(|project| (project.id.clone(), project))
            .collect())
    }

    fn conversation_search_texts(
        &self,
        conversation_ids: Vec<ConversationId>,
    ) -> Result<HashMap<ConversationId, String>> {
        if conversation_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let mut conn = self.conn()?;
        let rows = conversation_items::table
            .filter(conversation_items::conversation_id.eq_any(conversation_ids))
            .select((
                conversation_items::conversation_id,
                conversation_items::search_text,
            ))
            .load::<(String, String)>(&mut conn)?;
        let mut grouped = HashMap::<ConversationId, Vec<String>>::new();
        for (conversation_id, text) in rows {
            if !text.is_empty() {
                grouped.entry(conversation_id).or_default().push(text);
            }
        }

        Ok(grouped
            .into_iter()
            .map(|(conversation_id, parts)| (conversation_id, parts.join("\n")))
            .collect())
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
                created_at: now,
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
                created_at: now,
                started_at: next_started_at(None, input.status, now),
                completed_at: next_agent_run_completed_at(None, input.status, now),
                updated_at: now,
            };
            diesel::insert_into(agent_runs::table)
                .values(&row)
                .returning(SqlAgentRunRow::as_returning())
                .get_result::<SqlAgentRunRow>(conn)?
                .try_into()
        })
    }

    pub fn get_agent_run(&self, id: &str) -> Result<Option<AgentRunRecord>> {
        let mut conn = self.conn()?;
        agent_run_row(&mut conn, id)?
            .map(TryInto::try_into)
            .transpose()
    }

    pub fn agent_runs_for_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<AgentRunRecord>> {
        let mut conn = self.conn()?;
        agent_runs::table
            .filter(agent_runs::conversation_id.eq(conversation_id))
            .order(agent_runs::created_at.asc())
            .select(SqlAgentRunRow::as_select())
            .load::<SqlAgentRunRow>(&mut conn)?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }

    pub fn agent_runs_by_status(&self, status: AgentRunStatus) -> Result<Vec<AgentRunRecord>> {
        let mut conn = self.conn()?;
        agent_runs::table
            .filter(agent_runs::status.eq(db_label(&status)?))
            .order(agent_runs::created_at.asc())
            .select(SqlAgentRunRow::as_select())
            .load::<SqlAgentRunRow>(&mut conn)?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }

    pub fn update_agent_run_status(
        &self,
        id: &str,
        update: UpdateAgentRunStatus,
    ) -> Result<AgentRunRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| update_agent_run_status_with_conn(conn, id, update))
    }

    pub fn append_conversation_item_and_update_agent_run(
        &self,
        item: NewConversationItem,
        agent_run_id: &str,
        update: UpdateAgentRunStatus,
    ) -> Result<(ConversationItemRecord, AgentRunRecord)> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let item = append_conversation_item_with_conn(conn, item)?;
            let run = update_agent_run_status_with_conn(conn, agent_run_id, update)?;
            Ok((item, run))
        })
    }

    pub fn insert_provider_step(&self, input: NewProviderStep) -> Result<ProviderStepRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            validate_provider_step_snapshots(&input)?;
            let agent_run = load_agent_run_row(conn, &input.agent_run_id)?;
            validate_provider_step_input_items(conn, &agent_run.conversation_id, &input)?;
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
                created_at: now,
                started_at: next_started_at(None, input.status, now),
                completed_at: next_provider_step_completed_at(None, input.status, now),
                updated_at: now,
            };
            diesel::insert_into(provider_steps::table)
                .values(&row)
                .returning(SqlProviderStepRow::as_returning())
                .get_result::<SqlProviderStepRow>(conn)?
                .try_into()
        })
    }

    pub fn get_provider_step(&self, id: &str) -> Result<Option<ProviderStepRecord>> {
        let mut conn = self.conn()?;
        provider_step_row(&mut conn, id)?
            .map(TryInto::try_into)
            .transpose()
    }

    pub fn provider_steps_for_run(&self, agent_run_id: &str) -> Result<Vec<ProviderStepRecord>> {
        let mut conn = self.conn()?;
        provider_steps::table
            .filter(provider_steps::agent_run_id.eq(agent_run_id))
            .order(provider_steps::seq.asc())
            .select(SqlProviderStepRow::as_select())
            .load::<SqlProviderStepRow>(&mut conn)?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }

    pub fn next_provider_step_seq(&self, agent_run_id: &str) -> Result<i32> {
        let mut conn = self.conn()?;
        let max_seq = provider_steps::table
            .filter(provider_steps::agent_run_id.eq(agent_run_id))
            .select(diesel::dsl::max(provider_steps::seq))
            .first::<Option<i32>>(&mut conn)?;
        Ok(max_seq.unwrap_or(0) + 1)
    }

    pub fn update_provider_step_status(
        &self,
        id: &str,
        update: UpdateProviderStepStatus,
    ) -> Result<ProviderStepRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| update_provider_step_status_with_conn(conn, id, update))
    }

    pub fn append_conversation_item_and_update_provider_step(
        &self,
        item: NewConversationItem,
        provider_step_id: &str,
        update: UpdateProviderStepStatus,
    ) -> Result<(ConversationItemRecord, ProviderStepRecord)> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let item = append_conversation_item_with_conn(conn, item)?;
            let step = update_provider_step_status_with_conn(conn, provider_step_id, update)?;
            Ok((item, step))
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
                created_at: now,
                started_at: next_started_at(None, input.status, now),
                completed_at: next_tool_invocation_completed_at(None, input.status, now),
                updated_at: now,
            };
            diesel::insert_into(tool_invocations::table)
                .values(&row)
                .returning(SqlToolInvocationRow::as_returning())
                .get_result::<SqlToolInvocationRow>(conn)?
                .try_into()
        })
    }

    pub fn get_tool_invocation(&self, id: &str) -> Result<Option<ToolInvocationRecord>> {
        let mut conn = self.conn()?;
        tool_invocation_row(&mut conn, id)?
            .map(TryInto::try_into)
            .transpose()
    }

    pub fn tool_invocations_for_run(
        &self,
        agent_run_id: &str,
    ) -> Result<Vec<ToolInvocationRecord>> {
        let mut conn = self.conn()?;
        tool_invocations::table
            .filter(tool_invocations::agent_run_id.eq(agent_run_id))
            .order(tool_invocations::created_at.asc())
            .select(SqlToolInvocationRow::as_select())
            .load::<SqlToolInvocationRow>(&mut conn)?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }

    pub fn update_tool_invocation_status(
        &self,
        id: &str,
        update: UpdateToolInvocationStatus,
    ) -> Result<ToolInvocationRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| update_tool_invocation_status_with_conn(conn, id, update))
    }

    pub fn append_conversation_item_and_update_tool_invocation(
        &self,
        item: NewConversationItem,
        tool_invocation_id: &str,
        update: UpdateToolInvocationStatus,
    ) -> Result<(ConversationItemRecord, ToolInvocationRecord)> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let item = append_conversation_item_with_conn(conn, item)?;
            let invocation =
                update_tool_invocation_status_with_conn(conn, tool_invocation_id, update)?;
            Ok((item, invocation))
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
                requested_at: now,
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

    pub fn get_approval_decision(&self, id: &str) -> Result<Option<ApprovalDecisionRecord>> {
        let mut conn = self.conn()?;
        approval_decision_row(&mut conn, id)?
            .map(TryInto::try_into)
            .transpose()
    }

    pub fn approval_decisions_for_tool(
        &self,
        tool_invocation_id: &str,
    ) -> Result<Vec<ApprovalDecisionRecord>> {
        let mut conn = self.conn()?;
        approval_decisions::table
            .filter(approval_decisions::tool_invocation_id.eq(tool_invocation_id))
            .order(approval_decisions::requested_at.asc())
            .select(SqlApprovalDecisionRow::as_select())
            .load::<SqlApprovalDecisionRow>(&mut conn)?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }

    pub fn pending_approval_decisions(&self) -> Result<Vec<ApprovalDecisionRecord>> {
        let mut conn = self.conn()?;
        approval_decisions::table
            .filter(approval_decisions::status.eq(db_label(&ApprovalStatus::Pending)?))
            .order(approval_decisions::requested_at.asc())
            .select(SqlApprovalDecisionRow::as_select())
            .load::<SqlApprovalDecisionRow>(&mut conn)?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }

    pub fn update_approval_decision(
        &self,
        id: &str,
        outcome: NewApprovalDecisionOutcome,
    ) -> Result<ApprovalDecisionRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| update_approval_decision_with_conn(conn, id, outcome))
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

    pub fn usage_events_for_provider_step(
        &self,
        provider_step_id: &str,
    ) -> Result<Vec<UsageEventRecord>> {
        let mut conn = self.conn()?;
        usage_events::table
            .filter(usage_events::provider_step_id.eq(provider_step_id))
            .order(usage_events::created_at.asc())
            .select(SqlUsageEventRow::as_select())
            .load::<SqlUsageEventRow>(&mut conn)?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
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
                created_at: now,
                updated_at: now,
            };
            diesel::insert_into(shortcuts::table)
                .values(&row)
                .returning(SqlShortcutRow::as_returning())
                .get_result::<SqlShortcutRow>(conn)?
                .try_into()
        })
    }

    pub fn get_shortcut(&self, id: &str) -> Result<Option<ShortcutRecord>> {
        let mut conn = self.conn()?;
        shortcut_row(&mut conn, id)?
            .map(TryInto::try_into)
            .transpose()
    }

    pub fn list_shortcuts(&self) -> Result<Vec<ShortcutRecord>> {
        let mut conn = self.conn()?;
        shortcuts::table
            .order(shortcuts::created_at.asc())
            .select(SqlShortcutRow::as_select())
            .load::<SqlShortcutRow>(&mut conn)?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }

    pub fn set_app_settings(&self, settings: AppSettingsPayload) -> Result<AppSettingsRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let now = now_string()?;
            let row = SqlNewAppSettingsRow {
                id: "default".to_string(),
                settings_json: to_json(&settings)?,
                created_at: now,
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

fn approval_decision_row(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<Option<SqlApprovalDecisionRow>> {
    Ok(approval_decisions::table
        .find(id)
        .select(SqlApprovalDecisionRow::as_select())
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

fn append_conversation_item_with_conn(
    conn: &mut SqliteConnection,
    input: NewConversationItem,
) -> Result<ConversationItemRecord> {
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
        created_at: now,
        updated_at: now,
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

fn update_agent_run_status_with_conn(
    conn: &mut SqliteConnection,
    id: &str,
    update: UpdateAgentRunStatus,
) -> Result<AgentRunRecord> {
    let existing = load_agent_run_row(conn, id)?;
    let now = now_string()?;
    let changes = SqlAgentRunStatusChanges {
        status: db_label(&update.status)?,
        output_json: to_json_opt(&update.output)?,
        error_json: to_json_opt(&update.error)?,
        started_at: next_started_at(existing.started_at, update.status, now),
        completed_at: next_agent_run_completed_at(existing.completed_at, update.status, now),
        updated_at: now,
    };
    diesel::update(agent_runs::table.find(id))
        .set(&changes)
        .returning(SqlAgentRunRow::as_returning())
        .get_result::<SqlAgentRunRow>(conn)?
        .try_into()
}

fn update_provider_step_status_with_conn(
    conn: &mut SqliteConnection,
    id: &str,
    update: UpdateProviderStepStatus,
) -> Result<ProviderStepRecord> {
    let existing = load_provider_step_row(conn, id)?;
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

fn update_approval_decision_with_conn(
    conn: &mut SqliteConnection,
    id: &str,
    outcome: NewApprovalDecisionOutcome,
) -> Result<ApprovalDecisionRecord> {
    let existing = approval_decision_row(conn, id)?
        .ok_or_else(|| DbError::Invariant(format!("approval decision {id} is missing")))?;
    let status: ApprovalStatus = db_label_parse(existing.status)?;
    if status != ApprovalStatus::Pending {
        return Err(DbError::Invariant(format!(
            "approval decision {id} is {status:?}, not pending"
        )));
    }

    let now = now_string()?;
    let outcome = approval_outcome_columns(outcome, &now)?;
    let changes = SqlApprovalDecisionChanges {
        status: db_label(&outcome.status)?,
        decision_json: to_json_opt(&outcome.decision)?,
        decided_at: outcome.decided_at,
        expires_at: outcome.expires_at,
    };
    diesel::update(approval_decisions::table.find(id))
        .set(&changes)
        .returning(SqlApprovalDecisionRow::as_returning())
        .get_result::<SqlApprovalDecisionRow>(conn)?
        .try_into()
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

fn validate_provider_step_input_items(
    conn: &mut SqliteConnection,
    conversation_id: &str,
    input: &NewProviderStep,
) -> Result<()> {
    for item_id in &input.request_snapshot.input_item_ids {
        let item = conversation_item_row(conn, item_id)?.ok_or_else(|| {
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

fn approval_outcome_columns(
    outcome: NewApprovalDecisionOutcome,
    now: &OffsetDateTime,
) -> Result<ApprovalOutcomeColumns> {
    Ok(match outcome {
        NewApprovalDecisionOutcome::Pending { expires_at } => ApprovalOutcomeColumns {
            status: ApprovalStatus::Pending,
            decision: None,
            decided_at: None,
            expires_at,
        },
        NewApprovalDecisionOutcome::Approved { decided_by, reason } => ApprovalOutcomeColumns {
            status: ApprovalStatus::Approved,
            decision: Some(ApprovalDecisionPayload {
                approved: true,
                decided_by,
                reason,
            }),
            decided_at: Some(*now),
            expires_at: None,
        },
        NewApprovalDecisionOutcome::Denied { decided_by, reason } => ApprovalOutcomeColumns {
            status: ApprovalStatus::Denied,
            decision: Some(ApprovalDecisionPayload {
                approved: false,
                decided_by,
                reason,
            }),
            decided_at: Some(*now),
            expires_at: None,
        },
        NewApprovalDecisionOutcome::Expired => ApprovalOutcomeColumns {
            status: ApprovalStatus::Expired,
            decision: None,
            decided_at: Some(*now),
            expires_at: None,
        },
        NewApprovalDecisionOutcome::Canceled => ApprovalOutcomeColumns {
            status: ApprovalStatus::Canceled,
            decision: None,
            decided_at: Some(*now),
            expires_at: None,
        },
    })
}

struct ApprovalOutcomeColumns {
    status: ApprovalStatus,
    decision: Option<ApprovalDecisionPayload>,
    decided_at: Option<OffsetDateTime>,
    expires_at: Option<OffsetDateTime>,
}

trait ExecutionStatusTiming {
    fn starts_clock(self) -> bool;
}

impl ExecutionStatusTiming for AgentRunStatus {
    fn starts_clock(self) -> bool {
        !matches!(self, AgentRunStatus::Queued)
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

fn next_agent_run_completed_at(
    existing: Option<OffsetDateTime>,
    status: AgentRunStatus,
    now: OffsetDateTime,
) -> Option<OffsetDateTime> {
    existing.or_else(|| {
        matches!(
            status,
            AgentRunStatus::Completed | AgentRunStatus::Failed | AgentRunStatus::Canceled
        )
        .then_some(now)
    })
}

fn next_provider_step_completed_at(
    existing: Option<OffsetDateTime>,
    status: ProviderStepStatus,
    now: OffsetDateTime,
) -> Option<OffsetDateTime> {
    existing.or_else(|| {
        matches!(
            status,
            ProviderStepStatus::Completed
                | ProviderStepStatus::Failed
                | ProviderStepStatus::Canceled
        )
        .then_some(now)
    })
}

fn next_tool_invocation_completed_at(
    existing: Option<OffsetDateTime>,
    status: ToolInvocationStatus,
    now: OffsetDateTime,
) -> Option<OffsetDateTime> {
    existing.or_else(|| {
        matches!(
            status,
            ToolInvocationStatus::Succeeded
                | ToolInvocationStatus::Failed
                | ToolInvocationStatus::Denied
                | ToolInvocationStatus::Canceled
        )
        .then_some(now)
    })
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

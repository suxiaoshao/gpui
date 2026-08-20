use super::*;

impl FreshRepository {
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
                last_entry_seq: 0,
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
        self.insert_conversation_with_user_item_with_id(new_id(), input)
    }

    pub fn insert_conversation_with_user_item_with_id(
        &self,
        id: ConversationId,
        input: NewConversationWithUserItem,
    ) -> Result<ConversationWithUserItemRecord> {
        self.insert_conversation_with_user_item_with_id_and_attachments(id, input, Vec::new())
    }

    pub fn insert_conversation_with_user_item_with_id_and_attachments(
        &self,
        id: ConversationId,
        input: NewConversationWithUserItem,
        attachments: Vec<NewAttachment>,
    ) -> Result<ConversationWithUserItemRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let now = now_string()?;
            let new_conversation_row = SqlNewConversationRow {
                id,
                project_id: input.conversation.project_id,
                title: input.conversation.title,
                status: db_label(&ConversationStatus::Active)?,
                pinned: input.conversation.pinned,
                prompt_id: input.conversation.prompt_id,
                default_provider_id: input.conversation.default_provider_id,
                default_model_id: input.conversation.default_model_id,
                last_entry_seq: 0,
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
            let _attachments =
                insert_attachments_into_message_item_with_conn(conn, &mut user_item, attachments)?;
            let user_item = append_conversation_entry_with_conn(conn, user_item)?;
            let conversation = conversation_row(conn, &conversation.id)?
                .ok_or_else(|| DbError::Invariant("conversation is missing".to_string()))?
                .try_into()?;

            Ok(ConversationWithUserItemRecord {
                conversation,
                user_item,
            })
        })
    }

    pub fn create_conversation_transaction(
        &self,
        input: NewConversationTransaction,
    ) -> Result<CreatedConversationTransaction> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            if let Some((project_id, project)) = input.new_project {
                let now = now_string()?;
                let row = SqlNewProjectRow {
                    id: project_id,
                    path: project.path,
                    display_name: project.display_name,
                    kind: db_label(&project.kind)?,
                    pinned: project.pinned,
                    removed: project.removed,
                    metadata_json: to_json(&project.metadata)?,
                    created_at: now,
                    updated_at: now,
                    last_opened_at: None,
                };
                diesel::insert_into(projects::table)
                    .values(&row)
                    .execute(conn)?;
            }

            let now = now_string()?;
            let new_conversation_row = SqlNewConversationRow {
                id: input.conversation_id,
                project_id: input.conversation.conversation.project_id,
                title: input.conversation.conversation.title,
                status: db_label(&ConversationStatus::Active)?,
                pinned: input.conversation.conversation.pinned,
                prompt_id: input.conversation.conversation.prompt_id,
                default_provider_id: input.conversation.conversation.default_provider_id,
                default_model_id: input.conversation.conversation.default_model_id,
                last_entry_seq: 0,
                metadata_json: to_json(&input.conversation.conversation.metadata)?,
                settings_snapshot_json: to_json(
                    &input.conversation.conversation.settings_snapshot,
                )?,
                created_at: now,
                updated_at: now,
                archived_at: None,
                deleted_at: None,
            };
            diesel::insert_into(conversations::table)
                .values(&new_conversation_row)
                .execute(conn)?;

            let mut user_item = input.conversation.user_item;
            user_item.conversation_id = new_conversation_row.id.clone();
            let _attachments = insert_attachments_into_message_item_with_conn(
                conn,
                &mut user_item,
                input.attachments,
            )?;
            let user_item = append_conversation_entry_with_conn(conn, user_item)?;
            let conversation: ConversationRecord =
                conversation_row(conn, &new_conversation_row.id)?
                    .ok_or_else(|| DbError::Invariant("conversation is missing".to_string()))?
                    .try_into()?;

            let mut project: ProjectRecord = project_row(conn, &conversation.project_id)?
                .ok_or_else(|| DbError::Invariant("project is missing".to_string()))?
                .try_into()?;
            project.metadata.last_active_conversation_id = Some(conversation.id.clone());
            let project: ProjectRecord = diesel::update(projects::table.find(&project.id))
                .set((
                    projects::metadata_json.eq(to_json(&project.metadata)?),
                    projects::updated_at.eq(now_string()?),
                ))
                .returning(SqlProjectRow::as_returning())
                .get_result::<SqlProjectRow>(conn)?
                .try_into()?;

            Ok(CreatedConversationTransaction {
                project,
                record: ConversationWithUserItemRecord {
                    conversation,
                    user_item,
                },
            })
        })
    }

    pub fn send_conversation_transaction(
        &self,
        mut input: SendConversationTransaction,
    ) -> Result<SentConversationTransaction> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let conversation: ConversationRecord =
                conversation_row(conn, &input.entry.conversation_id)?
                    .ok_or_else(|| DbError::Invariant("conversation is missing".to_string()))?
                    .try_into()?;
            let mut project: ProjectRecord = project_row(conn, &conversation.project_id)?
                .ok_or_else(|| DbError::Invariant("project is missing".to_string()))?
                .try_into()?;

            let attachments = insert_attachments_into_message_item_with_conn(
                conn,
                &mut input.entry,
                input.attachments,
            )?;
            let entry = append_conversation_entry_with_conn(conn, input.entry)?;
            let commit = conversation_commit_with_conn(conn, entry.conversation_id.clone(), entry)?;

            project.metadata.last_active_conversation_id = Some(commit.conversation.id.clone());
            let project: ProjectRecord = diesel::update(projects::table.find(&project.id))
                .set((
                    projects::metadata_json.eq(to_json(&project.metadata)?),
                    projects::updated_at.eq(now_string()?),
                ))
                .returning(SqlProjectRow::as_returning())
                .get_result::<SqlProjectRow>(conn)?
                .try_into()?;

            Ok(SentConversationTransaction {
                project,
                commit,
                attachments,
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
        conn.immediate_transaction(|conn| {
            let has_active_run = diesel::select(diesel::dsl::exists(
                agent_runs::table
                    .filter(agent_runs::conversation_id.eq(id))
                    .filter(agent_runs::status.eq("running")),
            ))
            .get_result::<bool>(conn)?;
            if has_active_run {
                return Err(DbError::ConversationHasActiveRun {
                    conversation_id: id.to_string(),
                });
            }
            let now = now_string()?;
            diesel::update(conversations::table.find(id))
                .set((
                    conversations::status.eq(db_label(&ConversationStatus::Deleted)?),
                    conversations::deleted_at.eq(Some(now)),
                    conversations::updated_at.eq(now),
                ))
                .returning(SqlConversationRow::as_returning())
                .get_result::<SqlConversationRow>(conn)?
                .try_into()
        })
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

    pub fn append_conversation_entry(
        &self,
        input: NewConversationEntry,
    ) -> Result<ConversationCommit<ConversationEntryRecord>> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let entry = append_conversation_entry_with_conn(conn, input)?;
            conversation_commit_with_conn(conn, entry.conversation_id.clone(), entry)
        })
    }

    pub fn append_conversation_entry_with_attachments(
        &self,
        mut input: NewConversationEntry,
        attachments: Vec<NewAttachment>,
    ) -> Result<ConversationCommit<ConversationEntryRecord>> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let _attachments =
                insert_attachments_into_message_item_with_conn(conn, &mut input, attachments)?;
            let entry = append_conversation_entry_with_conn(conn, input)?;
            conversation_commit_with_conn(conn, entry.conversation_id.clone(), entry)
        })
    }

    pub fn conversation_entries(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<ConversationEntryRecord>> {
        let mut conn = self.conn()?;
        conversation_entries::table
            .filter(conversation_entries::conversation_id.eq(conversation_id))
            .order(conversation_entries::seq.asc())
            .select(SqlConversationEntryRow::as_select())
            .load::<SqlConversationEntryRow>(&mut conn)?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }

    pub fn conversation_attachments(&self, conversation_id: &str) -> Result<Vec<AttachmentRecord>> {
        let mut conn = self.conn()?;
        attachments::table
            .filter(attachments::conversation_id.eq(conversation_id))
            .order(attachments::created_at.asc())
            .select(SqlAttachmentRow::as_select())
            .load::<SqlAttachmentRow>(&mut conn)?
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
        let items = self.conversation_entries(conversation_id)?;
        let attachments = self.conversation_attachments(conversation_id)?;
        let runs = self.agent_runs_for_conversation(conversation_id)?;
        validate_timeline_run_entries(&runs, &items)?;
        let mut provider_steps = Vec::new();
        let mut tool_invocations = Vec::new();
        for run in &runs {
            provider_steps.extend(self.provider_steps_for_run(&run.id)?);
            tool_invocations.extend(self.tool_invocations_for_run(&run.id)?);
        }
        let usage_events = self.usage_events_for_conversation(conversation_id)?;

        let entries_by_id = items
            .iter()
            .map(|entry| (entry.id.as_str(), entry))
            .collect::<HashMap<_, _>>();
        let provider_steps_by_id = provider_steps
            .iter()
            .map(|step| (step.id.as_str(), step))
            .collect::<HashMap<_, _>>();
        let usage_events_by_step_id = usage_events
            .iter()
            .map(|usage| (usage.provider_step_id.as_str(), usage))
            .collect::<HashMap<_, _>>();
        let mut agent_message_request_usages = Vec::new();
        for run in &runs {
            let Some(output) = run.output.as_ref() else {
                continue;
            };
            let final_entry = entries_by_id
                .get(output.final_entry_id.as_str())
                .ok_or_else(|| {
                    DbError::Invariant(format!(
                        "final entry {} for run {} is missing",
                        output.final_entry_id, run.id
                    ))
                })?;
            if !is_completed_assistant_message(final_entry) {
                continue;
            }
            let Some(provider_step_id) = final_entry.provider_step_id.as_deref() else {
                continue;
            };
            let provider_step = provider_steps_by_id.get(provider_step_id).ok_or_else(|| {
                DbError::Invariant(format!(
                    "provider step {provider_step_id} for final entry {} is missing",
                    final_entry.id
                ))
            })?;
            if let Some(request_usage) = agent_message_request_usage_from_parts(
                run,
                final_entry,
                provider_step,
                usage_events_by_step_id.get(provider_step_id).copied(),
            )? {
                agent_message_request_usages.push((
                    final_entry.seq,
                    final_entry.id.clone(),
                    request_usage,
                ));
            }
        }
        agent_message_request_usages
            .sort_by(|left, right| (left.0, left.1.as_str()).cmp(&(right.0, right.1.as_str())));
        let agent_message_request_usages = agent_message_request_usages
            .into_iter()
            .map(|(_, _, usage)| usage)
            .collect();

        Ok(Some(ConversationTimelineRecords {
            conversation,
            project,
            items,
            attachments,
            runs,
            provider_steps,
            tool_invocations,
            agent_message_request_usages,
        }))
    }

    pub fn update_conversation_entry_payload(
        &self,
        item_id: &str,
        status: ConversationEntryStatus,
        payload: ConversationEntryPayload,
    ) -> Result<ConversationCommit<ConversationEntryRecord>> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let now = now_string()?;
            let changes = SqlConversationEntryPayloadChanges {
                kind: db_label(&payload.kind())?,
                status: db_label(&status)?,
                payload_json: to_json(&payload)?,
                search_text: payload.search_text(),
                updated_at: now,
            };
            let item = diesel::update(conversation_entries::table.find(item_id))
                .set(&changes)
                .returning(SqlConversationEntryRow::as_returning())
                .get_result::<SqlConversationEntryRow>(conn)?;
            diesel::update(conversations::table.find(&item.conversation_id))
                .set(conversations::updated_at.eq(now))
                .execute(conn)?;
            let item: ConversationEntryRecord = item.try_into()?;
            conversation_commit_with_conn(conn, item.conversation_id.clone(), item)
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
        let rows = conversation_entries::table
            .filter(conversation_entries::conversation_id.eq_any(conversation_ids))
            .select((
                conversation_entries::conversation_id,
                conversation_entries::search_text,
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
        conn.immediate_transaction(|conn| insert_attachment_with_conn(conn, input))
    }
}

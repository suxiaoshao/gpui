use super::*;

impl FreshRepository {
    pub fn insert_agent_run(&self, input: NewAgentRun) -> Result<AgentRunRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            validate_agent_run_trigger(conn, &input.conversation_id, &input.trigger_entry_id)?;
            let now = now_string()?;
            let row = SqlNewAgentRunRow {
                id: new_id(),
                conversation_id: input.conversation_id,
                trigger_entry_id: input.trigger_entry_id,
                trigger_kind: db_label(&input.trigger_kind)?,
                status: db_label(&input.status)?,
                input_json: to_json(&input.input)?,
                final_entry_id: None,
                stopped_reason: None,
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
        conn.immediate_transaction(|conn| {
            update_active_agent_run_status_with_conn(conn, id, update)
        })
    }

    pub fn finish_agent_run(
        &self,
        id: &str,
        finish: FinishAgentRun,
    ) -> Result<ConversationCommit<FinishedAgentRun>> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let finished = finish_agent_run_with_conn(conn, id, finish)?;
            conversation_commit_with_conn(conn, finished.run.conversation_id.clone(), finished)
        })
    }

    pub fn append_conversation_entry_and_update_agent_run(
        &self,
        item: NewConversationEntry,
        agent_run_id: &str,
        update: UpdateAgentRunStatus,
    ) -> Result<(ConversationEntryRecord, AgentRunRecord)> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let item = append_conversation_entry_with_conn(conn, item)?;
            let run = update_active_agent_run_status_with_conn(conn, agent_run_id, update)?;
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

    pub fn append_conversation_entry_and_update_provider_step(
        &self,
        item: NewConversationEntry,
        provider_step_id: &str,
        update: UpdateProviderStepStatus,
    ) -> Result<(ConversationEntryRecord, ProviderStepRecord)> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let item = append_conversation_entry_with_conn(conn, item)?;
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
                approval_json: None,
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

    pub fn append_conversation_entry_and_update_tool_invocation(
        &self,
        item: NewConversationEntry,
        tool_invocation_id: &str,
        update: UpdateToolInvocationStatus,
    ) -> Result<(ConversationEntryRecord, ToolInvocationRecord)> {
        let (mut items, invocation) = self.append_conversation_entries_and_update_tool_invocation(
            vec![item],
            tool_invocation_id,
            update,
        )?;
        Ok((items.remove(0), invocation))
    }

    pub fn append_conversation_entry_and_update_tool_invocation_full(
        &self,
        item: NewConversationEntry,
        tool_invocation_id: &str,
        update: UpdateToolInvocationStatus,
        approval: Option<ToolInvocationApproval>,
    ) -> Result<(ConversationEntryRecord, ToolInvocationRecord)> {
        let commit = self.append_conversation_entries_and_update_tool_invocation_full(
            vec![item],
            tool_invocation_id,
            update,
            approval,
        )?;
        let (mut items, invocation) = commit.value;
        Ok((items.remove(0), invocation))
    }

    pub fn append_conversation_entries_and_update_tool_invocation(
        &self,
        items: Vec<NewConversationEntry>,
        tool_invocation_id: &str,
        update: UpdateToolInvocationStatus,
    ) -> Result<(Vec<ConversationEntryRecord>, ToolInvocationRecord)> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            ensure_tool_invocation_not_terminal(conn, tool_invocation_id)?;
            let items = items
                .into_iter()
                .map(|item| append_conversation_entry_with_conn(conn, item))
                .collect::<Result<Vec<_>>>()?;
            let invocation =
                update_tool_invocation_status_with_conn(conn, tool_invocation_id, update)?;
            Ok((items, invocation))
        })
    }

    pub fn append_conversation_entries_and_update_tool_invocation_full(
        &self,
        items: Vec<NewConversationEntry>,
        tool_invocation_id: &str,
        update: UpdateToolInvocationStatus,
        approval: Option<ToolInvocationApproval>,
    ) -> Result<ConversationCommit<(Vec<ConversationEntryRecord>, ToolInvocationRecord)>> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            ensure_tool_invocation_not_terminal(conn, tool_invocation_id)?;
            let items = items
                .into_iter()
                .map(|item| append_conversation_entry_with_conn(conn, item))
                .collect::<Result<Vec<_>>>()?;
            let invocation =
                update_tool_invocation_full_with_conn(conn, tool_invocation_id, update, approval)?;
            let conversation_id = items
                .first()
                .map(|item| item.conversation_id.clone())
                .ok_or_else(|| {
                    DbError::Invariant(
                        "tool invocation entry transaction requires at least one entry".to_string(),
                    )
                })?;
            conversation_commit_with_conn(conn, conversation_id, (items, invocation))
        })
    }

    pub fn request_tool_invocation_approval(
        &self,
        id: &str,
        approval: NewToolInvocationApproval,
    ) -> Result<ToolInvocationRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let now = now_string()?;
            let approval = ToolInvocationApproval {
                status: ApprovalStatus::Pending,
                request: approval.request,
                decision: None,
                requested_at: now,
                decided_at: None,
                expires_at: approval.expires_at,
            };
            update_tool_invocation_approval_with_conn(
                conn,
                id,
                ToolInvocationStatus::AwaitingApproval,
                Some(approval),
            )
        })
    }

    pub fn request_tool_invocation_approval_with_entry(
        &self,
        id: &str,
        approval: NewToolInvocationApproval,
        entry: NewConversationEntry,
    ) -> Result<ConversationCommit<(ConversationEntryRecord, ToolInvocationRecord)>> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            ensure_tool_invocation_not_terminal(conn, id)?;
            let now = now_string()?;
            let approval = ToolInvocationApproval {
                status: ApprovalStatus::Pending,
                request: approval.request,
                decision: None,
                requested_at: now,
                decided_at: None,
                expires_at: approval.expires_at,
            };
            let entry = append_conversation_entry_with_conn(conn, entry)?;
            let invocation = update_tool_invocation_approval_with_conn(
                conn,
                id,
                ToolInvocationStatus::AwaitingApproval,
                Some(approval),
            )?;
            conversation_commit_with_conn(conn, entry.conversation_id.clone(), (entry, invocation))
        })
    }

    pub fn record_tool_invocation_approval(
        &self,
        id: &str,
        approval: ToolInvocationApproval,
        status: ToolInvocationStatus,
    ) -> Result<ToolInvocationRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            update_tool_invocation_approval_with_conn(conn, id, status, Some(approval))
        })
    }

    pub fn update_tool_invocation_approval(
        &self,
        id: &str,
        outcome: ToolInvocationApprovalOutcome,
        status: ToolInvocationStatus,
    ) -> Result<ToolInvocationRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let existing = load_tool_invocation_row(conn, id)?;
            let mut approval: ToolInvocationApproval =
                from_json_opt(existing.approval_json.clone())?.ok_or_else(|| {
                    DbError::Invariant(format!("tool invocation {id} has no approval"))
                })?;
            if approval.status != ApprovalStatus::Pending {
                return Err(DbError::Invariant(format!(
                    "tool invocation {id} approval is {:?}, not pending",
                    approval.status
                )));
            }
            let now = now_string()?;
            apply_approval_outcome(&mut approval, outcome, now);
            update_tool_invocation_approval_with_conn(conn, id, status, Some(approval))
        })
    }

    pub fn decide_tool_invocation_approval_with_entry(
        &self,
        id: &str,
        outcome: ToolInvocationApprovalOutcome,
        status: ToolInvocationStatus,
        entry: NewConversationEntry,
    ) -> Result<ConversationCommit<(ConversationEntryRecord, ToolInvocationRecord)>> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            ensure_tool_invocation_not_terminal(conn, id)?;
            let existing = load_tool_invocation_row(conn, id)?;
            let mut approval: ToolInvocationApproval =
                from_json_opt(existing.approval_json.clone())?.ok_or_else(|| {
                    DbError::Invariant(format!("tool invocation {id} has no approval"))
                })?;
            if approval.status != ApprovalStatus::Pending {
                return Err(DbError::Invariant(format!(
                    "tool invocation {id} approval is {:?}, not pending",
                    approval.status
                )));
            }
            let now = now_string()?;
            apply_approval_outcome(&mut approval, outcome, now);
            let entry = append_conversation_entry_with_conn(conn, entry)?;
            let invocation =
                update_tool_invocation_approval_with_conn(conn, id, status, Some(approval))?;
            conversation_commit_with_conn(conn, entry.conversation_id.clone(), (entry, invocation))
        })
    }

    pub fn record_auto_tool_invocation_approval_with_entries(
        &self,
        entries: Vec<NewConversationEntry>,
        id: &str,
        status: ToolInvocationStatus,
        approval: ToolInvocationApproval,
    ) -> Result<(Vec<ConversationEntryRecord>, ToolInvocationRecord)> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            ensure_tool_invocation_not_terminal(conn, id)?;
            let entries = entries
                .into_iter()
                .map(|entry| append_conversation_entry_with_conn(conn, entry))
                .collect::<Result<Vec<_>>>()?;
            let invocation =
                update_tool_invocation_approval_with_conn(conn, id, status, Some(approval))?;
            Ok((entries, invocation))
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
}

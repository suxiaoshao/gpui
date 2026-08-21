use super::*;

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = agent_runs)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub(crate) struct SqlAgentRunRow {
    pub(crate) id: String,
    pub(crate) conversation_id: String,
    pub(crate) trigger_entry_id: String,
    pub(crate) trigger_kind: String,
    pub(crate) status: String,
    pub(crate) input_json: Value,
    pub(crate) final_entry_id: Option<String>,
    pub(crate) stopped_reason: Option<String>,
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
    pub(crate) trigger_entry_id: String,
    pub(crate) trigger_kind: String,
    pub(crate) status: String,
    pub(crate) input_json: Value,
    pub(crate) final_entry_id: Option<String>,
    pub(crate) stopped_reason: Option<String>,
    pub(crate) error_json: Option<Value>,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) started_at: Option<OffsetDateTime>,
    pub(crate) completed_at: Option<OffsetDateTime>,
    pub(crate) updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, AsChangeset)]
#[diesel(table_name = agent_runs)]
#[diesel(treat_none_as_null = true)]
pub(crate) struct SqlAgentRunFinalChanges {
    pub(crate) status: String,
    pub(crate) final_entry_id: String,
    pub(crate) stopped_reason: String,
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
    pub(crate) continuation_kind: Option<String>,
    pub(crate) provider_response_id: Option<String>,
    pub(crate) reasoning_context: Option<String>,
    pub(crate) continuation_expires_at: Option<OffsetDateTime>,
    pub(crate) continuation_invalidated_at: Option<OffsetDateTime>,
    pub(crate) continuation_error_json: Option<Value>,
    pub(crate) settings_snapshot_json: Value,
    pub(crate) error_json: Option<Value>,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) started_at: Option<OffsetDateTime>,
    pub(crate) completed_at: Option<OffsetDateTime>,
    pub(crate) updated_at: OffsetDateTime,
    pub(crate) pricing_snapshot_json: Option<Value>,
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
    pub(crate) continuation_kind: Option<String>,
    pub(crate) provider_response_id: Option<String>,
    pub(crate) reasoning_context: Option<String>,
    pub(crate) continuation_expires_at: Option<OffsetDateTime>,
    pub(crate) continuation_invalidated_at: Option<OffsetDateTime>,
    pub(crate) continuation_error_json: Option<Value>,
    pub(crate) settings_snapshot_json: Value,
    pub(crate) error_json: Option<Value>,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) started_at: Option<OffsetDateTime>,
    pub(crate) completed_at: Option<OffsetDateTime>,
    pub(crate) updated_at: OffsetDateTime,
    pub(crate) pricing_snapshot_json: Option<Value>,
}

#[derive(Debug, Clone, AsChangeset)]
#[diesel(table_name = provider_steps)]
#[diesel(treat_none_as_null = true)]
pub(crate) struct SqlProviderStepStatusChanges {
    pub(crate) status: String,
    pub(crate) response_snapshot_json: Option<Value>,
    pub(crate) state_snapshot_json: Option<Value>,
    pub(crate) continuation_kind: Option<String>,
    pub(crate) provider_response_id: Option<String>,
    pub(crate) reasoning_context: Option<String>,
    pub(crate) continuation_expires_at: Option<OffsetDateTime>,
    pub(crate) continuation_invalidated_at: Option<OffsetDateTime>,
    pub(crate) continuation_error_json: Option<Value>,
    pub(crate) error_json: Option<Value>,
    pub(crate) started_at: Option<OffsetDateTime>,
    pub(crate) completed_at: Option<OffsetDateTime>,
    pub(crate) updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, AsChangeset)]
#[diesel(table_name = provider_steps)]
#[diesel(treat_none_as_null = true)]
pub(crate) struct SqlProviderContinuationChanges {
    pub(crate) continuation_kind: Option<String>,
    pub(crate) provider_response_id: Option<String>,
    pub(crate) reasoning_context: Option<String>,
    pub(crate) continuation_expires_at: Option<OffsetDateTime>,
    pub(crate) continuation_invalidated_at: Option<OffsetDateTime>,
    pub(crate) continuation_error_json: Option<Value>,
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
    pub(crate) approval_json: Option<Value>,
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
    pub(crate) approval_json: Option<Value>,
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

#[derive(Debug, Clone, AsChangeset)]
#[diesel(table_name = tool_invocations)]
#[diesel(treat_none_as_null = true)]
pub(crate) struct SqlToolInvocationApprovalChanges {
    pub(crate) status: String,
    pub(crate) approval_json: Option<Value>,
    pub(crate) started_at: Option<OffsetDateTime>,
    pub(crate) completed_at: Option<OffsetDateTime>,
    pub(crate) updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, AsChangeset)]
#[diesel(table_name = tool_invocations)]
#[diesel(treat_none_as_null = true)]
pub(crate) struct SqlToolInvocationFullChanges {
    pub(crate) status: String,
    pub(crate) output_json: Option<Value>,
    pub(crate) error_json: Option<Value>,
    pub(crate) approval_json: Option<Value>,
    pub(crate) started_at: Option<OffsetDateTime>,
    pub(crate) completed_at: Option<OffsetDateTime>,
    pub(crate) updated_at: OffsetDateTime,
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
    pub(crate) cost_amount_nano_usd: Option<i64>,
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
    pub(crate) cost_amount_nano_usd: Option<i64>,
}

impl TryFrom<SqlAgentRunRow> for AgentRunRecord {
    type Error = DbError;

    fn try_from(row: SqlAgentRunRow) -> Result<Self> {
        let output = match (row.final_entry_id, row.stopped_reason) {
            (None, None) => None,
            (Some(final_entry_id), Some(stopped_reason)) => Some(AgentRunOutput {
                final_entry_id,
                stopped_reason: db_label_parse(stopped_reason)?,
            }),
            (Some(_), None) | (None, Some(_)) => {
                return Err(DbError::Invariant(
                    "agent run final entry and stopped reason must be both null or both set"
                        .to_string(),
                ));
            }
        };
        Ok(Self {
            id: row.id,
            conversation_id: row.conversation_id,
            trigger_entry_id: row.trigger_entry_id,
            trigger_kind: db_label_parse(row.trigger_kind)?,
            status: db_label_parse(row.status)?,
            input: from_json(row.input_json)?,
            output,
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
            continuation: provider_continuation(
                row.continuation_kind,
                row.provider_response_id,
                row.reasoning_context,
                row.continuation_expires_at,
                row.continuation_invalidated_at,
                row.continuation_error_json,
            )?,
            settings_snapshot: from_json(row.settings_snapshot_json)?,
            error: from_json_opt(row.error_json)?,
            created_at: row.created_at,
            started_at: row.started_at,
            completed_at: row.completed_at,
            updated_at: row.updated_at,
            pricing_snapshot: from_json_opt(row.pricing_snapshot_json)?,
        })
    }
}

fn provider_continuation(
    kind: Option<String>,
    response_id: Option<String>,
    reasoning_context: Option<String>,
    expires_at: Option<OffsetDateTime>,
    invalidated_at: Option<OffsetDateTime>,
    invalidation_error: Option<Value>,
) -> Result<Option<ProviderContinuationSnapshot>> {
    match (kind, response_id, reasoning_context, expires_at) {
        (None, None, None, None) => Ok(None),
        (Some(kind), Some(response_id), Some(reasoning_context), Some(expires_at)) => {
            let kind = match kind.as_str() {
                "openai_responses" => ProviderContinuationKind::OpenAiResponses,
                _ => {
                    return Err(DbError::Invariant(format!(
                        "unknown provider continuation kind `{kind}`"
                    )));
                }
            };
            Ok(Some(ProviderContinuationSnapshot {
                kind,
                response_id,
                reasoning_context,
                expires_at,
                invalidated_at,
                invalidation_error: from_json_opt(invalidation_error)?,
            }))
        }
        _ => Err(DbError::Invariant(
            "provider continuation columns must be all null or fully populated".to_string(),
        )),
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
            approval: from_json_opt(row.approval_json)?,
            created_at: row.created_at,
            started_at: row.started_at,
            completed_at: row.completed_at,
            updated_at: row.updated_at,
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
            cost_amount: row
                .cost_amount_nano_usd
                .map(|amount| {
                    let amount = u64::try_from(amount).map_err(|_| {
                        DbError::Invariant(
                            "usage event cost amount must be non-negative".to_string(),
                        )
                    })?;
                    UsdNanoAmount::new(amount)
                        .map_err(|error| DbError::Invariant(error.to_string()))
                })
                .transpose()?,
        })
    }
}

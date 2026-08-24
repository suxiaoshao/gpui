use super::*;

#[test]
fn fresh_schema_declares_structured_sqlite_types_and_checks() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let mut conn = store.pool().get().unwrap();

    let schema_migrations_sql = table_sql(&mut conn, "schema_migrations");
    assert!(schema_migrations_sql.contains("executed_at DateTime NOT NULL"));

    let providers_sql = table_sql(&mut conn, "providers");
    assert!(providers_sql.contains("enabled BOOLEAN NOT NULL DEFAULT 1"));
    assert!(providers_sql.contains("CHECK (enabled IN (0, 1))"));
    assert!(providers_sql.contains("created_at DateTime NOT NULL"));
    assert!(providers_sql.contains("updated_at DateTime NOT NULL"));

    let provider_models_sql = table_sql(&mut conn, "provider_models");
    assert!(provider_models_sql.contains("enabled BOOLEAN NOT NULL DEFAULT 1"));
    assert!(provider_models_sql.contains("CHECK (enabled IN (0, 1))"));
    assert!(provider_models_sql.contains("pricing_json JSON"));

    let agent_runs_sql = table_sql(&mut conn, "agent_runs");
    assert!(agent_runs_sql.contains(
        "status TEXT NOT NULL CHECK (status IN ('running', 'completed', 'failed', 'canceled'))"
    ));
    assert!(agent_runs_sql.contains("started_at DateTime"));
    assert!(agent_runs_sql.contains("completed_at DateTime"));

    let provider_steps_sql = table_sql(&mut conn, "provider_steps");
    assert!(provider_steps_sql.contains("pricing_snapshot_json JSON"));

    let usage_events_sql = table_sql(&mut conn, "usage_events");
    assert!(usage_events_sql.contains("cost_amount_nano_usd INTEGER"));
    assert!(
        usage_events_sql
            .contains("CHECK (cost_amount_nano_usd IS NULL OR cost_amount_nano_usd >= 0)")
    );

    let conversation_entries_sql = table_sql(&mut conn, "conversation_entries");
    assert!(conversation_entries_sql.contains(
        "kind TEXT NOT NULL CHECK (kind IN ('message', 'skill_activation', 'reasoning', 'tool_call', 'tool_result', 'approval_request', 'approval_decision', 'status', 'error'))"
    ));
    assert!(conversation_entries_sql.contains(
        "status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'completed', 'failed', 'canceled', 'waiting_for_approval'))"
    ));

    let tool_invocations_sql = table_sql(&mut conn, "tool_invocations");
    assert!(tool_invocations_sql.contains(
        "status TEXT NOT NULL CHECK (status IN ('requested', 'awaiting_approval', 'running', 'succeeded', 'failed', 'denied', 'canceled'))"
    ));
    assert!(tool_invocations_sql.contains("approval_json JSON"));
    assert!(
        store
            .repository()
            .table_names()
            .unwrap()
            .iter()
            .all(|name| name != "approval_decisions")
    );

    let conversation_columns = sql_query(
        "SELECT COUNT(*) AS value FROM pragma_table_info('conversations')
         WHERE name = 'recency_at' AND \"notnull\" = 1",
    )
    .load::<CountRow>(&mut conn)
    .unwrap()[0]
        .value;
    assert_eq!(conversation_columns, 1);
}

#[test]
fn conversation_recency_column_rejects_null_values() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let project = store
        .repository()
        .insert_project(project("null-recency"))
        .unwrap();
    let conversation = store
        .repository()
        .insert_conversation(conversation(&project))
        .unwrap();
    let mut conn = store.pool().get().unwrap();
    let error = sql_query("UPDATE conversations SET recency_at = NULL WHERE id = ?")
        .bind::<Text, _>(&conversation.id)
        .execute(&mut conn)
        .unwrap_err();
    assert!(error.to_string().contains("NOT NULL constraint failed"));
}

#[test]
fn provider_step_status_constraints_reject_invalid_lifecycle_shapes() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let project = repo
        .insert_project(project("provider-step-status-checks"))
        .unwrap();
    let conversation = repo.insert_conversation(conversation(&project)).unwrap();
    let provider = repo.insert_provider(provider()).unwrap();
    let model = repo
        .upsert_provider_model(provider_model(&provider.id, "gpt-5.6", "GPT-5.6"))
        .unwrap();
    let trigger = repo
        .append_conversation_entry(message_item(&conversation.id, "run"))
        .unwrap();
    let run = repo
        .insert_agent_run(NewAgentRun {
            conversation_id: conversation.id,
            trigger_entry_id: trigger.id.clone(),
            trigger_kind: AgentRunTriggerKind::User,
            input: agent_run_input(&trigger.id, &provider.id, &model.model_id),
        })
        .unwrap();
    let step = repo
        .insert_provider_step(NewProviderStep {
            agent_run_id: run.id,
            seq: 1,
            status: ProviderStepStatus::Running,
            request_snapshot: provider_step_request(&provider.id, &model.model_id, &trigger.id),
            response_snapshot: None,
            state_snapshot: None,
            settings_snapshot: run_settings(&provider.id, &model.model_id),
            error: None,
        })
        .unwrap();
    let mut conn = store.pool().get().unwrap();

    for invalid_update in [
        "UPDATE provider_steps SET status = 'queued' WHERE id = ?",
        "UPDATE provider_steps SET status = 'completed', completed_at = CURRENT_TIMESTAMP WHERE id = ?",
        "UPDATE provider_steps SET status = 'failed', completed_at = CURRENT_TIMESTAMP WHERE id = ?",
    ] {
        assert!(
            sql_query(invalid_update)
                .bind::<Text, _>(&step.id)
                .execute(&mut conn)
                .is_err()
        );
    }
}

#[test]
fn fresh_schema_rejects_invalid_boolean_and_closed_enum_values() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let project = repo.insert_project(project("checks")).unwrap();
    let conversation = repo.insert_conversation(conversation(&project)).unwrap();
    let user_item = repo
        .append_conversation_entry(message_item(&conversation.id, "hello"))
        .unwrap();
    let provider = repo.insert_provider(provider()).unwrap();
    let agent_run = repo
        .insert_agent_run(NewAgentRun {
            conversation_id: conversation.id.clone(),
            trigger_kind: AgentRunTriggerKind::User,
            trigger_entry_id: user_item.id.clone(),
            input: agent_run_input(&user_item.id, &provider.id, "gpt-5"),
        })
        .unwrap();

    let mut conn = store.pool().get().unwrap();
    assert!(
        sql_query(
            "INSERT INTO providers \
             (id, kind, display_name, enabled, settings_json, secret_refs_json, created_at, updated_at) \
             VALUES ('bad_provider', 'openai', 'Bad', 2, '{}', '{}', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"
        )
        .execute(&mut conn)
        .is_err()
    );
    assert!(
        sql_query(
            "INSERT INTO agent_runs \
             (id, conversation_id, trigger_entry_id, trigger_kind, status, input_json, created_at, updated_at) \
             VALUES ('bad_run', ?, ?, 'user', 'bogus', '{}', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"
        )
        .bind::<Text, _>(&conversation.id)
        .bind::<Text, _>(&user_item.id)
        .execute(&mut conn)
        .is_err()
    );
    assert!(
        sql_query(
            "INSERT INTO provider_steps \
             (id, agent_run_id, seq, provider_id, model_id, status, request_snapshot_json, settings_snapshot_json, created_at, updated_at) \
             VALUES ('bad_step', ?, 99, ?, 'gpt-5', 'bogus', '{}', '{}', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"
        )
        .bind::<Text, _>(&agent_run.id)
        .bind::<Text, _>(&provider.id)
        .execute(&mut conn)
        .is_err()
    );
    assert!(
        sql_query(
            "INSERT INTO tool_invocations \
             (id, agent_run_id, call_id, source, tool_name, runtime_tool_name, status, input_json, created_at, updated_at) \
             VALUES ('bad_tool', ?, 'call_bad', 'local', 'read_file', 'read_file', 'bogus', '{}', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"
        )
        .bind::<Text, _>(&agent_run.id)
        .execute(&mut conn)
        .is_err()
    );
    assert!(
        sql_query(
            "INSERT INTO conversation_entries \
             (id, conversation_id, seq, kind, status, payload_json, created_at, updated_at) \
             VALUES ('bad_item_kind', ?, 99, 'bogus', 'completed', '{}', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"
        )
        .bind::<Text, _>(&conversation.id)
        .execute(&mut conn)
        .is_err()
    );
    assert!(
        sql_query(
            "INSERT INTO conversation_entries \
             (id, conversation_id, seq, kind, status, payload_json, created_at, updated_at) \
             VALUES ('bad_item_status', ?, 100, 'message', 'bogus', '{}', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"
        )
        .bind::<Text, _>(&conversation.id)
        .execute(&mut conn)
        .is_err()
    );
}

#[test]
fn foreign_keys_transactions_and_cascades_are_enforced() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();

    let invalid = repo.insert_conversation(NewConversation {
        project_id: "missing".to_string(),
        title: "invalid".to_string(),
        pinned: false,
        prompt_id: None,
        default_provider_id: None,
        default_model_id: None,
        metadata: conversation_metadata(),
        settings_snapshot: conversation_settings(),
    });
    assert!(invalid.is_err());

    let project = repo.insert_project(project("fk")).unwrap();
    let conversation = repo.insert_conversation(conversation(&project)).unwrap();
    repo.append_conversation_entry(message_item(&conversation.id, "cascade probe"))
        .unwrap();

    let mut conn = store.pool().get().unwrap();
    sql_query("DELETE FROM projects WHERE id = ?")
        .bind::<diesel::sql_types::Text, _>(&project.id)
        .execute(&mut conn)
        .unwrap();
    assert!(repo.get_conversation(&conversation.id).unwrap().is_none());
    assert_eq!(count(&mut conn, "conversation_entries"), 0);
}

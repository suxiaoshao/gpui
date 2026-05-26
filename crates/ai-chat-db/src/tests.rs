use crate::{
    FreshStore, NewAgentRun, NewApprovalDecision, NewApprovalDecisionOutcome, NewAttachment,
    NewConversation, NewConversationItem, NewProject, NewPrompt, NewProvider, NewProviderModel,
    NewProviderStep, NewShortcut, NewToolInvocation, NewUsageEvent,
};
use ai_chat_core::*;
use diesel::{
    Connection, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Integer, Text},
};
use serde_json::json;
use std::{collections::HashSet, fs};
use tempfile::tempdir;

#[test]
fn creates_fresh_database_and_reads_internal_version() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_in_dir(dir.path()).unwrap();
    assert_eq!(store.path(), &dir.path().join(crate::DATABASE_FILE));

    let metadata = store.repository().metadata().unwrap();
    assert_eq!(metadata.schema_version, crate::repository::schema_version());
    assert_eq!(metadata.payload.store_kind, "fresh");
    assert_eq!(metadata.payload.legacy_policy, LegacyStorePolicy::Ignore);
}

#[test]
fn bootstrap_is_idempotent() {
    let dir = tempdir().unwrap();
    let path = dir.path().join(crate::DATABASE_FILE);
    let first = FreshStore::open(&path).unwrap();
    let first_updated_at = first.repository().metadata().unwrap().updated_at;

    let second = FreshStore::open(&path).unwrap();
    let metadata = second.repository().metadata().unwrap();
    assert_eq!(metadata.schema_version, crate::repository::schema_version());
    assert!(metadata.updated_at >= first_updated_at);

    let mut conn = second.pool().get().unwrap();
    assert_eq!(count(&mut conn, "schema_migrations"), 1);
}

#[test]
fn pooled_connections_configure_sqlite_busy_timeout() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_in_dir(dir.path()).unwrap();
    let mut conn = store.pool().get().unwrap();

    assert_eq!(
        busy_timeout(&mut conn),
        crate::store::SQLITE_BUSY_TIMEOUT_MS
    );
}

#[test]
fn bootstrap_rejects_newer_schema_without_downgrading_metadata() {
    let dir = tempdir().unwrap();
    let path = dir.path().join(crate::DATABASE_FILE);
    FreshStore::open(&path).unwrap();

    let newer_version = crate::repository::schema_version() + 1;
    let mut conn = SqliteConnection::establish(path.to_str().unwrap()).unwrap();
    sql_query("UPDATE schema_metadata SET schema_version = ? WHERE id = 'default'")
        .bind::<Integer, _>(newer_version)
        .execute(&mut conn)
        .unwrap();

    let err = FreshStore::open(&path).unwrap_err();
    assert!(err.to_string().contains("newer than supported"));

    let stored_version =
        sql_query("SELECT schema_version AS value FROM schema_metadata WHERE id = 'default'")
            .load::<IntRow>(&mut conn)
            .unwrap()[0]
            .value;
    assert_eq!(stored_version, newer_version);
}

#[test]
fn failed_migration_rolls_back_partial_schema() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("broken.sqlite3");
    let migration = crate::migrations::broken_migration_for_test();
    let err = FreshStore::open_with_migrations(&path, &[migration]).unwrap_err();
    assert!(err.to_string().contains("database query failed"));

    let mut conn = SqliteConnection::establish(path.to_str().unwrap()).unwrap();
    assert_eq!(count(&mut conn, "broken_rollback_probe"), 0);
    assert_eq!(count(&mut conn, "schema_migrations"), 0);
}

#[test]
fn empty_first_run_has_no_user_data_or_source_tables() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_in_dir(dir.path()).unwrap();
    let mut conn = store.pool().get().unwrap();

    assert_eq!(count(&mut conn, "projects"), 0);
    assert_eq!(count(&mut conn, "conversations"), 0);

    let tables: HashSet<_> = store
        .repository()
        .table_names()
        .unwrap()
        .into_iter()
        .collect();
    for disallowed in [
        "skills",
        "skill_roots",
        "mcp_servers",
        "mcp_tools",
        "conversation_item_fts",
    ] {
        assert!(!tables.contains(disallowed));
    }
}

#[test]
fn fresh_schema_declares_structured_sqlite_types_and_checks() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_in_dir(dir.path()).unwrap();
    let mut conn = store.pool().get().unwrap();

    let schema_migrations_sql = table_sql(&mut conn, "schema_migrations");
    assert!(schema_migrations_sql.contains("executed_at DateTime NOT NULL"));

    let providers_sql = table_sql(&mut conn, "providers");
    assert!(providers_sql.contains("enabled BOOLEAN NOT NULL DEFAULT 1"));
    assert!(providers_sql.contains("CHECK (enabled IN (0, 1))"));
    assert!(providers_sql.contains("created_at DateTime NOT NULL"));
    assert!(providers_sql.contains("updated_at DateTime NOT NULL"));

    let agent_runs_sql = table_sql(&mut conn, "agent_runs");
    assert!(agent_runs_sql.contains(
        "status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'waiting_for_approval', 'completed', 'failed', 'canceled'))"
    ));
    assert!(agent_runs_sql.contains("started_at DateTime"));
    assert!(agent_runs_sql.contains("completed_at DateTime"));

    let conversation_items_sql = table_sql(&mut conn, "conversation_items");
    assert!(conversation_items_sql.contains(
        "kind TEXT NOT NULL CHECK (kind IN ('message', 'skill_activation', 'reasoning', 'tool_call', 'tool_result', 'approval_request', 'approval_decision', 'status', 'error'))"
    ));
    assert!(conversation_items_sql.contains(
        "status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'completed', 'failed', 'canceled', 'waiting_for_approval'))"
    ));

    let tool_invocations_sql = table_sql(&mut conn, "tool_invocations");
    assert!(tool_invocations_sql.contains(
        "status TEXT NOT NULL CHECK (status IN ('requested', 'awaiting_approval', 'running', 'succeeded', 'failed', 'denied', 'canceled'))"
    ));
}

#[test]
fn fresh_schema_rejects_invalid_boolean_and_closed_enum_values() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_in_dir(dir.path()).unwrap();
    let repo = store.repository();
    let project = repo.insert_project(project("checks")).unwrap();
    let conversation = repo.insert_conversation(conversation(&project)).unwrap();
    let user_item = repo
        .append_conversation_item(message_item(&conversation.id, "hello"))
        .unwrap();
    let provider = repo.insert_provider(provider()).unwrap();
    let agent_run = repo
        .insert_agent_run(NewAgentRun {
            trigger_kind: AgentRunTriggerKind::User,
            status: AgentRunStatus::Running,
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
             (id, conversation_id, trigger_kind, status, input_json, created_at, updated_at) \
             VALUES ('bad_run', ?, 'user', 'bogus', '{}', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"
        )
        .bind::<Text, _>(&conversation.id)
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
            "INSERT INTO conversation_items \
             (id, conversation_id, seq, kind, status, payload_json, created_at, updated_at) \
             VALUES ('bad_item_kind', ?, 99, 'bogus', 'completed', '{}', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"
        )
        .bind::<Text, _>(&conversation.id)
        .execute(&mut conn)
        .is_err()
    );
    assert!(
        sql_query(
            "INSERT INTO conversation_items \
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
    let store = FreshStore::open_in_dir(dir.path()).unwrap();
    let repo = store.repository();

    let invalid = repo.insert_conversation(NewConversation {
        project_id: "missing".to_string(),
        title: "invalid".to_string(),
        prompt_id: None,
        default_provider_id: None,
        default_model_id: None,
        metadata: conversation_metadata(),
        settings_snapshot: conversation_settings(),
    });
    assert!(invalid.is_err());

    let project = repo.insert_project(project("fk")).unwrap();
    let conversation = repo.insert_conversation(conversation(&project)).unwrap();
    repo.append_conversation_item(message_item(&conversation.id, "cascade probe"))
        .unwrap();

    let mut conn = store.pool().get().unwrap();
    sql_query("DELETE FROM projects WHERE id = ?")
        .bind::<diesel::sql_types::Text, _>(&project.id)
        .execute(&mut conn)
        .unwrap();
    assert!(repo.get_conversation(&conversation.id).unwrap().is_none());
    assert_eq!(count(&mut conn, "conversation_items"), 0);
}

#[test]
fn append_items_updates_order_last_seq_and_search_text() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_in_dir(dir.path()).unwrap();
    let repo = store.repository();
    let project = repo.insert_project(project("items")).unwrap();
    let conversation = repo.insert_conversation(conversation(&project)).unwrap();

    let first = repo
        .append_conversation_item(message_item(&conversation.id, "hello alpha"))
        .unwrap();
    let second = repo
        .append_conversation_item(message_item(&conversation.id, "hello beta"))
        .unwrap();
    assert_eq!((first.seq, second.seq), (1, 2));

    let conversation = repo.get_conversation(&conversation.id).unwrap().unwrap();
    assert_eq!(conversation.last_item_seq, 2);
    let items = repo.conversation_items(&conversation.id).unwrap();
    assert_eq!(
        items.iter().map(|item| item.seq).collect::<Vec<_>>(),
        [1, 2]
    );

    assert_eq!(first.search_text, "hello alpha");

    repo.update_conversation_item_payload(
        &first.id,
        ConversationItemStatus::Completed,
        ConversationItemPayload::Message {
            role: TranscriptRole::User,
            content: vec![ContentPart::Text {
                text: "gamma".to_string(),
            }],
        },
    )
    .unwrap();
    let updated = repo.conversation_items(&conversation.id).unwrap();
    assert_eq!(updated[0].search_text, "gamma");

    let remaining = repo.conversation_items(&conversation.id).unwrap();
    assert_eq!(
        remaining
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        [first.id.as_str(), second.id.as_str()]
    );
}

#[test]
fn update_item_payload_bumps_parent_conversation_timestamp() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_in_dir(dir.path()).unwrap();
    let repo = store.repository();
    let project = repo.insert_project(project("item-update")).unwrap();
    let conversation = repo.insert_conversation(conversation(&project)).unwrap();
    let item = repo
        .append_conversation_item(message_item(&conversation.id, "before"))
        .unwrap();

    let updated = repo
        .update_conversation_item_payload(
            &item.id,
            ConversationItemStatus::Completed,
            ConversationItemPayload::Message {
                role: TranscriptRole::Assistant,
                content: vec![ContentPart::Text {
                    text: "after".to_string(),
                }],
            },
        )
        .unwrap();
    let parent = repo.get_conversation(&conversation.id).unwrap().unwrap();

    assert!(updated.updated_at >= item.updated_at);
    assert_eq!(parent.updated_at, updated.updated_at);
}

#[test]
fn append_item_rejects_cross_conversation_execution_links() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_in_dir(dir.path()).unwrap();
    let repo = store.repository();
    let project = repo.insert_project(project("execution-links")).unwrap();
    let conversation_a = repo.insert_conversation(conversation(&project)).unwrap();
    let conversation_b = repo.insert_conversation(conversation(&project)).unwrap();
    let provider = repo.insert_provider(provider()).unwrap();
    let model = repo
        .upsert_provider_model(provider_model(&provider.id, "gpt-5.2", "GPT-5.2"))
        .unwrap();
    let user_item = repo
        .append_conversation_item(message_item(&conversation_a.id, "run input"))
        .unwrap();
    let agent_run = repo
        .insert_agent_run(NewAgentRun {
            trigger_kind: AgentRunTriggerKind::User,
            status: AgentRunStatus::Running,
            input: agent_run_input(&user_item.id, &provider.id, &model.model_id),
        })
        .unwrap();
    let provider_step = repo
        .insert_provider_step(NewProviderStep {
            agent_run_id: agent_run.id.clone(),
            seq: 1,
            status: ProviderStepStatus::Completed,
            request_snapshot: provider_step_request(&provider.id, &model.model_id, &user_item.id),
            response_snapshot: None,
            state_snapshot: None,
            settings_snapshot: run_settings(&provider.id, &model.model_id),
            error: None,
        })
        .unwrap();
    let tool = repo
        .insert_tool_invocation(NewToolInvocation {
            agent_run_id: agent_run.id.clone(),
            provider_step_id: Some(provider_step.id.clone()),
            status: ToolInvocationStatus::Succeeded,
            input: tool_input(),
            output: Some(tool_output()),
            error: None,
        })
        .unwrap();

    let mut same_conversation = message_item(&conversation_a.id, "linked ok");
    same_conversation.agent_run_id = Some(agent_run.id.clone());
    same_conversation.provider_step_id = Some(provider_step.id.clone());
    same_conversation.tool_invocation_id = Some(tool.id.clone());
    repo.append_conversation_item(same_conversation).unwrap();

    let mut cross_agent = message_item(&conversation_b.id, "cross agent");
    cross_agent.agent_run_id = Some(agent_run.id.clone());
    assert!(repo.append_conversation_item(cross_agent).is_err());

    let mut cross_step = message_item(&conversation_b.id, "cross step");
    cross_step.provider_step_id = Some(provider_step.id.clone());
    assert!(repo.append_conversation_item(cross_step).is_err());

    let mut cross_tool = message_item(&conversation_b.id, "cross tool");
    cross_tool.tool_invocation_id = Some(tool.id.clone());
    assert!(repo.append_conversation_item(cross_tool).is_err());

    let second_run = repo
        .insert_agent_run(NewAgentRun {
            trigger_kind: AgentRunTriggerKind::Retry,
            status: AgentRunStatus::Running,
            input: agent_run_input(&user_item.id, &provider.id, &model.model_id),
        })
        .unwrap();
    let mut mismatched_chain = message_item(&conversation_a.id, "mismatched chain");
    mismatched_chain.agent_run_id = Some(second_run.id);
    mismatched_chain.provider_step_id = Some(provider_step.id);
    assert!(repo.append_conversation_item(mismatched_chain).is_err());
}

#[test]
fn insert_agent_run_derives_conversation_and_rejects_invalid_user_item() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_in_dir(dir.path()).unwrap();
    let repo = store.repository();
    let project = repo.insert_project(project("agent-run-input")).unwrap();
    let conversation = repo.insert_conversation(conversation(&project)).unwrap();
    let provider = repo.insert_provider(provider()).unwrap();
    let model = repo
        .upsert_provider_model(provider_model(&provider.id, "gpt-5.2", "GPT-5.2"))
        .unwrap();
    let user_item = repo
        .append_conversation_item(message_item(&conversation.id, "run input"))
        .unwrap();
    let assistant_item = repo
        .append_conversation_item(message_item_with_role(
            &conversation.id,
            TranscriptRole::Assistant,
            "assistant output",
        ))
        .unwrap();

    let valid = repo.insert_agent_run(NewAgentRun {
        trigger_kind: AgentRunTriggerKind::User,
        status: AgentRunStatus::Running,
        input: agent_run_input(&user_item.id, &provider.id, &model.model_id),
    });
    assert_eq!(valid.unwrap().conversation_id, conversation.id);

    let missing_item = repo.insert_agent_run(NewAgentRun {
        trigger_kind: AgentRunTriggerKind::User,
        status: AgentRunStatus::Running,
        input: agent_run_input("missing-item", &provider.id, &model.model_id),
    });
    assert!(missing_item.is_err());

    let non_user_item = repo.insert_agent_run(NewAgentRun {
        trigger_kind: AgentRunTriggerKind::User,
        status: AgentRunStatus::Running,
        input: agent_run_input(&assistant_item.id, &provider.id, &model.model_id),
    });
    assert!(non_user_item.is_err());
}

#[test]
fn insert_tool_invocation_rejects_provider_step_from_other_run() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_in_dir(dir.path()).unwrap();
    let repo = store.repository();
    let project = repo.insert_project(project("tool-step-link")).unwrap();
    let conversation = repo.insert_conversation(conversation(&project)).unwrap();
    let provider = repo.insert_provider(provider()).unwrap();
    let model = repo
        .upsert_provider_model(provider_model(&provider.id, "gpt-5.2", "GPT-5.2"))
        .unwrap();
    let first_item = repo
        .append_conversation_item(message_item(&conversation.id, "first run"))
        .unwrap();
    let second_item = repo
        .append_conversation_item(message_item(&conversation.id, "second run"))
        .unwrap();
    let first_run = repo
        .insert_agent_run(NewAgentRun {
            trigger_kind: AgentRunTriggerKind::User,
            status: AgentRunStatus::Running,
            input: agent_run_input(&first_item.id, &provider.id, &model.model_id),
        })
        .unwrap();
    let second_run = repo
        .insert_agent_run(NewAgentRun {
            trigger_kind: AgentRunTriggerKind::Retry,
            status: AgentRunStatus::Running,
            input: agent_run_input(&second_item.id, &provider.id, &model.model_id),
        })
        .unwrap();
    let first_step = repo
        .insert_provider_step(NewProviderStep {
            agent_run_id: first_run.id.clone(),
            seq: 1,
            status: ProviderStepStatus::Completed,
            request_snapshot: provider_step_request(&provider.id, &model.model_id, &first_item.id),
            response_snapshot: None,
            state_snapshot: None,
            settings_snapshot: run_settings(&provider.id, &model.model_id),
            error: None,
        })
        .unwrap();

    let mismatched = repo.insert_tool_invocation(NewToolInvocation {
        agent_run_id: second_run.id,
        provider_step_id: Some(first_step.id),
        status: ToolInvocationStatus::Succeeded,
        input: tool_input(),
        output: Some(tool_output()),
        error: None,
    });
    assert!(mismatched.is_err());
}

#[test]
fn usage_event_derives_dimensions_from_provider_step() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_in_dir(dir.path()).unwrap();
    let repo = store.repository();
    let project = repo.insert_project(project("usage-dimensions")).unwrap();
    let conversation = repo.insert_conversation(conversation(&project)).unwrap();
    let provider = repo.insert_provider(provider()).unwrap();
    let model = repo
        .upsert_provider_model(provider_model(&provider.id, "gpt-5.2", "GPT-5.2"))
        .unwrap();
    let user_item = repo
        .append_conversation_item(message_item(&conversation.id, "usage input"))
        .unwrap();
    let agent_run = repo
        .insert_agent_run(NewAgentRun {
            trigger_kind: AgentRunTriggerKind::User,
            status: AgentRunStatus::Running,
            input: agent_run_input(&user_item.id, &provider.id, &model.model_id),
        })
        .unwrap();
    let provider_step = repo
        .insert_provider_step(NewProviderStep {
            agent_run_id: agent_run.id,
            seq: 1,
            status: ProviderStepStatus::Completed,
            request_snapshot: provider_step_request(&provider.id, &model.model_id, &user_item.id),
            response_snapshot: None,
            state_snapshot: None,
            settings_snapshot: run_settings(&provider.id, &model.model_id),
            error: None,
        })
        .unwrap();

    let usage = repo
        .insert_usage_event(NewUsageEvent {
            provider_step_id: provider_step.id,
            date_key: "2026-05-24".to_string(),
            usage: usage_snapshot(),
        })
        .unwrap();

    assert_eq!(usage.conversation_id, conversation.id);
    assert_eq!(usage.provider_id, provider.id);
    assert_eq!(usage.model_id, model.model_id);
}

#[test]
fn provider_step_derives_dimensions_from_request_snapshot() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_in_dir(dir.path()).unwrap();
    let repo = store.repository();
    let project = repo
        .insert_project(project("provider-step-dimensions"))
        .unwrap();
    let conversation = repo.insert_conversation(conversation(&project)).unwrap();
    let provider = repo.insert_provider(provider()).unwrap();
    let model = repo
        .upsert_provider_model(provider_model(&provider.id, "gpt-5.2", "GPT-5.2"))
        .unwrap();
    let user_item = repo
        .append_conversation_item(message_item(&conversation.id, "step input"))
        .unwrap();
    let agent_run = repo
        .insert_agent_run(NewAgentRun {
            trigger_kind: AgentRunTriggerKind::User,
            status: AgentRunStatus::Running,
            input: agent_run_input(&user_item.id, &provider.id, &model.model_id),
        })
        .unwrap();

    let provider_step = repo
        .insert_provider_step(NewProviderStep {
            agent_run_id: agent_run.id.clone(),
            seq: 1,
            status: ProviderStepStatus::Completed,
            request_snapshot: provider_step_request(&provider.id, &model.model_id, &user_item.id),
            response_snapshot: None,
            state_snapshot: None,
            settings_snapshot: run_settings(&provider.id, &model.model_id),
            error: None,
        })
        .unwrap();
    assert_eq!(provider_step.provider_id, provider.id);
    assert_eq!(provider_step.model_id, model.model_id);

    let bad_settings = repo.insert_provider_step(NewProviderStep {
        agent_run_id: agent_run.id.clone(),
        seq: 2,
        status: ProviderStepStatus::Completed,
        request_snapshot: provider_step_request(&provider.id, &model.model_id, &user_item.id),
        response_snapshot: None,
        state_snapshot: None,
        settings_snapshot: run_settings("other-provider", &model.model_id),
        error: None,
    });
    assert!(bad_settings.is_err());

    let bad_settings_model = repo.insert_provider_step(NewProviderStep {
        agent_run_id: agent_run.id.clone(),
        seq: 2,
        status: ProviderStepStatus::Completed,
        request_snapshot: provider_step_request(&provider.id, &model.model_id, &user_item.id),
        response_snapshot: None,
        state_snapshot: None,
        settings_snapshot: run_settings(&provider.id, "other-model"),
        error: None,
    });
    assert!(bad_settings_model.is_err());

    let bad_state = repo.insert_provider_step(NewProviderStep {
        agent_run_id: agent_run.id,
        seq: 2,
        status: ProviderStepStatus::Completed,
        request_snapshot: provider_step_request(&provider.id, &model.model_id, &user_item.id),
        response_snapshot: None,
        state_snapshot: Some(provider_run_state("other-provider")),
        settings_snapshot: run_settings(&provider.id, &model.model_id),
        error: None,
    });
    assert!(bad_state.is_err());
}

#[test]
fn provider_step_validates_input_item_ownership() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_in_dir(dir.path()).unwrap();
    let repo = store.repository();
    let project = repo
        .insert_project(project("provider-step-input-items"))
        .unwrap();
    let primary_conversation = repo.insert_conversation(conversation(&project)).unwrap();
    let other_conversation = repo.insert_conversation(conversation(&project)).unwrap();
    let provider = repo.insert_provider(provider()).unwrap();
    let model = repo
        .upsert_provider_model(provider_model(&provider.id, "gpt-5.2", "GPT-5.2"))
        .unwrap();
    let user_item = repo
        .append_conversation_item(message_item(&primary_conversation.id, "step input"))
        .unwrap();
    let context_item = repo
        .append_conversation_item(message_item(
            &primary_conversation.id,
            "same conversation context",
        ))
        .unwrap();
    let other_item = repo
        .append_conversation_item(message_item(&other_conversation.id, "other context"))
        .unwrap();
    let agent_run = repo
        .insert_agent_run(NewAgentRun {
            trigger_kind: AgentRunTriggerKind::User,
            status: AgentRunStatus::Running,
            input: agent_run_input(&user_item.id, &provider.id, &model.model_id),
        })
        .unwrap();

    let mut same_conversation_request =
        provider_step_request(&provider.id, &model.model_id, &user_item.id);
    same_conversation_request
        .input_item_ids
        .push(context_item.id.clone());
    let provider_step = repo
        .insert_provider_step(NewProviderStep {
            agent_run_id: agent_run.id.clone(),
            seq: 1,
            status: ProviderStepStatus::Completed,
            request_snapshot: same_conversation_request,
            response_snapshot: None,
            state_snapshot: None,
            settings_snapshot: run_settings(&provider.id, &model.model_id),
            error: None,
        })
        .unwrap();
    assert_eq!(
        provider_step.request_snapshot.input_item_ids,
        [user_item.id.clone(), context_item.id]
    );

    let mut missing_request = provider_step_request(&provider.id, &model.model_id, &user_item.id);
    missing_request.input_item_ids = vec!["missing-item".to_string()];
    let missing_item = repo.insert_provider_step(NewProviderStep {
        agent_run_id: agent_run.id.clone(),
        seq: 2,
        status: ProviderStepStatus::Completed,
        request_snapshot: missing_request,
        response_snapshot: None,
        state_snapshot: None,
        settings_snapshot: run_settings(&provider.id, &model.model_id),
        error: None,
    });
    assert!(missing_item.is_err());

    let mut cross_conversation_request =
        provider_step_request(&provider.id, &model.model_id, &user_item.id);
    cross_conversation_request.input_item_ids = vec![other_item.id];
    let cross_conversation = repo.insert_provider_step(NewProviderStep {
        agent_run_id: agent_run.id,
        seq: 2,
        status: ProviderStepStatus::Completed,
        request_snapshot: cross_conversation_request,
        response_snapshot: None,
        state_snapshot: None,
        settings_snapshot: run_settings(&provider.id, &model.model_id),
        error: None,
    });
    assert!(cross_conversation.is_err());
}

#[test]
fn approval_outcome_derives_status_and_decision_columns() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_in_dir(dir.path()).unwrap();
    let repo = store.repository();
    let project = repo.insert_project(project("approval-outcome")).unwrap();
    let conversation = repo.insert_conversation(conversation(&project)).unwrap();
    let provider = repo.insert_provider(provider()).unwrap();
    let model = repo
        .upsert_provider_model(provider_model(&provider.id, "gpt-5.2", "GPT-5.2"))
        .unwrap();
    let user_item = repo
        .append_conversation_item(message_item(&conversation.id, "approval input"))
        .unwrap();
    let agent_run = repo
        .insert_agent_run(NewAgentRun {
            trigger_kind: AgentRunTriggerKind::User,
            status: AgentRunStatus::Running,
            input: agent_run_input(&user_item.id, &provider.id, &model.model_id),
        })
        .unwrap();
    let provider_step = repo
        .insert_provider_step(NewProviderStep {
            agent_run_id: agent_run.id.clone(),
            seq: 1,
            status: ProviderStepStatus::Completed,
            request_snapshot: provider_step_request(&provider.id, &model.model_id, &user_item.id),
            response_snapshot: None,
            state_snapshot: None,
            settings_snapshot: run_settings(&provider.id, &model.model_id),
            error: None,
        })
        .unwrap();
    let pending_tool = repo
        .insert_tool_invocation(NewToolInvocation {
            agent_run_id: agent_run.id.clone(),
            provider_step_id: Some(provider_step.id.clone()),
            status: ToolInvocationStatus::Running,
            input: tool_input(),
            output: None,
            error: None,
        })
        .unwrap();
    let denied_tool = repo
        .insert_tool_invocation(NewToolInvocation {
            agent_run_id: agent_run.id,
            provider_step_id: Some(provider_step.id),
            status: ToolInvocationStatus::Denied,
            input: tool_input(),
            output: None,
            error: None,
        })
        .unwrap();

    let pending = repo
        .insert_approval_decision(NewApprovalDecision {
            tool_invocation_id: pending_tool.id,
            request: approval_request(),
            outcome: NewApprovalDecisionOutcome::Pending { expires_at: None },
        })
        .unwrap();
    assert_eq!(pending.status, ApprovalStatus::Pending);
    assert!(pending.decision.is_none());
    assert!(pending.decided_at.is_none());

    let denied = repo
        .insert_approval_decision(NewApprovalDecision {
            tool_invocation_id: denied_tool.id,
            request: approval_request(),
            outcome: NewApprovalDecisionOutcome::Denied {
                decided_by: "user".to_string(),
                reason: Some("no".to_string()),
            },
        })
        .unwrap();
    assert_eq!(denied.status, ApprovalStatus::Denied);
    assert_eq!(
        denied.decision,
        Some(ApprovalDecisionPayload {
            approved: false,
            decided_by: "user".to_string(),
            reason: Some("no".to_string()),
        })
    );
    assert!(denied.decided_at.is_some());
}

#[test]
fn typed_json_roundtrips_for_repository_records() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_in_dir(dir.path()).unwrap();
    let repo = store.repository();

    let project = repo.insert_project(project("json")).unwrap();
    assert_eq!(project.metadata, project_metadata());

    let provider = repo.insert_provider(provider()).unwrap();
    assert_eq!(provider.settings, provider_settings());
    assert_eq!(provider.secret_refs, provider_secret_refs());

    let model = repo
        .upsert_provider_model(provider_model(&provider.id, "gpt-5.2", "GPT-5.2"))
        .unwrap();
    assert_eq!(model.capabilities, model_capabilities());
    assert_eq!(model.metadata, provider_model_metadata("GPT-5.2"));

    let prompt = repo.insert_prompt(prompt()).unwrap();
    assert_eq!(prompt.content, prompt_content());

    let conversation = repo
        .insert_conversation(NewConversation {
            project_id: project.id.clone(),
            title: "JSON".to_string(),
            prompt_id: Some(prompt.id.clone()),
            default_provider_id: Some(provider.id.clone()),
            default_model_id: Some(model.model_id.clone()),
            metadata: conversation_metadata(),
            settings_snapshot: conversation_settings(),
        })
        .unwrap();
    assert_eq!(conversation.metadata, conversation_metadata());
    assert_eq!(conversation.settings_snapshot, conversation_settings());

    let user_item = repo
        .append_conversation_item(message_item(&conversation.id, "hello json"))
        .unwrap();
    assert!(matches!(
        user_item.payload,
        ConversationItemPayload::Message {
            role: TranscriptRole::User,
            ..
        }
    ));

    let attachment = repo
        .insert_attachment(NewAttachment {
            conversation_id: conversation.id.clone(),
            kind: AttachmentKind::File,
            storage_kind: AttachmentStorageKind::LocalFile,
            mime_type: Some("text/plain".to_string()),
            name: Some("notes.txt".to_string()),
            path: Some("/tmp/notes.txt".to_string()),
            external_uri: None,
            provider_id: Some(provider.id.clone()),
            provider_file_id: None,
            sha256: Some("hash".to_string()),
            size_bytes: Some(42),
            metadata: attachment_metadata(),
        })
        .unwrap();
    assert_eq!(attachment.metadata, attachment_metadata());

    let agent_run = repo
        .insert_agent_run(NewAgentRun {
            trigger_kind: AgentRunTriggerKind::User,
            status: AgentRunStatus::Running,
            input: agent_run_input(&user_item.id, &provider.id, &model.model_id),
        })
        .unwrap();
    assert_eq!(
        agent_run.input.runtime_snapshot.engine,
        AgentEngineKind::Rig
    );

    let provider_step = repo
        .insert_provider_step(NewProviderStep {
            agent_run_id: agent_run.id.clone(),
            seq: 1,
            status: ProviderStepStatus::Completed,
            request_snapshot: provider_step_request(&provider.id, &model.model_id, &user_item.id),
            response_snapshot: Some(provider_step_response()),
            state_snapshot: Some(provider_run_state(&provider.id)),
            settings_snapshot: run_settings(&provider.id, &model.model_id),
            error: Some(run_error()),
        })
        .unwrap();
    assert_eq!(
        provider_step.request_snapshot.snapshot_kind,
        ProviderStepSnapshotKind::RigCompletionRequest
    );
    assert_eq!(provider_step.error, Some(run_error()));

    let tool = repo
        .insert_tool_invocation(NewToolInvocation {
            agent_run_id: agent_run.id.clone(),
            provider_step_id: Some(provider_step.id.clone()),
            status: ToolInvocationStatus::Succeeded,
            input: tool_input(),
            output: Some(tool_output()),
            error: None,
        })
        .unwrap();
    assert_eq!(tool.input.runtime_tool_name, "filesystem__read_file");
    assert_eq!(tool.output, Some(tool_output()));

    let approval = repo
        .insert_approval_decision(NewApprovalDecision {
            tool_invocation_id: tool.id.clone(),
            request: approval_request(),
            outcome: NewApprovalDecisionOutcome::Approved {
                decided_by: "user".to_string(),
                reason: Some("ok".to_string()),
            },
        })
        .unwrap();
    assert_eq!(approval.request, approval_request());
    assert_eq!(approval.status, ApprovalStatus::Approved);
    assert_eq!(approval.decision, Some(approval_decision()));

    let usage = repo
        .insert_usage_event(NewUsageEvent {
            provider_step_id: provider_step.id.clone(),
            date_key: "2026-05-24".to_string(),
            usage: usage_snapshot(),
        })
        .unwrap();
    assert_eq!(usage.usage, usage_snapshot());
    assert_eq!(usage.conversation_id, conversation.id);
    assert_eq!(usage.provider_id.as_str(), provider.id.as_str());
    assert_eq!(usage.model_id.as_str(), model.model_id.as_str());

    let shortcut = repo
        .insert_shortcut(NewShortcut {
            hotkey: "cmd+shift+j".to_string(),
            enabled: true,
            prompt_id: Some(prompt.id.clone()),
            provider_id: Some(provider.id.clone()),
            model_id: Some(model.model_id.clone()),
            input_source: ShortcutInputSource::SelectionOrClipboard,
            action: ShortcutAction::OpenTemporaryConversation,
            settings_snapshot: run_settings(&provider.id, &model.model_id),
        })
        .unwrap();
    assert_eq!(shortcut.action, ShortcutAction::OpenTemporaryConversation);

    let app_settings = repo
        .set_app_settings(AppSettingsPayload {
            language: Some("zh-CN".to_string()),
            theme: Some("system".to_string()),
            default_project_id: Some(project.id.clone()),
        })
        .unwrap();
    assert_eq!(app_settings.settings.default_project_id, Some(project.id));
}

#[test]
fn provider_model_manual_refresh_updates_cached_row() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_in_dir(dir.path()).unwrap();
    let repo = store.repository();
    let provider = repo.insert_provider(provider()).unwrap();

    let first = repo
        .upsert_provider_model(provider_model(&provider.id, "gpt-5.2", "Old"))
        .unwrap();
    let second = repo
        .upsert_provider_model(provider_model(&provider.id, "gpt-5.2", "New"))
        .unwrap();

    assert_eq!(first.id, second.id);
    assert_eq!(second.display_name.as_deref(), Some("New"));
    assert_eq!(
        repo.get_provider_model(&provider.id, "gpt-5.2")
            .unwrap()
            .unwrap()
            .metadata
            .display_name
            .as_deref(),
        Some("New")
    );
}

#[test]
fn legacy_store_files_coexist_with_fresh_database() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("history.sqlite3"), "legacy-v1").unwrap();
    fs::write(dir.path().join("history_v6.sqlite3"), "legacy-v6").unwrap();

    let store = FreshStore::open_in_dir(dir.path()).unwrap();

    assert!(store.path().exists());
    assert_eq!(
        fs::read_to_string(dir.path().join("history.sqlite3")).unwrap(),
        "legacy-v1"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("history_v6.sqlite3")).unwrap(),
        "legacy-v6"
    );
}

fn count(conn: &mut SqliteConnection, table: &str) -> i64 {
    let sql = format!(
        "SELECT COUNT(*) AS value FROM sqlite_master WHERE type IN ('table', 'view') AND name = '{table}'"
    );
    let exists = sql_query(sql).load::<CountRow>(conn).unwrap()[0].value;
    if exists == 0 {
        return 0;
    }
    let sql = format!("SELECT COUNT(*) AS value FROM {table}");
    sql_query(sql).load::<CountRow>(conn).unwrap()[0].value
}

fn busy_timeout(conn: &mut SqliteConnection) -> i32 {
    sql_query("PRAGMA busy_timeout")
        .load::<BusyTimeoutRow>(conn)
        .unwrap()[0]
        .timeout
}

fn table_sql(conn: &mut SqliteConnection, table: &str) -> String {
    sql_query("SELECT sql AS value FROM sqlite_master WHERE type = 'table' AND name = ?")
        .bind::<Text, _>(table)
        .load::<TextRow>(conn)
        .unwrap()[0]
        .value
        .clone()
}

#[derive(diesel::QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

#[derive(diesel::QueryableByName)]
struct BusyTimeoutRow {
    #[diesel(sql_type = Integer)]
    timeout: i32,
}

#[derive(diesel::QueryableByName)]
struct IntRow {
    #[diesel(sql_type = Integer)]
    value: i32,
}

#[derive(diesel::QueryableByName)]
struct TextRow {
    #[diesel(sql_type = Text)]
    value: String,
}

fn project(suffix: &str) -> NewProject {
    NewProject {
        path: format!("/tmp/ai-chat-{suffix}"),
        display_name: format!("Project {suffix}"),
        kind: ProjectKind::Normal,
        metadata: project_metadata(),
    }
}

fn project_metadata() -> ProjectMetadata {
    ProjectMetadata {
        scratch_reason: None,
        git_root: Some("/tmp".to_string()),
        last_active_conversation_id: None,
    }
}

fn conversation(project: &crate::ProjectRecord) -> NewConversation {
    NewConversation {
        project_id: project.id.clone(),
        title: "Conversation".to_string(),
        prompt_id: None,
        default_provider_id: None,
        default_model_id: None,
        metadata: conversation_metadata(),
        settings_snapshot: conversation_settings(),
    }
}

fn conversation_metadata() -> ConversationMetadata {
    ConversationMetadata {
        summary: Some("summary".to_string()),
        tags: vec!["tag".to_string()],
        pinned: true,
    }
}

fn conversation_settings() -> ConversationSettingsSnapshot {
    ConversationSettingsSnapshot {
        prompt: Some(prompt_content()),
        provider_id: Some("provider".to_string()),
        model_id: Some("model".to_string()),
        model_capabilities: Some(model_capabilities()),
        tool_policy: tool_policy(),
    }
}

fn message_item(conversation_id: &str, text: &str) -> NewConversationItem {
    message_item_with_role(conversation_id, TranscriptRole::User, text)
}

fn message_item_with_role(
    conversation_id: &str,
    role: TranscriptRole,
    text: &str,
) -> NewConversationItem {
    NewConversationItem {
        conversation_id: conversation_id.to_string(),
        status: ConversationItemStatus::Completed,
        agent_run_id: None,
        provider_step_id: None,
        tool_invocation_id: None,
        provider_item_id: None,
        payload: ConversationItemPayload::Message {
            role,
            content: vec![ContentPart::Text {
                text: text.to_string(),
            }],
        },
    }
}

fn provider() -> NewProvider {
    NewProvider {
        kind: "openai".to_string(),
        display_name: "OpenAI".to_string(),
        enabled: true,
        settings: provider_settings(),
        secret_refs: provider_secret_refs(),
    }
}

fn provider_settings() -> ProviderSettingsPayload {
    ProviderSettingsPayload {
        provider_kind: "openai".to_string(),
        fields: vec![ProviderSettingFieldValue {
            key: "base_url".to_string(),
            value: ProviderSettingValue::String {
                value: "https://api.openai.com/v1".to_string(),
            },
        }],
    }
}

fn provider_secret_refs() -> ProviderSecretRefs {
    ProviderSecretRefs {
        refs: vec![ProviderSecretRef {
            key: "api_key".to_string(),
            storage: "keychain".to_string(),
            ref_id: "openai-api-key".to_string(),
        }],
    }
}

fn provider_model(provider_id: &str, model_id: &str, display_name: &str) -> NewProviderModel {
    NewProviderModel {
        provider_id: provider_id.to_string(),
        model_id: model_id.to_string(),
        display_name: Some(display_name.to_string()),
        capabilities: model_capabilities(),
        metadata: provider_model_metadata(display_name),
    }
}

fn provider_model_metadata(display_name: &str) -> ProviderModelMetadata {
    ProviderModelMetadata {
        display_name: Some(display_name.to_string()),
        family: Some("gpt".to_string()),
        raw: Some(provider_raw(json!({ "owned_by": "openai" }))),
    }
}

fn prompt() -> NewPrompt {
    NewPrompt {
        name: "Default".to_string(),
        content: prompt_content(),
        enabled: true,
        sort_order: 10,
    }
}

fn prompt_content() -> PromptContent {
    PromptContent {
        messages: vec![PromptMessage {
            role: TranscriptRole::System,
            content: vec![ContentPart::Text {
                text: "You are useful.".to_string(),
            }],
        }],
    }
}

fn attachment_metadata() -> AttachmentMetadata {
    AttachmentMetadata {
        source: AttachmentSource::LocalFile {
            path: "/tmp/notes.txt".to_string(),
        },
        width: None,
        height: None,
        duration_ms: None,
        preview_attachment_id: None,
    }
}

fn agent_run_input(user_item_id: &str, provider_id: &str, model_id: &str) -> AgentRunInput {
    AgentRunInput {
        user_item_id: user_item_id.to_string(),
        prompt_snapshot: Some(prompt_content()),
        provider_id: provider_id.to_string(),
        model_id: model_id.to_string(),
        settings_snapshot: run_settings(provider_id, model_id),
        runtime_snapshot: AgentRuntimeSnapshot {
            engine: AgentEngineKind::Rig,
            engine_version: "0.22.0".to_string(),
            skill_catalog_hash: Some("skills".to_string()),
            mcp_config_hash: Some("mcp".to_string()),
            tool_name_strategy: ToolNameStrategy::Namespaced,
        },
        max_steps: 8,
    }
}

fn provider_step_request(
    provider_id: &str,
    model_id: &str,
    input_item_id: &str,
) -> ProviderStepRequestSnapshot {
    ProviderStepRequestSnapshot {
        provider_id: provider_id.to_string(),
        model_id: model_id.to_string(),
        input_item_ids: vec![input_item_id.to_string()],
        snapshot_kind: ProviderStepSnapshotKind::RigCompletionRequest,
        request_body: provider_raw(json!({ "messages": [] })),
    }
}

fn provider_step_response() -> ProviderStepResponseSnapshot {
    ProviderStepResponseSnapshot {
        provider_run_id: Some("resp_1".to_string()),
        output_item_ids: vec!["item_1".to_string()],
        response_body: Some(provider_raw(json!({ "id": "resp_1" }))),
    }
}

fn provider_run_state(provider_id: &str) -> ProviderRunStateSnapshot {
    ProviderRunStateSnapshot {
        provider_id: provider_id.to_string(),
        provider_run_id: Some("resp_1".to_string()),
        output_item_ids: vec!["item_1".to_string()],
        continuation: Some(provider_raw(json!({ "previous_response_id": "resp_1" }))),
    }
}

fn tool_input() -> ToolInvocationInput {
    ToolInvocationInput {
        source: ToolSource::Mcp {
            server_id: "filesystem".to_string(),
        },
        namespace: Some("filesystem".to_string()),
        tool_name: "read_file".to_string(),
        runtime_tool_name: "filesystem__read_file".to_string(),
        call_id: "call_1".to_string(),
        arguments: ToolArguments {
            value: json!({ "path": "/tmp/notes.txt" }),
        },
        approval_policy: ToolApprovalPolicy::OnRequest,
        execution_policy: ToolExecutionPolicy::Foreground,
    }
}

fn tool_output() -> ToolInvocationOutput {
    ToolInvocationOutput {
        content: vec![ContentPart::Text {
            text: "file body".to_string(),
        }],
        structured_output: Some(StructuredOutput {
            value: json!({ "bytes": 9 }),
        }),
        raw_output: Some(provider_raw(json!({ "stdout": "file body" }))),
        is_error: false,
    }
}

fn approval_request() -> ApprovalRequestPayload {
    ApprovalRequestPayload {
        reason: "Read a local file".to_string(),
        tool_source: ToolSource::Mcp {
            server_id: "filesystem".to_string(),
        },
        tool_name: "read_file".to_string(),
        arguments_preview: "{\"path\":\"/tmp/notes.txt\"}".to_string(),
    }
}

fn approval_decision() -> ApprovalDecisionPayload {
    ApprovalDecisionPayload {
        approved: true,
        decided_by: "user".to_string(),
        reason: Some("ok".to_string()),
    }
}

fn usage_snapshot() -> ProviderUsageSnapshot {
    ProviderUsageSnapshot {
        input_tokens: 10,
        output_tokens: 20,
        cached_input_tokens: 2,
        cache_write_input_tokens: 3,
        reasoning_tokens: 4,
        total_tokens: 39,
        metadata: Some(provider_raw(json!({ "source": "test" }))),
    }
}

fn run_settings(provider_id: &str, model_id: &str) -> RunSettingsSnapshot {
    RunSettingsSnapshot {
        prompt: Some(prompt_content()),
        provider_id: provider_id.to_string(),
        model_id: model_id.to_string(),
        model_capabilities: model_capabilities(),
        provider_settings: provider_settings(),
        tool_policy: tool_policy(),
    }
}

fn tool_policy() -> ToolPolicySnapshot {
    ToolPolicySnapshot {
        approval_policy: ToolApprovalPolicy::OnRequest,
        enabled_sources: vec![
            ToolSource::Local,
            ToolSource::Mcp {
                server_id: "filesystem".to_string(),
            },
        ],
        max_steps: 8,
    }
}

fn model_capabilities() -> ModelCapabilitiesSnapshot {
    ModelCapabilitiesSnapshot {
        text_input: true,
        text_output: true,
        streaming: true,
        image_input: Some(ImageInputCapabilitySnapshot {
            max_images: Some(4),
        }),
        file_input: Some(FileInputCapabilitySnapshot { max_files: Some(8) }),
        audio_input: false,
        image_generation: false,
        tool_calling: Some(ToolCallingCapabilitySnapshot {
            parallel_tool_calls: true,
        }),
        hosted_web_search: true,
        remote_mcp: false,
        reasoning: Some(ReasoningCapabilitySnapshot {
            default_effort: "medium".to_string(),
            efforts: vec!["low".to_string(), "medium".to_string()],
            summaries: true,
        }),
        structured_output: true,
        stateful_response_continuation: true,
        extension: ProviderCapabilityExtensionSnapshot::OpenAi {
            responses_api: true,
            raw: Some(provider_raw(json!({ "family": "gpt" }))),
        },
    }
}

fn run_error() -> RunErrorPayload {
    RunErrorPayload {
        code: "provider_error".to_string(),
        message: "temporary".to_string(),
        retryable: true,
        provider: Some("openai".to_string()),
        raw: Some(provider_raw(json!({ "status": 500 }))),
    }
}

fn provider_raw(value: serde_json::Value) -> ProviderRawPayload {
    ProviderRawPayload {
        provider_kind: "openai".to_string(),
        value,
    }
}

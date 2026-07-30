use super::*;

#[test]
fn soft_delete_conversation_rejects_active_run_and_succeeds_after_terminal_status() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let project = repo.insert_project(project("delete-active-run")).unwrap();
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
            conversation_id: conversation.id.clone(),
            trigger_entry_id: trigger.id.clone(),
            trigger_kind: AgentRunTriggerKind::User,
            input: agent_run_input(&trigger.id, &provider.id, &model.model_id),
        })
        .unwrap();

    let error = repo.soft_delete_conversation(&conversation.id).unwrap_err();
    assert!(matches!(
        error,
        crate::DbError::ConversationHasActiveRun { conversation_id }
            if conversation_id == conversation.id
    ));
    assert_eq!(
        repo.get_conversation(&conversation.id)
            .unwrap()
            .unwrap()
            .status,
        ConversationStatus::Active
    );

    repo.finish_agent_run(
        &run.id,
        FinishAgentRun {
            status: AgentRunStatus::Canceled,
            stopped_reason: AgentStoppedReason::Canceled,
            error: None,
            final_entry: AgentRunFinalEntry::Append(Box::new(NewConversationEntry {
                conversation_id: conversation.id.clone(),
                status: ConversationEntryStatus::Completed,
                agent_run_id: Some(run.id.clone()),
                provider_step_id: None,
                tool_invocation_id: None,
                provider_item_id: None,
                payload: ConversationEntryPayload::Status(ConversationStatusEntry {
                    code: ConversationStatusCode::Canceled,
                    message: None,
                }),
            })),
        },
    )
    .unwrap();
    let deleted = repo.soft_delete_conversation(&conversation.id).unwrap();
    assert_eq!(deleted.status, ConversationStatus::Deleted);
    assert!(deleted.deleted_at.is_some());
}

#[test]
fn complete_provider_step_commits_usage_and_continuation_atomically() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let project = repo
        .insert_project(project("complete-provider-step"))
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
            conversation_id: conversation.id.clone(),
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
    let continuation = ProviderContinuationSnapshot::openai_responses(
        "resp_1".to_string(),
        "all_turns".to_string(),
        time::OffsetDateTime::now_utc(),
    )
    .unwrap();
    let completion = crate::CompleteProviderStep {
        response_snapshot: provider_step_response(),
        state_snapshot: provider_run_state(&provider.id),
        continuation: Some(continuation.clone()),
        usage: usage_snapshot(),
    };

    let completed = repo
        .complete_provider_step_with_usage(&step.id, completion.clone())
        .unwrap();
    assert_eq!(completed.step.status, ProviderStepStatus::Completed);
    assert_eq!(completed.step.continuation, Some(continuation));
    assert_eq!(completed.usage.usage, usage_snapshot());
    assert_eq!(
        repo.usage_events_for_provider_step(&step.id).unwrap().len(),
        1
    );
    assert!(
        repo.insert_usage_event(NewUsageEvent {
            provider_step_id: step.id.clone(),
            date_key: "2026-07-29".to_string(),
            usage: usage_snapshot(),
        })
        .is_err()
    );
    assert!(
        repo.complete_provider_step_with_usage(&step.id, completion)
            .is_err()
    );
}

#[test]
fn complete_provider_step_rolls_back_when_usage_insert_fails() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let project = repo
        .insert_project(project("provider-step-rollback"))
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
            conversation_id: conversation.id.clone(),
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
    let continuation = ProviderContinuationSnapshot::openai_responses(
        "resp_rollback".to_string(),
        "all_turns".to_string(),
        time::OffsetDateTime::now_utc(),
    )
    .unwrap();
    {
        let mut conn = store.pool().get().unwrap();
        conn.batch_execute(
            "CREATE TRIGGER inject_usage_failure \
             BEFORE INSERT ON usage_events \
             BEGIN SELECT RAISE(ABORT, 'inject-usage-failure'); END;",
        )
        .unwrap();
    }

    let result = repo.complete_provider_step_with_usage(
        &step.id,
        crate::CompleteProviderStep {
            response_snapshot: provider_step_response(),
            state_snapshot: provider_run_state(&provider.id),
            continuation: Some(continuation),
            usage: usage_snapshot(),
        },
    );

    assert!(result.is_err());
    let step = repo.get_provider_step(&step.id).unwrap().unwrap();
    assert_eq!(step.status, ProviderStepStatus::Running);
    assert!(step.response_snapshot.is_none());
    assert!(step.state_snapshot.is_none());
    assert!(step.continuation.is_none());
    assert!(step.completed_at.is_none());
    assert!(
        repo.usage_events_for_provider_step(&step.id)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn previous_id_fallback_persists_failed_and_completed_attempts() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let project = repo
        .insert_project(project("previous-id-fallback"))
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
    let mut previous_request = provider_step_request(&provider.id, &model.model_id, &trigger.id);
    previous_request.transport = ProviderTransportSnapshot::WebSocket;
    previous_request.context_mode = ProviderRequestContextSnapshot::PreviousResponse;
    previous_request.previous_response_id = Some("resp_expired".to_string());
    let rejected = repo
        .insert_provider_step(NewProviderStep {
            agent_run_id: run.id.clone(),
            seq: 1,
            status: ProviderStepStatus::Running,
            request_snapshot: previous_request,
            response_snapshot: None,
            state_snapshot: None,
            settings_snapshot: run_settings(&provider.id, &model.model_id),
            error: None,
        })
        .unwrap();
    let rejection = run_error();
    repo.update_provider_step_status(
        &rejected.id,
        UpdateProviderStepStatus {
            status: ProviderStepStatus::Failed,
            response_snapshot: None,
            state_snapshot: None,
            error: Some(rejection.clone()),
        },
    )
    .unwrap();

    let mut fallback_request = provider_step_request(&provider.id, &model.model_id, &trigger.id);
    fallback_request.transport = ProviderTransportSnapshot::WebSocket;
    fallback_request.context_mode = ProviderRequestContextSnapshot::FullHistoryFallback;
    let fallback = repo
        .insert_provider_step(NewProviderStep {
            agent_run_id: run.id.clone(),
            seq: 2,
            status: ProviderStepStatus::Running,
            request_snapshot: fallback_request,
            response_snapshot: None,
            state_snapshot: None,
            settings_snapshot: run_settings(&provider.id, &model.model_id),
            error: None,
        })
        .unwrap();
    let continuation = ProviderContinuationSnapshot::openai_responses(
        "resp_1".to_string(),
        "current_turn".to_string(),
        time::OffsetDateTime::now_utc(),
    )
    .unwrap();
    repo.complete_provider_step_with_usage(
        &fallback.id,
        crate::CompleteProviderStep {
            response_snapshot: provider_step_response(),
            state_snapshot: provider_run_state(&provider.id),
            continuation: Some(continuation),
            usage: usage_snapshot(),
        },
    )
    .unwrap();
    let tool = repo
        .insert_tool_invocation(NewToolInvocation {
            agent_run_id: run.id.clone(),
            provider_step_id: Some(fallback.id.clone()),
            status: ToolInvocationStatus::Succeeded,
            input: tool_input(),
            output: Some(tool_output()),
            error: None,
        })
        .unwrap();

    let steps = repo.provider_steps_for_run(&run.id).unwrap();
    assert_eq!(
        steps.iter().map(|step| step.seq).collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(steps[0].status, ProviderStepStatus::Failed);
    assert_eq!(steps[0].error, Some(rejection));
    assert!(
        repo.usage_events_for_provider_step(&rejected.id)
            .unwrap()
            .is_empty()
    );
    assert_eq!(steps[1].status, ProviderStepStatus::Completed);
    assert_eq!(
        repo.usage_events_for_provider_step(&fallback.id)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(tool.provider_step_id.as_deref(), Some(fallback.id.as_str()));
}

#[test]
fn append_items_updates_order_last_seq_and_search_text() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let project = repo.insert_project(project("items")).unwrap();
    let conversation = repo.insert_conversation(conversation(&project)).unwrap();

    let first = repo
        .append_conversation_entry(message_item(&conversation.id, "hello alpha"))
        .unwrap();
    let second = repo
        .append_conversation_entry(message_item(&conversation.id, "hello beta"))
        .unwrap();
    assert_eq!((first.seq, second.seq), (1, 2));

    let conversation = repo.get_conversation(&conversation.id).unwrap().unwrap();
    assert_eq!(conversation.last_entry_seq, 2);
    let items = repo.conversation_entries(&conversation.id).unwrap();
    assert_eq!(
        items.iter().map(|item| item.seq).collect::<Vec<_>>(),
        [1, 2]
    );

    assert_eq!(first.search_text, "hello alpha");

    repo.update_conversation_entry_payload(
        &first.id,
        ConversationEntryStatus::Completed,
        ConversationEntryPayload::Message {
            role: TranscriptRole::User,
            content: vec![ContentPart::Text {
                text: "gamma".to_string(),
            }],
        },
    )
    .unwrap();
    let updated = repo.conversation_entries(&conversation.id).unwrap();
    assert_eq!(updated[0].search_text, "gamma");

    let remaining = repo.conversation_entries(&conversation.id).unwrap();
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
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let project = repo.insert_project(project("item-update")).unwrap();
    let conversation = repo.insert_conversation(conversation(&project)).unwrap();
    let item = repo
        .append_conversation_entry(message_item(&conversation.id, "before"))
        .unwrap();

    let updated = repo
        .update_conversation_entry_payload(
            &item.id,
            ConversationEntryStatus::Completed,
            ConversationEntryPayload::Message {
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
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let project = repo.insert_project(project("execution-links")).unwrap();
    let conversation_a = repo.insert_conversation(conversation(&project)).unwrap();
    let conversation_b = repo.insert_conversation(conversation(&project)).unwrap();
    let provider = repo.insert_provider(provider()).unwrap();
    let model = repo
        .upsert_provider_model(provider_model(&provider.id, "gpt-5.2", "GPT-5.2"))
        .unwrap();
    let user_item = repo
        .append_conversation_entry(message_item(&conversation_a.id, "run input"))
        .unwrap();
    let agent_run = repo
        .insert_agent_run(NewAgentRun {
            conversation_id: conversation_a.id.clone(),
            trigger_kind: AgentRunTriggerKind::User,
            trigger_entry_id: user_item.id.clone(),
            input: agent_run_input(&user_item.id, &provider.id, &model.model_id),
        })
        .unwrap();
    let provider_step = repo
        .insert_provider_step(NewProviderStep {
            agent_run_id: agent_run.id.clone(),
            seq: 1,
            status: ProviderStepStatus::Completed,
            request_snapshot: provider_step_request(&provider.id, &model.model_id, &user_item.id),
            response_snapshot: Some(provider_step_response()),
            state_snapshot: Some(provider_run_state(&provider.id)),
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
    repo.append_conversation_entry(same_conversation).unwrap();

    let mut cross_agent = message_item(&conversation_b.id, "cross agent");
    cross_agent.agent_run_id = Some(agent_run.id.clone());
    assert!(repo.append_conversation_entry(cross_agent).is_err());

    let mut cross_step = message_item(&conversation_b.id, "cross step");
    cross_step.provider_step_id = Some(provider_step.id.clone());
    assert!(repo.append_conversation_entry(cross_step).is_err());

    let mut cross_tool = message_item(&conversation_b.id, "cross tool");
    cross_tool.tool_invocation_id = Some(tool.id.clone());
    assert!(repo.append_conversation_entry(cross_tool).is_err());

    let second_run = repo
        .insert_agent_run(NewAgentRun {
            conversation_id: conversation_a.id.clone(),
            trigger_kind: AgentRunTriggerKind::Retry,
            trigger_entry_id: user_item.id.clone(),
            input: agent_run_input(&user_item.id, &provider.id, &model.model_id),
        })
        .unwrap();
    let mut mismatched_chain = message_item(&conversation_a.id, "mismatched chain");
    mismatched_chain.agent_run_id = Some(second_run.id);
    mismatched_chain.provider_step_id = Some(provider_step.id);
    assert!(repo.append_conversation_entry(mismatched_chain).is_err());
}

#[test]
fn insert_agent_run_validates_trigger_entry_and_rejects_invalid_user_entry() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let project = repo.insert_project(project("agent-run-input")).unwrap();
    let conversation = repo.insert_conversation(conversation(&project)).unwrap();
    let provider = repo.insert_provider(provider()).unwrap();
    let model = repo
        .upsert_provider_model(provider_model(&provider.id, "gpt-5.2", "GPT-5.2"))
        .unwrap();
    let user_item = repo
        .append_conversation_entry(message_item(&conversation.id, "run input"))
        .unwrap();
    let assistant_item = repo
        .append_conversation_entry(message_item_with_role(
            &conversation.id,
            TranscriptRole::Assistant,
            "assistant output",
        ))
        .unwrap();

    let valid = repo.insert_agent_run(NewAgentRun {
        conversation_id: conversation.id.clone(),
        trigger_kind: AgentRunTriggerKind::User,
        trigger_entry_id: user_item.id.clone(),
        input: agent_run_input(&user_item.id, &provider.id, &model.model_id),
    });
    assert_eq!(valid.unwrap().conversation_id, conversation.id);

    let missing_item = repo.insert_agent_run(NewAgentRun {
        conversation_id: conversation.id.clone(),
        trigger_kind: AgentRunTriggerKind::User,
        trigger_entry_id: "missing-item".to_string(),
        input: agent_run_input("missing-item", &provider.id, &model.model_id),
    });
    assert!(missing_item.is_err());

    let non_user_item = repo.insert_agent_run(NewAgentRun {
        conversation_id: conversation.id.clone(),
        trigger_kind: AgentRunTriggerKind::User,
        trigger_entry_id: assistant_item.id.clone(),
        input: agent_run_input(&assistant_item.id, &provider.id, &model.model_id),
    });
    assert!(non_user_item.is_err());
}

#[test]
fn insert_tool_invocation_rejects_provider_step_from_other_run() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let project = repo.insert_project(project("tool-step-link")).unwrap();
    let conversation = repo.insert_conversation(conversation(&project)).unwrap();
    let provider = repo.insert_provider(provider()).unwrap();
    let model = repo
        .upsert_provider_model(provider_model(&provider.id, "gpt-5.2", "GPT-5.2"))
        .unwrap();
    let first_item = repo
        .append_conversation_entry(message_item(&conversation.id, "first run"))
        .unwrap();
    let second_item = repo
        .append_conversation_entry(message_item(&conversation.id, "second run"))
        .unwrap();
    let first_run = repo
        .insert_agent_run(NewAgentRun {
            conversation_id: conversation.id.clone(),
            trigger_kind: AgentRunTriggerKind::User,
            trigger_entry_id: first_item.id.clone(),
            input: agent_run_input(&first_item.id, &provider.id, &model.model_id),
        })
        .unwrap();
    let second_run = repo
        .insert_agent_run(NewAgentRun {
            conversation_id: conversation.id.clone(),
            trigger_kind: AgentRunTriggerKind::Retry,
            trigger_entry_id: second_item.id.clone(),
            input: agent_run_input(&second_item.id, &provider.id, &model.model_id),
        })
        .unwrap();
    let first_step = repo
        .insert_provider_step(NewProviderStep {
            agent_run_id: first_run.id.clone(),
            seq: 1,
            status: ProviderStepStatus::Completed,
            request_snapshot: provider_step_request(&provider.id, &model.model_id, &first_item.id),
            response_snapshot: Some(provider_step_response()),
            state_snapshot: Some(provider_run_state(&provider.id)),
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
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let project = repo.insert_project(project("usage-dimensions")).unwrap();
    let conversation = repo.insert_conversation(conversation(&project)).unwrap();
    let provider = repo.insert_provider(provider()).unwrap();
    let model = repo
        .upsert_provider_model(provider_model(&provider.id, "gpt-5.2", "GPT-5.2"))
        .unwrap();
    let user_item = repo
        .append_conversation_entry(message_item(&conversation.id, "usage input"))
        .unwrap();
    let agent_run = repo
        .insert_agent_run(NewAgentRun {
            conversation_id: conversation.id.clone(),
            trigger_kind: AgentRunTriggerKind::User,
            trigger_entry_id: user_item.id.clone(),
            input: agent_run_input(&user_item.id, &provider.id, &model.model_id),
        })
        .unwrap();
    let provider_step = repo
        .insert_provider_step(NewProviderStep {
            agent_run_id: agent_run.id,
            seq: 1,
            status: ProviderStepStatus::Completed,
            request_snapshot: provider_step_request(&provider.id, &model.model_id, &user_item.id),
            response_snapshot: Some(provider_step_response()),
            state_snapshot: Some(provider_run_state(&provider.id)),
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
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
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
        .append_conversation_entry(message_item(&conversation.id, "step input"))
        .unwrap();
    let agent_run = repo
        .insert_agent_run(NewAgentRun {
            conversation_id: conversation.id.clone(),
            trigger_kind: AgentRunTriggerKind::User,
            trigger_entry_id: user_item.id.clone(),
            input: agent_run_input(&user_item.id, &provider.id, &model.model_id),
        })
        .unwrap();

    let provider_step = repo
        .insert_provider_step(NewProviderStep {
            agent_run_id: agent_run.id.clone(),
            seq: 1,
            status: ProviderStepStatus::Completed,
            request_snapshot: provider_step_request(&provider.id, &model.model_id, &user_item.id),
            response_snapshot: Some(provider_step_response()),
            state_snapshot: Some(provider_run_state(&provider.id)),
            settings_snapshot: run_settings(&provider.id, &model.model_id),
            error: None,
        })
        .unwrap();
    assert_eq!(provider_step.provider_id, provider.id);
    assert_eq!(provider_step.model_id, model.model_id);
    let timeline = repo
        .conversation_timeline_records(&conversation.id)
        .unwrap()
        .unwrap();
    assert_eq!(timeline.provider_steps, vec![provider_step.clone()]);

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
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
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
        .append_conversation_entry(message_item(&primary_conversation.id, "step input"))
        .unwrap();
    let context_item = repo
        .append_conversation_entry(message_item(
            &primary_conversation.id,
            "same conversation context",
        ))
        .unwrap();
    let other_item = repo
        .append_conversation_entry(message_item(&other_conversation.id, "other context"))
        .unwrap();
    let agent_run = repo
        .insert_agent_run(NewAgentRun {
            conversation_id: primary_conversation.id.clone(),
            trigger_kind: AgentRunTriggerKind::User,
            trigger_entry_id: user_item.id.clone(),
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
            response_snapshot: Some(provider_step_response()),
            state_snapshot: Some(provider_run_state(&provider.id)),
            settings_snapshot: run_settings(&provider.id, &model.model_id),
            error: None,
        })
        .unwrap();
    assert_eq!(
        provider_step.request_snapshot.input_item_ids,
        [user_item.id.clone(), context_item.id.clone()]
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
    cross_conversation_request.input_item_ids = vec![other_item.id.clone()];
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
fn tool_invocation_approval_derives_status_and_decision_columns() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let project = repo.insert_project(project("approval-outcome")).unwrap();
    let conversation = repo.insert_conversation(conversation(&project)).unwrap();
    let provider = repo.insert_provider(provider()).unwrap();
    let model = repo
        .upsert_provider_model(provider_model(&provider.id, "gpt-5.2", "GPT-5.2"))
        .unwrap();
    let user_item = repo
        .append_conversation_entry(message_item(&conversation.id, "approval input"))
        .unwrap()
        .value;
    let agent_run = repo
        .insert_agent_run(NewAgentRun {
            conversation_id: conversation.id.clone(),
            trigger_kind: AgentRunTriggerKind::User,
            trigger_entry_id: user_item.id.clone(),
            input: agent_run_input(&user_item.id, &provider.id, &model.model_id),
        })
        .unwrap();
    let provider_step = repo
        .insert_provider_step(NewProviderStep {
            agent_run_id: agent_run.id.clone(),
            seq: 1,
            status: ProviderStepStatus::Completed,
            request_snapshot: provider_step_request(&provider.id, &model.model_id, &user_item.id),
            response_snapshot: Some(provider_step_response()),
            state_snapshot: Some(provider_run_state(&provider.id)),
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
            status: ToolInvocationStatus::Requested,
            input: tool_input(),
            output: None,
            error: None,
        })
        .unwrap();

    let pending = repo
        .request_tool_invocation_approval(
            &pending_tool.id,
            NewToolInvocationApproval {
                request: approval_request(),
                expires_at: None,
            },
        )
        .unwrap();
    let pending_approval = pending.approval.unwrap();
    assert_eq!(pending.status, ToolInvocationStatus::AwaitingApproval);
    assert_eq!(pending_approval.status, ApprovalStatus::Pending);
    assert!(pending_approval.decision.is_none());
    assert!(pending_approval.decided_at.is_none());

    let denied = repo
        .request_tool_invocation_approval(
            &denied_tool.id,
            NewToolInvocationApproval {
                request: approval_request(),
                expires_at: None,
            },
        )
        .unwrap();
    let denied = repo
        .update_tool_invocation_approval(
            &denied.id,
            ToolInvocationApprovalOutcome::Denied {
                decided_by: "user".to_string(),
                reason: Some("no".to_string()),
            },
            ToolInvocationStatus::Denied,
        )
        .unwrap();
    let denied_approval = denied.approval.unwrap();
    assert_eq!(denied.status, ToolInvocationStatus::Denied);
    assert_eq!(denied_approval.status, ApprovalStatus::Denied);
    assert_eq!(
        denied_approval.decision,
        Some(ApprovalDecisionPayload {
            approved: false,
            decided_by: "user".to_string(),
            reason: Some("no".to_string()),
        })
    );
    assert!(denied_approval.decided_at.is_some());
}

#[test]
fn execution_status_updates_and_tool_invocation_approval_roundtrip() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let project = repo.insert_project(project("execution-updates")).unwrap();
    let conversation = repo.insert_conversation(conversation(&project)).unwrap();
    let provider = repo.insert_provider(provider()).unwrap();
    let model = repo
        .upsert_provider_model(provider_model(&provider.id, "gpt-5.2", "GPT-5.2"))
        .unwrap();
    let user_item = repo
        .append_conversation_entry(message_item(&conversation.id, "update input"))
        .unwrap();
    let agent_run = repo
        .insert_agent_run(NewAgentRun {
            conversation_id: conversation.id.clone(),
            trigger_kind: AgentRunTriggerKind::User,
            trigger_entry_id: user_item.id.clone(),
            input: agent_run_input(&user_item.id, &provider.id, &model.model_id),
        })
        .unwrap();

    assert_eq!(agent_run.status, AgentRunStatus::Running);
    assert!(agent_run.started_at.is_some());
    assert!(agent_run.completed_at.is_none());

    let provider_step = repo
        .insert_provider_step(NewProviderStep {
            agent_run_id: agent_run.id.clone(),
            seq: 1,
            status: ProviderStepStatus::Queued,
            request_snapshot: provider_step_request(&provider.id, &model.model_id, &user_item.id),
            response_snapshot: None,
            state_snapshot: None,
            settings_snapshot: run_settings(&provider.id, &model.model_id),
            error: None,
        })
        .unwrap();
    let completed_step = repo
        .update_provider_step_status(
            &provider_step.id,
            UpdateProviderStepStatus {
                status: ProviderStepStatus::Completed,
                response_snapshot: Some(provider_step_response()),
                state_snapshot: Some(provider_run_state(&provider.id)),
                error: None,
            },
        )
        .unwrap();
    assert_eq!(completed_step.status, ProviderStepStatus::Completed);
    assert_eq!(
        completed_step.response_snapshot,
        Some(provider_step_response())
    );
    assert!(completed_step.started_at.is_some());
    assert!(completed_step.completed_at.is_some());

    let tool = repo
        .insert_tool_invocation(NewToolInvocation {
            agent_run_id: agent_run.id.clone(),
            provider_step_id: Some(provider_step.id.clone()),
            status: ToolInvocationStatus::Requested,
            input: tool_input(),
            output: None,
            error: None,
        })
        .unwrap();
    let approval = repo
        .request_tool_invocation_approval(
            &tool.id,
            NewToolInvocationApproval {
                request: approval_request(),
                expires_at: None,
            },
        )
        .unwrap();
    assert_eq!(approval.status, ToolInvocationStatus::AwaitingApproval);
    assert_eq!(
        approval.approval.as_ref().map(|approval| approval.status),
        Some(ApprovalStatus::Pending)
    );

    let approved = repo
        .update_tool_invocation_approval(
            &approval.id,
            ToolInvocationApprovalOutcome::Approved {
                decided_by: "user".to_string(),
                reason: Some("ok".to_string()),
            },
            ToolInvocationStatus::Running,
        )
        .unwrap();
    assert_eq!(approved.status, ToolInvocationStatus::Running);
    assert_eq!(
        approved.approval.as_ref().map(|approval| approval.status),
        Some(ApprovalStatus::Approved)
    );

    let succeeded_tool = repo
        .update_tool_invocation_status(
            &tool.id,
            UpdateToolInvocationStatus {
                status: ToolInvocationStatus::Succeeded,
                output: Some(tool_output()),
                error: None,
            },
        )
        .unwrap();
    assert_eq!(succeeded_tool.status, ToolInvocationStatus::Succeeded);
    assert_eq!(succeeded_tool.output, Some(tool_output()));
    assert_eq!(
        succeeded_tool
            .approval
            .as_ref()
            .and_then(|approval| approval.decision.as_ref()),
        Some(&approval_decision())
    );
    assert!(succeeded_tool.started_at.is_some());
    assert!(succeeded_tool.completed_at.is_some());

    let finished = repo
        .finish_agent_run(
            &agent_run.id,
            FinishAgentRun {
                status: AgentRunStatus::Completed,
                stopped_reason: AgentStoppedReason::Completed,
                error: None,
                final_entry: AgentRunFinalEntry::Append(Box::new(NewConversationEntry {
                    conversation_id: conversation.id.clone(),
                    status: ConversationEntryStatus::Completed,
                    agent_run_id: Some(agent_run.id.clone()),
                    provider_step_id: None,
                    tool_invocation_id: None,
                    provider_item_id: None,
                    payload: ConversationEntryPayload::Status(ConversationStatusEntry {
                        code: ConversationStatusCode::CompletedWithoutOutput,
                        message: None,
                    }),
                })),
            },
        )
        .unwrap();
    assert_eq!(finished.run.status, AgentRunStatus::Completed);
    assert_eq!(
        finished.run.output.as_ref().unwrap().final_entry_id,
        finished.final_entry.id
    );
    assert!(finished.run.completed_at.is_some());
    assert_eq!(repo.provider_steps_for_run(&agent_run.id).unwrap().len(), 1);
    assert_eq!(
        repo.tool_invocations_for_run(&agent_run.id).unwrap().len(),
        1
    );
}

#[test]
fn agent_run_finalization_persists_terminal_entry_and_is_idempotent() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let project = repo
        .insert_project(project("agent-run-finalization"))
        .unwrap();
    let conversation = repo.insert_conversation(conversation(&project)).unwrap();
    let provider = repo.insert_provider(provider()).unwrap();
    let model = repo
        .upsert_provider_model(provider_model(&provider.id, "gpt-5.2", "GPT-5.2"))
        .unwrap();
    let user_item = repo
        .append_conversation_entry(message_item(&conversation.id, "failed input"))
        .unwrap();
    let run = repo
        .insert_agent_run(NewAgentRun {
            conversation_id: conversation.id.clone(),
            trigger_kind: AgentRunTriggerKind::User,
            trigger_entry_id: user_item.id.clone(),
            input: agent_run_input(&user_item.id, &provider.id, &model.model_id),
        })
        .unwrap();
    let error = RunErrorPayload {
        code: "prompt_error".to_string(),
        message: "forced provider-open failure".to_string(),
        retryable: true,
        provider: None,
        raw: None,
    };

    let failed = repo
        .finish_agent_run(
            &run.id,
            FinishAgentRun {
                status: AgentRunStatus::Failed,
                stopped_reason: AgentStoppedReason::Failed,
                error: Some(error.clone()),
                final_entry: AgentRunFinalEntry::Append(Box::new(NewConversationEntry {
                    conversation_id: conversation.id.clone(),
                    status: ConversationEntryStatus::Failed,
                    agent_run_id: Some(run.id.clone()),
                    provider_step_id: None,
                    tool_invocation_id: None,
                    provider_item_id: None,
                    payload: ConversationEntryPayload::Error(error.clone()),
                })),
            },
        )
        .unwrap();
    let entry = failed.final_entry.clone();
    assert!(matches!(entry.payload, ConversationEntryPayload::Error(_)));
    assert_eq!(entry.status, ConversationEntryStatus::Failed);
    assert_eq!(entry.agent_run_id.as_deref(), Some(run.id.as_str()));
    assert_eq!(failed.run.status, AgentRunStatus::Failed);
    assert_eq!(failed.run.error, Some(error.clone()));
    assert_eq!(
        failed.run.output.as_ref().unwrap().final_entry_id,
        entry.id.clone()
    );
    let item_count = repo.conversation_entries(&conversation.id).unwrap().len();

    let duplicate = repo
        .finish_agent_run(
            &run.id,
            FinishAgentRun {
                status: AgentRunStatus::Failed,
                stopped_reason: AgentStoppedReason::Failed,
                error: Some(error.clone()),
                final_entry: AgentRunFinalEntry::Append(Box::new(NewConversationEntry {
                    conversation_id: conversation.id.clone(),
                    status: ConversationEntryStatus::Failed,
                    agent_run_id: Some(run.id.clone()),
                    provider_step_id: None,
                    tool_invocation_id: None,
                    provider_item_id: None,
                    payload: ConversationEntryPayload::Error(error.clone()),
                })),
            },
        )
        .unwrap();
    assert!(!duplicate.appended_final_entry);
    assert_eq!(duplicate.run, failed.run);
    assert_eq!(duplicate.final_entry, failed.final_entry);
    assert_eq!(
        repo.conversation_entries(&conversation.id).unwrap().len(),
        item_count
    );

    let mismatched_input = repo
        .append_conversation_entry(message_item(&conversation.id, "mismatched failure input"))
        .unwrap();
    let mismatched_run = repo
        .insert_agent_run(NewAgentRun {
            conversation_id: conversation.id.clone(),
            trigger_kind: AgentRunTriggerKind::User,
            trigger_entry_id: mismatched_input.id.clone(),
            input: agent_run_input(&mismatched_input.id, &provider.id, &model.model_id),
        })
        .unwrap();
    let count_before_mismatched_finish = repo.conversation_entries(&conversation.id).unwrap().len();
    let mismatched = repo.finish_agent_run(
        &mismatched_run.id,
        FinishAgentRun {
            status: AgentRunStatus::Failed,
            stopped_reason: AgentStoppedReason::Failed,
            error: Some(error.clone()),
            final_entry: AgentRunFinalEntry::Append(Box::new(NewConversationEntry {
                conversation_id: conversation.id.clone(),
                status: ConversationEntryStatus::Completed,
                agent_run_id: Some(mismatched_run.id.clone()),
                provider_step_id: None,
                tool_invocation_id: None,
                provider_item_id: None,
                payload: ConversationEntryPayload::Error(error.clone()),
            })),
        },
    );
    assert!(mismatched.is_err());
    assert_eq!(
        repo.conversation_entries(&conversation.id).unwrap().len(),
        count_before_mismatched_finish
    );

    let completed_input = repo
        .append_conversation_entry(message_item(&conversation.id, "completed input"))
        .unwrap();
    let completed_run = repo
        .insert_agent_run(NewAgentRun {
            conversation_id: conversation.id.clone(),
            trigger_kind: AgentRunTriggerKind::User,
            trigger_entry_id: completed_input.id.clone(),
            input: agent_run_input(&completed_input.id, &provider.id, &model.model_id),
        })
        .unwrap();
    let completed = repo
        .finish_agent_run(
            &completed_run.id,
            FinishAgentRun {
                status: AgentRunStatus::Completed,
                stopped_reason: AgentStoppedReason::Completed,
                error: None,
                final_entry: AgentRunFinalEntry::Append(Box::new(NewConversationEntry {
                    conversation_id: conversation.id.clone(),
                    status: ConversationEntryStatus::Completed,
                    agent_run_id: Some(completed_run.id.clone()),
                    provider_step_id: None,
                    tool_invocation_id: None,
                    provider_item_id: None,
                    payload: ConversationEntryPayload::Status(ConversationStatusEntry {
                        code: ConversationStatusCode::CompletedWithoutOutput,
                        message: None,
                    }),
                })),
            },
        )
        .unwrap()
        .value;
    let status_entry = completed.final_entry;
    assert!(matches!(
        status_entry.payload,
        ConversationEntryPayload::Status(ConversationStatusEntry {
            code: ConversationStatusCode::CompletedWithoutOutput,
            message: None,
        })
    ));
    assert_eq!(completed.run.status, AgentRunStatus::Completed);
    assert_eq!(
        completed.run.output.unwrap().final_entry_id,
        status_entry.id
    );

    let mut conn = store.pool().get().unwrap();
    sql_query("UPDATE agent_runs SET final_entry_id = ? WHERE id = ?")
        .bind::<Text, _>(&status_entry.id)
        .bind::<Text, _>(&failed.run.id)
        .execute(&mut conn)
        .unwrap();
    let timeline_error = repo
        .conversation_timeline_records(&conversation.id)
        .unwrap_err();
    assert!(
        timeline_error
            .to_string()
            .contains("belongs to a different run")
    );
}

#[test]
fn approval_entries_are_atomic_and_duplicate_decisions_do_not_append() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let project = repo.insert_project(project("approval-entries")).unwrap();
    let conversation = repo.insert_conversation(conversation(&project)).unwrap();
    let provider = repo.insert_provider(provider()).unwrap();
    let model = repo
        .upsert_provider_model(provider_model(&provider.id, "gpt-5.2", "GPT-5.2"))
        .unwrap();
    let user_item = repo
        .append_conversation_entry(message_item(&conversation.id, "approval input"))
        .unwrap();
    let run = repo
        .insert_agent_run(NewAgentRun {
            conversation_id: conversation.id.clone(),
            trigger_kind: AgentRunTriggerKind::User,
            trigger_entry_id: user_item.id.clone(),
            input: agent_run_input(&user_item.id, &provider.id, &model.model_id),
        })
        .unwrap();
    let tool = repo
        .insert_tool_invocation(NewToolInvocation {
            agent_run_id: run.id.clone(),
            provider_step_id: None,
            status: ToolInvocationStatus::Requested,
            input: tool_input(),
            output: None,
            error: None,
        })
        .unwrap();
    let request = approval_request();
    let (request_entry, pending) = repo
        .request_tool_invocation_approval_with_entry(
            &tool.id,
            NewToolInvocationApproval {
                request: request.clone(),
                expires_at: None,
            },
            NewConversationEntry {
                conversation_id: conversation.id.clone(),
                status: ConversationEntryStatus::WaitingForApproval,
                agent_run_id: Some(run.id.clone()),
                provider_step_id: None,
                tool_invocation_id: Some(tool.id.clone()),
                provider_item_id: None,
                payload: ConversationEntryPayload::ApprovalRequest(ApprovalRequestEntry {
                    tool_invocation_id: tool.id.clone(),
                    request,
                }),
            },
        )
        .unwrap()
        .value;
    assert!(matches!(
        request_entry.payload,
        ConversationEntryPayload::ApprovalRequest(_)
    ));
    assert_eq!(pending.status, ToolInvocationStatus::AwaitingApproval);
    assert_eq!(
        pending.approval.as_ref().unwrap().status,
        ApprovalStatus::Pending
    );

    let decision = approval_decision();
    let (decision_entry, approved) = repo
        .decide_tool_invocation_approval_with_entry(
            &tool.id,
            ToolInvocationApprovalOutcome::Approved {
                decided_by: decision.decided_by.clone(),
                reason: decision.reason.clone(),
            },
            ToolInvocationStatus::Running,
            NewConversationEntry {
                conversation_id: conversation.id.clone(),
                status: ConversationEntryStatus::Completed,
                agent_run_id: Some(run.id.clone()),
                provider_step_id: None,
                tool_invocation_id: Some(tool.id.clone()),
                provider_item_id: None,
                payload: ConversationEntryPayload::ApprovalDecision(ApprovalDecisionEntry {
                    tool_invocation_id: tool.id.clone(),
                    decision: decision.clone(),
                }),
            },
        )
        .unwrap()
        .value;
    assert!(matches!(
        decision_entry.payload,
        ConversationEntryPayload::ApprovalDecision(_)
    ));
    assert_eq!(approved.status, ToolInvocationStatus::Running);
    assert_eq!(approved.approval.unwrap().status, ApprovalStatus::Approved);
    let entry_count = repo.conversation_entries(&conversation.id).unwrap().len();

    let duplicate = repo.decide_tool_invocation_approval_with_entry(
        &tool.id,
        ToolInvocationApprovalOutcome::Approved {
            decided_by: "second-user".to_string(),
            reason: None,
        },
        ToolInvocationStatus::Running,
        NewConversationEntry {
            conversation_id: conversation.id.clone(),
            status: ConversationEntryStatus::Completed,
            agent_run_id: Some(run.id),
            provider_step_id: None,
            tool_invocation_id: Some(tool.id.clone()),
            provider_item_id: None,
            payload: ConversationEntryPayload::ApprovalDecision(ApprovalDecisionEntry {
                tool_invocation_id: "duplicate-tool".to_string(),
                decision,
            }),
        },
    );
    assert!(duplicate.is_err());
    assert_eq!(
        repo.conversation_entries(&conversation.id).unwrap().len(),
        entry_count
    );
}

#[test]
fn active_execution_inserts_stamp_start_times() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let project = repo.insert_project(project("active-starts")).unwrap();
    let provider = repo.insert_provider(provider()).unwrap();
    let model = repo
        .upsert_provider_model(provider_model(&provider.id, "gpt-5.2", "GPT-5.2"))
        .unwrap();
    let conversation = repo.insert_conversation(conversation(&project)).unwrap();
    let user_item = repo
        .append_conversation_entry(message_item(&conversation.id, "run input"))
        .unwrap();

    let agent_run = repo
        .insert_agent_run(NewAgentRun {
            conversation_id: conversation.id.clone(),
            trigger_kind: AgentRunTriggerKind::User,
            trigger_entry_id: user_item.id.clone(),
            input: agent_run_input(&user_item.id, &provider.id, &model.model_id),
        })
        .unwrap();
    assert!(agent_run.started_at.is_some());
    assert!(agent_run.completed_at.is_none());

    let provider_step = repo
        .insert_provider_step(NewProviderStep {
            agent_run_id: agent_run.id.clone(),
            seq: 1,
            status: ProviderStepStatus::Running,
            request_snapshot: provider_step_request(&provider.id, &model.model_id, &user_item.id),
            response_snapshot: None,
            state_snapshot: None,
            settings_snapshot: run_settings(&provider.id, &model.model_id),
            error: None,
        })
        .unwrap();
    assert!(provider_step.started_at.is_some());
    assert!(provider_step.completed_at.is_none());
    let completed_step = repo
        .update_provider_step_status(
            &provider_step.id,
            UpdateProviderStepStatus {
                status: ProviderStepStatus::Completed,
                response_snapshot: Some(provider_step_response()),
                state_snapshot: Some(provider_run_state(&provider.id)),
                error: None,
            },
        )
        .unwrap();
    assert_eq!(completed_step.started_at, provider_step.started_at);
    assert!(completed_step.completed_at.is_some());

    let mut running_tool_input = tool_input();
    running_tool_input.call_id = "call_running".to_string();
    let running_tool = repo
        .insert_tool_invocation(NewToolInvocation {
            agent_run_id: agent_run.id.clone(),
            provider_step_id: Some(provider_step.id.clone()),
            status: ToolInvocationStatus::Running,
            input: running_tool_input,
            output: None,
            error: None,
        })
        .unwrap();
    assert!(running_tool.started_at.is_some());
    assert!(running_tool.completed_at.is_none());
    let succeeded_tool = repo
        .update_tool_invocation_status(
            &running_tool.id,
            UpdateToolInvocationStatus {
                status: ToolInvocationStatus::Succeeded,
                output: Some(tool_output()),
                error: None,
            },
        )
        .unwrap();
    assert_eq!(succeeded_tool.started_at, running_tool.started_at);
    assert!(succeeded_tool.completed_at.is_some());

    let mut awaiting_tool_input = tool_input();
    awaiting_tool_input.call_id = "call_awaiting".to_string();
    let awaiting_tool = repo
        .insert_tool_invocation(NewToolInvocation {
            agent_run_id: agent_run.id.clone(),
            provider_step_id: Some(provider_step.id),
            status: ToolInvocationStatus::AwaitingApproval,
            input: awaiting_tool_input,
            output: None,
            error: None,
        })
        .unwrap();
    assert!(awaiting_tool.started_at.is_some());
    assert!(awaiting_tool.completed_at.is_none());

    let mut requested_tool_input = tool_input();
    requested_tool_input.call_id = "call_requested".to_string();
    let requested_tool = repo
        .insert_tool_invocation(NewToolInvocation {
            agent_run_id: agent_run.id,
            provider_step_id: None,
            status: ToolInvocationStatus::Requested,
            input: requested_tool_input,
            output: None,
            error: None,
        })
        .unwrap();
    assert!(requested_tool.started_at.is_none());
    assert!(requested_tool.completed_at.is_none());
}

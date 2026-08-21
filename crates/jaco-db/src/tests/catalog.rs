use super::*;

#[test]
fn typed_json_roundtrips_for_repository_records() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
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
            pinned: false,
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
        .append_conversation_entry(message_item(&conversation.id, "hello json"))
        .unwrap();
    assert!(matches!(
        user_item.payload,
        ConversationEntryPayload::Message {
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
            conversation_id: conversation.id.clone(),
            trigger_kind: AgentRunTriggerKind::User,
            trigger_entry_id: user_item.id.clone(),
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
            error: None,
        })
        .unwrap();
    assert_eq!(
        provider_step.request_snapshot.snapshot_kind,
        ProviderStepSnapshotKind::RigCompletionRequest
    );
    assert!(provider_step.error.is_none());

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
        .record_tool_invocation_approval(
            &tool.id,
            approved_tool_invocation_approval(),
            ToolInvocationStatus::Succeeded,
        )
        .unwrap();
    assert_eq!(
        approval.approval.as_ref().map(|approval| &approval.request),
        Some(&approval_request())
    );
    assert_eq!(
        approval.approval.as_ref().map(|approval| approval.status),
        Some(ApprovalStatus::Approved)
    );
    assert_eq!(
        approval.approval.and_then(|approval| approval.decision),
        Some(approval_decision())
    );

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
    assert_eq!(
        repo.get_shortcut(&shortcut.id).unwrap().unwrap().hotkey,
        "cmd+shift+j"
    );
    let shortcuts = repo.list_shortcuts().unwrap();
    assert_eq!(shortcuts.len(), 1);
    assert_eq!(shortcuts[0].id, shortcut.id);

    let updated_shortcut = repo
        .update_shortcut(
            &shortcut.id,
            UpdateShortcut {
                hotkey: "cmd+shift+k".to_string(),
                enabled: false,
                prompt_id: None,
                provider_id: Some(provider.id.clone()),
                model_id: Some(model.model_id.clone()),
                input_source: ShortcutInputSource::Screenshot,
                action: ShortcutAction::OpenTemporaryConversation,
                settings_snapshot: run_settings(&provider.id, &model.model_id),
            },
        )
        .unwrap();
    assert_eq!(updated_shortcut.hotkey, "cmd+shift+k");
    assert!(!updated_shortcut.enabled);
    assert_eq!(updated_shortcut.prompt_id, None);
    assert_eq!(
        updated_shortcut.input_source,
        ShortcutInputSource::Screenshot
    );

    let enabled_shortcut = repo
        .set_shortcut_enabled(&updated_shortcut.id, true)
        .unwrap();
    assert!(enabled_shortcut.enabled);

    assert_eq!(repo.delete_shortcut(&enabled_shortcut.id).unwrap(), 1);
    assert!(repo.list_shortcuts().unwrap().is_empty());
}

#[test]
fn provider_model_manual_refresh_updates_cached_row() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
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
fn provider_model_pricing_roundtrips_and_route_change_clears_cached_price() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let provider = repo.insert_provider(provider()).unwrap();
    let pricing = model_pricing(
        "gpt-5.6",
        2_500_000_000,
        15_000_000_000,
        Some(250_000_000),
        None,
    );
    let mut input = provider_model(&provider.id, "gpt-5.6", "GPT-5.6");
    input.pricing = Some(pricing.clone());

    let stored = repo.upsert_provider_model(input).unwrap();
    assert_eq!(stored.pricing, Some(pricing.clone()));
    assert_eq!(
        repo.list_provider_models(&provider.id).unwrap()[0].pricing,
        Some(pricing)
    );

    let mut custom_settings = provider_settings();
    custom_settings.fields[0].value = ProviderSettingValue::String {
        value: "https://proxy.example.com/v1".to_string(),
    };
    repo.update_provider(
        &provider.id,
        UpdateProvider {
            display_name: provider.display_name,
            enabled: provider.enabled,
            settings: custom_settings,
            secret_refs: provider.secret_refs,
        },
    )
    .unwrap();

    assert_eq!(
        repo.get_provider_model(&provider.id, "gpt-5.6")
            .unwrap()
            .unwrap()
            .pricing,
        None
    );
}

#[test]
fn provider_repository_lists_updates_and_deletes_provider_rows() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let provider = repo.insert_provider(provider()).unwrap();

    let listed = repo.list_providers().unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, provider.id);

    let updated = repo
        .update_provider(
            &provider.id,
            UpdateProvider {
                display_name: "OpenAI API".to_string(),
                enabled: false,
                settings: provider_settings(),
                secret_refs: ProviderSecretRefs { refs: Vec::new() },
            },
        )
        .unwrap();
    assert_eq!(updated.display_name, "OpenAI API");
    assert!(!updated.enabled);
    assert!(updated.secret_refs.refs.is_empty());

    assert_eq!(repo.delete_provider(&provider.id).unwrap(), 1);
    assert!(repo.get_provider(&provider.id).unwrap().is_none());
}

#[test]
fn provider_repository_can_insert_with_preallocated_id() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let provider_id = "provider-preallocated-id".to_string();

    let provider = repo
        .insert_provider_with_id(provider_id.clone(), provider())
        .unwrap();

    assert_eq!(provider.id, provider_id);
    assert_eq!(
        repo.get_provider(&provider_id).unwrap().unwrap().id,
        provider_id
    );
}

#[test]
fn prompt_repository_lists_updates_and_deletes_prompt_rows() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let second = repo
        .insert_prompt(NewPrompt {
            name: "Second".to_string(),
            content: PromptContent {
                text: "Second prompt".to_string(),
            },
            enabled: true,
            sort_order: 20,
        })
        .unwrap();
    let first = repo
        .insert_prompt(NewPrompt {
            name: "First".to_string(),
            content: PromptContent {
                text: "First prompt".to_string(),
            },
            enabled: true,
            sort_order: 10,
        })
        .unwrap();

    let listed = repo.list_prompts().unwrap();
    assert_eq!(
        listed.iter().map(|prompt| &prompt.id).collect::<Vec<_>>(),
        vec![&first.id, &second.id]
    );

    let updated = repo
        .update_prompt(
            &first.id,
            UpdatePrompt {
                name: "Updated".to_string(),
                content: PromptContent {
                    text: "Updated prompt".to_string(),
                },
                enabled: false,
                sort_order: 30,
            },
        )
        .unwrap();
    assert_eq!(updated.name, "Updated");
    assert_eq!(updated.content.text, "Updated prompt");
    assert!(!updated.enabled);
    assert_eq!(updated.sort_order, 30);

    assert_eq!(repo.delete_prompt(&updated.id).unwrap(), 1);
    assert!(repo.get_prompt(&updated.id).unwrap().is_none());
}

#[test]
fn provider_model_repository_lists_toggles_replaces_and_deletes_rows() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let provider = repo.insert_provider(provider()).unwrap();

    let model = repo
        .upsert_provider_model(provider_model(&provider.id, "gpt-5.2", "GPT-5.2"))
        .unwrap();
    assert!(model.enabled);
    assert_eq!(repo.list_provider_models(&provider.id).unwrap().len(), 1);

    let disabled = repo
        .set_provider_model_enabled(&provider.id, "gpt-5.2", false)
        .unwrap();
    assert!(!disabled.enabled);

    let refreshed = repo
        .replace_fetched_provider_models(
            &provider.id,
            vec![
                provider_model(&provider.id, "gpt-5.2", "GPT-5.2 Fresh"),
                provider_model(&provider.id, "gpt-4.1", "GPT-4.1"),
            ],
        )
        .unwrap();
    assert_eq!(refreshed.len(), 2);

    let existing = repo
        .get_provider_model(&provider.id, "gpt-5.2")
        .unwrap()
        .unwrap();
    assert_eq!(existing.display_name.as_deref(), Some("GPT-5.2 Fresh"));
    assert!(!existing.enabled);

    let new_model = repo
        .get_provider_model(&provider.id, "gpt-4.1")
        .unwrap()
        .unwrap();
    assert!(new_model.enabled);

    let refreshed = repo
        .replace_fetched_provider_models(
            &provider.id,
            vec![provider_model(&provider.id, "gpt-5.2", "GPT-5.2 Latest")],
        )
        .unwrap();
    assert_eq!(refreshed.len(), 1);

    let existing = repo
        .get_provider_model(&provider.id, "gpt-5.2")
        .unwrap()
        .unwrap();
    assert_eq!(existing.display_name.as_deref(), Some("GPT-5.2 Latest"));
    assert!(!existing.enabled);
    assert!(
        repo.get_provider_model(&provider.id, "gpt-4.1")
            .unwrap()
            .is_none()
    );

    assert_eq!(
        repo.delete_provider_model(&provider.id, "gpt-5.2").unwrap(),
        1
    );
    assert!(
        repo.get_provider_model(&provider.id, "gpt-5.2")
            .unwrap()
            .is_none()
    );
}

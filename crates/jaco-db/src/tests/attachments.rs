use super::*;

#[test]
fn conversation_timeline_includes_attachments() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let project = repo
        .insert_project(project("timeline-attachments"))
        .unwrap();
    let conversation = repo.insert_conversation(conversation(&project)).unwrap();
    let item = repo
        .append_conversation_entry(message_item(&conversation.id, "check this"))
        .unwrap();
    let attachment = repo
        .insert_attachment(NewAttachment {
            id: "attachment-timeline".to_string(),
            conversation_id: conversation.id.clone(),
            kind: AttachmentKind::File,
            storage_kind: AttachmentStorageKind::LocalFile,
            mime_type: Some("text/plain".to_string()),
            name: Some("notes.txt".to_string()),
            path: Some("/tmp/notes.txt".to_string()),
            external_uri: None,
            provider_id: None,
            provider_file_id: None,
            sha256: None,
            size_bytes: Some(42),
            metadata: attachment_metadata(),
        })
        .unwrap();

    let timeline = repo
        .conversation_timeline_records(&conversation.id)
        .unwrap()
        .unwrap();

    assert_eq!(timeline.items.len(), 1);
    assert_eq!(timeline.items[0].id, item.id);
    assert_eq!(timeline.attachments, vec![attachment]);
}

#[test]
fn multimodal_user_message_persists_text_and_attachments_in_one_entry() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let project = repo.insert_project(project("multimodal-entry")).unwrap();
    let conversation = repo.insert_conversation(conversation(&project)).unwrap();

    let entry = repo
        .append_conversation_entry_with_attachments(
            NewConversationEntry {
                conversation_id: conversation.id.clone(),
                status: ConversationEntryStatus::Completed,
                agent_run_id: None,
                provider_step_id: None,
                tool_invocation_id: None,
                provider_item_id: None,
                payload: ConversationEntryPayload::Message {
                    role: TranscriptRole::User,
                    content: vec![ContentPart::Text {
                        text: "describe these files".to_string(),
                    }],
                },
            },
            vec![
                NewAttachment {
                    id: "attachment-screenshot".to_string(),
                    conversation_id: conversation.id.clone(),
                    kind: AttachmentKind::Image,
                    storage_kind: AttachmentStorageKind::LocalFile,
                    mime_type: Some("image/png".to_string()),
                    name: Some("screenshot.png".to_string()),
                    path: Some("/tmp/screenshot.png".to_string()),
                    external_uri: None,
                    provider_id: None,
                    provider_file_id: None,
                    sha256: None,
                    size_bytes: Some(4),
                    metadata: AttachmentMetadata {
                        source: AttachmentSource::LocalFile {
                            path: "/tmp/screenshot.png".to_string(),
                        },
                        width: Some(640),
                        height: Some(480),
                        duration_ms: None,
                        preview_attachment_id: None,
                    },
                },
                NewAttachment {
                    id: "attachment-notes".to_string(),
                    conversation_id: conversation.id.clone(),
                    kind: AttachmentKind::File,
                    storage_kind: AttachmentStorageKind::LocalFile,
                    mime_type: Some("text/plain".to_string()),
                    name: Some("notes.txt".to_string()),
                    path: Some("/tmp/notes.txt".to_string()),
                    external_uri: None,
                    provider_id: None,
                    provider_file_id: None,
                    sha256: None,
                    size_bytes: Some(42),
                    metadata: attachment_metadata(),
                },
            ],
        )
        .unwrap();

    let attachments = repo.conversation_attachments(&conversation.id).unwrap();
    assert_eq!(attachments.len(), 2);
    let image = attachments
        .iter()
        .find(|attachment| attachment.kind == AttachmentKind::Image)
        .unwrap();
    let file = attachments
        .iter()
        .find(|attachment| attachment.kind == AttachmentKind::File)
        .unwrap();
    assert!(matches!(
        &entry.payload,
        ConversationEntryPayload::Message {
            role: TranscriptRole::User,
            content,
        } if content == &vec![
            ContentPart::Text {
                text: "describe these files".to_string(),
            },
            ContentPart::Image {
                attachment_id: image.id.clone(),
            },
            ContentPart::File {
                attachment_id: file.id.clone(),
            },
        ]
    ));

    let timeline = repo
        .conversation_timeline_records(&conversation.id)
        .unwrap()
        .unwrap();
    assert_eq!(timeline.items, vec![entry.value]);
    assert_eq!(timeline.attachments, attachments);
}

#[test]
fn attachment_insert_uses_caller_assigned_id_and_rejects_duplicates() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let project = repo
        .insert_project(project("caller-attachment-id"))
        .unwrap();
    let conversation = repo.insert_conversation(conversation(&project)).unwrap();
    let attachment = NewAttachment {
        id: "stable-attachment-id".to_string(),
        conversation_id: conversation.id.clone(),
        kind: AttachmentKind::File,
        storage_kind: AttachmentStorageKind::LocalFile,
        mime_type: Some("text/plain".to_string()),
        name: Some("notes.txt".to_string()),
        path: Some("/tmp/notes.txt".to_string()),
        external_uri: None,
        provider_id: None,
        provider_file_id: None,
        sha256: None,
        size_bytes: Some(42),
        metadata: attachment_metadata(),
    };

    let inserted = repo.insert_attachment(attachment.clone()).unwrap();
    assert_eq!(inserted.id, attachment.id);
    assert!(repo.insert_attachment(attachment).is_err());
    assert_eq!(
        repo.conversation_attachments(&conversation.id)
            .unwrap()
            .len(),
        1
    );
}

fn generated_attachment(
    id: &str,
    conversation_id: &str,
    provider_id: Option<String>,
) -> NewAttachment {
    let path = format!("/tmp/{id}.png");
    NewAttachment {
        id: id.to_string(),
        conversation_id: conversation_id.to_string(),
        kind: AttachmentKind::Image,
        storage_kind: AttachmentStorageKind::GeneratedFile,
        mime_type: Some("image/png".to_string()),
        name: Some(format!("{id}.png")),
        path: Some(path.clone()),
        external_uri: None,
        provider_id,
        provider_file_id: None,
        sha256: Some(format!("hash-{id}")),
        size_bytes: Some(4),
        metadata: AttachmentMetadata {
            source: AttachmentSource::GeneratedFile { path },
            width: Some(1),
            height: Some(1),
            duration_ms: None,
            preview_attachment_id: None,
        },
    }
}

fn completed_batch_context(
    repo: &crate::FreshRepository,
    suffix: &str,
) -> (
    crate::ConversationRecord,
    crate::ProviderRecord,
    crate::AgentRunRecord,
    crate::ProviderStepRecord,
) {
    let project = repo.insert_project(project(suffix)).unwrap();
    let conversation = repo.insert_conversation(conversation(&project)).unwrap();
    let trigger = repo
        .append_conversation_entry(message_item(&conversation.id, "generate an image"))
        .unwrap();
    let provider = repo.insert_provider(provider()).unwrap();
    let model = repo
        .upsert_provider_model(provider_model(&provider.id, "image-model", "Image Model"))
        .unwrap();
    let run = repo
        .insert_agent_run(NewAgentRun {
            conversation_id: conversation.id.clone(),
            trigger_kind: AgentRunTriggerKind::User,
            trigger_entry_id: trigger.id.clone(),
            input: agent_run_input(&trigger.id, &provider.id, &model.model_id),
        })
        .unwrap();
    let step = repo
        .insert_provider_step(NewProviderStep {
            agent_run_id: run.id.clone(),
            seq: 1,
            status: ProviderStepStatus::Completed,
            request_snapshot: provider_step_request(&provider.id, &model.model_id, &trigger.id),
            response_snapshot: Some(provider_step_response()),
            state_snapshot: Some(provider_run_state(&provider.id)),
            settings_snapshot: run_settings(&provider.id, &model.model_id),
            error: None,
        })
        .unwrap();
    (conversation, provider, run, step)
}

fn generated_batch_entry(
    conversation_id: &str,
    run_id: &str,
    step_id: &str,
    content: Vec<ContentPart>,
    attachments: Vec<NewAttachment>,
) -> NewConversationEntryBatchItem {
    NewConversationEntryBatchItem {
        entry: NewConversationEntry {
            conversation_id: conversation_id.to_string(),
            status: ConversationEntryStatus::Completed,
            agent_run_id: Some(run_id.to_string()),
            provider_step_id: Some(step_id.to_string()),
            tool_invocation_id: None,
            provider_item_id: None,
            payload: ConversationEntryPayload::Message {
                role: TranscriptRole::Assistant,
                content,
            },
        },
        attachments,
    }
}

#[test]
fn prelinked_batch_persists_ordered_entries_attachments_and_lineage() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let (conversation, provider, run, step) = completed_batch_context(&repo, "prelinked-batch");
    let image_b = generated_attachment("generated-b", &conversation.id, Some(provider.id.clone()));
    let image_a = generated_attachment("generated-a", &conversation.id, Some(provider.id.clone()));
    let content = vec![
        ContentPart::Text {
            text: "before".to_string(),
        },
        ContentPart::Image {
            attachment_id: image_b.id.clone(),
        },
        ContentPart::Text {
            text: "between".to_string(),
        },
        ContentPart::Image {
            attachment_id: image_a.id.clone(),
        },
    ];
    let reasoning = NewConversationEntryBatchItem {
        entry: NewConversationEntry {
            conversation_id: conversation.id.clone(),
            status: ConversationEntryStatus::Completed,
            agent_run_id: Some(run.id.clone()),
            provider_step_id: Some(step.id.clone()),
            tool_invocation_id: None,
            provider_item_id: None,
            payload: ConversationEntryPayload::Reasoning {
                text: "reasoning".to_string(),
                summary: None,
            },
        },
        attachments: Vec::new(),
    };

    let commit = repo
        .append_conversation_entries_with_attachments(vec![
            reasoning,
            generated_batch_entry(
                &conversation.id,
                &run.id,
                &step.id,
                content.clone(),
                vec![image_a, image_b],
            ),
        ])
        .unwrap();

    assert_eq!(commit.entries.len(), 2);
    assert_eq!(commit.entries[0].seq, 2);
    assert_eq!(commit.entries[1].seq, 3);
    assert!(matches!(
        &commit.entries[1].payload,
        ConversationEntryPayload::Message { content: actual, .. } if actual == &content
    ));
    assert_eq!(
        commit
            .attachments
            .iter()
            .map(|attachment| attachment.id.as_str())
            .collect::<Vec<_>>(),
        vec!["generated-b", "generated-a"]
    );
    assert_eq!(commit.conversation.last_entry_seq, 3);
    assert!(commit.entries.iter().all(|entry| {
        entry.agent_run_id.as_deref() == Some(run.id.as_str())
            && entry.provider_step_id.as_deref() == Some(step.id.as_str())
    }));

    let timeline = repo
        .conversation_timeline_records(&conversation.id)
        .unwrap()
        .unwrap();
    assert_eq!(timeline.items[1..], commit.entries);
    repo.insert_attachment(NewAttachment {
        id: "local-not-generated".to_string(),
        conversation_id: conversation.id.clone(),
        kind: AttachmentKind::Image,
        storage_kind: AttachmentStorageKind::LocalFile,
        mime_type: Some("image/png".to_string()),
        name: Some("local.png".to_string()),
        path: Some("/tmp/local.png".to_string()),
        external_uri: None,
        provider_id: None,
        provider_file_id: None,
        sha256: None,
        size_bytes: Some(4),
        metadata: AttachmentMetadata {
            source: AttachmentSource::LocalFile {
                path: "/tmp/local.png".to_string(),
            },
            width: Some(1),
            height: Some(1),
            duration_ms: None,
            preview_attachment_id: None,
        },
    })
    .unwrap();
    let other_project = repo
        .insert_project(project("generated-index-other"))
        .unwrap();
    let other_conversation = repo
        .insert_conversation(super::conversation(&other_project))
        .unwrap();
    repo.insert_attachment(generated_attachment(
        "generated-other",
        &other_conversation.id,
        None,
    ))
    .unwrap();
    let generated = repo.generated_file_attachments().unwrap();
    assert_eq!(generated.len(), 3);
    assert!(
        generated
            .iter()
            .all(|attachment| { attachment.storage_kind == AttachmentStorageKind::GeneratedFile })
    );
    let keys = generated
        .iter()
        .map(|attachment| (attachment.conversation_id.clone(), attachment.id.clone()))
        .collect::<Vec<_>>();
    let mut sorted_keys = keys.clone();
    sorted_keys.sort();
    assert_eq!(keys, sorted_keys);
    assert_eq!(
        generated
            .iter()
            .map(|attachment| attachment.id.as_str())
            .collect::<HashSet<_>>(),
        HashSet::from(["generated-a", "generated-b", "generated-other"])
    );
}

#[test]
fn prelinked_batch_rejects_invalid_graphs_before_writes() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let (conversation, _, run, step) = completed_batch_context(&repo, "prelinked-invalid");
    let before = repo.get_conversation(&conversation.id).unwrap().unwrap();
    let unused = generated_attachment("generated-unused", &conversation.id, None);
    let unused_attachment = generated_batch_entry(
        &conversation.id,
        &run.id,
        &step.id,
        vec![ContentPart::Text {
            text: "missing image reference".to_string(),
        }],
        vec![unused],
    );
    let unprovided = generated_batch_entry(
        &conversation.id,
        &run.id,
        &step.id,
        vec![ContentPart::Image {
            attachment_id: "generated-unprovided".to_string(),
        }],
        Vec::new(),
    );
    let duplicated = generated_attachment("generated-duplicated", &conversation.id, None);
    let duplicate_id = generated_batch_entry(
        &conversation.id,
        &run.id,
        &step.id,
        vec![ContentPart::Image {
            attachment_id: duplicated.id.clone(),
        }],
        vec![duplicated.clone(), duplicated],
    );
    let repeated = generated_attachment("generated-repeated", &conversation.id, None);
    let repeated_reference = generated_batch_entry(
        &conversation.id,
        &run.id,
        &step.id,
        vec![
            ContentPart::Image {
                attachment_id: repeated.id.clone(),
            },
            ContentPart::Image {
                attachment_id: repeated.id.clone(),
            },
        ],
        vec![repeated],
    );
    let wrong_role = NewConversationEntryBatchItem {
        entry: NewConversationEntry {
            conversation_id: conversation.id.clone(),
            status: ConversationEntryStatus::Completed,
            agent_run_id: Some(run.id.clone()),
            provider_step_id: Some(step.id.clone()),
            tool_invocation_id: None,
            provider_item_id: None,
            payload: ConversationEntryPayload::Message {
                role: TranscriptRole::User,
                content: vec![ContentPart::Text {
                    text: "not assistant".to_string(),
                }],
            },
        },
        attachments: Vec::new(),
    };
    let mut wrong_kind = generated_attachment("generated-wrong-kind", &conversation.id, None);
    wrong_kind.kind = AttachmentKind::File;
    let wrong_kind = generated_batch_entry(
        &conversation.id,
        &run.id,
        &step.id,
        vec![ContentPart::Image {
            attachment_id: wrong_kind.id.clone(),
        }],
        vec![wrong_kind],
    );
    let cross_item = generated_attachment("generated-cross-item", &conversation.id, None);
    let cross_item_batches = vec![
        generated_batch_entry(
            &conversation.id,
            &run.id,
            &step.id,
            vec![ContentPart::Text {
                text: "first".to_string(),
            }],
            vec![cross_item.clone()],
        ),
        generated_batch_entry(
            &conversation.id,
            &run.id,
            &step.id,
            vec![ContentPart::Image {
                attachment_id: cross_item.id,
            }],
            Vec::new(),
        ),
    ];

    assert!(
        repo.append_conversation_entries_with_attachments(Vec::new())
            .is_err()
    );
    for invalid in [
        unused_attachment,
        unprovided,
        duplicate_id,
        repeated_reference,
        wrong_role,
        wrong_kind,
    ] {
        assert!(
            repo.append_conversation_entries_with_attachments(vec![invalid])
                .is_err()
        );
    }
    assert!(
        repo.append_conversation_entries_with_attachments(cross_item_batches)
            .is_err()
    );
    let after = repo.get_conversation(&conversation.id).unwrap().unwrap();
    assert_eq!(after.last_entry_seq, before.last_entry_seq);
    assert_eq!(after.updated_at, before.updated_at);
    assert_eq!(after.recency_at, before.recency_at);
    assert!(
        repo.conversation_attachments(&conversation.id)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn prelinked_batch_rolls_back_prior_inserts_when_later_attachment_fails() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let (conversation, provider, run, step) = completed_batch_context(&repo, "prelinked-rollback");
    let before = repo.get_conversation(&conversation.id).unwrap().unwrap();
    let first = generated_attachment(
        "generated-first",
        &conversation.id,
        Some(provider.id.clone()),
    );
    let second = generated_attachment(
        "generated-second",
        &conversation.id,
        Some("missing-provider".to_string()),
    );
    let batch = generated_batch_entry(
        &conversation.id,
        &run.id,
        &step.id,
        vec![
            ContentPart::Image {
                attachment_id: first.id.clone(),
            },
            ContentPart::Image {
                attachment_id: second.id.clone(),
            },
        ],
        vec![first, second],
    );

    assert!(
        repo.append_conversation_entries_with_attachments(vec![batch])
            .is_err()
    );
    let after = repo.get_conversation(&conversation.id).unwrap().unwrap();
    assert_eq!(after.last_entry_seq, before.last_entry_seq);
    assert_eq!(after.updated_at, before.updated_at);
    assert_eq!(after.recency_at, before.recency_at);
    assert!(
        repo.conversation_attachments(&conversation.id)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        repo.conversation_entries(&conversation.id).unwrap().len(),
        1
    );
}

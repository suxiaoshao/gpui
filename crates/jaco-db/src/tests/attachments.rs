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

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use crate::{AgentRuntimeError, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use jaco_core::*;
use jaco_db::{AttachmentRecord, ConversationEntryRecord};
use rig_core::{
    OneOrMany,
    completion::{AssistantContent, Message as RigMessage},
    message::{
        self, DocumentMediaType, ImageDetail, ImageMediaType, MimeType, ToolCall, ToolFunction,
        ToolResult, ToolResultContent, UserContent,
    },
};

type AttachmentMap<'a> = HashMap<&'a str, &'a AttachmentRecord>;

pub(crate) struct PromptHistory {
    pub(crate) prompt: RigMessage,
    pub(crate) history: Vec<RigMessage>,
    pub(crate) input_item_ids: Vec<ConversationEntryId>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PromptHistoryOptions {
    pub(crate) include_reasoning: bool,
    pub(crate) preserve_tool_protocol: bool,
}

impl Default for PromptHistoryOptions {
    fn default() -> Self {
        Self {
            include_reasoning: true,
            preserve_tool_protocol: true,
        }
    }
}

pub(crate) fn build_prompt_history_with_options(
    items: &[ConversationEntryRecord],
    attachments: &[AttachmentRecord],
    trigger_entry_id: &str,
    agent_run_id: &str,
    options: PromptHistoryOptions,
) -> Result<PromptHistory> {
    let attachment_map = attachment_map(attachments);
    let user_index = items
        .iter()
        .position(|item| item.id == trigger_entry_id)
        .ok_or_else(|| {
            AgentRuntimeError::Invariant(format!("user item {trigger_entry_id} is missing"))
        })?;
    let current_run_skill_items = items[user_index + 1..]
        .iter()
        .filter(|item| {
            item.agent_run_id.as_deref() == Some(agent_run_id)
                && matches!(item.payload, ConversationEntryPayload::SkillActivation(_))
        })
        .collect::<Vec<_>>();
    let prompt = if current_run_skill_items.is_empty() {
        conversation_entry_to_rig_message_with_options(
            &items[user_index],
            &attachment_map,
            options,
        )?
        .ok_or_else(|| {
            AgentRuntimeError::Invariant(format!(
                "user item {trigger_entry_id} cannot be used as prompt"
            ))
        })?
    } else {
        user_prompt_with_skill_context(
            &items[user_index],
            &current_run_skill_items,
            &attachment_map,
        )?
    };
    let history = items[..user_index]
        .iter()
        .filter_map(|item| {
            conversation_entry_to_rig_message_with_options(item, &attachment_map, options)
                .transpose()
        })
        .collect::<Result<Vec<_>>>()?;
    let mut input_item_ids = items[..=user_index]
        .iter()
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    input_item_ids.extend(current_run_skill_items.iter().map(|item| item.id.clone()));
    Ok(PromptHistory {
        prompt,
        history,
        input_item_ids,
    })
}

#[cfg(test)]
pub(crate) fn conversation_entry_to_rig_message(
    item: &ConversationEntryRecord,
    attachments: &AttachmentMap<'_>,
) -> Result<Option<RigMessage>> {
    conversation_entry_to_rig_message_with_options(
        item,
        attachments,
        PromptHistoryOptions::default(),
    )
}

fn conversation_entry_to_rig_message_with_options(
    item: &ConversationEntryRecord,
    attachments: &AttachmentMap<'_>,
    options: PromptHistoryOptions,
) -> Result<Option<RigMessage>> {
    Ok(match &item.payload {
        ConversationEntryPayload::Message { role, content } => match role {
            TranscriptRole::System => Some(RigMessage::system(content_text(content))),
            TranscriptRole::Developer => Some(RigMessage::system(format!(
                "Developer instruction:\n{}",
                content_text(content)
            ))),
            TranscriptRole::User => {
                let content = user_content_parts(content, attachments)?;
                if content.is_empty() {
                    None
                } else {
                    Some(RigMessage::User {
                        content: one_or_many_user_content(content, &item.id)?,
                    })
                }
            }
            TranscriptRole::Assistant => Some(RigMessage::assistant(content_text(content))),
            TranscriptRole::Tool => Some(RigMessage::user(content_text(content))),
        },
        ConversationEntryPayload::SkillActivation(skill) => {
            Some(RigMessage::user(skill_activation_context(skill)))
        }
        ConversationEntryPayload::Reasoning { text, summary } if options.include_reasoning => {
            let reasoning = summary.as_ref().map_or_else(
                || rig_core::message::Reasoning::new(text),
                |summary| rig_core::message::Reasoning::summaries(vec![summary.clone()]),
            );
            Some(RigMessage::Assistant {
                id: item.provider_item_id.clone(),
                content: OneOrMany::one(AssistantContent::Reasoning(reasoning)),
            })
        }
        ConversationEntryPayload::Reasoning { .. } => None,
        ConversationEntryPayload::ToolCall(_) if !options.preserve_tool_protocol => None,
        ConversationEntryPayload::ToolCall(call) => Some(RigMessage::Assistant {
            id: item.provider_item_id.clone(),
            content: OneOrMany::one(AssistantContent::ToolCall(
                ToolCall::new(
                    call.call_id.clone(),
                    ToolFunction::new(call.runtime_tool_name.clone(), call.arguments.value.clone()),
                )
                .with_call_id(call.call_id.clone()),
            )),
        }),
        ConversationEntryPayload::ToolResult(result) if !options.preserve_tool_protocol => {
            Some(RigMessage::user(textualized_tool_result(result)))
        }
        ConversationEntryPayload::ToolResult(result) => Some(RigMessage::User {
            content: OneOrMany::one(UserContent::ToolResult(ToolResult {
                id: result.call_id.clone(),
                call_id: Some(result.call_id.clone()),
                content: OneOrMany::one(ToolResultContent::text(tool_result_model_text(result))),
            })),
        }),
        ConversationEntryPayload::Error(error) => Some(RigMessage::system(format!(
            "Previous run error [{}]: {}",
            error.code, error.message
        ))),
        ConversationEntryPayload::ApprovalRequest(_)
        | ConversationEntryPayload::ApprovalDecision(_)
        | ConversationEntryPayload::Status(_) => None,
    })
}

fn attachment_map(attachments: &[AttachmentRecord]) -> AttachmentMap<'_> {
    attachments
        .iter()
        .map(|attachment| (attachment.id.as_str(), attachment))
        .collect()
}

fn user_content_parts(
    content: &[ContentPart],
    attachments: &AttachmentMap<'_>,
) -> Result<Vec<UserContent>> {
    let mut result = Vec::new();
    for part in content {
        match part {
            ContentPart::Text { text } => {
                if !text.is_empty() {
                    result.push(UserContent::text(text.clone()));
                }
            }
            ContentPart::Image { attachment_id } => {
                result.push(image_attachment_content(attachment_id, attachments)?);
            }
            ContentPart::File { attachment_id } | ContentPart::Attachment { attachment_id } => {
                result.push(file_attachment_content(attachment_id, attachments)?);
            }
            ContentPart::Audio { attachment_id } => {
                return Err(AgentRuntimeError::Unsupported(format!(
                    "audio attachment {attachment_id} cannot be sent to the model yet"
                )));
            }
        }
    }
    Ok(result)
}

fn one_or_many_user_content(
    content: Vec<UserContent>,
    item_id: &str,
) -> Result<OneOrMany<UserContent>> {
    OneOrMany::many(content).map_err(|_| {
        AgentRuntimeError::Invariant(format!("message item {item_id} has no model content"))
    })
}

fn image_attachment_content(
    attachment_id: &str,
    attachments: &AttachmentMap<'_>,
) -> Result<UserContent> {
    let attachment = required_attachment(attachment_id, attachments)?;
    let media_type = image_media_type(attachment).ok_or_else(|| {
        AgentRuntimeError::Unsupported(format!(
            "image attachment {attachment_id} has unsupported media type"
        ))
    })?;

    if let Some(uri) = attachment_uri(attachment) {
        return Ok(UserContent::image_url(
            uri,
            Some(media_type),
            Some(ImageDetail::Auto),
        ));
    }

    let path = attachment_local_path(attachment).ok_or_else(|| {
        AgentRuntimeError::Unsupported(format!(
            "image attachment {attachment_id} does not have a readable source"
        ))
    })?;
    let data = STANDARD.encode(fs::read(&path)?);
    Ok(UserContent::image_base64(
        data,
        Some(media_type),
        Some(ImageDetail::Auto),
    ))
}

fn file_attachment_content(
    attachment_id: &str,
    attachments: &AttachmentMap<'_>,
) -> Result<UserContent> {
    let attachment = required_attachment(attachment_id, attachments)?;
    let media_type = document_media_type(attachment);

    if let Some(file_id) = attachment_provider_file_id(attachment) {
        return Ok(UserContent::Document(message::Document {
            data: message::DocumentSourceKind::FileId(file_id),
            media_type,
            additional_params: None,
        }));
    }

    if let Some(uri) = attachment_uri(attachment) {
        let media_type = media_type.ok_or_else(|| {
            AgentRuntimeError::Unsupported(format!(
                "file attachment {attachment_id} has unsupported media type"
            ))
        })?;
        return Ok(UserContent::document_url(uri, Some(media_type)));
    }

    let path = attachment_local_path(attachment).ok_or_else(|| {
        AgentRuntimeError::Unsupported(format!(
            "file attachment {attachment_id} does not have a readable source"
        ))
    })?;
    local_file_document_content(attachment_id, &path, media_type)
}

fn local_file_document_content(
    attachment_id: &str,
    path: &Path,
    media_type: Option<DocumentMediaType>,
) -> Result<UserContent> {
    let bytes = fs::read(path)?;
    if media_type == Some(DocumentMediaType::PDF) {
        return Ok(UserContent::Document(message::Document {
            data: message::DocumentSourceKind::Base64(STANDARD.encode(bytes)),
            media_type: Some(DocumentMediaType::PDF),
            additional_params: None,
        }));
    }

    match String::from_utf8(bytes) {
        Ok(text) => Ok(UserContent::document(
            text,
            Some(media_type.unwrap_or(DocumentMediaType::TXT)),
        )),
        Err(_) => Err(AgentRuntimeError::Unsupported(format!(
            "file attachment {attachment_id} is not a supported text or PDF document"
        ))),
    }
}

fn required_attachment<'a>(
    attachment_id: &str,
    attachments: &'a AttachmentMap<'_>,
) -> Result<&'a AttachmentRecord> {
    attachments.get(attachment_id).copied().ok_or_else(|| {
        AgentRuntimeError::Invariant(format!("attachment {attachment_id} is missing"))
    })
}

fn attachment_local_path(attachment: &AttachmentRecord) -> Option<PathBuf> {
    attachment
        .path
        .as_deref()
        .map(PathBuf::from)
        .or_else(|| match &attachment.metadata.source {
            AttachmentSource::LocalFile { path } | AttachmentSource::GeneratedFile { path } => {
                Some(PathBuf::from(path))
            }
            AttachmentSource::ExternalUri { .. } | AttachmentSource::ProviderFile { .. } => None,
        })
}

fn attachment_uri(attachment: &AttachmentRecord) -> Option<String> {
    attachment
        .external_uri
        .clone()
        .or_else(|| match &attachment.metadata.source {
            AttachmentSource::ExternalUri { uri } => Some(uri.clone()),
            AttachmentSource::LocalFile { .. }
            | AttachmentSource::ProviderFile { .. }
            | AttachmentSource::GeneratedFile { .. } => None,
        })
}

fn attachment_provider_file_id(attachment: &AttachmentRecord) -> Option<String> {
    attachment
        .provider_file_id
        .clone()
        .or_else(|| match &attachment.metadata.source {
            AttachmentSource::ProviderFile { file_id, .. } => Some(file_id.clone()),
            AttachmentSource::LocalFile { .. }
            | AttachmentSource::ExternalUri { .. }
            | AttachmentSource::GeneratedFile { .. } => None,
        })
}

fn image_media_type(attachment: &AttachmentRecord) -> Option<ImageMediaType> {
    attachment
        .mime_type
        .as_deref()
        .and_then(ImageMediaType::from_mime_type)
        .or_else(|| {
            attachment
                .path
                .as_deref()
                .and_then(|path| Path::new(path).extension())
                .and_then(|extension| image_media_type_for_extension(&extension.to_string_lossy()))
        })
}

fn image_media_type_for_extension(extension: &str) -> Option<ImageMediaType> {
    match extension.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => Some(ImageMediaType::JPEG),
        "png" => Some(ImageMediaType::PNG),
        "gif" => Some(ImageMediaType::GIF),
        "webp" => Some(ImageMediaType::WEBP),
        _ => None,
    }
}

fn document_media_type(attachment: &AttachmentRecord) -> Option<DocumentMediaType> {
    attachment
        .mime_type
        .as_deref()
        .and_then(document_media_type_from_mime)
        .or_else(|| {
            attachment
                .path
                .as_deref()
                .and_then(|path| Path::new(path).extension())
                .and_then(|extension| {
                    document_media_type_for_extension(&extension.to_string_lossy())
                })
        })
}

fn document_media_type_from_mime(mime_type: &str) -> Option<DocumentMediaType> {
    match mime_type {
        "application/xml" => Some(DocumentMediaType::XML),
        "application/json" | "application/toml" | "application/yaml" => {
            Some(DocumentMediaType::TXT)
        }
        _ => DocumentMediaType::from_mime_type(mime_type),
    }
}

fn document_media_type_for_extension(extension: &str) -> Option<DocumentMediaType> {
    match extension.to_ascii_lowercase().as_str() {
        "pdf" => Some(DocumentMediaType::PDF),
        "txt" | "text" | "json" | "toml" | "yaml" | "yml" | "rs" => Some(DocumentMediaType::TXT),
        "rtf" => Some(DocumentMediaType::RTF),
        "html" | "htm" => Some(DocumentMediaType::HTML),
        "css" => Some(DocumentMediaType::CSS),
        "md" | "markdown" => Some(DocumentMediaType::MARKDOWN),
        "csv" => Some(DocumentMediaType::CSV),
        "xml" => Some(DocumentMediaType::XML),
        "js" | "mjs" | "cjs" => Some(DocumentMediaType::Javascript),
        "py" => Some(DocumentMediaType::Python),
        _ => None,
    }
}

fn tool_result_model_text(result: &ToolResultEntry) -> String {
    if let Some(structured) = result.structured_output.as_ref() {
        return structured.value.to_string();
    }
    content_text(&result.content)
}

fn textualized_tool_result(result: &ToolResultEntry) -> String {
    let status = if result.is_error { "error" } else { "success" };
    format!(
        "Approved tool call `{}` completed with {status} result:\n{}",
        result.call_id,
        tool_result_model_text(result)
    )
}

fn user_prompt_with_skill_context(
    user_item: &ConversationEntryRecord,
    skill_items: &[&ConversationEntryRecord],
    attachments: &AttachmentMap<'_>,
) -> Result<RigMessage> {
    let ConversationEntryPayload::Message {
        role: TranscriptRole::User,
        content,
    } = &user_item.payload
    else {
        return Err(AgentRuntimeError::Invariant(format!(
            "user item {} cannot be merged with skill context",
            user_item.id
        )));
    };

    let mut content = user_content_parts(content, attachments)?;
    for item in skill_items {
        let ConversationEntryPayload::SkillActivation(skill) = &item.payload else {
            continue;
        };
        content.push(UserContent::text(skill_activation_context(skill)));
    }
    Ok(RigMessage::User {
        content: one_or_many_user_content(content, &user_item.id)?,
    })
}

fn skill_activation_context(skill: &SkillActivationEntry) -> String {
    format!(
        "<skill>\n<name>{}</name>\n<path>{}</path>\n{}\n</skill>",
        skill.name,
        skill.skill_file_path,
        content_text(&skill.content)
    )
}

pub(crate) fn content_text(content: &[ContentPart]) -> String {
    content
        .iter()
        .filter_map(ContentPart::search_text)
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::OffsetDateTime;

    #[test]
    fn user_message_preserves_image_attachment_content() {
        let temp_dir = tempfile::tempdir().unwrap();
        let image_path = temp_dir.path().join("image.png");
        fs::write(&image_path, [0x89, b'P', b'N', b'G']).unwrap();
        let item = conversation_entry(
            "item-1",
            vec![
                ContentPart::Text {
                    text: "describe it".to_string(),
                },
                ContentPart::Image {
                    attachment_id: "att-1".to_string(),
                },
            ],
        );
        let attachment = attachment_record(
            "att-1",
            AttachmentKind::Image,
            Some("image/png"),
            Some(image_path.to_string_lossy().as_ref()),
        );
        let attachments = [attachment];
        let attachment_map = attachment_map(&attachments);

        let message = conversation_entry_to_rig_message(&item, &attachment_map)
            .unwrap()
            .unwrap();

        let RigMessage::User { content } = message else {
            panic!("expected user message");
        };
        let parts = content.iter().collect::<Vec<_>>();
        assert_eq!(parts.len(), 2);
        assert!(matches!(parts[0], UserContent::Text(_)));
        let UserContent::Image(image) = parts[1] else {
            panic!("expected image content");
        };
        assert_eq!(image.media_type.as_ref(), Some(&ImageMediaType::PNG));
        assert!(matches!(
            &image.data,
            message::DocumentSourceKind::Base64(_)
        ));
    }

    #[test]
    fn unsupported_binary_file_is_rejected() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("archive.zip");
        fs::write(&file_path, [0xff, 0x00, 0x10]).unwrap();
        let item = conversation_entry(
            "item-1",
            vec![ContentPart::File {
                attachment_id: "att-1".to_string(),
            }],
        );
        let attachment = attachment_record(
            "att-1",
            AttachmentKind::File,
            Some("application/zip"),
            Some(file_path.to_string_lossy().as_ref()),
        );
        let attachments = [attachment];
        let attachment_map = attachment_map(&attachments);

        let error = conversation_entry_to_rig_message(&item, &attachment_map).unwrap_err();

        assert!(matches!(error, AgentRuntimeError::Unsupported(_)));
    }

    #[test]
    fn prompt_history_includes_errors_but_skips_status_and_approval_entries() {
        let items = vec![
            conversation_entry_with_payload(
                "earlier-user",
                1,
                None,
                ConversationEntryKind::Message,
                ConversationEntryPayload::Message {
                    role: TranscriptRole::User,
                    content: vec![ContentPart::Text {
                        text: "earlier question".to_string(),
                    }],
                },
            ),
            conversation_entry_with_payload(
                "status",
                2,
                Some("run-1"),
                ConversationEntryKind::Status,
                ConversationEntryPayload::Status(ConversationStatusEntry {
                    code: ConversationStatusCode::CompletedWithoutOutput,
                    message: None,
                }),
            ),
            conversation_entry_with_payload(
                "approval",
                3,
                Some("run-2"),
                ConversationEntryKind::ApprovalRequest,
                ConversationEntryPayload::ApprovalRequest(ApprovalRequestEntry {
                    tool_invocation_id: "tool-1".to_string(),
                    request: ApprovalRequestPayload {
                        reason: "Read a file".to_string(),
                        tool_source: ToolSource::Mcp {
                            server_id: "filesystem".to_string(),
                        },
                        tool_name: "read_file".to_string(),
                        arguments_preview: "{}".to_string(),
                        access_requests: Vec::new(),
                    },
                }),
            ),
            conversation_entry_with_payload(
                "error",
                4,
                Some("run-3"),
                ConversationEntryKind::Error,
                ConversationEntryPayload::Error(RunErrorPayload {
                    code: "provider_error".to_string(),
                    message: "forced provider-open failure".to_string(),
                    retryable: true,
                    provider: Some("openai".to_string()),
                    raw: None,
                }),
            ),
            conversation_entry_with_payload(
                "current-user",
                5,
                None,
                ConversationEntryKind::Message,
                ConversationEntryPayload::Message {
                    role: TranscriptRole::User,
                    content: vec![ContentPart::Text {
                        text: "retry now".to_string(),
                    }],
                },
            ),
        ];

        let history = build_prompt_history_with_options(
            &items,
            &[],
            "current-user",
            "run-4",
            PromptHistoryOptions::default(),
        )
        .unwrap();

        assert_eq!(history.history.len(), 2);
        assert!(matches!(&history.history[0], RigMessage::User { .. }));
        assert!(matches!(
            &history.history[1],
            RigMessage::System { content }
                if content == "Previous run error [provider_error]: forced provider-open failure"
        ));
        assert_eq!(history.prompt, RigMessage::user("retry now"));
        assert_eq!(
            history.input_item_ids,
            vec![
                "earlier-user".to_string(),
                "status".to_string(),
                "approval".to_string(),
                "error".to_string(),
                "current-user".to_string(),
            ]
        );
    }

    fn conversation_entry(id: &str, content: Vec<ContentPart>) -> ConversationEntryRecord {
        conversation_entry_with_payload(
            id,
            1,
            None,
            ConversationEntryKind::Message,
            ConversationEntryPayload::Message {
                role: TranscriptRole::User,
                content,
            },
        )
    }

    fn conversation_entry_with_payload(
        id: &str,
        seq: i32,
        agent_run_id: Option<&str>,
        kind: ConversationEntryKind,
        payload: ConversationEntryPayload,
    ) -> ConversationEntryRecord {
        ConversationEntryRecord {
            id: id.to_string(),
            conversation_id: "conversation-1".to_string(),
            seq,
            kind,
            status: ConversationEntryStatus::Completed,
            agent_run_id: agent_run_id.map(str::to_string),
            provider_step_id: None,
            tool_invocation_id: None,
            provider_item_id: None,
            payload,
            search_text: String::new(),
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    fn attachment_record(
        id: &str,
        kind: AttachmentKind,
        mime_type: Option<&str>,
        path: Option<&str>,
    ) -> AttachmentRecord {
        let path = path.map(str::to_string);
        AttachmentRecord {
            id: id.to_string(),
            conversation_id: "conversation-1".to_string(),
            kind,
            storage_kind: AttachmentStorageKind::LocalFile,
            mime_type: mime_type.map(str::to_string),
            name: Some("attachment".to_string()),
            path: path.clone(),
            external_uri: None,
            provider_id: None,
            provider_file_id: None,
            sha256: None,
            size_bytes: None,
            metadata: AttachmentMetadata {
                source: AttachmentSource::LocalFile {
                    path: path.unwrap_or_default(),
                },
                width: None,
                height: None,
                duration_ms: None,
                preview_attachment_id: None,
            },
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }
    }
}

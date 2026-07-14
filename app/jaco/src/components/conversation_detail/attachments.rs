use std::{collections::HashMap, path::PathBuf};

use gpui::*;
use gpui_component::{ActiveTheme, h_flex};
use jaco_core::{
    AttachmentId, AttachmentKind, AttachmentSource, ContentPart, ConversationEntryPayload,
    TranscriptRole,
};
use jaco_db::{AttachmentRecord, ConversationEntryRecord};

use crate::components::image_preview::{self, ImagePreviewAttachment, ImagePreviewSource};

const USER_IMAGE_SIZE: f32 = 80.;
const USER_IMAGE_GAP: f32 = 8.;
const USER_IMAGE_RADIUS: f32 = 8.;
const USER_IMAGE_INNER_RADIUS: f32 = 6.;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct UserImageAttachment {
    id: AttachmentId,
    path: PathBuf,
    name: String,
    width: Option<u32>,
    height: Option<u32>,
}

impl UserImageAttachment {
    fn preview_attachment(&self) -> ImagePreviewAttachment {
        ImagePreviewAttachment {
            source: ImagePreviewSource::Path(self.path.clone()),
            name: self.name.clone(),
            width: self.width,
            height: self.height,
        }
    }
}

pub(super) fn attachments_by_id(
    attachments: &[AttachmentRecord],
) -> HashMap<AttachmentId, AttachmentRecord> {
    attachments
        .iter()
        .cloned()
        .map(|attachment| (attachment.id.clone(), attachment))
        .collect()
}

pub(super) fn user_image_attachments(
    item: &ConversationEntryRecord,
    attachments_by_id: &HashMap<AttachmentId, AttachmentRecord>,
) -> Vec<UserImageAttachment> {
    let ConversationEntryPayload::Message {
        role: TranscriptRole::User,
        content,
    } = &item.payload
    else {
        return Vec::new();
    };

    content
        .iter()
        .filter_map(|part| match part {
            ContentPart::Image { attachment_id } => attachments_by_id
                .get(attachment_id)
                .and_then(user_image_attachment_from_record),
            ContentPart::Text { .. }
            | ContentPart::File { .. }
            | ContentPart::Audio { .. }
            | ContentPart::Attachment { .. } => None,
        })
        .collect()
}

pub(super) fn render_user_image_attachments(
    message_id: &str,
    attachments: Vec<UserImageAttachment>,
    cx: &mut App,
) -> AnyElement {
    h_flex()
        .id(format!("conversation-user-images-{message_id}"))
        .max_w(px(680.))
        .justify_end()
        .gap(px(USER_IMAGE_GAP))
        .overflow_x_scroll()
        .children(
            attachments
                .into_iter()
                .map(|attachment| render_user_image_attachment(attachment, cx)),
        )
        .into_any_element()
}

fn render_user_image_attachment(attachment: UserImageAttachment, cx: &mut App) -> AnyElement {
    let attachment_id = attachment.id.clone();
    let image_path = attachment.path.clone();
    let preview_attachment = attachment.preview_attachment();
    div()
        .id(format!("conversation-user-image-{attachment_id}"))
        .flex_none()
        .size(px(USER_IMAGE_SIZE))
        .rounded(px(USER_IMAGE_RADIUS))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().muted.opacity(0.18))
        .overflow_hidden()
        .cursor(CursorStyle::PointingHand)
        .hover(|this| this.border_color(cx.theme().primary.opacity(0.55)))
        .on_click(move |_, window, cx| {
            image_preview::open_image_preview_dialog(preview_attachment.clone(), window, cx);
            cx.stop_propagation();
        })
        .child(
            img(image_path)
                .size_full()
                .rounded(px(USER_IMAGE_INNER_RADIUS))
                .object_fit(ObjectFit::Cover),
        )
        .into_any_element()
}

fn user_image_attachment_from_record(record: &AttachmentRecord) -> Option<UserImageAttachment> {
    if record.kind != AttachmentKind::Image {
        return None;
    }

    Some(UserImageAttachment {
        id: record.id.clone(),
        path: attachment_path(record)?,
        name: record
            .name
            .as_deref()
            .filter(|name| !name.trim().is_empty())
            .unwrap_or("image")
            .to_string(),
        width: record.metadata.width,
        height: record.metadata.height,
    })
}

fn attachment_path(record: &AttachmentRecord) -> Option<PathBuf> {
    record
        .path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| match &record.metadata.source {
            AttachmentSource::LocalFile { path } | AttachmentSource::GeneratedFile { path } => {
                (!path.trim().is_empty()).then(|| PathBuf::from(path))
            }
            AttachmentSource::ExternalUri { .. } | AttachmentSource::ProviderFile { .. } => None,
        })
}

#[cfg(test)]
mod tests {
    use super::{attachments_by_id, user_image_attachments};
    use jaco_core::{
        AttachmentKind, AttachmentMetadata, AttachmentSource, AttachmentStorageKind, ContentPart,
        ConversationEntryKind, ConversationEntryPayload, ConversationEntryStatus, TranscriptRole,
    };
    use jaco_db::{AttachmentRecord, ConversationEntryRecord};
    use std::path::PathBuf;
    use time::OffsetDateTime;

    #[test]
    fn extracts_user_image_attachments_in_content_order() {
        let item = user_message(vec![
            ContentPart::Text {
                text: "look".to_string(),
            },
            ContentPart::Image {
                attachment_id: "image-2".to_string(),
            },
            ContentPart::Image {
                attachment_id: "image-1".to_string(),
            },
            ContentPart::File {
                attachment_id: "file-1".to_string(),
            },
        ]);
        let attachments = attachments_by_id(&[
            image_record("image-1", "/tmp/one.png", 640, 480),
            image_record("image-2", "/tmp/two.png", 320, 240),
            file_record("file-1", "/tmp/file.txt"),
            image_record_without_path("missing-path"),
        ]);

        let images = user_image_attachments(&item, &attachments);

        assert_eq!(
            images
                .iter()
                .map(|attachment| attachment.id.as_str())
                .collect::<Vec<_>>(),
            vec!["image-2", "image-1"]
        );
        assert_eq!(images[0].path, PathBuf::from("/tmp/two.png"));
        assert_eq!(images[0].width, Some(320));
        assert_eq!(images[0].height, Some(240));
    }

    fn user_message(content: Vec<ContentPart>) -> ConversationEntryRecord {
        let now = OffsetDateTime::UNIX_EPOCH;
        ConversationEntryRecord {
            id: "item-1".to_string(),
            conversation_id: "conversation-1".to_string(),
            seq: 1,
            kind: ConversationEntryKind::Message,
            status: ConversationEntryStatus::Completed,
            agent_run_id: None,
            provider_step_id: None,
            tool_invocation_id: None,
            provider_item_id: None,
            payload: ConversationEntryPayload::Message {
                role: TranscriptRole::User,
                content,
            },
            search_text: "look".to_string(),
            created_at: now,
            updated_at: now,
        }
    }

    fn image_record(id: &str, path: &str, width: u32, height: u32) -> AttachmentRecord {
        let mut record = attachment_record(id, AttachmentKind::Image, path);
        record.metadata.width = Some(width);
        record.metadata.height = Some(height);
        record
    }

    fn image_record_without_path(id: &str) -> AttachmentRecord {
        let mut record = attachment_record(id, AttachmentKind::Image, "");
        record.path = None;
        record
    }

    fn file_record(id: &str, path: &str) -> AttachmentRecord {
        attachment_record(id, AttachmentKind::File, path)
    }

    fn attachment_record(id: &str, kind: AttachmentKind, path: &str) -> AttachmentRecord {
        let now = OffsetDateTime::UNIX_EPOCH;
        AttachmentRecord {
            id: id.to_string(),
            conversation_id: "conversation-1".to_string(),
            kind,
            storage_kind: AttachmentStorageKind::LocalFile,
            mime_type: None,
            name: Some(format!("{id}.png")),
            path: Some(path.to_string()),
            external_uri: None,
            provider_id: None,
            provider_file_id: None,
            sha256: None,
            size_bytes: None,
            metadata: AttachmentMetadata {
                source: AttachmentSource::LocalFile {
                    path: path.to_string(),
                },
                width: None,
                height: None,
                duration_ms: None,
                preview_attachment_id: None,
            },
            created_at: now,
            updated_at: now,
        }
    }
}

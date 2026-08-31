use std::{collections::HashMap, path::PathBuf};

use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    spinner::Spinner,
    text::{TextView, TextViewState},
    tooltip::Tooltip,
    v_flex,
};
use jaco_core::{
    AttachmentId, AttachmentKind, AttachmentSource, AttachmentStorageKind, ContentPart,
    ConversationAttachment, ConversationEntry, ConversationEntryId, ConversationEntryPayload,
};

use crate::{
    components::chat::image_preview::{self, ImagePreviewAttachment, ImagePreviewSource},
    foundation::{I18n, assets::IconName},
};

use super::attachment_access::{
    AttachmentAccessProblem, AttachmentAccessView, AttachmentAction, AttachmentActionTarget,
    AttachmentAvailability, AttachmentSourceHint, AttachmentSourceLabel, format_persisted_size,
    safe_display_name, safe_mime_type,
};
use super::message::OnAttachmentAction;

const USER_IMAGE_SIZE: f32 = 80.;
const USER_IMAGE_GAP: f32 = 8.;
const USER_IMAGE_RADIUS: f32 = 8.;
const USER_IMAGE_INNER_RADIUS: f32 = 6.;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) enum TimelineTextKey {
    WholeEntry(ConversationEntryId),
    MessageBlock {
        entry_id: ConversationEntryId,
        start_part_index: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum AttachmentCardKind {
    File,
    Attachment,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PersistedAttachmentCard {
    pub(super) attachment_id: AttachmentId,
    pub(super) kind: AttachmentCardKind,
    pub(super) part_index: usize,
    pub(super) display_name: String,
    pub(super) mime_type: Option<String>,
    pub(super) size_label: Option<String>,
    pub(super) source_hint: AttachmentSourceHint,
    pub(super) static_problem: Option<AttachmentAccessProblem>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum MessageContentBlock {
    Text {
        start_part_index: usize,
        markdown: String,
    },
    Images {
        start_part_index: usize,
        attachments: Vec<PersistedImageAttachment>,
    },
    File(PersistedAttachmentCard),
    Attachment(PersistedAttachmentCard),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MessageContentAppearance {
    User,
    Assistant,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PersistedImageAttachment {
    id: AttachmentId,
    name: String,
    width: Option<u32>,
    height: Option<u32>,
}

impl PersistedImageAttachment {
    pub(super) fn attachment_id(&self) -> &AttachmentId {
        &self.id
    }

    fn preview_attachment(&self, path: &std::path::Path) -> ImagePreviewAttachment {
        ImagePreviewAttachment {
            source: ImagePreviewSource::Path(path.to_path_buf()),
            name: self.name.clone(),
            width: self.width,
            height: self.height,
        }
    }
}

pub(super) fn attachments_by_id(
    attachments: &[ConversationAttachment],
) -> HashMap<AttachmentId, ConversationAttachment> {
    attachments
        .iter()
        .cloned()
        .map(|attachment| (attachment.id.clone(), attachment))
        .collect()
}

pub(super) fn project_message_content(
    item: &ConversationEntry,
    attachments_by_id: &HashMap<AttachmentId, ConversationAttachment>,
) -> Vec<MessageContentBlock> {
    let ConversationEntryPayload::Message { content, .. } = &item.payload else {
        return Vec::new();
    };

    let mut blocks = Vec::new();
    let mut text_start_part_index = None;
    let mut markdown = String::new();
    let mut image_start_part_index = None;
    let mut image_attachments = Vec::new();

    for (part_index, part) in content.iter().enumerate() {
        match part {
            ContentPart::Text { text } => {
                push_image_block(
                    &mut blocks,
                    &mut image_start_part_index,
                    &mut image_attachments,
                );
                if text_start_part_index.is_none() {
                    text_start_part_index = Some(part_index);
                } else {
                    markdown.push('\n');
                }
                markdown.push_str(text);
            }
            ContentPart::Image { attachment_id } => {
                push_text_block(&mut blocks, &mut text_start_part_index, &mut markdown);
                let Some(record) = attachments_by_id.get(attachment_id) else {
                    push_image_block(
                        &mut blocks,
                        &mut image_start_part_index,
                        &mut image_attachments,
                    );
                    continue;
                };
                let Some(image) = persisted_image_attachment_from_record(record) else {
                    // Invalid images retain the old behavior (they are omitted),
                    // while still delimiting adjacent valid image groups.
                    push_image_block(
                        &mut blocks,
                        &mut image_start_part_index,
                        &mut image_attachments,
                    );
                    continue;
                };
                if image_start_part_index.is_none() {
                    image_start_part_index = Some(part_index);
                }
                image_attachments.push(image);
            }
            ContentPart::File { attachment_id } => {
                push_text_block(&mut blocks, &mut text_start_part_index, &mut markdown);
                push_image_block(
                    &mut blocks,
                    &mut image_start_part_index,
                    &mut image_attachments,
                );
                blocks.push(MessageContentBlock::File(persisted_attachment_card(
                    attachment_id,
                    AttachmentCardKind::File,
                    part_index,
                    attachments_by_id,
                )));
            }
            ContentPart::Audio { attachment_id } => {
                push_text_block(&mut blocks, &mut text_start_part_index, &mut markdown);
                push_image_block(
                    &mut blocks,
                    &mut image_start_part_index,
                    &mut image_attachments,
                );
                blocks.push(MessageContentBlock::Attachment(persisted_attachment_card(
                    attachment_id,
                    AttachmentCardKind::Attachment,
                    part_index,
                    attachments_by_id,
                )));
            }
            ContentPart::Attachment { attachment_id } => {
                push_text_block(&mut blocks, &mut text_start_part_index, &mut markdown);
                push_image_block(
                    &mut blocks,
                    &mut image_start_part_index,
                    &mut image_attachments,
                );
                blocks.push(MessageContentBlock::Attachment(persisted_attachment_card(
                    attachment_id,
                    AttachmentCardKind::Attachment,
                    part_index,
                    attachments_by_id,
                )));
            }
        }
    }

    push_text_block(&mut blocks, &mut text_start_part_index, &mut markdown);
    push_image_block(
        &mut blocks,
        &mut image_start_part_index,
        &mut image_attachments,
    );

    blocks
}

fn push_text_block(
    blocks: &mut Vec<MessageContentBlock>,
    start_part_index: &mut Option<usize>,
    markdown: &mut String,
) {
    let Some(start_part_index) = start_part_index.take() else {
        return;
    };
    let markdown = std::mem::take(markdown);
    if !markdown.is_empty() {
        blocks.push(MessageContentBlock::Text {
            start_part_index,
            markdown,
        });
    }
}

fn push_image_block(
    blocks: &mut Vec<MessageContentBlock>,
    start_part_index: &mut Option<usize>,
    attachments: &mut Vec<PersistedImageAttachment>,
) {
    let Some(start_part_index) = start_part_index.take() else {
        return;
    };
    let attachments = std::mem::take(attachments);
    if !attachments.is_empty() {
        blocks.push(MessageContentBlock::Images {
            start_part_index,
            attachments,
        });
    }
}

fn persisted_attachment_card(
    attachment_id: &AttachmentId,
    kind: AttachmentCardKind,
    part_index: usize,
    attachments_by_id: &HashMap<AttachmentId, ConversationAttachment>,
) -> PersistedAttachmentCard {
    let Some(record) = attachments_by_id.get(attachment_id) else {
        return PersistedAttachmentCard {
            attachment_id: attachment_id.clone(),
            kind,
            part_index,
            display_name: String::new(),
            mime_type: None,
            size_label: None,
            source_hint: AttachmentSourceHint::Unknown,
            static_problem: Some(AttachmentAccessProblem::MissingRecord),
        };
    };

    PersistedAttachmentCard {
        attachment_id: attachment_id.clone(),
        kind,
        part_index,
        display_name: safe_display_name(record.name.as_deref()).unwrap_or_default(),
        mime_type: safe_mime_type(record.mime_type.as_deref()),
        size_label: format_persisted_size(record.size_bytes),
        source_hint: attachment_source_hint(record),
        static_problem: attachment_static_problem(record, kind),
    }
}

fn attachment_source_hint(record: &ConversationAttachment) -> AttachmentSourceHint {
    match record.storage_kind {
        AttachmentStorageKind::ExternalUri => AttachmentSourceHint::External,
        AttachmentStorageKind::ProviderFile => AttachmentSourceHint::Provider,
        AttachmentStorageKind::GeneratedFile => AttachmentSourceHint::Generated,
        AttachmentStorageKind::LocalFile => match &record.metadata.source {
            AttachmentSource::LocalFile { .. } => AttachmentSourceHint::Local,
            AttachmentSource::GeneratedFile { .. } => AttachmentSourceHint::Generated,
            AttachmentSource::ExternalUri { .. } => AttachmentSourceHint::External,
            AttachmentSource::ProviderFile { .. } => AttachmentSourceHint::Provider,
        },
    }
}

fn attachment_static_problem(
    record: &ConversationAttachment,
    expected_kind: AttachmentCardKind,
) -> Option<AttachmentAccessProblem> {
    if !attachment_kind_matches(expected_kind, record.kind) {
        return Some(AttachmentAccessProblem::KindMismatch);
    }

    let unsupported_source = matches!(
        record.storage_kind,
        AttachmentStorageKind::ExternalUri | AttachmentStorageKind::ProviderFile
    ) || matches!(
        &record.metadata.source,
        AttachmentSource::ExternalUri { .. } | AttachmentSource::ProviderFile { .. }
    ) || matches!(
        (&record.storage_kind, &record.metadata.source),
        (
            AttachmentStorageKind::GeneratedFile,
            AttachmentSource::LocalFile { .. }
        )
    );
    if unsupported_source {
        return Some(AttachmentAccessProblem::UnsupportedSource);
    }

    if !record_has_locator(record) {
        return Some(AttachmentAccessProblem::MissingLocator);
    }

    None
}

fn attachment_kind_matches(expected_kind: AttachmentCardKind, record_kind: AttachmentKind) -> bool {
    match expected_kind {
        AttachmentCardKind::File => record_kind == AttachmentKind::File,
        AttachmentCardKind::Attachment => {
            matches!(
                record_kind,
                AttachmentKind::Audio | AttachmentKind::Attachment
            )
        }
    }
}

fn record_has_locator(record: &ConversationAttachment) -> bool {
    record
        .path
        .as_deref()
        .is_some_and(|path| !path.trim().is_empty())
        || match &record.metadata.source {
            AttachmentSource::LocalFile { path } | AttachmentSource::GeneratedFile { path } => {
                !path.trim().is_empty()
            }
            AttachmentSource::ExternalUri { .. } | AttachmentSource::ProviderFile { .. } => false,
        }
}

#[cfg(test)]
fn user_image_attachments(
    item: &ConversationEntry,
    attachments_by_id: &HashMap<AttachmentId, ConversationAttachment>,
) -> Vec<PersistedImageAttachment> {
    if !matches!(
        &item.payload,
        ConversationEntryPayload::Message {
            role: jaco_core::TranscriptRole::User,
            ..
        }
    ) {
        return Vec::new();
    }

    project_message_content(item, attachments_by_id)
        .into_iter()
        .flat_map(|block| match block {
            MessageContentBlock::Images { attachments, .. } => attachments,
            MessageContentBlock::Text { .. }
            | MessageContentBlock::File(_)
            | MessageContentBlock::Attachment(_) => Vec::new(),
        })
        .collect()
}

pub(super) fn render_message_content(
    entry_id: &ConversationEntryId,
    blocks: Vec<MessageContentBlock>,
    text_states: &HashMap<TimelineTextKey, Entity<TextViewState>>,
    access: &HashMap<AttachmentId, AttachmentAccessView>,
    appearance: MessageContentAppearance,
    on_attachment_action: OnAttachmentAction,
    cx: &mut App,
) -> AnyElement {
    let align_end = appearance == MessageContentAppearance::User;
    let content = blocks.into_iter().map(|block| match block {
        MessageContentBlock::Text {
            start_part_index,
            markdown,
        } => render_message_text(
            entry_id,
            start_part_index,
            markdown,
            text_states,
            appearance,
            cx,
        ),
        MessageContentBlock::Images {
            start_part_index,
            attachments,
        } => render_image_attachments(
            &format!("{entry_id}-{start_part_index}"),
            attachments,
            align_end,
            access,
            cx,
        ),
        MessageContentBlock::File(card) | MessageContentBlock::Attachment(card) => {
            render_attachment_card(entry_id, card, access, on_attachment_action.clone(), cx)
        }
    });

    v_flex()
        .id(format!("conversation-message-content-{entry_id}"))
        .max_w(if align_end { px(680.) } else { px(760.) })
        .min_w_0()
        .gap_2()
        .when(align_end, |this| this.items_end())
        .children(content)
        .into_any_element()
}

fn render_message_text(
    entry_id: &ConversationEntryId,
    start_part_index: usize,
    markdown: String,
    text_states: &HashMap<TimelineTextKey, Entity<TextViewState>>,
    appearance: MessageContentAppearance,
    cx: &mut App,
) -> AnyElement {
    let key = TimelineTextKey::MessageBlock {
        entry_id: entry_id.clone(),
        start_part_index,
    };
    let text = text_states.get(&key).cloned().map_or_else(
        || {
            TextView::markdown(
                format!("conversation-message-markdown-{entry_id}-{start_part_index}"),
                &markdown,
            )
            .selectable(true)
            .into_any_element()
        },
        |state| TextView::new(&state).selectable(true).into_any_element(),
    );
    let block = div()
        .id(format!(
            "conversation-message-text-{entry_id}-{start_part_index}"
        ))
        .max_w_full()
        .min_w_0()
        .text_color(cx.theme().foreground)
        .child(text);

    match appearance {
        MessageContentAppearance::User => block
            .rounded(px(8.))
            .px_3()
            .py_2()
            .bg(cx.theme().tokens.primary.background.opacity(0.12))
            .border_1()
            .border_color(cx.theme().primary.opacity(0.18))
            .into_any_element(),
        MessageContentAppearance::Assistant => block.into_any_element(),
    }
}

fn render_attachment_card(
    entry_id: &ConversationEntryId,
    card: PersistedAttachmentCard,
    access: &HashMap<AttachmentId, AttachmentAccessView>,
    on_attachment_action: OnAttachmentAction,
    cx: &mut App,
) -> AnyElement {
    let access_view = access.get(&card.attachment_id);
    let availability = card
        .static_problem
        .clone()
        .map(AttachmentAvailability::Unavailable)
        .or_else(|| access_view.map(|view| view.availability.clone()))
        .unwrap_or(AttachmentAvailability::Checking);
    let source = access_view
        .map(|view| view.source)
        .unwrap_or_else(|| card.source_hint.into());
    let busy_actions = access_view
        .map(|view| view.busy_actions.clone())
        .unwrap_or_default();
    let i18n = cx.global::<I18n>();
    let name = if card.display_name.is_empty() {
        i18n.t("conversation-attachment-fallback-name").to_string()
    } else {
        card.display_name.clone()
    };
    let type_key = match card.kind {
        AttachmentCardKind::File => "conversation-attachment-type-file",
        AttachmentCardKind::Attachment => "conversation-attachment-type-attachment",
    };
    let mut metadata = vec![i18n.t(type_key).to_string()];
    metadata.extend(card.mime_type.iter().cloned());
    metadata.extend(card.size_label.iter().cloned());
    if let Some(source_key) = attachment_source_key(source) {
        metadata.push(i18n.t(source_key).to_string());
    }
    let metadata = metadata.join(" · ");
    let name_tooltip = name.clone();
    let name_label = div()
        .id(format!(
            "conversation-attachment-name-{entry_id}-{}",
            card.part_index
        ))
        .min_w_0()
        .tooltip(move |window, cx| Tooltip::new(name_tooltip.clone()).build(window, cx))
        .child(Label::new(name).text_sm().truncate());
    let center = v_flex()
        .min_w_0()
        .flex_1()
        .gap_1()
        .child(name_label)
        .child(
            Label::new(metadata)
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .truncate(),
        )
        .when_some(
            match &availability {
                AttachmentAvailability::Checking => {
                    Some(i18n.t("conversation-attachment-status-checking"))
                }
                AttachmentAvailability::Unavailable(problem) => {
                    Some(i18n.t(attachment_problem_key(problem)))
                }
                AttachmentAvailability::Available => None,
            },
            |this, status| {
                this.child(
                    Label::new(status)
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .truncate(),
                )
            },
        );
    let leading_icon = match card.kind {
        AttachmentCardKind::File => IconName::File,
        AttachmentCardKind::Attachment => IconName::Paperclip,
    };
    let trailing = match availability {
        AttachmentAvailability::Checking => Spinner::new().small().into_any_element(),
        AttachmentAvailability::Unavailable(_) => Icon::new(IconName::CircleAlert)
            .size_4()
            .text_color(cx.theme().danger)
            .into_any_element(),
        AttachmentAvailability::Available => render_attachment_actions(
            entry_id,
            card.part_index,
            &card,
            &busy_actions,
            on_attachment_action,
            cx,
        ),
    };

    h_flex()
        .id(format!(
            "conversation-attachment-card-{entry_id}-{block_index}-{}",
            card.attachment_id,
            block_index = card.part_index,
        ))
        .w(px(360.))
        .max_w_full()
        .min_h(px(64.))
        .items_center()
        .gap_2()
        .p_2()
        .rounded(px(8.))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().tokens.muted.background.opacity(0.18))
        .child(
            Icon::new(leading_icon)
                .size_5()
                .text_color(cx.theme().muted_foreground),
        )
        .child(center)
        .child(trailing)
        .into_any_element()
}

fn render_attachment_actions(
    entry_id: &ConversationEntryId,
    block_index: usize,
    card: &PersistedAttachmentCard,
    busy_actions: &std::collections::HashSet<AttachmentAction>,
    on_attachment_action: OnAttachmentAction,
    cx: &mut App,
) -> AnyElement {
    let target = AttachmentActionTarget {
        attachment_id: card.attachment_id.clone(),
        kind: card.kind,
    };
    h_flex()
        .items_center()
        .gap_1()
        .child(attachment_action_button(
            format!("conversation-attachment-open-{entry_id}-{block_index}"),
            target.clone(),
            AttachmentAction::Open,
            IconName::ExternalLink,
            cx.global::<I18n>().t("conversation-attachment-open"),
            busy_actions.contains(&AttachmentAction::Open),
            false,
            on_attachment_action.clone(),
        ))
        .child(attachment_action_button(
            format!("conversation-attachment-reveal-{entry_id}-{block_index}"),
            target.clone(),
            AttachmentAction::Reveal,
            IconName::FolderOpen,
            cx.global::<I18n>().t(reveal_i18n_key()),
            busy_actions.contains(&AttachmentAction::Reveal),
            false,
            on_attachment_action.clone(),
        ))
        .child(attachment_action_button(
            format!("conversation-attachment-save-{entry_id}-{block_index}"),
            target,
            AttachmentAction::SaveCopy,
            IconName::Download,
            cx.global::<I18n>().t("conversation-attachment-save-copy"),
            busy_actions.contains(&AttachmentAction::SaveCopy),
            true,
            on_attachment_action,
        ))
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn attachment_action_button(
    id: String,
    target: AttachmentActionTarget,
    action: AttachmentAction,
    icon: IconName,
    tooltip: String,
    busy: bool,
    show_loading: bool,
    on_attachment_action: OnAttachmentAction,
) -> Button {
    Button::new(id)
        .ghost()
        .xsmall()
        .icon(icon)
        .tooltip(tooltip)
        .loading(show_loading && busy)
        .disabled(busy)
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            on_attachment_action(target.clone(), action, window, cx);
        })
}

fn attachment_source_key(source: AttachmentSourceLabel) -> Option<&'static str> {
    match source {
        AttachmentSourceLabel::Managed => Some("conversation-attachment-source-managed"),
        AttachmentSourceLabel::Local => Some("conversation-attachment-source-local"),
        AttachmentSourceLabel::Generated => Some("conversation-attachment-source-generated"),
        AttachmentSourceLabel::External => Some("conversation-attachment-source-external"),
        AttachmentSourceLabel::Provider => Some("conversation-attachment-source-provider"),
        AttachmentSourceLabel::Unknown => None,
    }
}

fn attachment_problem_key(problem: &AttachmentAccessProblem) -> &'static str {
    match problem {
        AttachmentAccessProblem::MissingRecord => {
            "conversation-attachment-unavailable-missing-record"
        }
        AttachmentAccessProblem::WrongConversation
        | AttachmentAccessProblem::KindMismatch
        | AttachmentAccessProblem::LocatorMismatch
        | AttachmentAccessProblem::UnsafeGeneratedPath => {
            "conversation-attachment-unavailable-invalid-record"
        }
        AttachmentAccessProblem::UnsupportedSource | AttachmentAccessProblem::MissingLocator => {
            "conversation-attachment-unavailable-source"
        }
        AttachmentAccessProblem::MissingFile | AttachmentAccessProblem::NotRegularFile => {
            "conversation-attachment-unavailable-missing-file"
        }
        AttachmentAccessProblem::Io(_) => "conversation-attachment-unavailable-access",
    }
}

fn reveal_i18n_key() -> &'static str {
    if cfg!(target_os = "macos") {
        "conversation-attachment-reveal-macos"
    } else if cfg!(target_os = "windows") {
        "conversation-attachment-reveal-windows"
    } else {
        "conversation-attachment-reveal-linux"
    }
}

pub(super) fn render_image_attachments(
    message_id: &str,
    attachments: Vec<PersistedImageAttachment>,
    align_end: bool,
    access: &HashMap<AttachmentId, AttachmentAccessView>,
    cx: &mut App,
) -> AnyElement {
    h_flex()
        .id(format!("conversation-message-images-{message_id}"))
        .max_w(px(680.))
        .when(align_end, |this| this.justify_end())
        .gap(px(USER_IMAGE_GAP))
        .overflow_x_scroll()
        .children(
            attachments
                .into_iter()
                .enumerate()
                .filter_map(|(index, attachment)| {
                    let image_path = access
                        .get(attachment.attachment_id())?
                        .resolved
                        .as_ref()?
                        .image_path()?
                        .to_path_buf();
                    Some(render_image_attachment(
                        message_id, index, attachment, image_path, cx,
                    ))
                }),
        )
        .into_any_element()
}

fn render_image_attachment(
    message_id: &str,
    index: usize,
    attachment: PersistedImageAttachment,
    image_path: PathBuf,
    cx: &mut App,
) -> AnyElement {
    let attachment_id = attachment.id.clone();
    let preview_attachment = attachment.preview_attachment(&image_path);
    div()
        .id(format!(
            "conversation-message-image-{message_id}-{index}-{attachment_id}"
        ))
        .flex_none()
        .size(px(USER_IMAGE_SIZE))
        .rounded(px(USER_IMAGE_RADIUS))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().tokens.muted.background.opacity(0.18))
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

fn persisted_image_attachment_from_record(
    record: &ConversationAttachment,
) -> Option<PersistedImageAttachment> {
    if record.kind != AttachmentKind::Image {
        return None;
    }
    match (&record.storage_kind, &record.metadata.source) {
        (
            AttachmentStorageKind::LocalFile,
            AttachmentSource::LocalFile { .. } | AttachmentSource::GeneratedFile { .. },
        )
        | (AttachmentStorageKind::GeneratedFile, AttachmentSource::GeneratedFile { .. }) => {}
        (AttachmentStorageKind::ExternalUri, _)
        | (AttachmentStorageKind::ProviderFile, _)
        | (
            AttachmentStorageKind::LocalFile,
            AttachmentSource::ExternalUri { .. } | AttachmentSource::ProviderFile { .. },
        )
        | (AttachmentStorageKind::GeneratedFile, AttachmentSource::LocalFile { .. })
        | (
            AttachmentStorageKind::GeneratedFile,
            AttachmentSource::ExternalUri { .. } | AttachmentSource::ProviderFile { .. },
        ) => return None,
    }
    if !record_has_locator(record) {
        return None;
    }

    Some(PersistedImageAttachment {
        id: record.id.clone(),
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

#[cfg(test)]
mod tests {
    use super::{
        AttachmentCardKind, MessageContentBlock, attachments_by_id, project_message_content,
        user_image_attachments,
    };
    use jaco_core::{
        AttachmentKind, AttachmentMetadata, AttachmentSource, AttachmentStorageKind, ContentPart,
        ConversationAttachment, ConversationEntry, ConversationEntryKind, ConversationEntryPayload,
        ConversationEntryStatus, TranscriptRole,
    };
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
        assert_eq!(images[0].name, "image-2.png");
        assert_eq!(images[0].width, Some(320));
        assert_eq!(images[0].height, Some(240));
    }

    #[test]
    fn projects_mixed_content_in_source_order() {
        let item = user_message(vec![
            ContentPart::Text {
                text: "before".to_string(),
            },
            ContentPart::Image {
                attachment_id: "image-1".to_string(),
            },
            ContentPart::Image {
                attachment_id: "image-2".to_string(),
            },
            ContentPart::Text {
                text: "between".to_string(),
            },
            ContentPart::File {
                attachment_id: "file-1".to_string(),
            },
            ContentPart::Audio {
                attachment_id: "audio-1".to_string(),
            },
            ContentPart::Attachment {
                attachment_id: "attachment-1".to_string(),
            },
            ContentPart::Text {
                text: "after".to_string(),
            },
        ]);
        let attachments = attachments_by_id(&[
            image_record("image-1", "/tmp/one.png", 640, 480),
            image_record("image-2", "/tmp/two.png", 320, 240),
            file_record("file-1", "/tmp/file.txt"),
            attachment_record("audio-1", AttachmentKind::Audio, "/tmp/audio.wav"),
            attachment_record("attachment-1", AttachmentKind::Attachment, "/tmp/data.bin"),
        ]);

        let blocks = project_message_content(&item, &attachments);

        assert_eq!(blocks.len(), 7);
        assert!(matches!(
            &blocks[0],
            MessageContentBlock::Text {
                start_part_index: 0,
                markdown
            } if markdown == "before"
        ));
        assert!(matches!(
            &blocks[1],
            MessageContentBlock::Images {
                start_part_index: 1,
                attachments
            } if attachments.iter().map(|attachment| attachment.id.as_str()).collect::<Vec<_>>()
                == ["image-1", "image-2"]
        ));
        assert!(matches!(
            &blocks[2],
            MessageContentBlock::Text {
                start_part_index: 3,
                markdown
            } if markdown == "between"
        ));
        assert!(matches!(
            &blocks[3],
            MessageContentBlock::File(card)
                if card.attachment_id == "file-1"
                    && card.kind == AttachmentCardKind::File
                    && card.static_problem.is_none()
        ));
        assert!(matches!(
            &blocks[4],
            MessageContentBlock::Attachment(card)
                if card.attachment_id == "audio-1"
                    && card.kind == AttachmentCardKind::Attachment
                    && card.static_problem.is_none()
        ));
        assert!(matches!(
            &blocks[5],
            MessageContentBlock::Attachment(card)
                if card.attachment_id == "attachment-1"
                    && card.kind == AttachmentCardKind::Attachment
                    && card.static_problem.is_none()
        ));
        assert!(matches!(
            &blocks[6],
            MessageContentBlock::Text {
                start_part_index: 7,
                markdown
            } if markdown == "after"
        ));
    }

    #[test]
    fn audio_and_attachment_parts_normalize_to_attachment_cards() {
        let item = user_message(vec![
            ContentPart::Audio {
                attachment_id: "legacy-audio".to_string(),
            },
            ContentPart::Attachment {
                attachment_id: "generic-audio".to_string(),
            },
        ]);
        let attachments = attachments_by_id(&[
            attachment_record("legacy-audio", AttachmentKind::Attachment, "/tmp/legacy"),
            attachment_record("generic-audio", AttachmentKind::Audio, "/tmp/generic"),
        ]);

        let blocks = project_message_content(&item, &attachments);

        assert_eq!(blocks.len(), 2);
        for (block, expected_id) in blocks.iter().zip(["legacy-audio", "generic-audio"]) {
            assert!(matches!(
                block,
                MessageContentBlock::Attachment(card)
                    if card.attachment_id == expected_id
                        && card.kind == AttachmentCardKind::Attachment
                        && card.static_problem.is_none()
            ));
        }
    }

    #[test]
    fn attachment_projection_preserves_slots_for_missing_and_mismatched_records() {
        let cases = [
            (
                ContentPart::File {
                    attachment_id: "attachment".to_string(),
                },
                Some(AttachmentKind::File),
                AttachmentCardKind::File,
                None,
            ),
            (
                ContentPart::File {
                    attachment_id: "attachment".to_string(),
                },
                Some(AttachmentKind::Audio),
                AttachmentCardKind::File,
                Some(super::AttachmentAccessProblem::KindMismatch),
            ),
            (
                ContentPart::File {
                    attachment_id: "attachment".to_string(),
                },
                Some(AttachmentKind::Attachment),
                AttachmentCardKind::File,
                Some(super::AttachmentAccessProblem::KindMismatch),
            ),
            (
                ContentPart::File {
                    attachment_id: "attachment".to_string(),
                },
                Some(AttachmentKind::Image),
                AttachmentCardKind::File,
                Some(super::AttachmentAccessProblem::KindMismatch),
            ),
            (
                ContentPart::Audio {
                    attachment_id: "attachment".to_string(),
                },
                Some(AttachmentKind::Audio),
                AttachmentCardKind::Attachment,
                None,
            ),
            (
                ContentPart::Audio {
                    attachment_id: "attachment".to_string(),
                },
                Some(AttachmentKind::Attachment),
                AttachmentCardKind::Attachment,
                None,
            ),
            (
                ContentPart::Audio {
                    attachment_id: "attachment".to_string(),
                },
                Some(AttachmentKind::File),
                AttachmentCardKind::Attachment,
                Some(super::AttachmentAccessProblem::KindMismatch),
            ),
            (
                ContentPart::Audio {
                    attachment_id: "attachment".to_string(),
                },
                Some(AttachmentKind::Image),
                AttachmentCardKind::Attachment,
                Some(super::AttachmentAccessProblem::KindMismatch),
            ),
            (
                ContentPart::Attachment {
                    attachment_id: "attachment".to_string(),
                },
                Some(AttachmentKind::Attachment),
                AttachmentCardKind::Attachment,
                None,
            ),
            (
                ContentPart::Attachment {
                    attachment_id: "attachment".to_string(),
                },
                Some(AttachmentKind::Audio),
                AttachmentCardKind::Attachment,
                None,
            ),
            (
                ContentPart::Attachment {
                    attachment_id: "attachment".to_string(),
                },
                Some(AttachmentKind::File),
                AttachmentCardKind::Attachment,
                Some(super::AttachmentAccessProblem::KindMismatch),
            ),
            (
                ContentPart::Attachment {
                    attachment_id: "attachment".to_string(),
                },
                Some(AttachmentKind::Image),
                AttachmentCardKind::Attachment,
                Some(super::AttachmentAccessProblem::KindMismatch),
            ),
            (
                ContentPart::File {
                    attachment_id: "attachment".to_string(),
                },
                None,
                AttachmentCardKind::File,
                Some(super::AttachmentAccessProblem::MissingRecord),
            ),
            (
                ContentPart::Audio {
                    attachment_id: "attachment".to_string(),
                },
                None,
                AttachmentCardKind::Attachment,
                Some(super::AttachmentAccessProblem::MissingRecord),
            ),
            (
                ContentPart::Attachment {
                    attachment_id: "attachment".to_string(),
                },
                None,
                AttachmentCardKind::Attachment,
                Some(super::AttachmentAccessProblem::MissingRecord),
            ),
        ];

        for (part, record_kind, expected_kind, expected_problem) in cases {
            let record = record_kind.map(|kind| attachment_record("attachment", kind, "/tmp/data"));
            let attachments = record.as_ref().map_or_else(
                || attachments_by_id(&[]),
                |record| attachments_by_id(std::slice::from_ref(record)),
            );
            let blocks = project_message_content(&user_message(vec![part]), &attachments);

            assert_eq!(blocks.len(), 1);
            let card = match &blocks[0] {
                MessageContentBlock::File(card) | MessageContentBlock::Attachment(card) => card,
                MessageContentBlock::Text { .. } | MessageContentBlock::Images { .. } => {
                    panic!("attachment part did not produce a card")
                }
            };
            assert_eq!(card.kind, expected_kind);
            assert_eq!(card.static_problem, expected_problem);
        }
    }

    #[test]
    fn invalid_images_are_omitted_and_split_image_groups() {
        let item = user_message(vec![
            ContentPart::Image {
                attachment_id: "image-1".to_string(),
            },
            ContentPart::Image {
                attachment_id: "missing-image".to_string(),
            },
            ContentPart::Image {
                attachment_id: "image-2".to_string(),
            },
        ]);
        let attachments = attachments_by_id(&[
            image_record("image-1", "/tmp/one.png", 640, 480),
            image_record("image-2", "/tmp/two.png", 320, 240),
        ]);

        let blocks = project_message_content(&item, &attachments);

        assert_eq!(blocks.len(), 2);
        assert!(matches!(
            &blocks[0],
            MessageContentBlock::Images {
                start_part_index: 0,
                attachments
            } if attachments.len() == 1 && attachments[0].id == "image-1"
        ));
        assert!(matches!(
            &blocks[1],
            MessageContentBlock::Images {
                start_part_index: 2,
                attachments
            } if attachments.len() == 1 && attachments[0].id == "image-2"
        ));
    }

    #[test]
    fn external_and_provider_images_ignore_stale_local_paths() {
        let item = user_message(vec![
            ContentPart::Image {
                attachment_id: "external".to_string(),
            },
            ContentPart::Image {
                attachment_id: "provider".to_string(),
            },
        ]);
        let mut external = image_record("external", "/private/stale.png", 10, 10);
        external.storage_kind = AttachmentStorageKind::ExternalUri;
        external.metadata.source = AttachmentSource::ExternalUri {
            uri: "https://example.invalid/private.png".to_string(),
        };
        let mut provider = image_record("provider", "/private/provider.png", 10, 10);
        provider.storage_kind = AttachmentStorageKind::ProviderFile;
        provider.metadata.source = AttachmentSource::ProviderFile {
            provider_id: "secret-provider".to_string(),
            file_id: "secret-file".to_string(),
        };

        let blocks = project_message_content(&item, &attachments_by_id(&[external, provider]));

        assert!(blocks.is_empty());
    }

    fn user_message(content: Vec<ContentPart>) -> ConversationEntry {
        let now = OffsetDateTime::UNIX_EPOCH;
        ConversationEntry {
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

    fn image_record(id: &str, path: &str, width: u32, height: u32) -> ConversationAttachment {
        let mut record = attachment_record(id, AttachmentKind::Image, path);
        record.metadata.width = Some(width);
        record.metadata.height = Some(height);
        record
    }

    fn image_record_without_path(id: &str) -> ConversationAttachment {
        let mut record = attachment_record(id, AttachmentKind::Image, "");
        record.path = None;
        record
    }

    fn file_record(id: &str, path: &str) -> ConversationAttachment {
        attachment_record(id, AttachmentKind::File, path)
    }

    fn attachment_record(id: &str, kind: AttachmentKind, path: &str) -> ConversationAttachment {
        let now = OffsetDateTime::UNIX_EPOCH;
        ConversationAttachment {
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

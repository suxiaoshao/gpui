use super::utils::{deserialize_offset_date_time, serialize_offset_date_time};
use crate::{
    database::{
        Role, Status,
        model::{
            SqlConversation, SqlMessage, SqlMessageAttachment, SqlMessageOutputItem,
            SqlMessageRunState, SqlNewMessage, SqlNewMessageAttachment, SqlNewMessageOutputItem,
            SqlNewMessageRunState,
        },
    },
    errors::{AiChatError, AiChatResult},
    llm::{
        LlmAttachmentRef, LlmContentPart, LlmHostedToolCall, LlmMcpApprovalRequest, LlmOutputItem,
        LlmToolCall, LlmToolResult, ProviderRunState, ProviderUsage,
    },
};
use diesel::SqliteConnection;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, ops::AddAssign};
use time::OffsetDateTime;

#[derive(Debug, Serialize, Clone, Deserialize, PartialEq, Eq)]
pub struct UrlCitation {
    pub title: Option<String>,
    pub url: String,
    #[serde(rename = "startIndex")]
    pub start_index: Option<usize>,
    #[serde(rename = "endIndex")]
    pub end_index: Option<usize>,
}

#[derive(Debug, Default, Serialize, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Content {
    #[serde(default)]
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub citations: Vec<UrlCitation>,
}

impl AddAssign<String> for Content {
    fn add_assign(&mut self, rhs: String) {
        self.text += &rhs;
    }
}

impl AddAssign<&str> for Content {
    fn add_assign(&mut self, rhs: &str) {
        self.text += rhs;
    }
}

impl Content {
    pub(crate) fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            reasoning_summary: None,
            citations: Vec::new(),
        }
    }

    pub(crate) fn with_citations(text: impl Into<String>, citations: Vec<UrlCitation>) -> Self {
        Self {
            text: text.into(),
            reasoning_summary: None,
            citations,
        }
    }

    pub(crate) fn send_content(&self) -> &str {
        &self.text
    }

    pub(crate) fn display_markdown(&self, sources_label: &str) -> String {
        let mut markdown = self.text.clone();
        let sources = format_sources_markdown(&self.citations, sources_label);
        if !sources.is_empty() {
            if !markdown.is_empty() {
                markdown.push_str("\n\n");
            }
            markdown.push_str(&sources);
        }
        markdown
    }

    pub(crate) fn append_reasoning_summary(&mut self, delta: &str) {
        let summary = self.reasoning_summary.get_or_insert_with(String::new);
        summary.push_str(delta);
        if summary.trim().is_empty() {
            self.reasoning_summary = None;
        }
    }
}

fn format_sources_markdown(citations: &[UrlCitation], sources_label: &str) -> String {
    let mut seen = HashSet::new();
    let mut lines = Vec::new();
    for citation in citations {
        if !seen.insert(citation.url.as_str()) {
            continue;
        }
        let title = citation
            .title
            .as_deref()
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .unwrap_or(citation.url.as_str())
            .replace(']', "\\]");
        let url = citation.url.replace(')', "\\)");
        lines.push(format!("- [{title}]({url})"));
    }
    if lines.is_empty() {
        String::new()
    } else {
        format!("{sources_label}\n{}", lines.join("\n"))
    }
}

#[cfg(test)]
mod content_tests {
    use super::{Content, UrlCitation};

    #[test]
    fn add_assign_appends_text_content() {
        let mut content = Content::new("hello");
        content += " world";
        assert_eq!(content, Content::new("hello world"));
    }

    #[test]
    fn add_assign_appends_text_when_citations_exist() {
        let mut content = Content::with_citations("hello", vec![]);
        content += " world";
        assert_eq!(content.text, "hello world");
    }

    #[test]
    fn send_content_uses_text_body() {
        let content = Content::with_citations(
            "body",
            vec![UrlCitation {
                title: Some("Example".to_string()),
                url: "https://example.com".to_string(),
                start_index: Some(0),
                end_index: Some(4),
            }],
        );
        assert_eq!(content.send_content(), "body");
    }

    #[test]
    fn display_markdown_renders_deduped_sources() {
        let content = Content {
            text: "answer".to_string(),
            reasoning_summary: None,
            citations: vec![
                UrlCitation {
                    title: Some("Example".to_string()),
                    url: "https://example.com".to_string(),
                    start_index: Some(0),
                    end_index: Some(4),
                },
                UrlCitation {
                    title: Some("Duplicate".to_string()),
                    url: "https://example.com".to_string(),
                    start_index: Some(5),
                    end_index: Some(9),
                },
            ],
        };
        assert_eq!(
            content.display_markdown("Sources"),
            "answer\n\nSources\n- [Example](https://example.com)"
        );
    }

    #[test]
    fn append_reasoning_summary_creates_and_appends_text() {
        let mut content = Content::default();
        content.append_reasoning_summary("thinking");
        content.append_reasoning_summary(" more");
        assert_eq!(content.reasoning_summary.as_deref(), Some("thinking more"));
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct MessageRunPersistence {
    pub(crate) run_state: Option<MessageRunState>,
    pub(crate) output_items: Vec<MessageOutputItem>,
    pub(crate) attachments: Vec<MessageAttachment>,
}

impl MessageRunPersistence {
    pub(crate) fn is_empty(&self) -> bool {
        self.run_state.is_none() && self.output_items.is_empty() && self.attachments.is_empty()
    }

    pub(crate) fn with_deduped_attachments(mut self) -> Self {
        let mut seen = HashSet::new();
        self.attachments
            .retain(|attachment| seen.insert((attachment.attachment_id.clone(), attachment.kind)));
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MessageRunState {
    pub(crate) provider: String,
    pub(crate) run_id: Option<String>,
    pub(crate) output_item_ids: Vec<String>,
    pub(crate) continuation_metadata: serde_json::Value,
    pub(crate) request_body: serde_json::Value,
    pub(crate) usage: Option<ProviderUsage>,
    pub(crate) model: Option<String>,
    pub(crate) settings: Option<serde_json::Value>,
}

impl MessageRunState {
    pub(crate) fn from_provider_state(
        state: ProviderRunState,
        usage: Option<ProviderUsage>,
        model: Option<String>,
        settings: Option<serde_json::Value>,
    ) -> Self {
        Self {
            provider: state.provider_name,
            run_id: state.run_id,
            output_item_ids: state.output_item_ids,
            continuation_metadata: state.continuation_metadata,
            request_body: state.request_body,
            usage,
            model,
            settings,
        }
    }

    pub(crate) fn from_request_snapshot(
        provider: impl Into<String>,
        request_body: serde_json::Value,
        usage: Option<ProviderUsage>,
        model: Option<String>,
        settings: Option<serde_json::Value>,
    ) -> Self {
        Self {
            provider: provider.into(),
            run_id: None,
            output_item_ids: Vec::new(),
            continuation_metadata: serde_json::Value::Null,
            request_body,
            usage,
            model,
            settings,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum MessageOutputItemStatus {
    Added,
    Done,
    ToolCallRequested,
    ToolResultReceived,
    McpApprovalRequested,
}

impl MessageOutputItemStatus {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Done => "done",
            Self::ToolCallRequested => "tool_call_requested",
            Self::ToolResultReceived => "tool_result_received",
            Self::McpApprovalRequested => "mcp_approval_requested",
        }
    }

    fn parse(value: &str) -> AiChatResult<Self> {
        Ok(match value {
            "added" => Self::Added,
            "done" => Self::Done,
            "tool_call_requested" => Self::ToolCallRequested,
            "tool_result_received" => Self::ToolResultReceived,
            "mcp_approval_requested" => Self::McpApprovalRequested,
            _ => {
                return Err(AiChatError::StreamError(format!(
                    "unknown message output item status: {value}"
                )));
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MessageOutputItem {
    pub(crate) sequence: i32,
    pub(crate) item: LlmOutputItem,
    pub(crate) status: MessageOutputItemStatus,
}

impl MessageOutputItem {
    pub(crate) fn new(sequence: i32, item: LlmOutputItem, status: MessageOutputItemStatus) -> Self {
        Self {
            sequence,
            item,
            status,
        }
    }

    fn item_kind(&self) -> &'static str {
        output_item_kind(&self.item)
    }

    fn provider_item_id(&self) -> Option<&str> {
        output_item_provider_item_id(&self.item)
    }

    pub(crate) fn attachments(&self) -> Vec<MessageAttachment> {
        output_item_attachments(&self.item)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub(crate) enum MessageAttachmentKind {
    Image,
    File,
    Audio,
    Generic,
}

impl MessageAttachmentKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::File => "file",
            Self::Audio => "audio",
            Self::Generic => "generic",
        }
    }

    fn parse(value: &str) -> AiChatResult<Self> {
        Ok(match value {
            "image" => Self::Image,
            "file" => Self::File,
            "audio" => Self::Audio,
            "generic" => Self::Generic,
            _ => {
                return Err(AiChatError::StreamError(format!(
                    "unknown message attachment kind: {value}"
                )));
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MessageAttachment {
    pub(crate) attachment_id: String,
    pub(crate) kind: MessageAttachmentKind,
    pub(crate) mime_type: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) metadata: serde_json::Value,
    pub(crate) external_uri: Option<String>,
    pub(crate) path: Option<String>,
    pub(crate) sha256: Option<String>,
}

impl MessageAttachment {
    pub(crate) fn from_ref(kind: MessageAttachmentKind, attachment: &LlmAttachmentRef) -> Self {
        Self {
            attachment_id: attachment.id.clone(),
            kind,
            mime_type: attachment.mime_type.clone(),
            name: attachment.name.clone(),
            metadata: serde_json::json!({}),
            external_uri: None,
            path: None,
            sha256: None,
        }
    }
}

fn output_item_kind(item: &LlmOutputItem) -> &'static str {
    match item {
        LlmOutputItem::Message { .. } => "message",
        LlmOutputItem::Reasoning { .. } => "reasoning",
        LlmOutputItem::ToolCall(_) => "tool_call",
        LlmOutputItem::ToolResult(_) => "tool_result",
        LlmOutputItem::McpApproval(_) => "mcp_approval",
        LlmOutputItem::HostedToolCall(_) => "hosted_tool_call",
    }
}

fn output_item_provider_item_id(item: &LlmOutputItem) -> Option<&str> {
    match item {
        LlmOutputItem::ToolCall(LlmToolCall { call_id, .. })
        | LlmOutputItem::ToolResult(LlmToolResult { call_id, .. })
        | LlmOutputItem::HostedToolCall(LlmHostedToolCall { call_id, .. }) => Some(call_id),
        LlmOutputItem::McpApproval(LlmMcpApprovalRequest { request_id, .. }) => Some(request_id),
        LlmOutputItem::Message { .. } | LlmOutputItem::Reasoning { .. } => None,
    }
}

fn output_item_attachments(item: &LlmOutputItem) -> Vec<MessageAttachment> {
    match item {
        LlmOutputItem::Message { content, .. }
        | LlmOutputItem::ToolResult(LlmToolResult { content, .. }) => {
            content_parts_attachments(content)
        }
        LlmOutputItem::Reasoning { .. }
        | LlmOutputItem::ToolCall(_)
        | LlmOutputItem::McpApproval(_)
        | LlmOutputItem::HostedToolCall(_) => Vec::new(),
    }
}

fn content_parts_attachments(content: &[LlmContentPart]) -> Vec<MessageAttachment> {
    content
        .iter()
        .filter_map(|part| match part {
            LlmContentPart::ImageRef(attachment) => Some(MessageAttachment::from_ref(
                MessageAttachmentKind::Image,
                attachment,
            )),
            LlmContentPart::FileRef(attachment) => Some(MessageAttachment::from_ref(
                MessageAttachmentKind::File,
                attachment,
            )),
            LlmContentPart::AudioRef(attachment) => Some(MessageAttachment::from_ref(
                MessageAttachmentKind::Audio,
                attachment,
            )),
            LlmContentPart::AttachmentRef(attachment) => Some(MessageAttachment::from_ref(
                MessageAttachmentKind::Generic,
                attachment,
            )),
            LlmContentPart::Text(_) => None,
        })
        .collect()
}

#[derive(Debug, Serialize, Clone, Deserialize, PartialEq, Eq)]
pub struct Message {
    pub id: i32,
    #[serde(rename = "conversationId")]
    pub conversation_id: i32,
    #[serde(rename = "conversationPath")]
    pub conversation_path: String,
    pub provider: String,
    pub role: Role,
    pub content: Content,
    #[serde(rename = "sendContent")]
    pub send_content: serde_json::Value,
    pub status: Status,
    #[serde(
        rename = "createdTime",
        serialize_with = "serialize_offset_date_time",
        deserialize_with = "deserialize_offset_date_time"
    )]
    pub created_time: OffsetDateTime,
    #[serde(
        rename = "updatedTime",
        serialize_with = "serialize_offset_date_time",
        deserialize_with = "deserialize_offset_date_time"
    )]
    pub updated_time: OffsetDateTime,
    #[serde(
        rename = "startTime",
        serialize_with = "serialize_offset_date_time",
        deserialize_with = "deserialize_offset_date_time"
    )]
    pub start_time: OffsetDateTime,
    #[serde(
        rename = "endTime",
        serialize_with = "serialize_offset_date_time",
        deserialize_with = "deserialize_offset_date_time"
    )]
    pub end_time: OffsetDateTime,
    pub error: Option<String>,
}

impl TryFrom<SqlMessage> for Message {
    type Error = AiChatError;

    fn try_from(value: SqlMessage) -> Result<Self, Self::Error> {
        Ok(Message {
            id: value.id,
            conversation_id: value.conversation_id,
            conversation_path: value.conversation_path,
            provider: value.provider,
            role: value.role.parse()?,
            content: serde_json::from_value(value.content)?,
            send_content: value.send_content,
            status: value.status.parse()?,
            created_time: value.created_time,
            updated_time: value.updated_time,
            start_time: value.start_time,
            end_time: value.end_time,
            error: value.error,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct NewMessage<'a> {
    pub conversation_id: i32,
    pub provider: &'a str,
    pub role: Role,
    pub content: &'a Content,
    pub send_content: &'a serde_json::Value,
    pub status: Status,
    pub error: Option<&'a str>,
    pub(crate) run_persistence: Option<&'a MessageRunPersistence>,
}

impl<'a> NewMessage<'a> {
    pub fn new(
        conversation_id: i32,
        provider: &'a str,
        role: Role,
        content: &'a Content,
        send_content: &'a serde_json::Value,
        status: Status,
    ) -> Self {
        Self {
            conversation_id,
            provider,
            role,
            content,
            send_content,
            status,
            error: None,
            run_persistence: None,
        }
    }

    pub fn with_error(mut self, error: &'a str) -> Self {
        self.error = Some(error);
        self
    }

    pub(crate) fn with_run_persistence(
        mut self,
        run_persistence: &'a MessageRunPersistence,
    ) -> Self {
        self.run_persistence = Some(run_persistence);
        self
    }
}

impl TryFrom<SqlMessageRunState> for MessageRunState {
    type Error = AiChatError;

    fn try_from(value: SqlMessageRunState) -> Result<Self, Self::Error> {
        Ok(Self {
            provider: value.provider,
            run_id: value.run_id,
            output_item_ids: serde_json::from_value(value.output_item_ids)?,
            continuation_metadata: value.continuation_metadata,
            request_body: value.request_body,
            usage: value.usage.map(serde_json::from_value).transpose()?,
            model: value.model,
            settings: value.settings,
        })
    }
}

impl TryFrom<SqlMessageOutputItem> for MessageOutputItem {
    type Error = AiChatError;

    fn try_from(value: SqlMessageOutputItem) -> Result<Self, Self::Error> {
        Ok(Self {
            sequence: value.sequence,
            item: serde_json::from_value(value.payload)?,
            status: MessageOutputItemStatus::parse(&value.status)?,
        })
    }
}

impl TryFrom<SqlMessageAttachment> for MessageAttachment {
    type Error = AiChatError;

    fn try_from(value: SqlMessageAttachment) -> Result<Self, Self::Error> {
        Ok(Self {
            attachment_id: value.attachment_id,
            kind: MessageAttachmentKind::parse(&value.kind)?,
            mime_type: value.mime_type,
            name: value.name,
            metadata: value.metadata,
            external_uri: value.external_uri,
            path: value.path,
            sha256: value.sha256,
        })
    }
}

fn clear_message_run_persistence(id: i32, conn: &mut SqliteConnection) -> AiChatResult<()> {
    SqlMessageAttachment::delete_by_message_id(id, conn)?;
    SqlMessageOutputItem::delete_by_message_id(id, conn)?;
    SqlMessageRunState::delete_by_message_id(id, conn)?;
    Ok(())
}

fn replace_message_run_persistence(
    id: i32,
    run_persistence: &MessageRunPersistence,
    time: OffsetDateTime,
    conn: &mut SqliteConnection,
) -> AiChatResult<()> {
    clear_message_run_persistence(id, conn)?;
    if run_persistence.is_empty() {
        return Ok(());
    }

    if let Some(run_state) = run_persistence.run_state.as_ref() {
        let output_item_ids = serde_json::to_value(&run_state.output_item_ids)?;
        let usage = run_state
            .usage
            .as_ref()
            .map(serde_json::to_value)
            .transpose()?;
        SqlNewMessageRunState {
            message_id: id,
            provider: &run_state.provider,
            run_id: run_state.run_id.as_deref(),
            output_item_ids: &output_item_ids,
            continuation_metadata: &run_state.continuation_metadata,
            request_body: &run_state.request_body,
            usage: usage.as_ref(),
            model: run_state.model.as_deref(),
            settings: run_state.settings.as_ref(),
            created_time: time,
            updated_time: time,
        }
        .insert(conn)?;
    }

    struct OutputInsertValue {
        sequence: i32,
        item_kind: &'static str,
        provider_item_id: Option<String>,
        status: &'static str,
        payload: serde_json::Value,
    }

    let output_values = run_persistence
        .output_items
        .iter()
        .map(|item| {
            Ok(OutputInsertValue {
                sequence: item.sequence,
                item_kind: item.item_kind(),
                provider_item_id: item.provider_item_id().map(ToString::to_string),
                status: item.status.as_str(),
                payload: serde_json::to_value(&item.item)?,
            })
        })
        .collect::<AiChatResult<Vec<_>>>()?;
    let output_rows = output_values
        .iter()
        .map(|value| SqlNewMessageOutputItem {
            message_id: id,
            sequence: value.sequence,
            item_kind: value.item_kind,
            provider_item_id: value.provider_item_id.as_deref(),
            status: value.status,
            payload: &value.payload,
            created_time: time,
            updated_time: time,
        })
        .collect::<Vec<_>>();
    SqlNewMessageOutputItem::insert_many(&output_rows, conn)?;

    let run_persistence = run_persistence.clone().with_deduped_attachments();
    let attachment_rows = run_persistence
        .attachments
        .iter()
        .map(|attachment| SqlNewMessageAttachment {
            message_id: id,
            attachment_id: &attachment.attachment_id,
            kind: attachment.kind.as_str(),
            mime_type: attachment.mime_type.as_deref(),
            name: attachment.name.as_deref(),
            metadata: &attachment.metadata,
            external_uri: attachment.external_uri.as_deref(),
            path: attachment.path.as_deref(),
            sha256: attachment.sha256.as_deref(),
            created_time: time,
            updated_time: time,
        })
        .collect::<Vec<_>>();
    SqlNewMessageAttachment::insert_many(&attachment_rows, conn)?;
    Ok(())
}

impl Message {
    pub fn insert(
        NewMessage {
            conversation_id,
            provider,
            role,
            content,
            send_content,
            status,
            error,
            run_persistence,
        }: NewMessage<'_>,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<Message> {
        conn.immediate_transaction(|conn| {
            let time = OffsetDateTime::now_utc();
            let SqlConversation { path, .. } = SqlConversation::find(conversation_id, conn)?;
            let role = role.to_string();
            let content = serde_json::to_value(content)?;
            let status = status.to_string();

            let new_message = SqlNewMessage {
                conversation_id,
                conversation_path: &path,
                provider,
                role: &role,
                content: &content,
                send_content,
                status: &status,
                created_time: time,
                updated_time: time,
                start_time: time,
                end_time: time,
                error,
            };
            let message = new_message.insert(conn)?;
            if let Some(run_persistence) = run_persistence {
                replace_message_run_persistence(message.id, run_persistence, time, conn)?;
            }
            Message::try_from(message)
        })
    }
    pub fn messages_by_conversation_id(
        conversation_id: i32,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<Vec<Message>> {
        let messages = SqlMessage::query_by_conversation_id(conversation_id, conn)?;
        messages
            .into_iter()
            .map(TryFrom::try_from)
            .collect::<AiChatResult<_>>()
    }
    pub fn update_status(id: i32, status: Status, conn: &mut SqliteConnection) -> AiChatResult<()> {
        let time = OffsetDateTime::now_utc();
        SqlMessage::update_status(id, status, time, conn)?;
        Ok(())
    }
    pub fn record_error(
        id: i32,
        error: impl Into<String>,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<()> {
        let time = OffsetDateTime::now_utc();
        SqlMessage::record_error(id, error.into(), time, conn)?;
        Ok(())
    }
    pub fn find(id: i32, conn: &mut SqliteConnection) -> AiChatResult<Message> {
        let message = SqlMessage::find(id, conn)?;
        Message::try_from(message)
    }
    pub fn delete(id: i32, conn: &mut SqliteConnection) -> AiChatResult<()> {
        SqlMessage::delete(id, conn)?;
        Ok(())
    }
    pub fn delete_by_conversation_id(
        conversation_id: i32,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<()> {
        SqlMessage::delete_by_conversation_id(conversation_id, conn)?;
        Ok(())
    }
    pub fn update_content(
        id: i32,
        content: &Content,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<OffsetDateTime> {
        let time = OffsetDateTime::now_utc();
        SqlMessage::update_content(id, serde_json::to_value(content)?, time, conn)?;
        Ok(time)
    }
    #[cfg(test)]
    pub(crate) fn run_persistence(
        id: i32,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<MessageRunPersistence> {
        let run_state = SqlMessageRunState::find_by_message_id(id, conn)?
            .map(TryFrom::try_from)
            .transpose()?;
        let output_items = SqlMessageOutputItem::query_by_message_id(id, conn)?
            .into_iter()
            .map(TryFrom::try_from)
            .collect::<AiChatResult<_>>()?;
        let attachments = SqlMessageAttachment::query_by_message_id(id, conn)?
            .into_iter()
            .map(TryFrom::try_from)
            .collect::<AiChatResult<_>>()?;
        Ok(MessageRunPersistence {
            run_state,
            output_items,
            attachments,
        })
    }
    pub(crate) fn replace_run_persistence(
        id: i32,
        run_persistence: &MessageRunPersistence,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<()> {
        let time = OffsetDateTime::now_utc();
        replace_message_run_persistence(id, run_persistence, time, conn)
    }
    pub fn reset_for_resend(id: i32, conn: &mut SqliteConnection) -> AiChatResult<()> {
        let time = OffsetDateTime::now_utc();
        conn.immediate_transaction(|conn| {
            SqlMessage::reset_for_resend(
                id,
                serde_json::to_value(Content::default())?,
                time,
                conn,
            )?;
            clear_message_run_persistence(id, conn)
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod run_persistence_tests {
    use super::{
        Content, Message, MessageAttachment, MessageAttachmentKind, MessageOutputItem,
        MessageOutputItemStatus, MessageRunPersistence, MessageRunState, NewMessage,
    };
    use crate::{
        database::{
            CREATE_TABLE_SQL, NewConversation, Role, Status,
            model::{SqlMessageAttachment, SqlMessageOutputItem, SqlMessageRunState},
        },
        llm::{LlmHostedToolCall, LlmOutputItem, ProviderRunState, ProviderUsage},
    };
    use diesel::{Connection, SqliteConnection, connection::SimpleConnection};

    fn conn() -> anyhow::Result<SqliteConnection> {
        let mut conn = SqliteConnection::establish(":memory:")?;
        conn.batch_execute(CREATE_TABLE_SQL)?;
        Ok(conn)
    }

    fn insert_conversation(conn: &mut SqliteConnection) -> anyhow::Result<i32> {
        insert_conversation_named("test", conn)
    }

    fn insert_conversation_named(title: &str, conn: &mut SqliteConnection) -> anyhow::Result<i32> {
        let conversation = crate::database::Conversation::insert(
            NewConversation {
                title,
                folder_id: None,
                icon: "T",
                info: None,
            },
            conn,
        )?;
        Ok(conversation.id)
    }

    fn insert_assistant_message(conn: &mut SqliteConnection) -> anyhow::Result<Message> {
        let conversation_id = insert_conversation(conn)?;
        Ok(Message::insert(
            NewMessage::new(
                conversation_id,
                "OpenAI",
                Role::Assistant,
                &Content::default(),
                &serde_json::json!({"model": "gpt-4o"}),
                Status::Normal,
            ),
            conn,
        )?)
    }

    fn sample_persistence() -> MessageRunPersistence {
        let state = ProviderRunState::new(
            "OpenAI",
            Some("resp_1".to_string()),
            vec!["item_1".to_string()],
            serde_json::json!({"model": "gpt-4o"}),
        );
        MessageRunPersistence {
            run_state: Some(MessageRunState::from_provider_state(
                state,
                Some(ProviderUsage::new(Some(3), Some(5), Some(8))),
                Some("gpt-4o".to_string()),
                Some(serde_json::json!({"base_url": "https://example.com"})),
            )),
            output_items: vec![
                MessageOutputItem::new(
                    0,
                    LlmOutputItem::Reasoning {
                        summary: Some("thinking".to_string()),
                    },
                    MessageOutputItemStatus::Added,
                ),
                MessageOutputItem::new(
                    1,
                    LlmOutputItem::HostedToolCall(LlmHostedToolCall {
                        call_id: "call_1".to_string(),
                        tool_type: "web_search_call".to_string(),
                        status: Some("completed".to_string()),
                    }),
                    MessageOutputItemStatus::Done,
                ),
            ],
            attachments: vec![MessageAttachment {
                attachment_id: "att_1".to_string(),
                kind: MessageAttachmentKind::File,
                mime_type: Some("text/plain".to_string()),
                name: Some("notes.txt".to_string()),
                metadata: serde_json::json!({"source": "test"}),
                external_uri: None,
                path: Some("/tmp/notes.txt".to_string()),
                sha256: Some("abc".to_string()),
            }],
        }
    }

    #[test]
    fn message_run_persistence_round_trips_state_usage_and_items() -> anyhow::Result<()> {
        let mut conn = conn()?;
        let message = insert_assistant_message(&mut conn)?;
        let persistence = sample_persistence();

        Message::replace_run_persistence(message.id, &persistence, &mut conn)?;

        let loaded = Message::run_persistence(message.id, &mut conn)?;
        assert_eq!(loaded.run_state, persistence.run_state);
        assert_eq!(loaded.output_items, persistence.output_items);
        assert_eq!(loaded.attachments, persistence.attachments);
        Ok(())
    }

    #[test]
    fn reset_for_resend_clears_run_persistence() -> anyhow::Result<()> {
        let mut conn = conn()?;
        let message = insert_assistant_message(&mut conn)?;
        Message::replace_run_persistence(message.id, &sample_persistence(), &mut conn)?;

        Message::reset_for_resend(message.id, &mut conn)?;

        assert!(Message::run_persistence(message.id, &mut conn)?.is_empty());
        let reloaded = Message::find(message.id, &mut conn)?;
        assert_eq!(reloaded.content, Content::default());
        assert_eq!(reloaded.status, Status::Loading);
        Ok(())
    }

    #[test]
    fn message_delete_and_conversation_clear_remove_run_rows() -> anyhow::Result<()> {
        let mut conn = conn()?;
        let message = insert_assistant_message(&mut conn)?;
        Message::replace_run_persistence(message.id, &sample_persistence(), &mut conn)?;

        Message::delete(message.id, &mut conn)?;

        assert!(SqlMessageRunState::all(&mut conn)?.is_empty());
        assert!(SqlMessageOutputItem::all(&mut conn)?.is_empty());
        assert!(SqlMessageAttachment::all(&mut conn)?.is_empty());

        let conversation_id = insert_conversation_named("test-2", &mut conn)?;
        let message = Message::insert(
            NewMessage::new(
                conversation_id,
                "OpenAI",
                Role::Assistant,
                &Content::default(),
                &serde_json::json!({"model": "gpt-4o"}),
                Status::Normal,
            ),
            &mut conn,
        )?;
        Message::replace_run_persistence(message.id, &sample_persistence(), &mut conn)?;

        Message::delete_by_conversation_id(conversation_id, &mut conn)?;

        assert!(SqlMessageRunState::all(&mut conn)?.is_empty());
        assert!(SqlMessageOutputItem::all(&mut conn)?.is_empty());
        assert!(SqlMessageAttachment::all(&mut conn)?.is_empty());
        Ok(())
    }
}

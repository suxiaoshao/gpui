use super::utils::{deserialize_offset_date_time, serialize_offset_date_time};
use crate::{
    database::{
        Role, Status,
        model::{SqlConversation, SqlMessage, SqlNewMessage},
    },
    errors::{AiChatError, AiChatResult},
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
        }
    }

    pub fn with_error(mut self, error: &'a str) -> Self {
        self.error = Some(error);
        self
    }
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
    ) -> AiChatResult<()> {
        let time = OffsetDateTime::now_utc();
        SqlMessage::update_content(id, serde_json::to_value(content)?, time, conn)?;
        Ok(())
    }
    pub fn reset_for_resend(id: i32, conn: &mut SqliteConnection) -> AiChatResult<()> {
        let time = OffsetDateTime::now_utc();
        SqlMessage::reset_for_resend(id, serde_json::to_value(Content::default())?, time, conn)?;
        Ok(())
    }
}

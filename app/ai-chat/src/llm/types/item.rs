use serde::{Deserialize, Serialize};

use crate::{
    database::Role,
    errors::{AiChatError, AiChatResult},
};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) enum LlmContentPart {
    Text(String),
    ImageRef(LlmAttachmentRef),
    FileRef(LlmAttachmentRef),
    AudioRef(LlmAttachmentRef),
    AttachmentRef(LlmAttachmentRef),
}

impl LlmContentPart {
    pub(crate) fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    pub(crate) fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            Self::ImageRef(_) | Self::FileRef(_) | Self::AudioRef(_) | Self::AttachmentRef(_) => {
                None
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct LlmAttachmentRef {
    pub(crate) id: String,
    pub(crate) mime_type: Option<String>,
    pub(crate) name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) enum LlmInputItem {
    System { content: Vec<LlmContentPart> },
    Developer { content: Vec<LlmContentPart> },
    User { content: Vec<LlmContentPart> },
    Assistant { content: Vec<LlmContentPart> },
    ToolResult(LlmToolResult),
    ItemReference { item_id: String },
}

impl LlmInputItem {
    pub(crate) fn from_role_text(role: Role, text: impl Into<String>) -> Self {
        let content = vec![LlmContentPart::text(text)];
        match role {
            Role::Developer => Self::Developer { content },
            Role::User => Self::User { content },
            Role::Assistant => Self::Assistant { content },
        }
    }

    pub(crate) fn single_text(&self) -> AiChatResult<(&'static str, &str)> {
        match self {
            Self::System { content } => single_text_content("system", content),
            Self::Developer { content } => single_text_content("developer", content),
            Self::User { content } => single_text_content("user", content),
            Self::Assistant { content } => single_text_content("assistant", content),
            Self::ToolResult(_) => Err(unsupported_input_item("tool result")),
            Self::ItemReference { .. } => Err(unsupported_input_item("item reference")),
        }
    }
}

fn single_text_content<'a>(
    role: &'static str,
    content: &'a [LlmContentPart],
) -> AiChatResult<(&'static str, &'a str)> {
    match content {
        [part] => part
            .as_text()
            .map(|text| (role, text))
            .ok_or_else(|| unsupported_input_item("non-text content part")),
        [] => Err(unsupported_input_item("empty content")),
        _ => Err(unsupported_input_item("multi-part content")),
    }
}

fn unsupported_input_item(kind: &str) -> AiChatError {
    AiChatError::StreamError(format!(
        "unsupported LLM input item for current provider adapter: {kind}"
    ))
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct LlmToolCall {
    pub(crate) call_id: String,
    pub(crate) name: String,
    pub(crate) arguments: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct LlmToolResult {
    pub(crate) call_id: String,
    pub(crate) content: Vec<LlmContentPart>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct LlmHostedToolCall {
    pub(crate) call_id: String,
    pub(crate) tool_type: String,
    pub(crate) status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct LlmMcpApprovalRequest {
    pub(crate) request_id: String,
    pub(crate) server_label: String,
    pub(crate) tool_name: String,
    pub(crate) arguments: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) enum LlmOutputItem {
    Message {
        role: Role,
        content: Vec<LlmContentPart>,
    },
    Reasoning {
        summary: Option<String>,
    },
    ToolCall(LlmToolCall),
    ToolResult(LlmToolResult),
    McpApproval(LlmMcpApprovalRequest),
    HostedToolCall(LlmHostedToolCall),
}

#[cfg(test)]
mod tests {
    use super::{LlmAttachmentRef, LlmContentPart, LlmInputItem, LlmOutputItem};
    use crate::database::Role;

    #[test]
    fn from_role_text_builds_provider_neutral_text_items() -> anyhow::Result<()> {
        assert_eq!(
            LlmInputItem::from_role_text(Role::Developer, "system").single_text()?,
            ("developer", "system")
        );
        assert_eq!(
            LlmInputItem::from_role_text(Role::User, "hello").single_text()?,
            ("user", "hello")
        );
        assert_eq!(
            LlmInputItem::from_role_text(Role::Assistant, "answer").single_text()?,
            ("assistant", "answer")
        );
        Ok(())
    }

    #[test]
    fn text_content_extracts_text_only_parts() {
        assert_eq!(LlmContentPart::text("hello").as_text(), Some("hello"));
        assert_eq!(
            LlmContentPart::ImageRef(LlmAttachmentRef {
                id: "image-1".to_string(),
                mime_type: Some("image/png".to_string()),
                name: None,
            })
            .as_text(),
            None
        );
    }

    #[test]
    fn single_text_rejects_non_text_items() {
        let item = LlmInputItem::User {
            content: vec![LlmContentPart::ImageRef(LlmAttachmentRef {
                id: "image-1".to_string(),
                mime_type: Some("image/png".to_string()),
                name: None,
            })],
        };

        assert!(item.single_text().is_err());
    }

    #[test]
    fn output_items_are_provider_neutral() {
        let item = LlmOutputItem::HostedToolCall(super::LlmHostedToolCall {
            call_id: "call-1".to_string(),
            tool_type: "web_search".to_string(),
            status: Some("completed".to_string()),
        });

        assert!(matches!(item, LlmOutputItem::HostedToolCall(_)));
    }
}

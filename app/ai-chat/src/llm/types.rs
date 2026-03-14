use serde::Deserialize;

mod chat_request;
mod message;

pub use chat_request::{ChatRequest, HostedTool, ReasoningConfig};
pub use message::Message;

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum OpenAIResponseStreamEvent {
    #[serde(rename = "response.output_item.added")]
    ResponseOutputItemAdded { item: OpenAIOutputItemEvent },
    #[serde(rename = "response.reasoning_summary_text.delta")]
    ResponseReasoningSummaryTextDelta { delta: String },
    #[serde(rename = "response.output_text.delta")]
    ResponseOutputTextDelta { delta: String },
    #[serde(rename = "response.completed")]
    ResponseCompleted { response: serde_json::Value },
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "response.failed")]
    ResponseFailed { response: OpenAIResponse },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
pub struct OpenAIResponse {
    pub error: OpenAIResponseError,
}

#[derive(Debug, Deserialize)]
pub struct OpenAIResponseError {
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct OpenAIOutputItemEvent {
    #[serde(rename = "type")]
    pub item_type: String,
}

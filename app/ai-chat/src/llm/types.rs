use serde::{Deserialize, Serialize};

mod chat_request;
mod message;

pub use chat_request::ChatRequest;
pub use message::Message;

#[derive(Debug, Deserialize, Serialize)]
pub struct OpenAIResponseStreamEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub delta: Option<String>,
}

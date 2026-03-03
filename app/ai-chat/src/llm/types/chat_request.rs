/*
 * @Author: suxiaoshao suxiaoshao@gmail.com
 * @Date: 2024-01-06 01:08:42
 * @LastEditors: suxiaoshao suxiaoshao@gmail.com
 * @LastEditTime: 2024-04-28 07:30:56
 * @FilePath: /tauri/packages/ChatGPT/src-tauri/src/fetch/types/chat_request.rs
 */
use serde::{Deserialize, Serialize};

use super::message::Message;

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatRequest<'a> {
    pub model: &'a str,
    pub input: Vec<Message>,
    pub stream: bool,
    pub temperature: f64,
    pub top_p: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    pub presence_penalty: f64,
    pub frequency_penalty: f64,
}

use serde::{Deserialize, Serialize};

use crate::database::Content;

use super::{LlmInputItem, LlmMcpApprovalRequest, LlmOutputItem, LlmToolCall, LlmToolResult};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub(crate) struct ProviderRunRequest {
    pub(crate) provider_name: String,
    pub(crate) request_body: serde_json::Value,
    pub(crate) input_items: Vec<LlmInputItem>,
    pub(crate) state: Option<ProviderRunState>,
}

impl ProviderRunRequest {
    pub(crate) fn new(
        provider_name: impl Into<String>,
        request_body: serde_json::Value,
        input_items: Vec<LlmInputItem>,
    ) -> Self {
        Self {
            provider_name: provider_name.into(),
            request_body,
            input_items,
            state: None,
        }
    }

    pub(crate) fn from_request_body(
        provider_name: impl Into<String>,
        request_body: serde_json::Value,
    ) -> Self {
        Self::new(provider_name, request_body, Vec::new())
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub(crate) struct ProviderRunState {
    pub(crate) provider_name: String,
    pub(crate) run_id: Option<String>,
    pub(crate) output_item_ids: Vec<String>,
    pub(crate) continuation_metadata: serde_json::Value,
    pub(crate) request_body: serde_json::Value,
}

impl ProviderRunState {
    pub(crate) fn new(
        provider_name: impl Into<String>,
        run_id: Option<String>,
        output_item_ids: Vec<String>,
        request_body: serde_json::Value,
    ) -> Self {
        Self {
            provider_name: provider_name.into(),
            run_id,
            output_item_ids,
            continuation_metadata: serde_json::Value::Null,
            request_body,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub(crate) struct ProviderUsage {
    pub(crate) input_tokens: Option<u64>,
    pub(crate) output_tokens: Option<u64>,
    pub(crate) total_tokens: Option<u64>,
    pub(crate) metadata: serde_json::Value,
}

impl ProviderUsage {
    pub(crate) fn new(
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
        total_tokens: Option<u64>,
    ) -> Self {
        Self {
            input_tokens,
            output_tokens,
            total_tokens,
            metadata: serde_json::Value::Null,
        }
    }
}

#[allow(dead_code)]
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ProviderRunEvent {
    ThinkingStarted,
    ReasoningSummaryDelta(String),
    TextDelta(String),
    OutputItemAdded(LlmOutputItem),
    OutputItemDone(LlmOutputItem),
    ToolCallRequested(LlmToolCall),
    ToolResultReceived(LlmToolResult),
    McpApprovalRequested(LlmMcpApprovalRequest),
    UsageUpdated(ProviderUsage),
    Completed {
        content: Content,
        state: Option<ProviderRunState>,
        usage: Option<ProviderUsage>,
    },
    Failed {
        message: String,
    },
}

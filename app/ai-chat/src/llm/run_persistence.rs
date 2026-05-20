use crate::{
    database::{
        MessageOutputItem, MessageOutputItemStatus, MessageRunPersistence, MessageRunState,
    },
    state::AiChatConfig,
};

use super::{
    LlmOutputItem, LlmToolCall, LlmToolResult, ProviderRunRequest, ProviderRunState, ProviderUsage,
};

#[derive(Debug, Clone)]
pub(crate) struct ProviderRunPersistenceAccumulator {
    provider_name: String,
    request_body: serde_json::Value,
    model: Option<String>,
    settings: Option<serde_json::Value>,
    state: Option<ProviderRunState>,
    usage: Option<ProviderUsage>,
    output_items: Vec<MessageOutputItem>,
    next_sequence: i32,
}

impl ProviderRunPersistenceAccumulator {
    pub(crate) fn new(request: &ProviderRunRequest, config: &AiChatConfig) -> Self {
        let settings = config
            .get_provider_settings(&request.provider_name)
            .and_then(|settings| serde_json::to_value(settings).ok());
        Self {
            provider_name: request.provider_name.clone(),
            request_body: request.request_body.clone(),
            model: request
                .request_body
                .get("model")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string),
            settings,
            state: None,
            usage: None,
            output_items: Vec::new(),
            next_sequence: 0,
        }
    }

    pub(crate) fn record_output_item_added(&mut self, item: LlmOutputItem) {
        self.push_output_item(item, MessageOutputItemStatus::Added);
    }

    pub(crate) fn record_output_item_done(&mut self, item: LlmOutputItem) {
        self.push_output_item(item, MessageOutputItemStatus::Done);
    }

    pub(crate) fn record_tool_call_requested(&mut self, tool_call: LlmToolCall) {
        self.push_output_item(
            LlmOutputItem::ToolCall(tool_call),
            MessageOutputItemStatus::ToolCallRequested,
        );
    }

    pub(crate) fn record_tool_result_received(&mut self, tool_result: LlmToolResult) {
        self.push_output_item(
            LlmOutputItem::ToolResult(tool_result),
            MessageOutputItemStatus::ToolResultReceived,
        );
    }

    pub(crate) fn record_mcp_approval_requested(&mut self, request: super::LlmMcpApprovalRequest) {
        self.push_output_item(
            LlmOutputItem::McpApproval(request),
            MessageOutputItemStatus::McpApprovalRequested,
        );
    }

    pub(crate) fn record_usage(&mut self, usage: ProviderUsage) {
        self.usage = Some(usage);
    }

    pub(crate) fn record_completed(
        &mut self,
        state: Option<ProviderRunState>,
        usage: Option<ProviderUsage>,
    ) {
        if let Some(state) = state {
            self.state = Some(state);
        }
        if let Some(usage) = usage {
            self.usage = Some(usage);
        }
    }

    pub(crate) fn persistence(&self) -> Option<MessageRunPersistence> {
        let run_state = self
            .state
            .clone()
            .map(|state| {
                MessageRunState::from_provider_state(
                    state,
                    self.usage.clone(),
                    self.model.clone(),
                    self.settings.clone(),
                )
            })
            .or_else(|| {
                self.usage.as_ref().map(|usage| {
                    MessageRunState::from_request_snapshot(
                        self.provider_name.clone(),
                        self.request_body.clone(),
                        Some(usage.clone()),
                        self.model.clone(),
                        self.settings.clone(),
                    )
                })
            });
        let mut persistence = MessageRunPersistence {
            run_state,
            output_items: self.output_items.clone(),
            attachments: self
                .output_items
                .iter()
                .flat_map(MessageOutputItem::attachments)
                .collect(),
        }
        .with_deduped_attachments();
        if persistence.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut persistence))
        }
    }

    fn push_output_item(&mut self, item: LlmOutputItem, status: MessageOutputItemStatus) {
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        self.output_items
            .push(MessageOutputItem::new(sequence, item, status));
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        database::MessageOutputItemStatus,
        llm::{LlmHostedToolCall, LlmOutputItem, ProviderRunState, ProviderUsage},
        state::AiChatConfig,
    };

    use super::{ProviderRunPersistenceAccumulator, ProviderRunRequest};

    fn request() -> ProviderRunRequest {
        ProviderRunRequest::new(
            "OpenAI",
            serde_json::json!({
                "model": "gpt-4o",
                "input": []
            }),
            Vec::new(),
        )
    }

    #[test]
    fn completed_event_persists_run_state_and_usage() {
        let request = request();
        let mut accumulator =
            ProviderRunPersistenceAccumulator::new(&request, &AiChatConfig::default());
        let usage = ProviderUsage::new(Some(10), Some(20), Some(30));

        accumulator.record_completed(
            Some(ProviderRunState::new(
                "OpenAI",
                Some("resp_1".to_string()),
                vec!["item_1".to_string()],
                request.request_body.clone(),
            )),
            Some(usage.clone()),
        );

        let persistence = accumulator.persistence().expect("persistence");
        let run_state = persistence.run_state.expect("run state");
        assert_eq!(run_state.provider, "OpenAI");
        assert_eq!(run_state.run_id.as_deref(), Some("resp_1"));
        assert_eq!(run_state.output_item_ids, vec!["item_1"]);
        assert_eq!(run_state.usage, Some(usage));
        assert_eq!(run_state.model.as_deref(), Some("gpt-4o"));
    }

    #[test]
    fn output_item_events_preserve_stream_order() {
        let request = request();
        let mut accumulator =
            ProviderRunPersistenceAccumulator::new(&request, &AiChatConfig::default());

        accumulator.record_output_item_added(LlmOutputItem::Reasoning { summary: None });
        accumulator.record_output_item_done(LlmOutputItem::HostedToolCall(LlmHostedToolCall {
            call_id: "call_1".to_string(),
            tool_type: "web_search_call".to_string(),
            status: Some("completed".to_string()),
        }));

        let persistence = accumulator.persistence().expect("persistence");
        assert_eq!(persistence.output_items.len(), 2);
        assert_eq!(persistence.output_items[0].sequence, 0);
        assert_eq!(
            persistence.output_items[0].status,
            MessageOutputItemStatus::Added
        );
        assert_eq!(persistence.output_items[1].sequence, 1);
        assert_eq!(
            persistence.output_items[1].status,
            MessageOutputItemStatus::Done
        );
    }

    #[test]
    fn usage_only_failed_run_still_persists_partial_state() {
        let request = request();
        let mut accumulator =
            ProviderRunPersistenceAccumulator::new(&request, &AiChatConfig::default());
        accumulator.record_usage(ProviderUsage::new(Some(1), None, Some(1)));

        let persistence = accumulator.persistence().expect("persistence");
        let run_state = persistence.run_state.expect("usage snapshot");
        assert!(run_state.run_id.is_none());
        assert_eq!(run_state.provider, "OpenAI");
        assert_eq!(run_state.request_body["model"], "gpt-4o");
        assert_eq!(
            run_state
                .usage
                .as_ref()
                .and_then(|usage| usage.input_tokens),
            Some(1)
        );
    }
}

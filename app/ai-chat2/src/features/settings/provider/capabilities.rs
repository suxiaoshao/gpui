#![allow(dead_code)]

use ai_chat_core::{ModelCapabilitiesSnapshot, conservative_model_capabilities};

use super::catalog::ProviderKindKey;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct CapabilityDraft {
    pub(super) text_input: bool,
    pub(super) text_output: bool,
    pub(super) streaming: bool,
    pub(super) image_input: bool,
    pub(super) image_generation: bool,
    pub(super) tool_calling: bool,
    pub(super) hosted_web_search: bool,
    pub(super) reasoning: bool,
    pub(super) structured_output: bool,
    pub(super) context_window_tokens: Option<u32>,
}

impl Default for CapabilityDraft {
    fn default() -> Self {
        Self {
            text_input: true,
            text_output: true,
            streaming: true,
            image_input: false,
            image_generation: false,
            tool_calling: false,
            hosted_web_search: false,
            reasoning: false,
            structured_output: false,
            context_window_tokens: None,
        }
    }
}

pub(super) fn conservative_capabilities(kind: &ProviderKindKey) -> ModelCapabilitiesSnapshot {
    conservative_model_capabilities(kind.as_str())
}

pub(super) fn snapshot_from_draft(
    draft: CapabilityDraft,
    kind: &ProviderKindKey,
) -> ModelCapabilitiesSnapshot {
    let mut snapshot = conservative_model_capabilities(kind.as_str());
    snapshot.text_input = draft.text_input;
    snapshot.text_output = draft.text_output;
    snapshot.streaming = draft.streaming;
    snapshot.image_input = draft
        .image_input
        .then_some(ai_chat_core::ImageInputCapabilitySnapshot { max_images: None });
    snapshot.image_generation = draft.image_generation;
    snapshot.tool_calling =
        draft
            .tool_calling
            .then_some(ai_chat_core::ToolCallingCapabilitySnapshot {
                parallel_tool_calls: true,
            });
    snapshot.hosted_web_search = draft.hosted_web_search;
    snapshot.reasoning = draft
        .reasoning
        .then_some(ai_chat_core::ReasoningCapabilitySnapshot {
            default_effort: "medium".to_string(),
            efforts: vec!["low".to_string(), "medium".to_string(), "high".to_string()],
            summaries: false,
        });
    snapshot.structured_output = draft.structured_output;
    snapshot
}

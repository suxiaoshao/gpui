#![allow(dead_code)]

use ai_chat_core::{
    ImageInputCapabilitySnapshot, ModelCapabilitiesSnapshot, ProviderCapabilityExtensionSnapshot,
    ReasoningCapabilitySnapshot, ToolCallingCapabilitySnapshot,
};

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
    let mut draft = CapabilityDraft::default();
    match kind.as_str() {
        "openai" | "anthropic" | "gemini" | "openrouter" => {
            draft.tool_calling = true;
            draft.reasoning = true;
            draft.structured_output = true;
        }
        "ollama" => {
            draft.tool_calling = true;
        }
        _ => {}
    }
    snapshot_from_draft(draft, kind)
}

pub(super) fn snapshot_from_draft(
    draft: CapabilityDraft,
    kind: &ProviderKindKey,
) -> ModelCapabilitiesSnapshot {
    ModelCapabilitiesSnapshot {
        text_input: draft.text_input,
        text_output: draft.text_output,
        streaming: draft.streaming,
        image_input: draft
            .image_input
            .then_some(ImageInputCapabilitySnapshot { max_images: None }),
        file_input: None,
        audio_input: false,
        image_generation: draft.image_generation,
        tool_calling: draft.tool_calling.then_some(ToolCallingCapabilitySnapshot {
            parallel_tool_calls: true,
        }),
        hosted_web_search: draft.hosted_web_search,
        remote_mcp: false,
        reasoning: draft.reasoning.then_some(ReasoningCapabilitySnapshot {
            default_effort: "medium".to_string(),
            efforts: vec!["low".to_string(), "medium".to_string(), "high".to_string()],
            summaries: false,
        }),
        structured_output: draft.structured_output,
        stateful_response_continuation: kind.as_str() == "openai",
        extension: match kind.as_str() {
            "openai" => ProviderCapabilityExtensionSnapshot::OpenAi {
                responses_api: true,
                raw: None,
            },
            "ollama" => ProviderCapabilityExtensionSnapshot::Ollama {
                raw_capabilities: Vec::new(),
                family: "unknown".to_string(),
                raw: None,
            },
            _ => ProviderCapabilityExtensionSnapshot::None,
        },
    }
}

use crate::{
    ImageInputCapabilitySnapshot, ModelCapabilitiesSnapshot, ProviderCapabilityExtensionSnapshot,
    ToolCallingCapabilitySnapshot,
};

pub fn conservative_model_capabilities(provider_kind: &str) -> ModelCapabilitiesSnapshot {
    let mut tool_calling = false;
    let mut structured_output = false;

    match provider_kind {
        "openai" | "anthropic" | "gemini" | "openrouter" => {
            tool_calling = true;
            structured_output = true;
        }
        "ollama" => {
            tool_calling = true;
        }
        _ => {}
    }

    ModelCapabilitiesSnapshot {
        text_input: true,
        text_output: true,
        streaming: true,
        image_input: None::<ImageInputCapabilitySnapshot>,
        file_input: None,
        audio_input: false,
        image_generation: false,
        tool_calling: tool_calling.then_some(ToolCallingCapabilitySnapshot {
            parallel_tool_calls: true,
        }),
        hosted_web_search: false,
        remote_mcp: false,
        reasoning: None,
        structured_output,
        stateful_response_continuation: provider_kind == "openai",
        extension: match provider_kind {
            "openai" => ProviderCapabilityExtensionSnapshot::OpenAi {
                responses_api: true,
                raw: None,
            },
            "ollama" => ProviderCapabilityExtensionSnapshot::Ollama {
                raw_capabilities: Vec::new(),
                family: "unknown".to_string(),
                families: Vec::new(),
                thinking: None,
                local_web_tools: false,
                raw: None,
            },
            _ => ProviderCapabilityExtensionSnapshot::None,
        },
    }
}

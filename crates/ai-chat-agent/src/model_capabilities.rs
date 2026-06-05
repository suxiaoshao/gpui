use ai_chat_core::{
    CapabilitySourceSnapshot, ImageInputCapabilitySnapshot, ModelCapabilitiesSnapshot,
    OllamaThinkingCapabilitySnapshot, ProviderCapabilityExtensionSnapshot, ProviderRawPayload,
    ReasoningCapabilitySnapshot, ReasoningControlSnapshot, ToolCallingCapabilitySnapshot,
    conservative_model_capabilities,
};

const CHECKED_AT: &str = "2026-06-03";

pub(crate) fn capabilities_for_model(
    provider_kind: &str,
    model_id: &str,
    raw: Option<ProviderRawPayload>,
) -> ModelCapabilitiesSnapshot {
    let mut snapshot = conservative_model_capabilities(provider_kind);
    snapshot.extension = provider_extension(provider_kind, raw);

    match provider_kind {
        "openai" => apply_openai_profile(model_id, &mut snapshot),
        "anthropic" => apply_anthropic_profile(model_id, &mut snapshot),
        "gemini" => apply_gemini_profile(model_id, &mut snapshot),
        "deepseek" => apply_deepseek_profile(model_id, &mut snapshot),
        "mistral" => apply_mistral_profile(model_id, &mut snapshot),
        _ => {}
    }

    snapshot
}

pub(crate) fn capabilities_from_ollama_show(
    raw_capabilities: Vec<String>,
    family: String,
    families: Vec<String>,
    raw: Option<ProviderRawPayload>,
) -> ModelCapabilitiesSnapshot {
    let mut snapshot = conservative_model_capabilities("ollama");
    snapshot.image_input = None;
    snapshot.tool_calling = None;
    let supports = |name: &str| raw_capabilities.iter().any(|capability| capability == name);
    let thinking = thinking_from_ollama_family(&raw_capabilities, &family, &families);

    if supports("vision") {
        snapshot.image_input = Some(ImageInputCapabilitySnapshot { max_images: None });
    }
    if supports("tools") {
        snapshot.tool_calling = Some(ToolCallingCapabilitySnapshot {
            parallel_tool_calls: true,
        });
    }
    snapshot.reasoning = thinking.map(|thinking| match thinking {
        OllamaThinkingCapabilitySnapshot::Levels => reasoning_capability(
            ReasoningControlSnapshot::Levels {
                values: values(["low", "medium", "high"]),
                default_value: Some("medium".to_string()),
            },
            source_api("ollama", "/api/show"),
            "medium",
            ["low", "medium", "high"],
            true,
        ),
        OllamaThinkingCapabilitySnapshot::Boolean => reasoning_capability(
            ReasoningControlSnapshot::Boolean {
                default_enabled: Some(false),
            },
            source_api("ollama", "/api/show"),
            "disabled",
            ["enabled"],
            true,
        ),
    });
    snapshot.extension = ProviderCapabilityExtensionSnapshot::Ollama {
        raw_capabilities: raw_capabilities.clone(),
        family,
        families,
        thinking,
        local_web_tools: supports("tools"),
        raw,
    };

    snapshot
}

pub(crate) fn capabilities_from_gemini_model(
    model_id: &str,
    thinking: Option<bool>,
    raw: Option<ProviderRawPayload>,
) -> ModelCapabilitiesSnapshot {
    let mut snapshot = capabilities_for_model("gemini", model_id, None);
    if thinking == Some(false) {
        snapshot.reasoning = None;
    } else if thinking == Some(true) && snapshot.reasoning.is_none() {
        snapshot.reasoning = Some(reasoning_capability(
            ReasoningControlSnapshot::Boolean {
                default_enabled: Some(true),
            },
            source_api("gemini", "/v1beta/models"),
            "enabled",
            ["enabled"],
            true,
        ));
    }
    snapshot.extension = ProviderCapabilityExtensionSnapshot::Gemini { thinking, raw };
    snapshot
}

pub(crate) fn capabilities_from_openrouter_model(
    supported_parameters: Vec<String>,
    raw: Option<ProviderRawPayload>,
) -> ModelCapabilitiesSnapshot {
    let mut snapshot = conservative_model_capabilities("openrouter");
    let supports = |name: &str| {
        supported_parameters
            .iter()
            .any(|parameter| parameter == name)
    };
    let supports_reasoning = supports("reasoning") || supports("include_reasoning");

    snapshot.tool_calling = supports("tools").then_some(ToolCallingCapabilitySnapshot {
        parallel_tool_calls: true,
    });
    snapshot.structured_output = supports("structured_outputs") || supports("response_format");

    if supports_reasoning {
        snapshot.reasoning = Some(reasoning_capability(
            ReasoningControlSnapshot::Composite {
                controls: vec![
                    ReasoningControlSnapshot::Boolean {
                        default_enabled: Some(true),
                    },
                    ReasoningControlSnapshot::Levels {
                        values: values(["none", "minimal", "low", "medium", "high", "xhigh"]),
                        default_value: Some("medium".to_string()),
                    },
                    ReasoningControlSnapshot::TokenBudget {
                        min: Some(0),
                        max: None,
                        default_value: None,
                        dynamic_supported: false,
                        off_supported: true,
                    },
                ],
            },
            CapabilitySourceSnapshot::OpenRouterNormalized,
            "medium",
            ["none", "minimal", "low", "medium", "high", "xhigh"],
            true,
        ));
    }

    snapshot.extension = ProviderCapabilityExtensionSnapshot::OpenRouter {
        supported_parameters,
        raw,
    };
    snapshot
}

fn provider_extension(
    provider_kind: &str,
    raw: Option<ProviderRawPayload>,
) -> ProviderCapabilityExtensionSnapshot {
    match provider_kind {
        "openai" => ProviderCapabilityExtensionSnapshot::OpenAi {
            responses_api: true,
            raw,
        },
        _ => raw
            .map(|raw| ProviderCapabilityExtensionSnapshot::Other { raw })
            .unwrap_or(ProviderCapabilityExtensionSnapshot::None),
    }
}

fn apply_openai_profile(model_id: &str, snapshot: &mut ModelCapabilitiesSnapshot) {
    let id = normalized_model_id(model_id);
    let source = source_docs(
        "openai",
        "https://platform.openai.com/docs/models",
        CHECKED_AT,
    );

    if id.starts_with("gpt-5.5") || id.starts_with("gpt-5.4") {
        snapshot.reasoning = Some(reasoning_capability(
            ReasoningControlSnapshot::Levels {
                values: values(["none", "low", "medium", "high", "xhigh"]),
                default_value: Some("medium".to_string()),
            },
            source,
            "medium",
            ["none", "low", "medium", "high", "xhigh"],
            true,
        ));
    } else if id.starts_with("gpt-5.2-codex") {
        snapshot.reasoning = Some(reasoning_capability(
            ReasoningControlSnapshot::Levels {
                values: values(["low", "medium", "high", "xhigh"]),
                default_value: Some("medium".to_string()),
            },
            source,
            "medium",
            ["low", "medium", "high", "xhigh"],
            true,
        ));
    } else if id.starts_with("gpt-5") {
        snapshot.reasoning = Some(reasoning_capability(
            ReasoningControlSnapshot::Levels {
                values: values(["minimal", "low", "medium", "high"]),
                default_value: Some("medium".to_string()),
            },
            source,
            "medium",
            ["minimal", "low", "medium", "high"],
            true,
        ));
    } else if id.starts_with("o1") || id.starts_with("o3") || id.starts_with("o4") {
        snapshot.reasoning = Some(reasoning_capability(
            ReasoningControlSnapshot::Levels {
                values: values(["low", "medium", "high"]),
                default_value: Some("medium".to_string()),
            },
            source,
            "medium",
            ["low", "medium", "high"],
            true,
        ));
    }
}

fn apply_anthropic_profile(model_id: &str, snapshot: &mut ModelCapabilitiesSnapshot) {
    let id = normalized_model_id(model_id);
    let source = source_docs(
        "anthropic",
        "https://platform.claude.com/docs/en/build-with-claude/effort",
        CHECKED_AT,
    );

    if contains_any(&id, ["opus-4-8", "opus-4.8", "opus-4-7", "opus-4.7"]) {
        snapshot.reasoning = Some(reasoning_capability(
            ReasoningControlSnapshot::AdaptiveLevels {
                values: values(["low", "medium", "high", "xhigh", "max"]),
                default_value: Some("high".to_string()),
            },
            source,
            "high",
            ["low", "medium", "high", "xhigh", "max"],
            true,
        ));
    } else if contains_any(&id, ["opus-4", "sonnet-4", "haiku-4"]) {
        snapshot.reasoning = Some(reasoning_capability(
            ReasoningControlSnapshot::AdaptiveLevels {
                values: values(["low", "medium", "high"]),
                default_value: Some("medium".to_string()),
            },
            source,
            "medium",
            ["low", "medium", "high"],
            true,
        ));
    } else if contains_any(&id, ["claude-3-7", "claude-3.7"]) {
        snapshot.reasoning = Some(reasoning_capability(
            ReasoningControlSnapshot::TokenBudget {
                min: Some(1024),
                max: None,
                default_value: Some(4096),
                dynamic_supported: false,
                off_supported: false,
            },
            source_docs(
                "anthropic",
                "https://platform.claude.com/docs/en/build-with-claude/extended-thinking",
                CHECKED_AT,
            ),
            "4096",
            ["4096"],
            true,
        ));
    }
}

fn apply_gemini_profile(model_id: &str, snapshot: &mut ModelCapabilitiesSnapshot) {
    let id = normalized_model_id(model_id);
    let source = source_docs(
        "gemini",
        "https://ai.google.dev/gemini-api/docs/thinking",
        CHECKED_AT,
    );

    if id.starts_with("gemini-2.5") {
        snapshot.reasoning = Some(reasoning_capability(
            ReasoningControlSnapshot::TokenBudget {
                min: Some(0),
                max: Some(32768),
                default_value: Some(-1),
                dynamic_supported: true,
                off_supported: true,
            },
            source,
            "dynamic",
            ["off", "dynamic", "custom"],
            true,
        ));
    } else if id.starts_with("gemini-3-pro") || id.starts_with("gemini-3-flash") {
        snapshot.reasoning = Some(reasoning_capability(
            ReasoningControlSnapshot::Levels {
                values: values(["minimal", "low", "medium", "high"]),
                default_value: Some("medium".to_string()),
            },
            source,
            "medium",
            ["minimal", "low", "medium", "high"],
            true,
        ));
    }
}

fn apply_deepseek_profile(model_id: &str, snapshot: &mut ModelCapabilitiesSnapshot) {
    let id = normalized_model_id(model_id);
    if contains_any(&id, ["deepseek-v4", "deepseek-reasoner"]) {
        snapshot.reasoning = Some(reasoning_capability(
            ReasoningControlSnapshot::Levels {
                values: values(["disabled", "high", "max"]),
                default_value: Some("high".to_string()),
            },
            source_docs(
                "deepseek",
                "https://api-docs.deepseek.com/guides/thinking_mode",
                CHECKED_AT,
            ),
            "high",
            ["disabled", "high", "max"],
            true,
        ));
    }
}

fn apply_mistral_profile(model_id: &str, snapshot: &mut ModelCapabilitiesSnapshot) {
    let id = normalized_model_id(model_id);
    let source = source_docs(
        "mistral",
        "https://docs.mistral.ai/capabilities/reasoning/",
        CHECKED_AT,
    );

    if id.contains("magistral") {
        snapshot.reasoning = Some(reasoning_capability(
            ReasoningControlSnapshot::AlwaysOn {
                visible_summary_supported: true,
            },
            source,
            "always_on",
            ["always_on"],
            true,
        ));
    } else if contains_any(
        &id,
        [
            "mistral-large",
            "mistral-medium",
            "mistral-small",
            "ministral",
        ],
    ) {
        snapshot.reasoning = Some(reasoning_capability(
            ReasoningControlSnapshot::Levels {
                values: values(["none", "high"]),
                default_value: Some("high".to_string()),
            },
            source,
            "high",
            ["none", "high"],
            true,
        ));
    }
}

fn thinking_from_ollama_family(
    capabilities: &[String],
    family: &str,
    families: &[String],
) -> Option<OllamaThinkingCapabilitySnapshot> {
    if !capabilities
        .iter()
        .any(|capability| capability == "thinking")
    {
        return None;
    }

    let family_matches = |family: &str| matches!(family, "gptoss" | "gpt-oss");
    let uses_levels =
        family_matches(family) || families.iter().any(|family| family_matches(family));

    Some(if uses_levels {
        OllamaThinkingCapabilitySnapshot::Levels
    } else {
        OllamaThinkingCapabilitySnapshot::Boolean
    })
}

fn reasoning_capability<const N: usize>(
    control: ReasoningControlSnapshot,
    source: CapabilitySourceSnapshot,
    default_effort: &str,
    efforts: [&str; N],
    summaries: bool,
) -> ReasoningCapabilitySnapshot {
    ReasoningCapabilitySnapshot {
        default_effort: default_effort.to_string(),
        efforts: values(efforts),
        summaries,
        control: Some(control),
        source,
    }
}

fn source_api(provider: &str, endpoint: &str) -> CapabilitySourceSnapshot {
    CapabilitySourceSnapshot::ApiDiscovered {
        provider: provider.to_string(),
        endpoint: endpoint.to_string(),
    }
}

fn source_docs(provider: &str, url: &str, checked_at: &str) -> CapabilitySourceSnapshot {
    CapabilitySourceSnapshot::OfficialDocs {
        provider: provider.to_string(),
        url: url.to_string(),
        checked_at: checked_at.to_string(),
    }
}

fn values<const N: usize>(values: [&str; N]) -> Vec<String> {
    values.into_iter().map(str::to_string).collect()
}

fn normalized_model_id(model_id: &str) -> String {
    model_id.trim().to_ascii_lowercase()
}

fn contains_any<const N: usize>(value: &str, needles: [&str; N]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openai_profiles_use_model_specific_efforts() {
        let gpt5 = capabilities_for_model("openai", "gpt-5", None);
        assert_eq!(
            gpt5.reasoning.unwrap().efforts,
            values(["minimal", "low", "medium", "high"])
        );

        let gpt55 = capabilities_for_model("openai", "gpt-5.5", None);
        assert_eq!(
            gpt55.reasoning.unwrap().efforts,
            values(["none", "low", "medium", "high", "xhigh"])
        );

        let codex = capabilities_for_model("openai", "gpt-5.2-codex", None);
        assert_eq!(
            codex.reasoning.unwrap().efforts,
            values(["low", "medium", "high", "xhigh"])
        );
    }

    #[test]
    fn ollama_thinking_distinguishes_boolean_and_level_models() {
        let qwen = capabilities_from_ollama_show(
            values(["completion", "thinking"]),
            "qwen3".to_string(),
            values(["qwen3"]),
            None,
        );
        assert!(matches!(
            qwen.reasoning.unwrap().control,
            Some(ReasoningControlSnapshot::Boolean { .. })
        ));
        assert!(qwen.tool_calling.is_none());

        let gpt_oss = capabilities_from_ollama_show(
            values(["completion", "thinking", "vision", "tools"]),
            "gpt-oss".to_string(),
            values(["gpt-oss"]),
            None,
        );
        assert!(gpt_oss.image_input.is_some());
        assert!(gpt_oss.tool_calling.is_some());
        assert!(matches!(
            gpt_oss.reasoning.unwrap().control,
            Some(ReasoningControlSnapshot::Levels { .. })
        ));
    }

    #[test]
    fn openrouter_reasoning_uses_normalized_control() {
        let snapshot = capabilities_from_openrouter_model(
            values(["tools", "structured_outputs", "reasoning"]),
            None,
        );
        let reasoning = snapshot.reasoning.expect("reasoning");
        assert!(snapshot.tool_calling.is_some());
        assert!(snapshot.structured_output);
        assert!(matches!(
            reasoning.source,
            CapabilitySourceSnapshot::OpenRouterNormalized
        ));
        assert!(matches!(
            reasoning.control,
            Some(ReasoningControlSnapshot::Composite { .. })
        ));

        let basic = capabilities_from_openrouter_model(values(["tools"]), None);
        assert!(basic.tool_calling.is_some());
        assert!(!basic.structured_output);
        assert!(basic.reasoning.is_none());
    }

    #[test]
    fn gemini_uses_api_thinking_signal_and_doc_profiles() {
        let unsupported = capabilities_from_gemini_model("gemini-2.5-flash", Some(false), None);
        assert!(unsupported.reasoning.is_none());

        let budget = capabilities_from_gemini_model("gemini-2.5-flash", Some(true), None);
        assert!(matches!(
            budget.reasoning.unwrap().control,
            Some(ReasoningControlSnapshot::TokenBudget { .. })
        ));

        let levels = capabilities_from_gemini_model("gemini-3-pro", Some(true), None);
        assert!(matches!(
            levels.reasoning.unwrap().control,
            Some(ReasoningControlSnapshot::Levels { .. })
        ));
    }

    #[test]
    fn deepseek_and_mistral_profiles_are_provider_native() {
        let deepseek = capabilities_for_model("deepseek", "deepseek-v4-flash", None);
        assert_eq!(
            deepseek.reasoning.unwrap().efforts,
            values(["disabled", "high", "max"])
        );

        let mistral = capabilities_for_model("mistral", "mistral-large-latest", None);
        assert_eq!(mistral.reasoning.unwrap().efforts, values(["none", "high"]));

        let magistral = capabilities_for_model("mistral", "magistral-medium-latest", None);
        assert!(matches!(
            magistral.reasoning.unwrap().control,
            Some(ReasoningControlSnapshot::AlwaysOn { .. })
        ));
    }
}

use jaco_core::{
    CapabilitySourceSnapshot, ContextWindowCapabilitySnapshot, FileInputCapabilitySnapshot,
    ImageInputCapabilitySnapshot, ModelCapabilitiesSnapshot, OllamaThinkingCapabilitySnapshot,
    ProviderCapabilityExtensionSnapshot, ProviderRawPayload, ReasoningCapabilitySnapshot,
    ReasoningControlSnapshot, ToolCallingCapabilitySnapshot, conservative_model_capabilities,
};

const CHECKED_AT: &str = "2026-06-14";
const OPENAI_MODELS_DOCS_URL: &str = "https://developers.openai.com/api/docs/models";
const OPENAI_MODELS_CHECKED_AT: &str = "2026-08-20";

pub(crate) fn capabilities_for_model(
    provider_kind: &str,
    model_id: &str,
    context_length: Option<u64>,
    raw: Option<ProviderRawPayload>,
) -> ModelCapabilitiesSnapshot {
    let mut snapshot = conservative_model_capabilities(provider_kind);
    snapshot.context_window = discovered_context_window(
        context_length,
        source_api(provider_kind, "rig model listing"),
    );
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

pub(crate) fn apply_documented_model_profile(
    provider_kind: &str,
    model_id: &str,
    official_endpoint: bool,
    capabilities: &mut ModelCapabilitiesSnapshot,
) {
    if capabilities.context_window.is_some() || provider_kind != "openai" || !official_endpoint {
        return;
    }

    let id = normalized_model_id(model_id);
    if !matches!(
        id.as_str(),
        "gpt-5.6" | "gpt-5.6-sol" | "gpt-5.6-terra" | "gpt-5.6-luna"
    ) {
        return;
    }

    capabilities.context_window = Some(ContextWindowCapabilitySnapshot {
        tokens: std::num::NonZeroU64::new(1_050_000).expect("documented OpenAI context window"),
        source: source_docs("openai", OPENAI_MODELS_DOCS_URL, OPENAI_MODELS_CHECKED_AT),
    });
}

pub(crate) fn capabilities_from_ollama_show(
    raw_capabilities: Vec<String>,
    family: String,
    families: Vec<String>,
    details_context_length: Option<serde_json::Value>,
    model_info: std::collections::BTreeMap<String, serde_json::Value>,
    raw: Option<ProviderRawPayload>,
) -> ModelCapabilitiesSnapshot {
    let mut snapshot = conservative_model_capabilities("ollama");
    snapshot.image_input = None;
    snapshot.tool_calling = None;
    let supports = |name: &str| raw_capabilities.iter().any(|capability| capability == name);
    let thinking = thinking_from_ollama_family(&raw_capabilities, &family, &families);
    snapshot.context_window = ollama_context_window(details_context_length, &model_info);

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
    input_token_limit: Option<serde_json::Value>,
    raw: Option<ProviderRawPayload>,
) -> ModelCapabilitiesSnapshot {
    let mut snapshot = capabilities_for_model("gemini", model_id, None, None);
    snapshot.context_window = discovered_context_window(
        input_token_limit.as_ref().and_then(json_positive_u64),
        source_api("gemini", "/v1beta/models"),
    );
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
    input_modalities: Vec<String>,
    output_modalities: Option<Vec<String>>,
    context_length: Option<serde_json::Value>,
    raw: Option<ProviderRawPayload>,
) -> ModelCapabilitiesSnapshot {
    let mut snapshot = conservative_model_capabilities("openrouter");
    snapshot.context_window = discovered_context_window(
        context_length.as_ref().and_then(openrouter_context_length),
        CapabilitySourceSnapshot::OpenRouterNormalized,
    );
    let supports = |name: &str| {
        supported_parameters
            .iter()
            .any(|parameter| parameter == name)
    };
    let supports_reasoning = supports("reasoning") || supports("include_reasoning");
    let supports_modality = |name: &str| {
        input_modalities
            .iter()
            .any(|modality| modality.eq_ignore_ascii_case(name))
    };

    snapshot.tool_calling = supports("tools").then_some(ToolCallingCapabilitySnapshot {
        parallel_tool_calls: true,
    });
    snapshot.structured_output = supports("structured_outputs") || supports("response_format");
    if supports_modality("image") {
        enable_image_input(&mut snapshot);
    }
    if supports_modality("file") {
        enable_file_input(&mut snapshot);
    }
    let known_output_modalities = output_modalities
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter_map(
            |modality| match modality.trim().to_ascii_lowercase().as_str() {
                "text" => Some("text"),
                "image" => Some("image"),
                _ => None,
            },
        )
        .collect::<std::collections::BTreeSet<_>>();
    if !known_output_modalities.is_empty() {
        snapshot.text_output = known_output_modalities.contains("text");
        snapshot.image_generation = known_output_modalities.contains("image");
    }

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

fn discovered_context_window(
    tokens: Option<u64>,
    source: CapabilitySourceSnapshot,
) -> Option<ContextWindowCapabilitySnapshot> {
    Some(ContextWindowCapabilitySnapshot {
        tokens: std::num::NonZeroU64::new(tokens?)?,
        source,
    })
}

fn ollama_context_window(
    details_context_length: Option<serde_json::Value>,
    model_info: &std::collections::BTreeMap<String, serde_json::Value>,
) -> Option<ContextWindowCapabilitySnapshot> {
    let mut context_lengths = std::collections::BTreeSet::new();
    if let Some(tokens) = details_context_length.as_ref().and_then(json_positive_u64) {
        context_lengths.insert(tokens);
    }
    for (key, value) in model_info {
        if (key == "context_length" || key.ends_with(".context_length"))
            && let Some(tokens) = json_positive_u64(value)
        {
            context_lengths.insert(tokens);
        }
    }
    let mut values = context_lengths.into_iter();
    let tokens = values.next()?;
    if values.next().is_some() {
        return None;
    }
    discovered_context_window(Some(tokens), source_api("ollama", "/api/show"))
}

fn json_positive_u64(value: &serde_json::Value) -> Option<u64> {
    value.as_u64().filter(|tokens| *tokens > 0)
}

fn openrouter_context_length(value: &serde_json::Value) -> Option<u64> {
    json_positive_u64(value).filter(|tokens| u32::try_from(*tokens).is_ok())
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

    if openai_model_supports_user_attachments(&id) {
        enable_image_input(snapshot);
        enable_file_input(snapshot);
    }

    if id.starts_with("gpt-5.6") {
        snapshot.reasoning = Some(reasoning_capability(
            ReasoningControlSnapshot::Levels {
                values: values(["none", "low", "medium", "high", "xhigh", "max"]),
                default_value: Some("medium".to_string()),
            },
            source,
            "medium",
            ["none", "low", "medium", "high", "xhigh", "max"],
            true,
        ));
    } else if id.starts_with("gpt-5.5") || id.starts_with("gpt-5.4") {
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

    if id.starts_with("claude-") {
        enable_image_input(snapshot);
        enable_file_input(snapshot);
    }

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

    if gemini_model_supports_user_attachments(&id) {
        enable_image_input(snapshot);
        enable_file_input(snapshot);
    }

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
    } else if id.starts_with("gemini-3") {
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

    if contains_any(&id, ["deepseek-v4", "deepseek-chat"]) {
        enable_tool_calling(snapshot);
        snapshot.structured_output = true;
    }
    if id.contains("deepseek-reasoner") {
        snapshot.structured_output = true;
    }

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

    if mistral_model_supports_structured_output(&id) {
        snapshot.structured_output = true;
    }
    if mistral_model_supports_tool_calling(&id) {
        enable_tool_calling(snapshot);
    }
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

fn enable_image_input(snapshot: &mut ModelCapabilitiesSnapshot) {
    snapshot.image_input = Some(ImageInputCapabilitySnapshot { max_images: None });
}

fn enable_file_input(snapshot: &mut ModelCapabilitiesSnapshot) {
    snapshot.file_input = Some(FileInputCapabilitySnapshot { max_files: None });
}

fn enable_tool_calling(snapshot: &mut ModelCapabilitiesSnapshot) {
    snapshot.tool_calling = Some(ToolCallingCapabilitySnapshot {
        parallel_tool_calls: true,
    });
}

fn openai_model_supports_user_attachments(id: &str) -> bool {
    id.starts_with("gpt-5.5") || id.starts_with("gpt-5.4") || openai_legacy_multimodal_model(id)
}

fn openai_legacy_multimodal_model(id: &str) -> bool {
    (id.starts_with("gpt-5") && !id.starts_with("gpt-5.2-codex"))
        || id.starts_with("gpt-4.1")
        || id.starts_with("gpt-4o")
        || id.starts_with("chatgpt-4o")
        || id.starts_with("o3")
        || id.starts_with("o4")
}

fn gemini_model_supports_user_attachments(id: &str) -> bool {
    id.starts_with("gemini-2.5") || id.starts_with("gemini-3")
}

fn mistral_model_supports_structured_output(id: &str) -> bool {
    contains_any(
        id,
        [
            "mistral-large",
            "mistral-medium",
            "mistral-small",
            "ministral",
            "devstral",
            "magistral",
            "codestral",
            "voxtral-small",
            "labs-mistral-small-creative",
        ],
    )
}

fn mistral_model_supports_tool_calling(id: &str) -> bool {
    mistral_model_supports_structured_output(id)
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
    fn authoritative_context_windows_require_positive_values_and_keep_provenance() {
        let rig = capabilities_for_model("anthropic", "claude-sonnet-4-6", Some(200_000), None);
        let rig_context = rig.context_window.expect("rig context window");
        assert_eq!(rig_context.tokens.get(), 200_000);
        assert_eq!(
            rig_context.source,
            source_api("anthropic", "rig model listing")
        );

        let gemini = capabilities_from_gemini_model(
            "gemini-2.5-flash",
            Some(true),
            Some(serde_json::json!(1_048_576)),
            None,
        );
        let gemini_context = gemini.context_window.expect("Gemini context window");
        assert_eq!(gemini_context.tokens.get(), 1_048_576);
        assert_eq!(
            gemini_context.source,
            source_api("gemini", "/v1beta/models")
        );

        let openrouter = capabilities_from_openrouter_model(
            values(["tools"]),
            values(["text"]),
            None,
            Some(serde_json::json!(272_000)),
            None,
        );
        let openrouter_context = openrouter
            .context_window
            .expect("OpenRouter context window");
        assert_eq!(openrouter_context.tokens.get(), 272_000);
        assert_eq!(
            openrouter_context.source,
            CapabilitySourceSnapshot::OpenRouterNormalized
        );

        assert!(
            capabilities_for_model("openai", "gpt-5", Some(0), None)
                .context_window
                .is_none()
        );
        assert!(
            capabilities_from_gemini_model(
                "gemini-2.5-flash",
                None,
                Some(serde_json::json!(0)),
                None,
            )
            .context_window
            .is_none()
        );
        for invalid in [
            serde_json::json!(-1),
            serde_json::json!(1.5),
            serde_json::json!("128k"),
        ] {
            assert!(
                capabilities_from_gemini_model(
                    "gemini-2.5-flash",
                    None,
                    Some(invalid.clone()),
                    None,
                )
                .context_window
                .is_none()
            );
            assert!(
                capabilities_from_openrouter_model(
                    Vec::new(),
                    Vec::new(),
                    None,
                    Some(invalid),
                    None,
                )
                .context_window
                .is_none()
            );
        }
        assert!(
            capabilities_from_openrouter_model(Vec::new(), Vec::new(), None, None, None)
                .context_window
                .is_none()
        );
        assert!(
            capabilities_from_openrouter_model(
                Vec::new(),
                Vec::new(),
                None,
                Some(serde_json::json!(u64::from(u32::MAX) + 1)),
                None,
            )
            .context_window
            .is_none()
        );
    }

    #[test]
    fn documented_openai_context_windows_cover_only_exact_gpt_5_6_ids() {
        for model_id in ["gpt-5.6", "gpt-5.6-sol", "gpt-5.6-terra", "gpt-5.6-luna"] {
            let mut snapshot = capabilities_for_model("openai", model_id, None, None);
            apply_documented_model_profile("openai", model_id, true, &mut snapshot);
            let context_window = snapshot
                .context_window
                .expect("documented OpenAI context window");
            assert_eq!(context_window.tokens.get(), 1_050_000);
            assert_eq!(
                context_window.source,
                CapabilitySourceSnapshot::OfficialDocs {
                    provider: "openai".to_string(),
                    url: OPENAI_MODELS_DOCS_URL.to_string(),
                    checked_at: OPENAI_MODELS_CHECKED_AT.to_string(),
                }
            );
        }

        let mut non_exact = capabilities_for_model("openai", "gpt-5.6-preview", None, None);
        apply_documented_model_profile("openai", "gpt-5.6-preview", true, &mut non_exact);
        assert!(non_exact.context_window.is_none());

        let mut other_provider = capabilities_for_model("anthropic", "gpt-5.6", None, None);
        apply_documented_model_profile("anthropic", "gpt-5.6", true, &mut other_provider);
        assert!(other_provider.context_window.is_none());

        let mut custom_endpoint = capabilities_for_model("openai", "gpt-5.6-sol", None, None);
        apply_documented_model_profile("openai", "gpt-5.6-sol", false, &mut custom_endpoint);
        assert!(custom_endpoint.context_window.is_none());
    }

    #[test]
    fn documented_openai_context_window_does_not_replace_existing_sources() {
        for source in [
            CapabilitySourceSnapshot::ApiDiscovered {
                provider: "openai".to_string(),
                endpoint: "rig model listing".to_string(),
            },
            CapabilitySourceSnapshot::Manual {
                source: "model settings".to_string(),
            },
        ] {
            let mut capabilities = conservative_model_capabilities("openai");
            capabilities.context_window = Some(ContextWindowCapabilitySnapshot {
                tokens: std::num::NonZeroU64::new(272_000).unwrap(),
                source,
            });
            let existing = capabilities.context_window.clone();

            apply_documented_model_profile("openai", "gpt-5.6", true, &mut capabilities);

            assert_eq!(capabilities.context_window, existing);
        }
    }

    #[test]
    fn ollama_context_window_requires_one_distinct_positive_value() {
        let details_only = capabilities_from_ollama_show(
            values(["completion"]),
            "qwen3".to_string(),
            values(["qwen3"]),
            Some(serde_json::json!(32_768)),
            std::collections::BTreeMap::new(),
            None,
        );
        let details_context = details_only.context_window.expect("details context window");
        assert_eq!(details_context.tokens.get(), 32_768);
        assert_eq!(details_context.source, source_api("ollama", "/api/show"));

        let duplicate_same = capabilities_from_ollama_show(
            values(["completion"]),
            "qwen3".to_string(),
            values(["qwen3"]),
            Some(serde_json::json!(32_768)),
            std::collections::BTreeMap::from([
                ("context_length".to_string(), serde_json::json!(32_768)),
                (
                    "qwen3.context_length".to_string(),
                    serde_json::json!(32_768),
                ),
                ("ignored_context_length".to_string(), serde_json::json!(1)),
                ("other.context_length".to_string(), serde_json::json!(0)),
                (
                    "invalid.context_length".to_string(),
                    serde_json::json!("32768"),
                ),
            ]),
            None,
        );
        assert_eq!(
            duplicate_same
                .context_window
                .expect("one distinct positive context window")
                .tokens
                .get(),
            32_768
        );

        let conflicting = capabilities_from_ollama_show(
            values(["completion"]),
            "qwen3".to_string(),
            values(["qwen3"]),
            Some(serde_json::json!(32_768)),
            std::collections::BTreeMap::from([(
                "qwen3.context_length".to_string(),
                serde_json::json!(65_536),
            )]),
            None,
        );
        assert!(conflicting.context_window.is_none());

        let no_positive = capabilities_from_ollama_show(
            values(["completion"]),
            "qwen3".to_string(),
            values(["qwen3"]),
            Some(serde_json::json!(0)),
            std::collections::BTreeMap::from([(
                "qwen3.context_length".to_string(),
                serde_json::json!(32_768.5),
            )]),
            None,
        );
        assert!(no_positive.context_window.is_none());
    }

    #[test]
    fn openai_profiles_use_model_specific_efforts() {
        let gpt5 = capabilities_for_model("openai", "gpt-5", None, None);
        assert_eq!(
            gpt5.reasoning.unwrap().efforts,
            values(["minimal", "low", "medium", "high"])
        );
        assert!(gpt5.image_input.is_some());
        assert!(gpt5.file_input.is_some());

        let gpt55 = capabilities_for_model("openai", "gpt-5.5", None, None);
        assert_eq!(
            gpt55.reasoning.unwrap().efforts,
            values(["none", "low", "medium", "high", "xhigh"])
        );
        assert!(gpt55.image_input.is_some());
        assert!(gpt55.file_input.is_some());

        let gpt56 = capabilities_for_model("openai", "gpt-5.6-sol-2026-07-29", None, None);
        assert_eq!(
            gpt56.reasoning.unwrap().efforts,
            values(["none", "low", "medium", "high", "xhigh", "max"])
        );
        assert!(gpt56.image_input.is_some());
        assert!(gpt56.file_input.is_some());

        let codex = capabilities_for_model("openai", "gpt-5.2-codex", None, None);
        assert_eq!(
            codex.reasoning.unwrap().efforts,
            values(["low", "medium", "high", "xhigh"])
        );
        assert!(codex.image_input.is_none());
        assert!(codex.file_input.is_none());
    }

    #[test]
    fn ollama_thinking_distinguishes_boolean_and_level_models() {
        let qwen = capabilities_from_ollama_show(
            values(["completion", "thinking"]),
            "qwen3".to_string(),
            values(["qwen3"]),
            None,
            std::collections::BTreeMap::new(),
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
            std::collections::BTreeMap::new(),
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
            values(["text", "image", "file"]),
            None,
            None,
            None,
        );
        let reasoning = snapshot.reasoning.expect("reasoning");
        assert!(snapshot.tool_calling.is_some());
        assert!(snapshot.structured_output);
        assert!(snapshot.image_input.is_some());
        assert!(snapshot.file_input.is_some());
        assert!(matches!(
            reasoning.source,
            CapabilitySourceSnapshot::OpenRouterNormalized
        ));
        assert!(matches!(
            reasoning.control,
            Some(ReasoningControlSnapshot::Composite { .. })
        ));

        let basic = capabilities_from_openrouter_model(
            values(["tools"]),
            values(["text"]),
            None,
            None,
            None,
        );
        assert!(basic.tool_calling.is_some());
        assert!(!basic.structured_output);
        assert!(basic.image_input.is_none());
        assert!(basic.file_input.is_none());
        assert!(basic.reasoning.is_none());
    }

    #[test]
    fn openrouter_output_modalities_are_nullable_and_known_values_are_authoritative() {
        for output in [None, Some(Vec::new()), Some(values(["future"]))] {
            let snapshot =
                capabilities_from_openrouter_model(Vec::new(), Vec::new(), output, None, None);
            assert!(snapshot.text_output);
            assert!(!snapshot.image_generation);
        }

        let text_and_image = capabilities_from_openrouter_model(
            Vec::new(),
            Vec::new(),
            Some(values([" Text ", "IMAGE", "future"])),
            None,
            None,
        );
        assert!(text_and_image.text_output);
        assert!(text_and_image.image_generation);

        let image_only = capabilities_from_openrouter_model(
            Vec::new(),
            Vec::new(),
            Some(values(["image", "future"])),
            None,
            None,
        );
        assert!(!image_only.text_output);
        assert!(image_only.image_generation);
    }

    #[test]
    fn gemini_uses_api_thinking_signal_and_doc_profiles() {
        let unsupported =
            capabilities_from_gemini_model("gemini-2.5-flash", Some(false), None, None);
        assert!(unsupported.reasoning.is_none());

        let budget = capabilities_from_gemini_model("gemini-2.5-flash", Some(true), None, None);
        assert!(matches!(
            budget.reasoning.unwrap().control,
            Some(ReasoningControlSnapshot::TokenBudget { .. })
        ));

        let levels = capabilities_from_gemini_model("gemini-3-pro", Some(true), None, None);
        assert!(matches!(
            levels.reasoning.unwrap().control,
            Some(ReasoningControlSnapshot::Levels { .. })
        ));
        assert!(levels.image_input.is_some());
        assert!(levels.file_input.is_some());
    }

    #[test]
    fn deepseek_and_mistral_profiles_are_provider_native() {
        let deepseek = capabilities_for_model("deepseek", "deepseek-v4-flash", None, None);
        assert_eq!(
            deepseek.reasoning.unwrap().efforts,
            values(["disabled", "high", "max"])
        );
        assert!(deepseek.tool_calling.is_some());
        assert!(deepseek.structured_output);
        assert!(deepseek.image_input.is_none());

        let deepseek_reasoner = capabilities_for_model("deepseek", "deepseek-reasoner", None, None);
        assert!(deepseek_reasoner.tool_calling.is_none());
        assert!(deepseek_reasoner.structured_output);

        let deepseek_chat = capabilities_for_model("deepseek", "deepseek-chat", None, None);
        assert!(deepseek_chat.tool_calling.is_some());
        assert!(deepseek_chat.structured_output);
        assert!(deepseek_chat.reasoning.is_none());

        let mistral = capabilities_for_model("mistral", "mistral-large-latest", None, None);
        assert_eq!(mistral.reasoning.unwrap().efforts, values(["none", "high"]));
        assert!(mistral.image_input.is_none());
        assert!(mistral.tool_calling.is_some());
        assert!(mistral.structured_output);

        let magistral = capabilities_for_model("mistral", "magistral-medium-latest", None, None);
        assert!(matches!(
            magistral.reasoning.unwrap().control,
            Some(ReasoningControlSnapshot::AlwaysOn { .. })
        ));
        assert!(magistral.tool_calling.is_some());
        assert!(magistral.structured_output);
    }

    #[test]
    fn anthropic_profiles_enable_current_multimodal_inputs() {
        let claude = capabilities_for_model("anthropic", "claude-sonnet-4-6", None, None);
        assert!(claude.image_input.is_some());
        assert!(claude.file_input.is_some());
        assert!(claude.tool_calling.is_some());
        assert!(claude.structured_output);
    }

    #[test]
    fn openai_legacy_multimodal_families_are_not_regressed() {
        let gpt4o = capabilities_for_model("openai", "gpt-4o", None, None);
        assert!(gpt4o.image_input.is_some());
        assert!(gpt4o.file_input.is_some());

        let o4 = capabilities_for_model("openai", "o4-mini", None, None);
        assert!(o4.image_input.is_some());
        assert!(o4.file_input.is_some());
    }
}

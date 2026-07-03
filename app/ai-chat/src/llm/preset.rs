use crate::{
    errors::AiChatResult,
    llm::{ExtSettingItem, ProviderModel, provider_by_name},
};

pub(crate) fn build_request_template(
    model: &ProviderModel,
    saved_template: Option<&serde_json::Value>,
) -> AiChatResult<serde_json::Value> {
    let provider = provider_by_name(&model.provider_name)?;
    let mut template = provider.default_template_for_model(model)?;

    if let Some(saved_template) = saved_template {
        let settings = match provider.ext_settings(model, saved_template) {
            Ok(settings) => settings,
            Err(_) => return Ok(template),
        };
        for setting in settings {
            if provider
                .apply_ext_setting(model, &mut template, &setting)
                .is_err()
            {
                continue;
            }
        }
    }

    Ok(template)
}

pub(crate) fn ext_settings(
    model: &ProviderModel,
    template: &serde_json::Value,
) -> AiChatResult<Vec<ExtSettingItem>> {
    provider_by_name(&model.provider_name)?.ext_settings(model, template)
}

pub(crate) fn apply_ext_setting(
    model: &ProviderModel,
    template: &mut serde_json::Value,
    setting: &ExtSettingItem,
) -> AiChatResult<()> {
    provider_by_name(&model.provider_name)?.apply_ext_setting(model, template, setting)
}

#[cfg(test)]
mod tests {
    use super::build_request_template;
    use crate::llm::{
        ModelCapabilities, OllamaModelCapabilities, OllamaThinkingCapability,
        OpenAIModelCapabilities, ProviderModel, ReasoningCapability, ReasoningEffort,
    };
    use serde_json::json;

    fn openai_reasoning_model(id: &str) -> ProviderModel {
        let mut capabilities = ModelCapabilities::text_streaming();
        capabilities.reasoning = Some(ReasoningCapability {
            default_effort: ReasoningEffort::Medium,
            efforts: vec![
                ReasoningEffort::Medium,
                ReasoningEffort::High,
                ReasoningEffort::XHigh,
            ],
            summaries: true,
        });
        ProviderModel::new(
            "OpenAI",
            id,
            capabilities.with_openai_extension(OpenAIModelCapabilities {
                responses_api: true,
                reasoning_summaries: true,
                hosted_web_search: true,
                stateful_response_continuation: true,
            }),
        )
    }

    fn ollama_model(
        id: &str,
        raw_capabilities: &[&str],
        family: &str,
        families: &[&str],
    ) -> ProviderModel {
        let raw_capabilities = raw_capabilities
            .iter()
            .map(|capability| (*capability).to_string())
            .collect::<Vec<_>>();
        let families = families
            .iter()
            .map(|family| (*family).to_string())
            .collect::<Vec<_>>();
        let thinking = raw_capabilities
            .iter()
            .any(|capability| capability == "thinking")
            .then_some({
                if matches!(family, "gptoss" | "gpt-oss") {
                    OllamaThinkingCapability::Levels
                } else {
                    OllamaThinkingCapability::Boolean
                }
            });
        let local_web_tools = raw_capabilities
            .iter()
            .any(|capability| capability == "tools");
        ProviderModel::new(
            "Ollama",
            id,
            ModelCapabilities::text_streaming().with_ollama_extension(OllamaModelCapabilities {
                raw_capabilities,
                family: family.to_string(),
                families,
                thinking,
                local_web_tools,
            }),
        )
    }

    #[test]
    fn build_request_template_replays_openai_reasoning_settings() -> anyhow::Result<()> {
        let model = openai_reasoning_model("gpt-5.2-pro");
        let template = build_request_template(
            &model,
            Some(&json!({
                "model": "gpt-5.2-pro",
                "reasoning": { "effort": "xhigh" }
            })),
        )?;

        assert_eq!(template["reasoning"]["effort"], "xhigh");
        Ok(())
    }

    #[test]
    fn build_request_template_replays_ollama_ext_settings() -> anyhow::Result<()> {
        let model = ollama_model(
            "gpt-oss",
            &["completion", "thinking", "tools"],
            "gptoss",
            &["gptoss"],
        );
        let template = build_request_template(
            &model,
            Some(&json!({
                "think": "high",
                "web_search": true
            })),
        )?;

        assert_eq!(template["think"], "high");
        assert_eq!(template["web_search"], true);
        Ok(())
    }

    #[test]
    fn build_request_template_defaults_ollama_boolean_thinking_to_false() -> anyhow::Result<()> {
        let model = ollama_model("qwen3", &["completion", "thinking"], "qwen3", &["qwen3"]);
        let template = build_request_template(
            &model,
            Some(&json!({
                "model": "qwen3",
                "stream": true
            })),
        )?;

        assert_eq!(template["think"], false);
        Ok(())
    }

    #[test]
    fn build_request_template_skips_invalid_saved_ext_settings() -> anyhow::Result<()> {
        let model = openai_reasoning_model("gpt-5.2-pro");
        let template = build_request_template(
            &model,
            Some(&json!({
                "model": "gpt-5.2-pro",
                "reasoning": { "effort": "invalid" }
            })),
        )?;

        assert_eq!(template["model"], "gpt-5.2-pro");
        assert_ne!(template["reasoning"]["effort"], "invalid");
        Ok(())
    }
}

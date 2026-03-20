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
        for setting in provider.ext_settings(model, saved_template)? {
            provider.apply_ext_setting(model, &mut template, &setting)?;
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
    use crate::llm::{ProviderModel, ProviderModelCapability};
    use serde_json::json;

    #[test]
    fn build_request_template_replays_openai_reasoning_settings() -> anyhow::Result<()> {
        let model = ProviderModel::new("OpenAI", "gpt-5.2-pro", ProviderModelCapability::Streaming);
        let template = build_request_template(&model, Some(&json!({
            "model": "gpt-5.2-pro",
            "reasoning": { "effort": "xhigh" }
        })))?;

        assert_eq!(template["reasoning"]["effort"], "xhigh");
        Ok(())
    }

    #[test]
    fn build_request_template_replays_ollama_ext_settings() -> anyhow::Result<()> {
        let model =
            ProviderModel::new("Ollama", "gpt-oss", ProviderModelCapability::Streaming).with_metadata(json!({
                "capabilities": ["completion", "thinking", "tools"],
                "family": "gptoss",
                "families": ["gptoss"]
            }));
        let template = build_request_template(&model, Some(&json!({
            "think": "high",
            "web_search": true
        })))?;

        assert_eq!(template["think"], "high");
        assert_eq!(template["web_search"], true);
        Ok(())
    }
}

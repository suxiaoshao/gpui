use super::{
    ExtSettingControl, OllamaChatMessage, OllamaProvider, OllamaStoredRequest, Provider,
    ProviderModel, ProviderModelCapability, THINK_HIGH, THINK_KEY, THINK_LOW, THINK_MEDIUM,
    WEB_SEARCH_KEY, WEB_SEARCH_TOOLTIP_KEY, should_bypass_proxy,
};
use crate::{database::Role, llm::ExtSettingItem};
use serde_json::json;

fn model_with_metadata(
    id: &str,
    capabilities: &[&str],
    family: &str,
    families: &[&str],
) -> ProviderModel {
    ProviderModel::new("Ollama", id, ProviderModelCapability::Streaming).with_metadata(json!({
        "capabilities": capabilities,
        "family": family,
        "families": families,
    }))
}

#[test]
fn ext_settings_use_boolean_for_standard_thinking_models() -> anyhow::Result<()> {
    let model = model_with_metadata("qwen3", &["completion", "thinking"], "qwen3", &["qwen3"]);
    let settings = OllamaProvider.ext_settings(&model, &json!({}))?;
    assert_eq!(settings.len(), 1);
    assert_eq!(settings[0].key, THINK_KEY);
    assert_eq!(settings[0].tooltip, None);
    assert_eq!(settings[0].control, ExtSettingControl::Boolean(false));
    Ok(())
}

#[test]
fn ext_settings_use_select_for_gptoss_models() -> anyhow::Result<()> {
    let model = model_with_metadata(
        "gpt-oss:20b",
        &["completion", "thinking", "tools"],
        "gptoss",
        &["gptoss"],
    );
    let settings = OllamaProvider.ext_settings(&model, &json!({ "web_search": true }))?;
    assert_eq!(
        settings[0].control,
        ExtSettingControl::Select {
            value: THINK_MEDIUM.to_string(),
            options: vec![
                crate::llm::ExtSettingOption {
                    value: THINK_LOW,
                    label_key: "reasoning-effort-low",
                },
                crate::llm::ExtSettingOption {
                    value: THINK_MEDIUM,
                    label_key: "reasoning-effort-medium",
                },
                crate::llm::ExtSettingOption {
                    value: THINK_HIGH,
                    label_key: "reasoning-effort-high",
                },
            ],
        }
    );
    assert_eq!(settings[1].key, WEB_SEARCH_KEY);
    assert_eq!(settings[1].tooltip, Some(WEB_SEARCH_TOOLTIP_KEY));
    assert_eq!(settings[1].control, ExtSettingControl::Boolean(true));
    Ok(())
}

#[test]
fn default_template_disables_standard_thinking_models() -> anyhow::Result<()> {
    let model = model_with_metadata("qwen3", &["completion", "thinking"], "qwen3", &["qwen3"]);
    let template = OllamaProvider.default_template_for_model(&model)?;
    assert_eq!(template["think"], false);
    Ok(())
}

#[test]
fn default_template_omits_think_for_non_thinking_models() -> anyhow::Result<()> {
    let model = model_with_metadata("llama3.2", &["completion"], "llama", &["llama"]);
    let template = OllamaProvider.default_template_for_model(&model)?;
    assert!(template.get("think").is_none());
    Ok(())
}

#[test]
fn default_template_keeps_gptoss_level_thinking_default() -> anyhow::Result<()> {
    let model = model_with_metadata(
        "gpt-oss:20b",
        &["completion", "thinking"],
        "gptoss",
        &["gptoss"],
    );
    let template = OllamaProvider.default_template_for_model(&model)?;
    assert!(template.get("think").is_none());
    Ok(())
}

#[test]
fn request_body_preserves_default_false_think() -> anyhow::Result<()> {
    let model = model_with_metadata("qwen3", &["completion", "thinking"], "qwen3", &["qwen3"]);
    let template = OllamaProvider.default_template_for_model(&model)?;
    let request = OllamaProvider.request_body(
        &template,
        vec![crate::llm::Message::new(Role::User, "hello".to_string())],
    )?;
    assert_eq!(request["think"], false);
    Ok(())
}

#[test]
fn apply_ext_setting_writes_boolean_think_explicitly() -> anyhow::Result<()> {
    let model = model_with_metadata("qwen3", &["completion", "thinking"], "qwen3", &["qwen3"]);
    let mut template = OllamaProvider.default_template_for_model(&model)?;
    OllamaProvider.apply_ext_setting(
        &model,
        &mut template,
        &ExtSettingItem {
            key: THINK_KEY,
            label_key: "field-thinking",
            tooltip: None,
            control: ExtSettingControl::Boolean(true),
        },
    )?;
    assert_eq!(template["think"], true);

    OllamaProvider.apply_ext_setting(
        &model,
        &mut template,
        &ExtSettingItem {
            key: THINK_KEY,
            label_key: "field-thinking",
            tooltip: None,
            control: ExtSettingControl::Boolean(false),
        },
    )?;
    assert_eq!(template["think"], false);
    Ok(())
}

#[test]
fn apply_ext_setting_writes_gptoss_think_levels() -> anyhow::Result<()> {
    let model = model_with_metadata(
        "gpt-oss:20b",
        &["completion", "thinking"],
        "gptoss",
        &["gptoss"],
    );
    let mut template = OllamaProvider.default_template_for_model(&model)?;
    OllamaProvider.apply_ext_setting(
        &model,
        &mut template,
        &ExtSettingItem {
            key: THINK_KEY,
            label_key: "field-thinking",
            tooltip: None,
            control: ExtSettingControl::Select {
                value: THINK_HIGH.to_string(),
                options: vec![],
            },
        },
    )?;
    assert_eq!(template["think"], THINK_HIGH);

    OllamaProvider.apply_ext_setting(
        &model,
        &mut template,
        &ExtSettingItem {
            key: THINK_KEY,
            label_key: "field-thinking",
            tooltip: None,
            control: ExtSettingControl::Select {
                value: THINK_MEDIUM.to_string(),
                options: vec![],
            },
        },
    )?;
    assert!(template.get("think").is_none());
    Ok(())
}

#[test]
fn bypass_proxy_for_loopback_ollama_hosts() {
    assert!(should_bypass_proxy("http://localhost:11434"));
    assert!(should_bypass_proxy("http://127.0.0.1:11434"));
    assert!(should_bypass_proxy("http://[::1]:11434"));
    assert!(should_bypass_proxy("http://localhost:11434/api/chat"));
}

#[test]
fn keep_proxy_for_non_loopback_ollama_hosts() {
    assert!(!should_bypass_proxy("http://192.168.1.10:11434"));
    assert!(!should_bypass_proxy("https://ollama.example.com"));
    assert!(!should_bypass_proxy("not-a-url"));
}

#[test]
fn request_body_maps_developer_role_to_system() -> anyhow::Result<()> {
    let request = OllamaProvider.request_body(
        &json!({
            "model": "qwen3",
            "stream": true,
            "web_search": true
        }),
        vec![crate::llm::Message::new(
            Role::Developer,
            "system prompt".to_string(),
        )],
    )?;
    let request = serde_json::from_value::<OllamaStoredRequest>(request)?;
    assert_eq!(request.messages.len(), 1);
    assert_eq!(request.messages[0].role, "system");
    assert_eq!(request.messages[0].content, "system prompt");
    assert!(request.web_search);
    Ok(())
}

#[test]
fn merge_content_does_not_duplicate_streamed_reasoning() {
    let content = OllamaProvider::merge_content(
        &OllamaChatMessage {
            content: "final answer".to_string(),
            thinking: "abc".to_string(),
            ..Default::default()
        },
        Vec::new(),
        Some("abc".to_string()),
    );
    assert_eq!(content.text, "final answer");
    assert_eq!(content.reasoning_summary.as_deref(), Some("abc"));
}

#[test]
fn show_response_treats_null_slices_as_empty_vectors() -> anyhow::Result<()> {
    let response = serde_json::from_value::<super::OllamaShowResponse>(json!({
        "details": {
            "family": "qwen3_5",
            "families": null
        },
        "capabilities": null
    }))?;
    assert_eq!(response.details.family, "qwen3_5");
    assert!(response.details.families.is_empty());
    assert!(response.capabilities.is_empty());
    Ok(())
}

#[test]
fn format_ollama_error_message_uses_json_error_field() {
    let message = super::format_ollama_error_message(
        "chat request",
        reqwest::StatusCode::INTERNAL_SERVER_ERROR,
        r#"{"error":"mlx runner failed"}"#,
    );
    assert_eq!(
        message,
        "Ollama chat request failed (500 Internal Server Error): mlx runner failed"
    );
}

#[test]
fn format_ollama_error_message_falls_back_to_plain_text_body() {
    let message = super::format_ollama_error_message(
        "show request",
        reqwest::StatusCode::BAD_GATEWAY,
        "upstream gateway failure",
    );
    assert_eq!(
        message,
        "Ollama show request failed (502 Bad Gateway): upstream gateway failure"
    );
}

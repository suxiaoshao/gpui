use super::{
    ExtSettingControl, HostedTool, OpenAIProvider, OpenAIRequestTemplate, Provider, ProviderModel,
    ProviderModelCapability, REASONING_EFFORT_KEY, REASONING_HIGH, REASONING_LOW, REASONING_MEDIUM,
    REASONING_NONE, REASONING_SUMMARY_AUTO, REASONING_XHIGH, ReasoningConfig,
    ResponsesCreateResponse, classify_model, normalize_base_url, parse_response_stream_event,
};
use crate::{
    database::Content,
    llm::{ExtSettingOption, FetchUpdate},
};
use serde_json::json;

#[test]
fn normalize_base_url_strips_terminal_api_paths() {
    assert_eq!(
        normalize_base_url("https://api.openai.com/v1/responses"),
        "https://api.openai.com/v1"
    );
    assert_eq!(
        normalize_base_url("https://api.openai.com/v1/models"),
        "https://api.openai.com/v1"
    );
}

#[test]
fn normalize_base_url_defaults_when_empty() {
    assert_eq!(normalize_base_url(""), "https://api.openai.com/v1");
    assert_eq!(normalize_base_url("   "), "https://api.openai.com/v1");
}

#[test]
fn openai_provider_requires_non_empty_api_key() {
    assert!(!OpenAIProvider.is_configured(&json!({})));
    assert!(!OpenAIProvider.is_configured(&json!({ "apiKey": "   " })));
    assert!(OpenAIProvider.is_configured(&json!({ "apiKey": "sk-test" })));
}

#[test]
fn default_template_uses_model_stream_capability() -> anyhow::Result<()> {
    let model = ProviderModel::new("OpenAI", "gpt-5", ProviderModelCapability::Streaming);
    let template = serde_json::from_value::<OpenAIRequestTemplate>(
        OpenAIProvider.default_template_for_model(&model)?,
    )?;
    assert_eq!(template.model, "gpt-5");
    assert!(template.stream);
    assert_eq!(
        template.tools,
        Some(vec![HostedTool {
            tool_type: "web_search".to_string(),
        }])
    );
    Ok(())
}

#[test]
fn unsupported_models_default_to_no_tools() -> anyhow::Result<()> {
    let model = ProviderModel::new("OpenAI", "gpt-4o", ProviderModelCapability::Streaming);
    let template = serde_json::from_value::<OpenAIRequestTemplate>(
        OpenAIProvider.default_template_for_model(&model)?,
    )?;
    assert_eq!(template.tools, None);
    Ok(())
}

#[test]
fn default_template_contains_only_runtime_fields() -> anyhow::Result<()> {
    let model = ProviderModel::new("OpenAI", "gpt-4o", ProviderModelCapability::Streaming);
    let template = OpenAIProvider.default_template_for_model(&model)?;
    let object = template.as_object().expect("template object");
    assert_eq!(object.len(), 2);
    assert!(object.contains_key("model"));
    assert!(object.contains_key("stream"));
    Ok(())
}

#[test]
fn classify_model_marks_o_series_as_non_streaming() {
    assert_eq!(
        classify_model("o3-mini"),
        Some(ProviderModelCapability::NonStreaming)
    );
    assert_eq!(
        classify_model("o4-mini"),
        Some(ProviderModelCapability::NonStreaming)
    );
}

#[test]
fn classify_model_marks_gpt_series_as_streaming() {
    assert_eq!(
        classify_model("gpt-5"),
        Some(ProviderModelCapability::Streaming)
    );
    assert_eq!(
        classify_model("chatgpt-4o-latest"),
        Some(ProviderModelCapability::Streaming)
    );
}

#[test]
fn classify_model_filters_dated_and_fp_models() {
    assert_eq!(classify_model("gpt-4o-2024-11-20"), None);
    assert_eq!(classify_model("gpt-3.5-0125"), None);
    assert_eq!(classify_model("gpt-4o-realtime-preview"), None);
    assert_eq!(classify_model("fp-model"), None);
}

#[test]
fn request_body_includes_web_search_tool_for_supported_models() -> anyhow::Result<()> {
    let request_body = OpenAIProvider.request_body(
        &json!({
            "model": "gpt-5",
            "stream": false,
            "tools": [{ "type": "web_search" }]
        }),
        vec![],
    )?;
    assert_eq!(request_body["tools"][0]["type"], "web_search");
    Ok(())
}

#[test]
fn request_body_omits_web_search_for_unsupported_models() -> anyhow::Result<()> {
    let request_body = OpenAIProvider.request_body(
        &json!({
            "model": "gpt-4o",
            "stream": false,
            "tools": [{ "type": "web_search" }]
        }),
        vec![],
    )?;
    assert!(request_body.get("tools").is_none());
    Ok(())
}

#[test]
fn ext_settings_omit_reasoning_for_unsupported_models() -> anyhow::Result<()> {
    let model = ProviderModel::new("OpenAI", "gpt-4o", ProviderModelCapability::Streaming);
    let settings = OpenAIProvider.ext_settings(&model, &json!({}))?;
    assert!(settings.is_empty());
    Ok(())
}

#[test]
fn ext_settings_use_medium_default_for_gpt_5_2_pro() -> anyhow::Result<()> {
    let model = ProviderModel::new("OpenAI", "gpt-5.2-pro", ProviderModelCapability::Streaming);
    let settings = OpenAIProvider.ext_settings(&model, &json!({}))?;
    assert_eq!(settings.len(), 1);
    assert_eq!(settings[0].key, REASONING_EFFORT_KEY);
    assert_eq!(
        settings[0].control,
        ExtSettingControl::Select {
            value: REASONING_MEDIUM.to_string(),
            options: vec![
                ExtSettingOption {
                    value: REASONING_MEDIUM,
                    label_key: "reasoning-effort-medium",
                },
                ExtSettingOption {
                    value: REASONING_HIGH,
                    label_key: "reasoning-effort-high",
                },
                ExtSettingOption {
                    value: REASONING_XHIGH,
                    label_key: "reasoning-effort-xhigh",
                },
            ]
        }
    );
    Ok(())
}

#[test]
fn ext_settings_use_none_default_for_gpt_5_1() -> anyhow::Result<()> {
    let model = ProviderModel::new("OpenAI", "gpt-5.1", ProviderModelCapability::Streaming);
    let settings = OpenAIProvider.ext_settings(&model, &json!({}))?;
    assert_eq!(
        settings[0].control,
        ExtSettingControl::Select {
            value: REASONING_NONE.to_string(),
            options: vec![
                ExtSettingOption {
                    value: REASONING_NONE,
                    label_key: "reasoning-effort-none",
                },
                ExtSettingOption {
                    value: REASONING_LOW,
                    label_key: "reasoning-effort-low",
                },
                ExtSettingOption {
                    value: REASONING_MEDIUM,
                    label_key: "reasoning-effort-medium",
                },
                ExtSettingOption {
                    value: REASONING_HIGH,
                    label_key: "reasoning-effort-high",
                },
            ]
        }
    );
    Ok(())
}

#[test]
fn ext_settings_use_high_only_for_gpt_5_pro() -> anyhow::Result<()> {
    let model = ProviderModel::new("OpenAI", "gpt-5-pro", ProviderModelCapability::Streaming);
    let settings = OpenAIProvider.ext_settings(&model, &json!({}))?;
    assert_eq!(
        settings[0].control,
        ExtSettingControl::Select {
            value: REASONING_HIGH.to_string(),
            options: vec![ExtSettingOption {
                value: REASONING_HIGH,
                label_key: "reasoning-effort-high",
            }]
        }
    );
    Ok(())
}

#[test]
fn apply_ext_setting_removes_default_reasoning() -> anyhow::Result<()> {
    let model = ProviderModel::new("OpenAI", "gpt-5.2-pro", ProviderModelCapability::Streaming);
    let mut template = OpenAIProvider.default_template_for_model(&model)?;
    OpenAIProvider.apply_ext_setting(
        &model,
        &mut template,
        &super::ExtSettingItem {
            key: REASONING_EFFORT_KEY,
            label_key: "field-reasoning-effort",
            tooltip: None,
            control: ExtSettingControl::Select {
                value: REASONING_MEDIUM.to_string(),
                options: vec![],
            },
        },
    )?;
    assert!(template.get("reasoning").is_none());
    Ok(())
}

#[test]
fn apply_ext_setting_writes_non_default_reasoning() -> anyhow::Result<()> {
    let model = ProviderModel::new("OpenAI", "gpt-5.2-pro", ProviderModelCapability::Streaming);
    let mut template = OpenAIProvider.default_template_for_model(&model)?;
    OpenAIProvider.apply_ext_setting(
        &model,
        &mut template,
        &super::ExtSettingItem {
            key: REASONING_EFFORT_KEY,
            label_key: "field-reasoning-effort",
            tooltip: None,
            control: ExtSettingControl::Select {
                value: REASONING_XHIGH.to_string(),
                options: vec![],
            },
        },
    )?;
    assert_eq!(
        serde_json::from_value::<OpenAIRequestTemplate>(template)?.reasoning,
        Some(ReasoningConfig {
            effort: REASONING_XHIGH.to_string(),
            summary: Some(REASONING_SUMMARY_AUTO.to_string()),
        })
    );
    Ok(())
}

#[test]
fn apply_ext_setting_rejects_unsupported_reasoning_values() {
    let model = ProviderModel::new("OpenAI", "gpt-5.2-pro", ProviderModelCapability::Streaming);
    let mut template = json!({});
    let err = OpenAIProvider
        .apply_ext_setting(
            &model,
            &mut template,
            &super::ExtSettingItem {
                key: REASONING_EFFORT_KEY,
                label_key: "field-reasoning-effort",
                tooltip: None,
                control: ExtSettingControl::Select {
                    value: REASONING_LOW.to_string(),
                    options: vec![],
                },
            },
        )
        .expect_err("unsupported effort");
    assert!(
        err.to_string()
            .contains("unsupported reasoning.effort 'low'")
    );
}

#[test]
fn request_body_includes_reasoning_when_supported() -> anyhow::Result<()> {
    let request_body = OpenAIProvider.request_body(
        &json!({
            "model": "gpt-5.4-pro",
            "stream": false,
            "reasoning": { "effort": "xhigh" }
        }),
        vec![],
    )?;
    assert_eq!(request_body["reasoning"]["effort"], "xhigh");
    assert_eq!(request_body["reasoning"]["summary"], REASONING_SUMMARY_AUTO);
    Ok(())
}

#[test]
fn request_body_includes_default_reasoning_when_supported() -> anyhow::Result<()> {
    let request_body = OpenAIProvider.request_body(
        &json!({
            "model": "gpt-5.4-pro",
            "stream": false
        }),
        vec![],
    )?;
    assert_eq!(request_body["reasoning"]["effort"], REASONING_MEDIUM);
    assert_eq!(request_body["reasoning"]["summary"], REASONING_SUMMARY_AUTO);
    Ok(())
}

#[test]
fn request_body_omits_reasoning_when_unsupported() -> anyhow::Result<()> {
    let request_body = OpenAIProvider.request_body(
        &json!({
            "model": "gpt-4o",
            "stream": false,
            "reasoning": { "effort": "medium" }
        }),
        vec![],
    )?;
    assert!(request_body.get("reasoning").is_none());
    Ok(())
}

#[test]
fn responses_create_response_extracts_web_search_citations() {
    let response = serde_json::from_value::<ResponsesCreateResponse>(json!({
        "output": [
            { "type": "web_search_call" },
            {
                "type": "message",
                "content": [
                    {
                        "type": "output_text",
                        "text": "hello",
                        "annotations": [
                            {
                                "type": "url_citation",
                                "title": "Example",
                                "url": "https://example.com",
                                "start_index": 0,
                                "end_index": 5
                            }
                        ]
                    }
                ]
            }
        ]
    }))
    .expect("response");
    assert_eq!(
        response.into_content(),
        Content {
            text: "hello".to_string(),
            reasoning_summary: None,
            citations: vec![crate::database::UrlCitation {
                title: Some("Example".to_string()),
                url: "https://example.com".to_string(),
                start_index: Some(0),
                end_index: Some(5),
            }]
        }
    );
}

#[test]
fn response_completed_event_yields_complete_update() -> anyhow::Result<()> {
    let update = parse_response_stream_event(
        r#"{"type":"response.completed","response":{"output":[{"type":"message","content":[{"type":"output_text","text":"done","annotations":[{"type":"url_citation","title":"Example","url":"https://example.com","start_index":0,"end_index":4}]}]}]}}"#,
    )?;
    assert_eq!(
        update,
        Some(FetchUpdate::Complete(Content {
            text: "done".to_string(),
            reasoning_summary: None,
            citations: vec![crate::database::UrlCitation {
                title: Some("Example".to_string()),
                url: "https://example.com".to_string(),
                start_index: Some(0),
                end_index: Some(4),
            }]
        }))
    );
    Ok(())
}

#[test]
fn reasoning_output_item_added_starts_thinking() -> anyhow::Result<()> {
    let update = parse_response_stream_event(
        r#"{"type":"response.output_item.added","item":{"type":"reasoning","summary":[]}}"#,
    )?;
    assert_eq!(update, Some(FetchUpdate::ThinkingStarted));
    Ok(())
}

#[test]
fn reasoning_summary_text_delta_yields_update() -> anyhow::Result<()> {
    let update = parse_response_stream_event(
        r#"{"type":"response.reasoning_summary_text.delta","delta":"thinking"}"#,
    )?;
    assert_eq!(
        update,
        Some(FetchUpdate::ReasoningSummaryDelta("thinking".to_string()))
    );
    Ok(())
}

#[test]
fn response_completed_event_extracts_reasoning_summary() -> anyhow::Result<()> {
    let update = parse_response_stream_event(
        r#"{"type":"response.completed","response":{"output":[{"type":"reasoning","summary":[{"type":"summary_text","text":"summarized"}]},{"type":"message","content":[{"type":"output_text","text":"done","annotations":[]}]}]}}"#,
    )?;
    assert_eq!(
        update,
        Some(FetchUpdate::Complete(Content {
            text: "done".to_string(),
            reasoning_summary: Some("summarized".to_string()),
            citations: vec![],
        }))
    );
    Ok(())
}

#[test]
fn response_completed_event_accumulates_reasoning_summaries() -> anyhow::Result<()> {
    let update = parse_response_stream_event(
        r#"{"type":"response.completed","response":{"output":[{"type":"reasoning","summary":[{"type":"summary_text","text":"first"}]},{"type":"reasoning","summary":[{"type":"summary_text","text":"second"}]},{"type":"message","content":[{"type":"output_text","text":"done","annotations":[]}]}]}}"#,
    )?;
    assert_eq!(
        update,
        Some(FetchUpdate::Complete(Content {
            text: "done".to_string(),
            reasoning_summary: Some("first\n\nsecond".to_string()),
            citations: vec![],
        }))
    );
    Ok(())
}

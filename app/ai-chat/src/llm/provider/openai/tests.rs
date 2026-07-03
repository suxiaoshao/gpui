use super::{
    ExtSettingControl, HostedTool, ModelCapabilities, OpenAIProvider, OpenAIRequestTemplate,
    OpenAIResponseStreamState, Provider, ProviderModel, REASONING_EFFORT_KEY, REASONING_HIGH,
    REASONING_LOW, REASONING_MEDIUM, REASONING_NONE, REASONING_SUMMARY_AUTO, REASONING_XHIGH,
    ReasoningConfig, ReasoningEffort, ResponsesCreateResponse, classify_model, normalize_base_url,
    parse_response_stream_event, parse_response_stream_events,
    parse_response_stream_events_with_state,
};
use crate::{
    database::{Content, Role},
    llm::{
        ExtSettingOption, LlmAttachmentRef, LlmContentPart, LlmInputItem, LlmOutputItem,
        ProviderRunEvent, ProviderRunState,
    },
};
use serde_json::json;

fn openai_model(id: &str) -> ProviderModel {
    ProviderModel::new(
        "OpenAI",
        id,
        classify_model(id).unwrap_or_else(ModelCapabilities::text_streaming),
    )
}

fn completed_content(update: Option<ProviderRunEvent>) -> Option<Content> {
    match update {
        Some(ProviderRunEvent::Completed { content, .. }) => Some(content),
        _ => None,
    }
}

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
    let model = openai_model("gpt-5");
    let template = serde_json::from_value::<OpenAIRequestTemplate>(
        OpenAIProvider.default_template_for_model(&model)?,
    )?;
    assert_eq!(template.model, "gpt-5");
    assert!(template.stream);
    assert_eq!(template.tools, Some(vec![HostedTool::new("web_search")]));
    Ok(())
}

#[test]
fn unsupported_models_default_to_no_tools() -> anyhow::Result<()> {
    let model = openai_model("gpt-4o");
    let template = serde_json::from_value::<OpenAIRequestTemplate>(
        OpenAIProvider.default_template_for_model(&model)?,
    )?;
    assert_eq!(template.tools, None);
    Ok(())
}

#[test]
fn default_template_contains_only_runtime_fields() -> anyhow::Result<()> {
    let model = openai_model("gpt-4o");
    let template = OpenAIProvider.default_template_for_model(&model)?;
    let object = template.as_object().expect("template object");
    assert_eq!(object.len(), 2);
    assert!(object.contains_key("model"));
    assert!(object.contains_key("stream"));
    Ok(())
}

#[test]
fn classify_model_marks_o_series_as_non_streaming() {
    assert!(!classify_model("o3-mini").unwrap().supports_streaming());
    assert!(!classify_model("o4-mini").unwrap().supports_streaming());
}

#[test]
fn classify_model_marks_gpt_series_as_streaming() {
    assert!(classify_model("gpt-5").unwrap().supports_streaming());
    assert!(
        classify_model("chatgpt-4o-latest")
            .unwrap()
            .supports_streaming()
    );
}

#[test]
fn classify_model_exposes_openai_typed_capabilities() {
    let capabilities = classify_model("gpt-5.2-pro").expect("classified model");
    assert!(capabilities.supports_streaming());
    assert!(capabilities.supports_hosted_web_search());
    assert!(capabilities.structured_output);
    assert!(capabilities.stateful_response_continuation);

    let reasoning = capabilities.reasoning.expect("reasoning capability");
    assert_eq!(reasoning.default_effort, ReasoningEffort::Medium);
    assert_eq!(
        reasoning.efforts,
        vec![
            ReasoningEffort::Medium,
            ReasoningEffort::High,
            ReasoningEffort::XHigh,
        ]
    );
    assert!(reasoning.summaries);
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
fn request_body_maps_text_input_items() -> anyhow::Result<()> {
    let request_body = OpenAIProvider.request_body(
        &json!({
            "model": "gpt-4o",
            "stream": false
        }),
        vec![
            LlmInputItem::from_role_text(Role::Developer, "system prompt"),
            LlmInputItem::from_role_text(Role::User, "hello"),
            LlmInputItem::from_role_text(Role::Assistant, "previous answer"),
        ],
    )?;

    assert_eq!(request_body["input"][0]["role"], "developer");
    assert_eq!(
        request_body["input"][0]["content"][0],
        json!({ "type": "input_text", "text": "system prompt" })
    );
    assert_eq!(request_body["input"][1]["role"], "user");
    assert_eq!(
        request_body["input"][1]["content"][0],
        json!({ "type": "input_text", "text": "hello" })
    );
    assert_eq!(request_body["input"][2]["role"], "assistant");
    assert_eq!(
        request_body["input"][2]["content"][0],
        json!({ "type": "input_text", "text": "previous answer" })
    );
    Ok(())
}

#[test]
fn request_body_maps_image_and_file_input_parts() -> anyhow::Result<()> {
    let request_body = OpenAIProvider.request_body(
        &json!({
            "model": "gpt-4o",
            "stream": false
        }),
        vec![LlmInputItem::User {
            content: vec![
                LlmContentPart::Text("describe".to_string()),
                LlmContentPart::ImageRef(LlmAttachmentRef {
                    id: "https://example.com/image.png".to_string(),
                    mime_type: Some("image/png".to_string()),
                    name: None,
                }),
                LlmContentPart::ImageRef(LlmAttachmentRef {
                    id: "file-img-1".to_string(),
                    mime_type: Some("image/png".to_string()),
                    name: None,
                }),
                LlmContentPart::FileRef(LlmAttachmentRef {
                    id: "file-doc-1".to_string(),
                    mime_type: Some("application/pdf".to_string()),
                    name: Some("doc.pdf".to_string()),
                }),
            ],
        }],
    )?;

    assert_eq!(
        request_body["input"][0]["content"],
        json!([
            { "type": "input_text", "text": "describe" },
            { "type": "input_image", "image_url": "https://example.com/image.png", "detail": "auto" },
            { "type": "input_image", "file_id": "file-img-1", "detail": "auto" },
            { "type": "input_file", "file_id": "file-doc-1" }
        ])
    );
    Ok(())
}

#[test]
fn request_body_rejects_unsupported_input_parts() {
    let err = OpenAIProvider
        .request_body(
            &json!({
                "model": "gpt-4o",
                "stream": false
            }),
            vec![LlmInputItem::User {
                content: vec![LlmContentPart::AudioRef(LlmAttachmentRef {
                    id: "audio-1".to_string(),
                    mime_type: Some("audio/wav".to_string()),
                    name: None,
                })],
            }],
        )
        .expect_err("audio input should be rejected");

    assert!(
        err.to_string()
            .contains("unsupported OpenAI Responses input item: audio content")
    );
}

#[test]
fn request_body_includes_previous_response_id_from_state() -> anyhow::Result<()> {
    let request_body = OpenAIProvider.request_body_with_state(
        &json!({
            "model": "gpt-4o",
            "stream": false
        }),
        vec![LlmInputItem::from_role_text(Role::User, "next")],
        Some(ProviderRunState::new(
            "OpenAI",
            Some("resp_1".to_string()),
            vec!["msg_1".to_string()],
            json!({ "model": "gpt-4o" }),
        )),
    )?;

    assert_eq!(request_body["previous_response_id"], "resp_1");
    assert_eq!(request_body["input"].as_array().unwrap().len(), 1);
    Ok(())
}

#[test]
fn request_body_preserves_responses_specific_template_fields() -> anyhow::Result<()> {
    let request_body = OpenAIProvider.request_body(
        &json!({
            "model": "gpt-4o",
            "stream": false,
            "include": ["web_search_call.results"],
            "text": { "format": { "type": "json_object" } },
            "tool_choice": "auto",
            "parallel_tool_calls": true,
            "tools": [
                {
                    "type": "function",
                    "name": "lookup",
                    "parameters": { "type": "object" },
                    "strict": true
                }
            ]
        }),
        vec![LlmInputItem::from_role_text(Role::User, "hello")],
    )?;

    assert_eq!(request_body["include"], json!(["web_search_call.results"]));
    assert_eq!(
        request_body["text"],
        json!({ "format": { "type": "json_object" } })
    );
    assert_eq!(request_body["tool_choice"], "auto");
    assert_eq!(request_body["parallel_tool_calls"], true);
    assert_eq!(request_body["tools"][0]["type"], "function");
    assert_eq!(request_body["tools"][0]["name"], "lookup");
    assert_eq!(request_body["tools"][0]["strict"], true);
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
    let model = openai_model("gpt-4o");
    let settings = OpenAIProvider.ext_settings(&model, &json!({}))?;
    assert!(settings.is_empty());
    Ok(())
}

#[test]
fn ext_settings_use_medium_default_for_gpt_5_2_pro() -> anyhow::Result<()> {
    let model = openai_model("gpt-5.2-pro");
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
    let model = openai_model("gpt-5.1");
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
    let model = openai_model("gpt-5-pro");
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
    let model = openai_model("gpt-5.2-pro");
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
    let model = openai_model("gpt-5.2-pro");
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
    let model = openai_model("gpt-5.2-pro");
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
fn responses_create_response_maps_output_items() {
    let response = serde_json::from_value::<ResponsesCreateResponse>(json!({
        "id": "resp_1",
        "output": [
            {
                "id": "msg_1",
                "type": "message",
                "role": "assistant",
                "content": [{ "type": "output_text", "text": "hello", "annotations": [] }]
            },
            {
                "id": "rs_1",
                "type": "reasoning",
                "summary": [{ "type": "summary_text", "text": "thinking" }]
            },
            {
                "id": "fc_1",
                "type": "function_call",
                "call_id": "call_1",
                "name": "lookup",
                "arguments": "{\"q\":\"rust\"}"
            },
            {
                "id": "mcp_1",
                "type": "mcp_call",
                "status": "completed"
            }
        ]
    }))
    .expect("response");

    let items = response.output_items();
    assert_eq!(items.len(), 4);
    assert!(matches!(
        &items[0],
        LlmOutputItem::Message {
            role: Role::Assistant,
            content
        } if content == &vec![LlmContentPart::Text("hello".to_string())]
    ));
    assert!(matches!(
        &items[1],
        LlmOutputItem::Reasoning { summary } if summary.as_deref() == Some("thinking")
    ));
    assert!(matches!(
        &items[2],
        LlmOutputItem::ToolCall(call)
            if call.call_id == "call_1"
                && call.name == "lookup"
                && call.arguments == json!({ "q": "rust" })
    ));
    assert!(matches!(
        &items[3],
        LlmOutputItem::HostedToolCall(call)
            if call.call_id == "mcp_1"
                && call.tool_type == "mcp_call"
                && call.status.as_deref() == Some("completed")
    ));
}

#[test]
fn response_completed_event_yields_complete_update() -> anyhow::Result<()> {
    let update = parse_response_stream_event(
        r#"{"type":"response.completed","response":{"output":[{"type":"message","content":[{"type":"output_text","text":"done","annotations":[{"type":"url_citation","title":"Example","url":"https://example.com","start_index":0,"end_index":4}]}]}]}}"#,
    )?;
    assert_eq!(
        completed_content(update),
        Some(Content {
            text: "done".to_string(),
            reasoning_summary: None,
            citations: vec![crate::database::UrlCitation {
                title: Some("Example".to_string()),
                url: "https://example.com".to_string(),
                start_index: Some(0),
                end_index: Some(4),
            }],
        })
    );
    Ok(())
}

#[test]
fn reasoning_output_item_added_starts_thinking() -> anyhow::Result<()> {
    let update = parse_response_stream_event(
        r#"{"type":"response.output_item.added","item":{"type":"reasoning","summary":[]}}"#,
    )?;
    assert_eq!(update, Some(ProviderRunEvent::ThinkingStarted));

    let updates = parse_response_stream_events(
        r#"{"type":"response.output_item.added","item":{"type":"reasoning","summary":[]}}"#,
    )?;
    assert_eq!(updates.len(), 2);
    assert!(matches!(updates[0], ProviderRunEvent::ThinkingStarted));
    assert!(matches!(
        updates[1],
        ProviderRunEvent::OutputItemAdded(LlmOutputItem::Reasoning { .. })
    ));
    Ok(())
}

#[test]
fn function_call_arguments_done_yields_tool_call() -> anyhow::Result<()> {
    let mut state = OpenAIResponseStreamState::default();
    parse_response_stream_events_with_state(
        r#"{"type":"response.output_item.added","item":{"id":"fc_1","type":"function_call","call_id":"call_1","name":"lookup","arguments":"","status":"in_progress"}}"#,
        &mut state,
    )?;
    let updates = parse_response_stream_events_with_state(
        r#"{"type":"response.function_call_arguments.done","item_id":"fc_1","name":"lookup","arguments":"{\"q\":\"rust\"}"}"#,
        &mut state,
    )?;
    assert!(matches!(
        updates.first(),
        Some(ProviderRunEvent::ToolCallRequested(call))
            if call.call_id == "call_1"
                && call.name == "lookup"
                && call.arguments == json!({ "q": "rust" })
    ));
    Ok(())
}

#[test]
fn function_call_arguments_done_waits_for_call_id_mapping() -> anyhow::Result<()> {
    let mut state = OpenAIResponseStreamState::default();
    let updates = parse_response_stream_events_with_state(
        r#"{"type":"response.function_call_arguments.done","item_id":"fc_1","name":"lookup","arguments":"{\"q\":\"rust\"}"}"#,
        &mut state,
    )?;
    assert!(updates.is_empty());

    let updates = parse_response_stream_events_with_state(
        r#"{"type":"response.output_item.done","item":{"id":"fc_1","type":"function_call","call_id":"call_1","name":"lookup","arguments":"{\"q\":\"rust\"}","status":"completed"}}"#,
        &mut state,
    )?;
    assert!(matches!(
        updates.as_slice(),
        [
            ProviderRunEvent::ToolCallRequested(requested),
            ProviderRunEvent::OutputItemDone(LlmOutputItem::ToolCall(done)),
        ] if requested.call_id == "call_1"
            && requested.name == "lookup"
            && requested.arguments == json!({ "q": "rust" })
            && done.call_id == "call_1"
    ));
    Ok(())
}

#[test]
fn output_item_done_yields_function_call_item() -> anyhow::Result<()> {
    let update = parse_response_stream_event(
        r#"{"type":"response.output_item.done","item":{"id":"fc_1","type":"function_call","call_id":"call_1","name":"lookup","arguments":"{\"q\":\"rust\"}","status":"completed"}}"#,
    )?;
    assert!(matches!(
        update,
        Some(ProviderRunEvent::OutputItemDone(LlmOutputItem::ToolCall(call)))
            if call.call_id == "call_1"
                && call.name == "lookup"
                && call.arguments == json!({ "q": "rust" })
    ));
    Ok(())
}

#[test]
fn reasoning_summary_text_delta_yields_update() -> anyhow::Result<()> {
    let update = parse_response_stream_event(
        r#"{"type":"response.reasoning_summary_text.delta","delta":"thinking"}"#,
    )?;
    assert_eq!(
        update,
        Some(ProviderRunEvent::ReasoningSummaryDelta(
            "thinking".to_string()
        ))
    );
    Ok(())
}

#[test]
fn top_level_error_event_returns_stream_error() {
    let error = parse_response_stream_event(
        r#"{"type":"error","message":"request failed","sequence_number":1}"#,
    )
    .expect_err("error event should fail");
    assert!(error.to_string().contains("request failed"));
}

#[test]
fn response_failed_event_returns_stream_error() {
    let error = parse_response_stream_event(
        r#"{"type":"response.failed","response":{"id":"resp_1","error":{"message":"model failed"}}}"#,
    )
    .expect_err("failed event should fail");
    assert!(error.to_string().contains("model failed"));
}

#[test]
fn response_incomplete_event_yields_failed_run_event() -> anyhow::Result<()> {
    let update = parse_response_stream_event(
        r#"{"type":"response.incomplete","response":{"id":"resp_1","incomplete_details":{"reason":"max_output_tokens"}}}"#,
    )?;
    assert_eq!(
        update,
        Some(ProviderRunEvent::Failed {
            message: "OpenAI response incomplete: max_output_tokens".to_string()
        })
    );
    Ok(())
}

#[test]
fn response_completed_event_extracts_reasoning_summary() -> anyhow::Result<()> {
    let update = parse_response_stream_event(
        r#"{"type":"response.completed","response":{"output":[{"type":"reasoning","summary":[{"type":"summary_text","text":"summarized"}]},{"type":"message","content":[{"type":"output_text","text":"done","annotations":[]}]}]}}"#,
    )?;
    assert_eq!(
        completed_content(update),
        Some(Content {
            text: "done".to_string(),
            reasoning_summary: Some("summarized".to_string()),
            citations: vec![],
        })
    );
    Ok(())
}

#[test]
fn response_completed_event_accumulates_reasoning_summaries() -> anyhow::Result<()> {
    let update = parse_response_stream_event(
        r#"{"type":"response.completed","response":{"output":[{"type":"reasoning","summary":[{"type":"summary_text","text":"first"}]},{"type":"reasoning","summary":[{"type":"summary_text","text":"second"}]},{"type":"message","content":[{"type":"output_text","text":"done","annotations":[]}]}]}}"#,
    )?;
    assert_eq!(
        completed_content(update),
        Some(Content {
            text: "done".to_string(),
            reasoning_summary: Some("first\n\nsecond".to_string()),
            citations: vec![],
        })
    );
    Ok(())
}

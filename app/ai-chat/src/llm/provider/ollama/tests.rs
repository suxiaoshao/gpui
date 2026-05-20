use super::{
    ExtSettingControl, ModelCapabilities, OllamaChatMessage, OllamaChatResponse,
    OllamaModelCapabilities, OllamaProvider, OllamaStoredRequest, OllamaThinkingCapability,
    Provider, ProviderModel, THINK_HIGH, THINK_KEY, THINK_LOW, THINK_MEDIUM, WEB_SEARCH_KEY,
    WEB_SEARCH_TOOLTIP_KEY, should_bypass_proxy,
};
use crate::{
    database::{Role, UrlCitation},
    llm::{
        ExtSettingItem, LlmAttachmentRef, LlmContentPart, LlmInputItem, LlmOutputItem,
        LlmToolResult, ProviderRunEvent,
    },
};
use serde_json::json;

fn model_with_metadata(
    id: &str,
    capabilities: &[&str],
    family: &str,
    families: &[&str],
) -> ProviderModel {
    let capabilities = capabilities
        .iter()
        .map(|capability| (*capability).to_string())
        .collect::<Vec<_>>();
    let families = families
        .iter()
        .map(|family| (*family).to_string())
        .collect::<Vec<_>>();
    let thinking = if capabilities
        .iter()
        .any(|capability| capability == "thinking")
    {
        let uses_levels = matches!(family, "gptoss" | "gpt-oss")
            || families
                .iter()
                .any(|family| matches!(family.as_str(), "gptoss" | "gpt-oss"));
        Some(if uses_levels {
            OllamaThinkingCapability::Levels
        } else {
            OllamaThinkingCapability::Boolean
        })
    } else {
        None
    };
    let local_web_tools = capabilities.iter().any(|capability| capability == "tools");
    ProviderModel::new(
        "Ollama",
        id,
        ModelCapabilities::text_streaming().with_ollama_extension(OllamaModelCapabilities {
            raw_capabilities: capabilities.clone(),
            family: family.to_string(),
            families: families.clone(),
            thinking,
            local_web_tools,
        }),
    )
    .with_metadata(json!({
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
fn show_response_maps_to_typed_capabilities() -> anyhow::Result<()> {
    let show = serde_json::from_value::<super::OllamaShowResponse>(json!({
        "details": {
            "family": "gptoss",
            "families": ["gptoss"]
        },
        "capabilities": ["completion", "thinking", "tools"]
    }))?;
    let capabilities = OllamaProvider::capabilities_from_show(&show);

    assert!(capabilities.supports_streaming());
    assert!(capabilities.tool_calling.is_some());
    assert!(capabilities.image_input.is_none());
    assert!(!capabilities.hosted_web_search);
    assert!(!capabilities.remote_mcp);
    assert!(!capabilities.stateful_response_continuation);
    let reasoning = capabilities.reasoning.expect("reasoning capability");
    assert!(reasoning.summaries);
    assert_eq!(
        reasoning.efforts,
        vec![
            crate::llm::ReasoningEffort::Low,
            crate::llm::ReasoningEffort::Medium,
            crate::llm::ReasoningEffort::High,
        ]
    );
    let crate::llm::ProviderCapabilityExtension::Ollama(extension) = capabilities.extension else {
        panic!("expected Ollama extension");
    };
    assert_eq!(extension.thinking, Some(OllamaThinkingCapability::Levels));
    assert!(extension.local_web_tools);
    assert_eq!(
        extension.raw_capabilities,
        vec![
            "completion".to_string(),
            "thinking".to_string(),
            "tools".to_string(),
        ]
    );
    Ok(())
}

#[test]
fn show_response_maps_vision_to_image_input_capability() -> anyhow::Result<()> {
    let show = serde_json::from_value::<super::OllamaShowResponse>(json!({
        "details": {
            "family": "llava",
            "families": ["llama", "clip"]
        },
        "capabilities": ["completion", "vision"]
    }))?;
    let capabilities = OllamaProvider::capabilities_from_show(&show);

    assert_eq!(
        capabilities.image_input,
        Some(super::super::ImageInputCapability { max_images: None })
    );
    assert!(capabilities.tool_calling.is_none());
    assert!(capabilities.reasoning.is_none());
    assert!(!capabilities.hosted_web_search);
    assert!(!capabilities.stateful_response_continuation);
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
        vec![LlmInputItem::from_role_text(Role::User, "hello")],
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
        vec![LlmInputItem::from_role_text(
            Role::Developer,
            "system prompt",
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
fn request_body_maps_system_item_to_system() -> anyhow::Result<()> {
    let request = OllamaProvider.request_body(
        &json!({
            "model": "qwen3",
            "stream": true
        }),
        vec![LlmInputItem::System {
            content: vec![crate::llm::LlmContentPart::text("system prompt")],
        }],
    )?;
    let request = serde_json::from_value::<OllamaStoredRequest>(request)?;
    assert_eq!(request.messages.len(), 1);
    assert_eq!(request.messages[0].role, "system");
    assert_eq!(request.messages[0].content, "system prompt");
    Ok(())
}

#[test]
fn request_body_maps_multipart_text_and_image_input() -> anyhow::Result<()> {
    let request = OllamaProvider.request_body(
        &json!({
            "model": "llava",
            "stream": true
        }),
        vec![LlmInputItem::User {
            content: vec![
                LlmContentPart::text("describe"),
                LlmContentPart::text("focus on the text"),
                LlmContentPart::ImageRef(LlmAttachmentRef {
                    id: "data:image/png;base64,aGVsbG8=".to_string(),
                    mime_type: Some("image/png".to_string()),
                    name: None,
                }),
                LlmContentPart::ImageRef(LlmAttachmentRef {
                    id: "iVBORw0KGgo=".to_string(),
                    mime_type: Some("image/png".to_string()),
                    name: None,
                }),
            ],
        }],
    )?;
    let request = serde_json::from_value::<OllamaStoredRequest>(request)?;

    assert_eq!(request.messages.len(), 1);
    assert_eq!(request.messages[0].role, "user");
    assert_eq!(request.messages[0].content, "describe\n\nfocus on the text");
    assert_eq!(
        request.messages[0].images,
        vec!["aGVsbG8=".to_string(), "iVBORw0KGgo=".to_string()]
    );
    Ok(())
}

#[test]
fn request_body_rejects_unsupported_image_refs() {
    for id in [
        "https://example.com/image.png",
        "file-img-1",
        "/tmp/image.png",
    ] {
        let err = OllamaProvider
            .request_body(
                &json!({
                    "model": "llava",
                    "stream": true
                }),
                vec![LlmInputItem::User {
                    content: vec![LlmContentPart::ImageRef(LlmAttachmentRef {
                        id: id.to_string(),
                        mime_type: Some("image/png".to_string()),
                        name: None,
                    })],
                }],
            )
            .expect_err("unsupported image ref should be rejected");

        assert!(
            err.to_string()
                .contains("unsupported Ollama input item: image input must be raw base64")
        );
    }
}

#[test]
fn request_body_rejects_non_text_input_parts() {
    let err = OllamaProvider
        .request_body(
            &json!({
                "model": "qwen3",
                "stream": true
            }),
            vec![LlmInputItem::User {
                content: vec![crate::llm::LlmContentPart::ImageRef(
                    crate::llm::LlmAttachmentRef {
                        id: "image-1".to_string(),
                        mime_type: Some("image/png".to_string()),
                        name: None,
                    },
                )],
            }],
        )
        .expect_err("non-text input should be rejected");

    assert!(
        err.to_string()
            .contains("unsupported Ollama input item: image input must be raw base64")
    );
}

#[test]
fn request_body_maps_text_tool_result() -> anyhow::Result<()> {
    let request = OllamaProvider.request_body(
        &json!({
            "model": "qwen3",
            "stream": true
        }),
        vec![LlmInputItem::ToolResult(LlmToolResult {
            call_id: "call-1".to_string(),
            content: vec![
                LlmContentPart::text("first"),
                LlmContentPart::text("second"),
            ],
        })],
    )?;
    let request = serde_json::from_value::<OllamaStoredRequest>(request)?;

    assert_eq!(request.messages.len(), 1);
    assert_eq!(request.messages[0].role, "tool");
    assert_eq!(request.messages[0].content, "first\n\nsecond");
    assert_eq!(request.messages[0].tool_call_id, "call-1");
    Ok(())
}

#[test]
fn request_body_rejects_item_reference_and_non_text_tool_result() {
    let item_reference_error = OllamaProvider
        .request_body(
            &json!({
                "model": "qwen3",
                "stream": true
            }),
            vec![LlmInputItem::ItemReference {
                item_id: "item-1".to_string(),
            }],
        )
        .expect_err("item references should be rejected");
    assert!(
        item_reference_error
            .to_string()
            .contains("unsupported Ollama input item: item reference")
    );

    let tool_result_error = OllamaProvider
        .request_body(
            &json!({
                "model": "qwen3",
                "stream": true
            }),
            vec![LlmInputItem::ToolResult(LlmToolResult {
                call_id: "call-1".to_string(),
                content: vec![LlmContentPart::ImageRef(LlmAttachmentRef {
                    id: "aGVsbG8=".to_string(),
                    mime_type: Some("image/png".to_string()),
                    name: None,
                })],
            })],
        )
        .expect_err("image tool results should be rejected");
    assert!(
        tool_result_error
            .to_string()
            .contains("unsupported Ollama input item: image tool result")
    );
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
fn merge_content_preserves_local_tool_citations() {
    let content = OllamaProvider::merge_content(
        &OllamaChatMessage {
            content: "final answer".to_string(),
            ..Default::default()
        },
        vec![UrlCitation {
            title: Some("Result".to_string()),
            url: "https://example.com".to_string(),
            start_index: None,
            end_index: None,
        }],
        None,
    );

    assert_eq!(content.text, "final answer");
    assert_eq!(content.citations.len(), 1);
    assert_eq!(content.citations[0].url, "https://example.com");
}

#[test]
fn stream_response_line_maps_ndjson_events_and_usage() -> anyhow::Result<()> {
    let mut round_message = OllamaChatMessage {
        role: "assistant".to_string(),
        ..Default::default()
    };
    let mut reasoning = String::new();
    let mut emitted_thinking_started = false;

    let (events, usage, done) = OllamaProvider::apply_stream_response_line(
        r#"{"message":{"thinking":"plan "},"done":false}"#,
        &mut round_message,
        &mut reasoning,
        &mut emitted_thinking_started,
    )?;
    assert_eq!(
        events,
        vec![
            ProviderRunEvent::ThinkingStarted,
            ProviderRunEvent::ReasoningSummaryDelta("plan ".to_string())
        ]
    );
    assert!(usage.is_none());
    assert!(!done);

    let (events, usage, done) = OllamaProvider::apply_stream_response_line(
        r#"{"message":{"content":"answer"},"done":false}"#,
        &mut round_message,
        &mut reasoning,
        &mut emitted_thinking_started,
    )?;
    assert_eq!(
        events,
        vec![ProviderRunEvent::TextDelta("answer".to_string())]
    );
    assert!(usage.is_none());
    assert!(!done);

    let (events, usage, done) = OllamaProvider::apply_stream_response_line(
        r#"{"message":{"tool_calls":[{"function":{"name":"web_search","arguments":{"query":"rust"}}}]},"done":false}"#,
        &mut round_message,
        &mut reasoning,
        &mut emitted_thinking_started,
    )?;
    assert!(matches!(
        events.as_slice(),
        [ProviderRunEvent::ToolCallRequested(call)]
            if call.call_id == "ollama-tool-0"
                && call.name == "web_search"
                && call.arguments == json!({"query": "rust"})
    ));
    assert!(usage.is_none());
    assert!(!done);

    let (events, usage, done) = OllamaProvider::apply_stream_response_line(
        r#"{"done":true,"done_reason":"stop","prompt_eval_count":3,"eval_count":5,"total_duration":900}"#,
        &mut round_message,
        &mut reasoning,
        &mut emitted_thinking_started,
    )?;
    assert!(events.is_empty());
    let usage = usage.expect("final usage");
    assert_eq!(usage.input_tokens, Some(3));
    assert_eq!(usage.output_tokens, Some(5));
    assert_eq!(usage.total_tokens, Some(8));
    assert_eq!(usage.metadata["done_reason"], "stop");
    assert_eq!(usage.metadata["total_duration"], 900);
    assert!(done);

    let output_events = OllamaProvider::output_item_done_events(&round_message);
    assert!(matches!(
        output_events.as_slice(),
        [
            ProviderRunEvent::OutputItemDone(LlmOutputItem::Reasoning { summary: Some(summary) }),
            ProviderRunEvent::OutputItemDone(LlmOutputItem::Message { role: Role::Assistant, content }),
            ProviderRunEvent::OutputItemDone(LlmOutputItem::ToolCall(call)),
        ] if summary == "plan "
            && content == &vec![LlmContentPart::text("answer")]
            && call.call_id == "ollama-tool-0"
    ));
    Ok(())
}

#[test]
fn non_stream_response_maps_output_items_and_usage() -> anyhow::Result<()> {
    let response = serde_json::from_value::<OllamaChatResponse>(json!({
        "message": {
            "role": "assistant",
            "content": "final answer",
            "thinking": "reasoning summary",
            "tool_calls": [
                {
                    "id": "call-1",
                    "function": {
                        "name": "web_fetch",
                        "arguments": { "url": "https://example.com" }
                    }
                }
            ]
        },
        "done": true,
        "done_reason": "stop",
        "prompt_eval_count": 4,
        "prompt_eval_duration": 40,
        "eval_count": 6,
        "eval_duration": 60
    }))?;

    let events = OllamaProvider::output_item_done_events(&response.message);
    assert!(matches!(
        events.as_slice(),
        [
            ProviderRunEvent::OutputItemDone(LlmOutputItem::Reasoning { summary: Some(summary) }),
            ProviderRunEvent::OutputItemDone(LlmOutputItem::Message { role: Role::Assistant, content }),
            ProviderRunEvent::OutputItemDone(LlmOutputItem::ToolCall(call)),
        ] if summary == "reasoning summary"
            && content == &vec![LlmContentPart::text("final answer")]
            && call.call_id == "call-1"
            && call.name == "web_fetch"
    ));

    let usage = response.usage().expect("usage");
    assert_eq!(usage.input_tokens, Some(4));
    assert_eq!(usage.output_tokens, Some(6));
    assert_eq!(usage.total_tokens, Some(10));
    assert_eq!(usage.metadata["prompt_eval_duration"], 40);
    assert_eq!(usage.metadata["eval_duration"], 60);
    Ok(())
}

#[test]
fn tool_result_output_item_uses_provider_neutral_type() {
    let message = OllamaChatMessage {
        role: "tool".to_string(),
        content: r#"{"results":[]}"#.to_string(),
        tool_call_id: "call-1".to_string(),
        ..Default::default()
    };
    let tool_result = OllamaProvider::provider_tool_result(&message);

    assert_eq!(tool_result.call_id, "call-1");
    assert_eq!(
        ProviderRunEvent::OutputItemDone(LlmOutputItem::ToolResult(tool_result)),
        ProviderRunEvent::OutputItemDone(LlmOutputItem::ToolResult(LlmToolResult {
            call_id: "call-1".to_string(),
            content: vec![LlmContentPart::text(r#"{"results":[]}"#)],
        }))
    );
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

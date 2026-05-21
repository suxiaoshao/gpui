use super::{
    TEMPORARY_SAVE_TITLE_MAX_CHARS, TemporaryDetailState, TemporaryMessage, build_history_messages,
    build_request_body, temporary_messages_to_add_conversation_messages, temporary_save_title,
};
use crate::database::{
    Content, ConversationTemplatePrompt, MessageOutputItem, MessageOutputItemStatus,
    MessageRunPersistence, MessageRunState, Mode, Role, Status,
};
use crate::features::conversation::detail::ConversationDetailViewExt;
use crate::llm::{LlmAttachmentRef, LlmContentPart, LlmInputItem, LlmOutputItem};
use crate::state::AiChatConfig;
use std::rc::Rc;
use time::OffsetDateTime;

const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";

fn input_texts(items: Vec<crate::llm::LlmInputItem>) -> Vec<(&'static str, String)> {
    items
        .into_iter()
        .map(|item| {
            let (role, text) = item.single_text().expect("text input item");
            (role, text.to_string())
        })
        .collect()
}

fn current_text(text: &str) -> Vec<LlmContentPart> {
    vec![LlmContentPart::text(text)]
}

fn make_message(role: Role, status: Status, content: Content) -> TemporaryMessage {
    let now = OffsetDateTime::now_utc();
    TemporaryMessage {
        id: 1,
        provider: "OpenAI".to_string(),
        role,
        content,
        input_content_parts: None,
        send_content: Rc::new(serde_json::json!({})),
        status,
        error: None,
        run_persistence: MessageRunPersistence::default(),
        created_time: now,
        updated_time: now,
        start_time: now,
        end_time: now,
    }
}

fn make_provider_message(
    provider: &str,
    role: Role,
    status: Status,
    content: Content,
    send_content: serde_json::Value,
    error: Option<String>,
) -> TemporaryMessage {
    let now = OffsetDateTime::now_utc();
    TemporaryMessage {
        id: 1,
        provider: provider.to_string(),
        role,
        content,
        input_content_parts: None,
        send_content: Rc::new(send_content),
        status,
        error,
        run_persistence: MessageRunPersistence::default(),
        created_time: now,
        updated_time: now,
        start_time: now,
        end_time: now,
    }
}

fn openai_config(base_url: &str) -> anyhow::Result<AiChatConfig> {
    let mut config = AiChatConfig::default();
    config.set_provider_settings(
        "OpenAI",
        toml::from_str(&format!("baseUrl = \"{base_url}\"\n"))?,
    );
    Ok(config)
}

fn openai_settings(base_url: &str) -> serde_json::Value {
    serde_json::json!({ "baseUrl": base_url })
}

fn openai_request_body(model: &str, stream: bool) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "stream": stream,
        "input": [{ "role": "user", "content": [{ "type": "input_text", "text": "old input" }] }]
    })
}

fn openai_run_persistence(run_id: &str, model: &str) -> MessageRunPersistence {
    openai_run_persistence_with_request(
        run_id,
        model,
        openai_settings(DEFAULT_OPENAI_BASE_URL),
        openai_request_body(model, false),
    )
}

fn openai_run_persistence_with_request(
    run_id: &str,
    model: &str,
    settings: serde_json::Value,
    request_body: serde_json::Value,
) -> MessageRunPersistence {
    MessageRunPersistence {
        run_state: Some(MessageRunState {
            provider: "OpenAI".to_string(),
            run_id: Some(run_id.to_string()),
            output_item_ids: vec!["msg_1".to_string()],
            continuation_metadata: serde_json::Value::Null,
            request_body,
            usage: None,
            model: Some(model.to_string()),
            settings: Some(settings),
        }),
        output_items: Vec::new(),
        attachments: Vec::new(),
    }
}

#[test]
fn runner_history_appends_current_user_message() {
    let history = build_history_messages(
        &[ConversationTemplatePrompt {
            prompt: "system".to_string(),
            role: Role::Developer,
        }],
        Mode::Contextual,
        &[
            make_message(Role::Assistant, Status::Normal, Content::new("a1")),
            make_message(Role::User, Status::Error, Content::new("bad")),
        ],
        Role::User,
        current_text("latest"),
    );
    let history = input_texts(history);

    assert_eq!(
        history,
        vec![
            ("developer", "system".to_string()),
            ("assistant", "a1".to_string()),
            ("user", "latest".to_string()),
        ]
    );
}

#[test]
fn runner_history_preserves_temporary_user_content_parts() {
    let mut image_message = make_message(Role::User, Status::Normal, Content::new("Screenshot"));
    let image_parts = vec![
        LlmContentPart::text("Screenshot"),
        LlmContentPart::ImageRef(LlmAttachmentRef {
            id: "data:image/png;base64,abc".to_string(),
            mime_type: Some("image/png".to_string()),
            name: Some("screenshot.png".to_string()),
        }),
    ];
    image_message.input_content_parts = Some(image_parts.clone());

    let history = build_history_messages(
        &[],
        Mode::Contextual,
        &[image_message],
        Role::User,
        current_text("latest"),
    );

    assert!(
        matches!(&history[0], LlmInputItem::User { content } if content == &image_parts),
        "previous user history should preserve the image input parts"
    );
    assert_eq!(
        history[1].single_text().expect("latest text input item"),
        ("user", "latest")
    );
}

#[test]
fn build_request_body_uses_override_template_model() -> anyhow::Result<()> {
    let mut template = serde_json::json!({
        "model": "gpt-4o",
        "stream": false
    });
    template["model"] = serde_json::json!("override-model");
    let request_body = build_request_body(
        "OpenAI",
        &template,
        &[],
        Mode::Contextual,
        &[],
        &AiChatConfig::default(),
        (Role::User, current_text("hello")),
    )?;
    assert_eq!(request_body["model"], serde_json::json!("override-model"));
    Ok(())
}

#[test]
fn contextual_openai_request_uses_latest_compatible_run_state() -> anyhow::Result<()> {
    let config = openai_config(DEFAULT_OPENAI_BASE_URL)?;
    let mut assistant = make_message(Role::Assistant, Status::Normal, Content::new("old answer"));
    assistant.run_persistence = openai_run_persistence("resp_1", "gpt-4o");
    let request_body = build_request_body(
        "OpenAI",
        &serde_json::json!({
            "model": "gpt-4o",
            "stream": false
        }),
        &[ConversationTemplatePrompt {
            prompt: "system".to_string(),
            role: Role::Developer,
        }],
        Mode::Contextual,
        &[
            make_message(Role::User, Status::Normal, Content::new("old user")),
            assistant,
            make_message(
                Role::User,
                Status::Normal,
                Content::new("after continuation"),
            ),
        ],
        &config,
        (Role::User, current_text("latest")),
    )?;

    assert_eq!(request_body["previous_response_id"], "resp_1");
    let input = request_body["input"].as_array().expect("input array");
    assert_eq!(input.len(), 3);
    assert_eq!(input[0]["content"][0]["text"], "system");
    assert_eq!(input[1]["content"][0]["text"], "after continuation");
    assert_eq!(input[2]["content"][0]["text"], "latest");
    Ok(())
}

#[test]
fn contextual_openai_request_falls_back_when_settings_differ() -> anyhow::Result<()> {
    let config = openai_config(DEFAULT_OPENAI_BASE_URL)?;
    let mut assistant = make_message(Role::Assistant, Status::Normal, Content::new("old answer"));
    assistant.run_persistence = openai_run_persistence_with_request(
        "resp_1",
        "gpt-4o",
        openai_settings("https://proxy.openai.local/v1"),
        openai_request_body("gpt-4o", false),
    );
    let request_body = build_request_body(
        "OpenAI",
        &serde_json::json!({
            "model": "gpt-4o",
            "stream": false
        }),
        &[ConversationTemplatePrompt {
            prompt: "system".to_string(),
            role: Role::Developer,
        }],
        Mode::Contextual,
        &[
            make_message(Role::User, Status::Normal, Content::new("old user")),
            assistant,
            make_message(
                Role::User,
                Status::Normal,
                Content::new("after continuation"),
            ),
        ],
        &config,
        (Role::User, current_text("latest")),
    )?;

    assert!(request_body.get("previous_response_id").is_none());
    let input = request_body["input"].as_array().expect("input array");
    assert_eq!(input.len(), 5);
    assert_eq!(input[1]["content"][0]["text"], "old user");
    assert_eq!(input[2]["content"][0]["text"], "old answer");
    Ok(())
}

#[test]
fn contextual_openai_request_falls_back_when_request_context_differs() -> anyhow::Result<()> {
    let config = openai_config(DEFAULT_OPENAI_BASE_URL)?;
    let mut assistant = make_message(Role::Assistant, Status::Normal, Content::new("old answer"));
    assistant.run_persistence = openai_run_persistence_with_request(
        "resp_1",
        "gpt-4o",
        openai_settings(DEFAULT_OPENAI_BASE_URL),
        openai_request_body("gpt-4o", true),
    );
    let request_body = build_request_body(
        "OpenAI",
        &serde_json::json!({
            "model": "gpt-4o",
            "stream": false
        }),
        &[ConversationTemplatePrompt {
            prompt: "system".to_string(),
            role: Role::Developer,
        }],
        Mode::Contextual,
        &[
            make_message(Role::User, Status::Normal, Content::new("old user")),
            assistant,
            make_message(
                Role::User,
                Status::Normal,
                Content::new("after continuation"),
            ),
        ],
        &config,
        (Role::User, current_text("latest")),
    )?;

    assert!(request_body.get("previous_response_id").is_none());
    let input = request_body["input"].as_array().expect("input array");
    assert_eq!(input.len(), 5);
    Ok(())
}

#[test]
fn contextual_openai_request_ignores_prior_previous_response_id_in_context_key()
-> anyhow::Result<()> {
    let config = openai_config(DEFAULT_OPENAI_BASE_URL)?;
    let mut assistant = make_message(Role::Assistant, Status::Normal, Content::new("old answer"));
    let mut request_body = openai_request_body("gpt-4o", false);
    request_body["previous_response_id"] = serde_json::json!("resp_0");
    assistant.run_persistence = openai_run_persistence_with_request(
        "resp_1",
        "gpt-4o",
        openai_settings(DEFAULT_OPENAI_BASE_URL),
        request_body,
    );
    let request_body = build_request_body(
        "OpenAI",
        &serde_json::json!({
            "model": "gpt-4o",
            "stream": false
        }),
        &[ConversationTemplatePrompt {
            prompt: "system".to_string(),
            role: Role::Developer,
        }],
        Mode::Contextual,
        &[
            make_message(Role::User, Status::Normal, Content::new("old user")),
            assistant,
            make_message(
                Role::User,
                Status::Normal,
                Content::new("after continuation"),
            ),
        ],
        &config,
        (Role::User, current_text("latest")),
    )?;

    assert_eq!(request_body["previous_response_id"], "resp_1");
    let input = request_body["input"].as_array().expect("input array");
    assert_eq!(input.len(), 3);
    Ok(())
}

#[test]
fn non_contextual_openai_request_ignores_run_state_continuation() -> anyhow::Result<()> {
    let mut assistant = make_message(Role::Assistant, Status::Normal, Content::new("old answer"));
    assistant.run_persistence = openai_run_persistence("resp_1", "gpt-4o");
    let request_body = build_request_body(
        "OpenAI",
        &serde_json::json!({
            "model": "gpt-4o",
            "stream": false
        }),
        &[],
        Mode::AssistantOnly,
        &[assistant],
        &AiChatConfig::default(),
        (Role::User, current_text("latest")),
    )?;

    assert!(request_body.get("previous_response_id").is_none());
    let input = request_body["input"].as_array().expect("input array");
    assert_eq!(input.len(), 2);
    assert_eq!(input[0]["content"][0]["text"], "old answer");
    assert_eq!(input[1]["content"][0]["text"], "latest");
    Ok(())
}

#[test]
fn clear_messages_resets_temporary_state() {
    let message = make_message(Role::Assistant, Status::Normal, Content::new("hello"));
    let mut source = TemporaryDetailState {
        messages: vec![message],
        autoincrement_id: 1,
    };

    source.clear_messages();

    assert!(source.messages.is_empty());
    assert_eq!(source.autoincrement_id, 0);
}

#[test]
fn empty_temporary_chat_hides_clear_and_save_actions() {
    let empty = TemporaryDetailState {
        messages: Vec::new(),
        autoincrement_id: 0,
    };
    assert!(!empty.supports_clear());
    assert!(!empty.supports_save());

    let message = make_message(Role::Assistant, Status::Normal, Content::new("hello"));
    let populated = TemporaryDetailState {
        messages: vec![message],
        autoincrement_id: 1,
    };
    assert!(populated.supports_clear());
    assert!(populated.supports_save());
}

#[test]
fn temporary_messages_convert_to_add_conversation_messages() {
    let send_content = serde_json::json!({"messages": [{"role": "user", "content": "hello"}]});
    let mut message = make_provider_message(
        "Ollama",
        Role::Assistant,
        Status::Error,
        Content::new("assistant reply"),
        send_content.clone(),
        Some("network failed".to_string()),
    );
    message
        .run_persistence
        .output_items
        .push(MessageOutputItem::new(
            0,
            LlmOutputItem::Reasoning { summary: None },
            MessageOutputItemStatus::Added,
        ));

    let messages = temporary_messages_to_add_conversation_messages(&[message]);

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].provider, "Ollama");
    assert_eq!(messages[0].role, Role::Assistant);
    assert_eq!(messages[0].content, Content::new("assistant reply"));
    assert_eq!(messages[0].send_content, send_content);
    assert_eq!(messages[0].status, Status::Error);
    assert_eq!(messages[0].error.as_deref(), Some("network failed"));
    assert_eq!(messages[0].run_persistence.output_items.len(), 1);
}

#[test]
fn temporary_save_title_uses_first_user_message() {
    let assistant = make_message(Role::Assistant, Status::Normal, Content::new("assistant"));
    let user = make_message(
        Role::User,
        Status::Normal,
        Content::new("QA temporary chat test. Reply OK."),
    );

    assert_eq!(
        temporary_save_title(&[assistant, user], "Temporary Conversation"),
        "QA temporary chat test. Reply OK."
    );
}

#[test]
fn temporary_save_title_falls_back_without_user_message() {
    let assistant = make_message(Role::Assistant, Status::Normal, Content::new("assistant"));

    assert_eq!(
        temporary_save_title(&[assistant], "Temporary Conversation"),
        "Temporary Conversation"
    );
}

#[test]
fn temporary_save_title_truncates_long_user_message() {
    let message = make_message(Role::User, Status::Normal, Content::new("a".repeat(80)));

    let title = temporary_save_title(&[message], "Temporary Conversation");

    assert_eq!(title.chars().count(), TEMPORARY_SAVE_TITLE_MAX_CHARS + 3);
    assert!(title.ends_with("..."));
}

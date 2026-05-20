use super::*;
use crate::database::{Content, ConversationTemplatePrompt, Status};
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

fn make_message(id: i32, role: Role, status: Status, content: Content) -> Message {
    let now = OffsetDateTime::now_utc();
    Message {
        id,
        conversation_id: 1,
        conversation_path: "/test".to_string(),
        provider: "OpenAI".to_string(),
        role,
        content,
        send_content: serde_json::json!({}),
        status,
        created_time: now,
        updated_time: now,
        start_time: now,
        end_time: now,
        error: None,
    }
}

fn openai_settings(base_url: &str) -> serde_json::Value {
    serde_json::json!({ "baseUrl": base_url })
}

fn message_run_state(
    settings: Option<serde_json::Value>,
    request_body: serde_json::Value,
) -> MessageRunState {
    MessageRunState {
        provider: "OpenAI".to_string(),
        run_id: Some("resp_1".to_string()),
        output_item_ids: vec!["msg_1".to_string()],
        continuation_metadata: serde_json::Value::Null,
        request_body,
        usage: None,
        model: Some("gpt-4o".to_string()),
        settings,
    }
}

#[test]
fn conversation_panel_message_list_uses_top_order_with_initial_reveal() {
    let state = ConversationPanelState {
        conversation_id: 1,
        conversation_icon: "chat".into(),
        conversation_title: "Default".into(),
        conversation_info: None,
    };

    assert_eq!(state.message_list_alignment(), ListAlignment::Top);
    assert!(state.measure_all_message_list());
    assert!(state.initially_reveal_latest_message());
}

#[test]
fn get_history_contextual_includes_all_normal_messages_and_user() {
    let contents = build_history_messages(
        vec![
            ConversationTemplatePrompt {
                prompt: "system".to_string(),
                role: Role::Developer,
            },
            ConversationTemplatePrompt {
                prompt: "primer".to_string(),
                role: Role::Assistant,
            },
        ],
        Mode::Contextual,
        &[
            make_message(1, Role::User, Status::Normal, Content::new("u1")),
            make_message(2, Role::Assistant, Status::Normal, Content::new("a1")),
            make_message(3, Role::User, Status::Error, Content::new("bad")),
        ],
        Role::User,
        "latest",
    );
    let contents = input_texts(contents);
    assert_eq!(
        contents,
        vec![
            ("developer", "system".to_string()),
            ("assistant", "primer".to_string()),
            ("user", "u1".to_string()),
            ("assistant", "a1".to_string()),
            ("user", "latest".to_string()),
        ]
    );
}

#[test]
fn get_history_single_only_prompts_and_user() {
    let contents = build_history_messages(
        vec![
            ConversationTemplatePrompt {
                prompt: "system".to_string(),
                role: Role::Developer,
            },
            ConversationTemplatePrompt {
                prompt: "primer".to_string(),
                role: Role::Assistant,
            },
        ],
        Mode::Single,
        &[make_message(
            1,
            Role::Assistant,
            Status::Normal,
            Content::new("a1"),
        )],
        Role::User,
        "latest",
    );
    let contents = input_texts(contents)
        .into_iter()
        .map(|(_, text)| text)
        .collect::<Vec<_>>();
    assert_eq!(
        contents,
        vec![
            "system".to_string(),
            "primer".to_string(),
            "latest".to_string()
        ]
    );
}

#[test]
fn get_history_assistant_only_filters_roles() {
    let contents = build_history_messages(
        vec![
            ConversationTemplatePrompt {
                prompt: "system".to_string(),
                role: Role::Developer,
            },
            ConversationTemplatePrompt {
                prompt: "primer".to_string(),
                role: Role::Assistant,
            },
        ],
        Mode::AssistantOnly,
        &[
            make_message(1, Role::User, Status::Normal, Content::new("u1")),
            make_message(2, Role::Assistant, Status::Normal, Content::new("a1")),
            make_message(3, Role::Assistant, Status::Error, Content::new("bad")),
        ],
        Role::User,
        "latest",
    );
    let contents = input_texts(contents);
    assert_eq!(
        contents,
        vec![
            ("developer", "system".to_string()),
            ("assistant", "primer".to_string()),
            ("assistant", "a1".to_string()),
            ("user", "latest".to_string()),
        ]
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
        vec![],
        Mode::Single,
        &[],
        Role::User,
        "hello",
    )?;
    assert_eq!(request_body["model"], serde_json::json!("override-model"));
    Ok(())
}

#[test]
fn build_run_request_with_openai_continuation_trims_prior_history() -> anyhow::Result<()> {
    let template = serde_json::json!({
        "model": "gpt-4o",
        "stream": false
    });
    let request = build_run_request_with_continuation(
        "OpenAI",
        &template,
        vec![ConversationTemplatePrompt {
            prompt: "system".to_string(),
            role: Role::Developer,
        }],
        Mode::Contextual,
        &[
            make_message(1, Role::User, Status::Normal, Content::new("old user")),
            make_message(
                2,
                Role::Assistant,
                Status::Normal,
                Content::new("old answer"),
            ),
            make_message(
                3,
                Role::User,
                Status::Normal,
                Content::new("after continuation"),
            ),
        ],
        (Role::User, "latest"),
        Some(ContinuationCandidate {
            after_index: 1,
            state: ProviderRunState::new(
                "OpenAI",
                Some("resp_1".to_string()),
                vec!["msg_1".to_string()],
                serde_json::json!({ "model": "gpt-4o" }),
            ),
        }),
    )?;

    assert_eq!(request.request_body["previous_response_id"], "resp_1");
    let input = request.request_body["input"]
        .as_array()
        .expect("input array");
    assert_eq!(input.len(), 3);
    assert_eq!(input[0]["role"], "developer");
    assert_eq!(input[0]["content"][0]["text"], "system");
    assert_eq!(input[1]["role"], "user");
    assert_eq!(input[1]["content"][0]["text"], "after continuation");
    assert_eq!(input[2]["role"], "user");
    assert_eq!(input[2]["content"][0]["text"], "latest");
    Ok(())
}

#[test]
fn compatible_openai_run_state_requires_settings_and_request_context() {
    let settings = openai_settings(DEFAULT_OPENAI_BASE_URL);
    let current_context = provider_run_request_context_key(&serde_json::json!({
        "model": "gpt-4o",
        "stream": false,
        "input": [{ "role": "user", "content": [{ "type": "input_text", "text": "current" }] }]
    }));
    let state = message_run_state(
        Some(settings.clone()),
        serde_json::json!({
            "model": "gpt-4o",
            "stream": false,
            "previous_response_id": "resp_0",
            "input": [{ "role": "user", "content": [{ "type": "input_text", "text": "old" }] }]
        }),
    );

    assert!(compatible_openai_run_state(
        "OpenAI",
        "gpt-4o",
        &settings,
        &current_context,
        &state,
    ));

    let missing_settings = message_run_state(None, state.request_body.clone());
    assert!(!compatible_openai_run_state(
        "OpenAI",
        "gpt-4o",
        &settings,
        &current_context,
        &missing_settings,
    ));

    let different_settings = message_run_state(
        Some(openai_settings("https://proxy.openai.local/v1")),
        state.request_body.clone(),
    );
    assert!(!compatible_openai_run_state(
        "OpenAI",
        "gpt-4o",
        &settings,
        &current_context,
        &different_settings,
    ));

    let different_context = message_run_state(
        Some(settings.clone()),
        serde_json::json!({
            "model": "gpt-4o",
            "stream": true,
            "input": [{ "role": "user", "content": [{ "type": "input_text", "text": "old" }] }]
        }),
    );
    assert!(!compatible_openai_run_state(
        "OpenAI",
        "gpt-4o",
        &settings,
        &current_context,
        &different_context,
    ));
}

#[test]
fn pause_message_snapshot_updates_loading_messages() {
    let now = OffsetDateTime::now_utc();
    let mut message = make_message(1, Role::Assistant, Status::Loading, Content::new("a1"));

    assert!(ConversationPanelView::pause_message_snapshot(
        &mut message,
        now
    ));
    assert_eq!(message.status, Status::Paused);
    assert_eq!(message.updated_time, now);
    assert_eq!(message.end_time, now);
}

#[test]
fn pause_message_snapshot_ignores_non_running_messages() {
    let now = OffsetDateTime::now_utc();
    let mut message = make_message(1, Role::Assistant, Status::Normal, Content::new("a1"));
    let original = message.clone();

    assert!(!ConversationPanelView::pause_message_snapshot(
        &mut message,
        now
    ));
    assert_eq!(message, original);
}

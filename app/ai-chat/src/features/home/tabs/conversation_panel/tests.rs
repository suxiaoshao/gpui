use super::*;
use crate::database::{Content, ConversationTemplatePrompt, Status};
use time::OffsetDateTime;

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
    )
    .into_iter()
    .map(|message| (message.role, message.content))
    .collect::<Vec<_>>();
    assert_eq!(
        contents,
        vec![
            (Role::Developer, "system".to_string()),
            (Role::Assistant, "primer".to_string()),
            (Role::User, "u1".to_string()),
            (Role::Assistant, "a1".to_string()),
            (Role::User, "latest".to_string()),
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
    )
    .into_iter()
    .map(|message| message.content)
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
    )
    .into_iter()
    .map(|message| (message.role, message.content))
    .collect::<Vec<_>>();
    assert_eq!(
        contents,
        vec![
            (Role::Developer, "system".to_string()),
            (Role::Assistant, "primer".to_string()),
            (Role::Assistant, "a1".to_string()),
            (Role::User, "latest".to_string()),
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

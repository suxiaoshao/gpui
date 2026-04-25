use super::{TemporaryDetailState, TemporaryMessage, build_history_messages, build_request_body};
use crate::database::{Content, ConversationTemplatePrompt, Mode, Role, Status};
use crate::features::conversation::detail::ConversationDetailViewExt;
use std::rc::Rc;
use time::OffsetDateTime;

fn make_message(role: Role, status: Status, content: Content) -> TemporaryMessage {
    let now = OffsetDateTime::now_utc();
    TemporaryMessage {
        id: 1,
        provider: "OpenAI".to_string(),
        role,
        content,
        send_content: Rc::new(serde_json::json!({})),
        status,
        error: None,
        created_time: now,
        updated_time: now,
        start_time: now,
        end_time: now,
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
        "latest",
    )
    .into_iter()
    .map(|message| (message.role, message.content))
    .collect::<Vec<_>>();

    assert_eq!(
        history,
        vec![
            (Role::Developer, "system".to_string()),
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
        &[],
        Mode::Contextual,
        &[],
        Role::User,
        "hello",
    )?;
    assert_eq!(request_body["model"], serde_json::json!("override-model"));
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

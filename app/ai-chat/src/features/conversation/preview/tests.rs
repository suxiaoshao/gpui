use crate::database::{Content, Message, Role, Status};
use time::{OffsetDateTime, UtcOffset};

use super::{format_time_with_offset, offset_label};

fn make_message(role: Role) -> Message {
    let now = OffsetDateTime::now_utc();
    Message {
        id: 1,
        conversation_id: 1,
        conversation_path: "/conversation/1".to_string(),
        provider: "OpenAI".to_string(),
        role,
        content: Content::new("hello"),
        send_content: serde_json::json!({}),
        status: Status::Normal,
        created_time: now,
        updated_time: now,
        start_time: now,
        end_time: now,
        error: None,
    }
}

#[test]
fn only_assistant_messages_can_resend() {
    let assistant = make_message(Role::Assistant);
    let user = make_message(Role::User);
    assert_eq!(assistant.role, Role::Assistant);
    assert_eq!(user.role, Role::User);
}

#[test]
fn utc_times_are_formatted_for_tooltips() {
    assert_eq!(
        format_time_with_offset(OffsetDateTime::UNIX_EPOCH, UtcOffset::UTC),
        "1970-01-01 00:00:00 UTC"
    );
}

#[test]
fn offset_labels_include_gmt_offsets() {
    assert_eq!(offset_label(UtcOffset::from_hms(8, 0, 0).unwrap()), "GMT+8");
    assert_eq!(
        offset_label(UtcOffset::from_hms(-5, -30, 0).unwrap()),
        "GMT-5:30"
    );
}

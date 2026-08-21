use fluent_bundle::FluentArgs;
use jaco_core::{
    AgentRun, AgentRunStatus, ContentPart, ConversationEntry, ConversationEntryPayload,
    ConversationStatusCode, TranscriptRole,
};
use time::{Month, OffsetDateTime, UtcOffset, Weekday};

use crate::foundation::I18n;

pub(crate) fn format_token_count(value: u64) -> String {
    let digits = value.to_string();
    let mut formatted = String::with_capacity(digits.len() + digits.len().saturating_sub(1) / 3);
    let first_group = digits.len() % 3;
    for (index, byte) in digits.bytes().enumerate() {
        if index > 0 && index % 3 == first_group {
            formatted.push(',');
        }
        formatted.push(char::from(byte));
    }
    formatted
}

pub(crate) fn format_compact_token_count(value: u64) -> String {
    const UNITS: [(u64, &str); 6] = [
        (1_000, "k"),
        (1_000_000, "M"),
        (1_000_000_000, "B"),
        (1_000_000_000_000, "T"),
        (1_000_000_000_000_000, "P"),
        (1_000_000_000_000_000_000, "E"),
    ];

    let Some(mut unit_index) = UNITS.iter().rposition(|(unit, _)| value >= *unit) else {
        return value.to_string();
    };
    let value = u128::from(value);

    loop {
        let (unit, suffix) = UNITS[unit_index];
        let unit = u128::from(unit);
        let whole = value / unit;

        if whole < 100 {
            let tenths = (value * 10 + unit / 2) / unit;
            if tenths.is_multiple_of(10) {
                return format!("{}{suffix}", tenths / 10);
            }
            return format!("{}.{}{suffix}", tenths / 10, tenths % 10);
        }

        let rounded = (value + unit / 2) / unit;
        if rounded >= 1_000 && unit_index + 1 < UNITS.len() {
            unit_index += 1;
            continue;
        }
        return format!("{rounded}{suffix}");
    }
}

pub(crate) fn content_parts_text(content: &[ContentPart]) -> String {
    content
        .iter()
        .filter_map(ContentPart::search_text)
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn item_markdown(item: &ConversationEntry) -> String {
    match &item.payload {
        ConversationEntryPayload::Message { content, .. } => content_parts_text(content),
        ConversationEntryPayload::SkillActivation(skill) => {
            let content = content_parts_text(&skill.content);
            if content.is_empty() {
                format!("Activated skill `{}`", skill.name)
            } else {
                format!("Activated skill `{}`\n\n{}", skill.name, content)
            }
        }
        ConversationEntryPayload::Reasoning { text, summary } => {
            summary.clone().unwrap_or_else(|| text.clone())
        }
        ConversationEntryPayload::ToolCall(_)
        | ConversationEntryPayload::ToolResult(_)
        | ConversationEntryPayload::ApprovalRequest(_)
        | ConversationEntryPayload::ApprovalDecision(_) => String::new(),
        ConversationEntryPayload::Status(status) => status
            .message
            .as_ref()
            .map(|message| format!("**{}**\n\n{}", status_code_label(status.code), message))
            .unwrap_or_else(|| status_code_label(status.code).to_string()),
        ConversationEntryPayload::Error(error) => format!("**Error:** {}", error.message),
    }
}

pub(crate) fn status_i18n_key(code: ConversationStatusCode) -> &'static str {
    match code {
        ConversationStatusCode::Canceled => "conversation-status-canceled",
        ConversationStatusCode::MaxStepsReached => "conversation-status-max-steps",
        ConversationStatusCode::CompletedWithoutOutput => {
            "conversation-status-completed-without-output"
        }
    }
}

pub(crate) fn status_code_label(code: ConversationStatusCode) -> &'static str {
    match code {
        ConversationStatusCode::Canceled => "canceled",
        ConversationStatusCode::MaxStepsReached => "max_steps_reached",
        ConversationStatusCode::CompletedWithoutOutput => "completed_without_output",
    }
}

pub(crate) fn is_user_message(item: &ConversationEntry) -> bool {
    matches!(
        item.payload,
        ConversationEntryPayload::Message {
            role: TranscriptRole::User,
            ..
        }
    )
}

pub(crate) fn is_terminal_run(run: &AgentRun) -> bool {
    matches!(
        run.status,
        AgentRunStatus::Completed | AgentRunStatus::Failed | AgentRunStatus::Canceled
    )
}

pub(crate) fn run_completed_time(run: &AgentRun) -> OffsetDateTime {
    run.completed_at
        .or(run.started_at)
        .unwrap_or(run.created_at)
}

pub(crate) fn run_started_time(run: &AgentRun) -> OffsetDateTime {
    run.started_at.unwrap_or(run.created_at)
}

pub(crate) fn run_duration_label(run: &AgentRun) -> String {
    let start = run_started_time(run);
    let end = run.completed_at.unwrap_or_else(OffsetDateTime::now_utc);
    duration_label((end - start).whole_seconds().max(0))
}

pub(crate) fn elapsed_since_label(start: OffsetDateTime) -> String {
    duration_label((OffsetDateTime::now_utc() - start).whole_seconds().max(0))
}

pub(crate) fn timestamp_label(time: OffsetDateTime, i18n: &I18n) -> String {
    let offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    timestamp_label_with_offset(time, OffsetDateTime::now_utc(), offset, i18n)
}

fn duration_label(seconds: i64) -> String {
    if seconds < 60 {
        return format!("{}s", seconds.max(1));
    }
    let minutes = seconds / 60;
    if minutes < 60 {
        return format!("{minutes}m {}s", seconds % 60);
    }
    format!("{}h {}m", minutes / 60, minutes % 60)
}

fn timestamp_label_with_offset(
    time: OffsetDateTime,
    now: OffsetDateTime,
    offset: UtcOffset,
    i18n: &I18n,
) -> String {
    let local = time.to_offset(offset);
    let now = now.to_offset(offset);
    let day_delta = local.date().to_julian_day() - now.date().to_julian_day();
    let clock = format!("{}:{:02}", local.hour(), local.minute());
    let mut args = FluentArgs::new();
    args.set("time", clock);

    if day_delta == 0 {
        return i18n.t_with_args("conversation-timestamp-time", &args);
    }

    if (-6..=-1).contains(&day_delta) {
        args.set("weekday", weekday_key(local.weekday()));
        return i18n.t_with_args("conversation-timestamp-weekday-time", &args);
    }

    args.set("month", month_key(local.month()));
    args.set("month_number", u8::from(local.month()).to_string());
    args.set("day", local.day().to_string());
    i18n.t_with_args("conversation-timestamp-month-day-time", &args)
}

fn weekday_key(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Monday => "monday",
        Weekday::Tuesday => "tuesday",
        Weekday::Wednesday => "wednesday",
        Weekday::Thursday => "thursday",
        Weekday::Friday => "friday",
        Weekday::Saturday => "saturday",
        Weekday::Sunday => "sunday",
    }
}

fn month_key(month: Month) -> &'static str {
    match month {
        Month::January => "january",
        Month::February => "february",
        Month::March => "march",
        Month::April => "april",
        Month::May => "may",
        Month::June => "june",
        Month::July => "july",
        Month::August => "august",
        Month::September => "september",
        Month::October => "october",
        Month::November => "november",
        Month::December => "december",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jaco_core::{
        ApprovalDecisionEntry, ApprovalDecisionPayload, ApprovalRequestEntry,
        ApprovalRequestPayload, ConversationEntryStatus, ToolArguments, ToolCallEntry,
        ToolResultEntry, ToolSource,
    };
    use time::{Date, Time};

    fn utc_datetime(year: i32, month: Month, day: u8, hour: u8, minute: u8) -> OffsetDateTime {
        Date::from_calendar_date(year, month, day)
            .unwrap()
            .with_time(Time::from_hms(hour, minute, 0).unwrap())
            .assume_utc()
    }

    #[test]
    fn token_count_formatters_preserve_exact_and_compact_contracts() {
        assert_eq!(format_token_count(0), "0");
        assert_eq!(format_token_count(999), "999");
        assert_eq!(format_token_count(1_000), "1,000");
        assert_eq!(format_token_count(24_716), "24,716");
        assert_eq!(format_token_count(u64::MAX), "18,446,744,073,709,551,615");

        assert_eq!(format_compact_token_count(0), "0");
        assert_eq!(format_compact_token_count(999), "999");
        assert_eq!(format_compact_token_count(1_000), "1k");
        assert_eq!(format_compact_token_count(1_050), "1.1k");
        assert_eq!(format_compact_token_count(25_132), "25.1k");
        assert_eq!(format_compact_token_count(128_000), "128k");
        assert_eq!(format_compact_token_count(999_499), "999k");
        assert_eq!(format_compact_token_count(999_500), "1M");
        assert_eq!(format_compact_token_count(1_250_000), "1.3M");
        assert_eq!(format_compact_token_count(u64::MAX), "18.4E");
    }

    fn entry(payload: ConversationEntryPayload) -> ConversationEntry {
        ConversationEntry {
            id: "entry-1".to_string(),
            conversation_id: "conversation-1".to_string(),
            seq: 1,
            kind: payload.kind(),
            status: ConversationEntryStatus::Completed,
            agent_run_id: Some("run-1".to_string()),
            provider_step_id: None,
            tool_invocation_id: Some("invocation-1".to_string()),
            provider_item_id: None,
            search_text: payload.search_text(),
            payload,
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    #[test]
    fn item_markdown_never_formats_tool_lifecycle_payloads() {
        let payloads = [
            ConversationEntryPayload::ToolCall(ToolCallEntry {
                tool_invocation_id: Some("inner-invocation".to_string()),
                call_id: "call-secret".to_string(),
                source: ToolSource::Local,
                name: "tool-secret".to_string(),
                runtime_tool_name: "runtime-secret".to_string(),
                arguments: ToolArguments {
                    value: serde_json::json!({"api_key": "synthetic-secret"}),
                },
            }),
            ConversationEntryPayload::ToolResult(ToolResultEntry {
                tool_invocation_id: Some("inner-invocation".to_string()),
                call_id: "call-secret".to_string(),
                content: vec![ContentPart::Text {
                    text: "tool output secret".to_string(),
                }],
                is_error: false,
                structured_output: None,
                raw_output: None,
            }),
            ConversationEntryPayload::ApprovalRequest(ApprovalRequestEntry {
                tool_invocation_id: "inner-invocation".to_string(),
                request: ApprovalRequestPayload {
                    reason: "approval reason secret".to_string(),
                    tool_source: ToolSource::Local,
                    tool_name: "tool-secret".to_string(),
                    arguments_preview: "approval arguments secret".to_string(),
                    access_requests: Vec::new(),
                },
            }),
            ConversationEntryPayload::ApprovalDecision(ApprovalDecisionEntry {
                tool_invocation_id: "inner-invocation".to_string(),
                decision: ApprovalDecisionPayload {
                    approved: true,
                    decided_by: "tester".to_string(),
                    reason: Some("decision reason secret".to_string()),
                },
            }),
        ];

        for payload in payloads {
            assert!(item_markdown(&entry(payload)).is_empty());
        }
    }

    #[test]
    fn timestamp_label_uses_time_for_same_day() {
        let i18n = I18n::for_locale_tag("en-US");
        let now = utc_datetime(2026, Month::June, 6, 13, 0);
        let time = utc_datetime(2026, Month::June, 6, 0, 33);

        assert_eq!(
            timestamp_label_with_offset(time, now, UtcOffset::UTC, &i18n),
            "0:33"
        );
    }

    #[test]
    fn timestamp_label_uses_weekday_for_recent_past_days() {
        let i18n = I18n::for_locale_tag("en-US");
        let now = utc_datetime(2026, Month::June, 6, 13, 0);
        let time = utc_datetime(2026, Month::June, 5, 0, 33);

        assert_eq!(
            timestamp_label_with_offset(time, now, UtcOffset::UTC, &i18n),
            "Friday 0:33"
        );
    }

    #[test]
    fn timestamp_label_uses_month_day_after_recent_window() {
        let i18n = I18n::for_locale_tag("zh-CN");
        let now = utc_datetime(2026, Month::June, 6, 13, 0);
        let time = utc_datetime(2026, Month::May, 30, 0, 33);

        assert_eq!(
            timestamp_label_with_offset(time, now, UtcOffset::UTC, &i18n),
            "5月30日 0:33"
        );
    }
}

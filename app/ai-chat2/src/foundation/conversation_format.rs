use ai_chat_core::{
    AgentRunStatus, ContentPart, ConversationItemPayload, ProviderRawPayload, TranscriptRole,
};
use ai_chat_db::{AgentRunRecord, ConversationItemRecord};
use fluent_bundle::FluentArgs;
use time::{Month, OffsetDateTime, UtcOffset, Weekday};

use crate::foundation::I18n;

pub(crate) fn content_parts_text(content: &[ContentPart]) -> String {
    content
        .iter()
        .filter_map(ContentPart::search_text)
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn item_markdown(item: &ConversationItemRecord) -> String {
    match &item.payload {
        ConversationItemPayload::Message { content, .. } => content_parts_text(content),
        ConversationItemPayload::SkillActivation(skill) => {
            let content = content_parts_text(&skill.content);
            if content.is_empty() {
                format!("Activated skill `{}`", skill.name)
            } else {
                format!("Activated skill `{}`\n\n{}", skill.name, content)
            }
        }
        ConversationItemPayload::Reasoning { text, summary } => {
            summary.clone().unwrap_or_else(|| text.clone())
        }
        ConversationItemPayload::ToolCall(call) => {
            let arguments = pretty_json(&call.arguments.value);
            format!(
                "**Tool call:** `{}`\n\n```json\n{}\n```",
                call.runtime_tool_name, arguments
            )
        }
        ConversationItemPayload::ToolResult(result) => {
            let mut parts = Vec::new();
            let content = content_parts_text(&result.content);
            if !content.is_empty() {
                parts.push(content);
            }
            if let Some(structured) = &result.structured_output {
                parts.push(format!("```json\n{}\n```", pretty_json(&structured.value)));
            }
            if let Some(raw) = &result.raw_output {
                parts.push(format_raw_payload(raw));
            }
            if parts.is_empty() {
                format!("Tool result `{}`", result.call_id)
            } else {
                parts.join("\n\n")
            }
        }
        ConversationItemPayload::ApprovalRequest(request) => format!(
            "**Approval requested:** `{}`\n\n{}",
            request.request.tool_name, request.request.arguments_preview
        ),
        ConversationItemPayload::ApprovalDecision(decision) => {
            if decision.decision.approved {
                "Approved".to_string()
            } else {
                "Denied".to_string()
            }
        }
        ConversationItemPayload::Status(status) => status
            .message
            .as_ref()
            .map(|message| format!("**{}**\n\n{}", status.label, message))
            .unwrap_or_else(|| status.label.clone()),
        ConversationItemPayload::Error(error) => format!("**Error:** {}", error.message),
    }
}

pub(crate) fn is_user_message(item: &ConversationItemRecord) -> bool {
    matches!(
        item.payload,
        ConversationItemPayload::Message {
            role: TranscriptRole::User,
            ..
        }
    )
}

pub(crate) fn is_assistant_message(item: &ConversationItemRecord) -> bool {
    matches!(
        item.payload,
        ConversationItemPayload::Message {
            role: TranscriptRole::Assistant,
            ..
        }
    )
}

pub(crate) fn is_terminal_run(run: &AgentRunRecord) -> bool {
    matches!(
        run.status,
        AgentRunStatus::Completed | AgentRunStatus::Failed | AgentRunStatus::Canceled
    )
}

pub(crate) fn run_completed_time(run: &AgentRunRecord) -> OffsetDateTime {
    run.completed_at
        .or(run.started_at)
        .unwrap_or(run.created_at)
}

pub(crate) fn run_started_time(run: &AgentRunRecord) -> OffsetDateTime {
    run.started_at.unwrap_or(run.created_at)
}

pub(crate) fn run_duration_label(run: &AgentRunRecord) -> String {
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

fn pretty_json(value: &serde_json::Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

fn format_raw_payload(raw: &ProviderRawPayload) -> String {
    format!(
        "```json\n{}\n```",
        serde_json::to_string_pretty(&raw.value).unwrap_or_else(|_| raw.value.to_string())
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::{Date, Time};

    fn utc_datetime(year: i32, month: Month, day: u8, hour: u8, minute: u8) -> OffsetDateTime {
        Date::from_calendar_date(year, month, day)
            .unwrap()
            .with_time(Time::from_hms(hour, minute, 0).unwrap())
            .assume_utc()
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

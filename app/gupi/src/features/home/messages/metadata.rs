use super::*;
use crate::foundation::i18n::t_with_args;
use fluent_bundle::FluentArgs;
use time::{OffsetDateTime, UtcOffset};

pub(super) fn process_title(messages: &[DisplayMessage], active: bool, cx: &App) -> String {
    if active {
        return t(cx, "conversation-working");
    }
    let key = if messages.iter().any(|m| m.value["stopReason"] == "error") {
        "conversation-processed-failed"
    } else if messages.iter().any(|m| m.value["stopReason"] == "aborted") {
        "conversation-processed-stopped"
    } else {
        "conversation-processed"
    };
    let mut args = FluentArgs::new();
    args.set(
        "duration",
        elapsed_ms(messages).map(duration_label).unwrap_or_default(),
    );
    t_with_args(cx, key, &args).trim().to_owned()
}
fn elapsed_ms(messages: &[DisplayMessage]) -> Option<i64> {
    let start = messages.iter().find(|m| m.role() == "assistant")?.value["timestamp"].as_i64()?;
    let end = messages.last()?.completed_at?;
    end.checked_sub(start).filter(|elapsed| *elapsed >= 0)
}
fn duration_label(ms: i64) -> String {
    let seconds = ms / 1000;
    if seconds < 1 {
        "<1s".into()
    } else if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{}h {}m", seconds / 3600, seconds % 3600 / 60)
    }
}

pub(super) fn timestamp(message: &DisplayMessage) -> Option<String> {
    let millis = if message.role() == "user" {
        message.value["timestamp"].as_i64()
    } else {
        message
            .completed_at
            .or_else(|| message.value["timestamp"].as_i64())
    }?;
    let offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    let date = OffsetDateTime::from_unix_timestamp_nanos(i128::from(millis) * 1_000_000)
        .ok()?
        .to_offset(offset);
    let now = OffsetDateTime::now_utc().to_offset(offset);
    let clock = format!("{:02}:{:02}", date.hour(), date.minute());
    Some(if date.date() == now.date() {
        clock
    } else {
        format!("{} {clock}", date.date())
    })
}

pub(super) fn usage_fields(message: &DisplayMessage, cx: &App) -> Vec<(String, String)> {
    let mut fields = vec![];
    for (field, label) in [
        ("model", "conversation-usage-model"),
        ("provider", "conversation-usage-provider"),
    ] {
        if let Some(value) = message.value[field].as_str().filter(|s| !s.is_empty()) {
            fields.push((t(cx, label), value.to_owned()));
        }
    }
    if let Some(usage) = message.value.get("usage").filter(|v| v.is_object()) {
        for (field, label) in [
            ("input", "conversation-usage-input"),
            ("output", "conversation-usage-output"),
            ("cacheRead", "conversation-usage-cache-read"),
            ("cacheWrite", "conversation-usage-cache-write"),
            ("totalTokens", "conversation-usage-total"),
        ] {
            if let Some(value) = usage[field].as_u64() {
                fields.push((t(cx, label), value.to_string()));
            }
        }
        if let Some(cost) = usage["cost"]["total"]
            .as_f64()
            .filter(|cost| cost.is_finite() && *cost >= 0.)
        {
            fields.push((t(cx, "conversation-usage-cost"), format!("${cost:.6}")));
        }
    }
    fields
}

#[cfg(test)]
mod tests {
    use super::{duration_label, elapsed_ms};
    use crate::state::history::DisplayMessage;
    #[test]
    fn duration_uses_completion_instead_of_last_request_start() {
        let message = |started, completed| DisplayMessage {
            id: "m".into(),
            entry: None,
            value: serde_json::json!({"role":"assistant", "timestamp":started}),
            completed_at: completed,
        };
        let messages = vec![message(1000, Some(2500)), message(3000, Some(6500))];
        assert_eq!(elapsed_ms(&messages), Some(5500));
        assert_eq!(elapsed_ms(&[message(3000, None)]), None);
        assert_eq!(elapsed_ms(&[message(3000, Some(2000))]), None);
        assert_eq!(duration_label(62000), "1m 2s");
    }
}

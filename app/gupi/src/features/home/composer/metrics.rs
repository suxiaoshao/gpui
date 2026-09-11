//! Context capacity and cumulative token traffic have separate meanings and surfaces.
use super::*;
use crate::state::conversation::Session;
use gpui_kit::component::{Icon, progress::ProgressCircle, tooltip::Tooltip};

fn details_text(title: String, fields: Vec<(String, String)>) -> String {
    std::iter::once(title)
        .chain(
            fields
                .into_iter()
                .map(|(label, value)| format!("{label}: {value}")),
        )
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn context(session: &Session, cx: &App) -> AnyElement {
    let usage = session
        .stats
        .data()
        .and_then(|stats| stats.context_usage.as_ref());
    let percent = usage
        .and_then(|usage| usage.percent)
        .filter(|v| v.is_finite());
    let unknown = t(cx, "conversation-unknown");
    let fields = vec![
        (
            t(cx, "composer-context-used"),
            usage
                .and_then(|u| u.tokens)
                .map(|n| n.to_string())
                .unwrap_or_else(|| unknown.clone()),
        ),
        (
            t(cx, "composer-context-limit"),
            usage
                .map(|u| compact(u.context_window))
                .unwrap_or_else(|| unknown.clone()),
        ),
        (
            t(cx, "composer-context-percent"),
            percent.map(|p| format!("{p:.1}%")).unwrap_or(unknown),
        ),
        (
            t(cx, "conversation-auto-compaction"),
            t(
                cx,
                if session
                    .state
                    .as_ref()
                    .is_some_and(|s| s.auto_compaction_enabled)
                {
                    "conversation-on"
                } else {
                    "conversation-off"
                },
            ),
        ),
    ];
    let label = t(cx, "conversation-context");
    let trigger = h_flex()
        .id("context-stats")
        .size(px(18.))
        .justify_center()
        .items_center()
        .role(Role::Image)
        .aria_label(label.clone())
        .focusable()
        .tab_stop(true)
        .child(if let Some(percent) = percent {
            ProgressCircle::new("context-ring")
                .value(percent.clamp(0., 100.) as f32)
                .size(px(18.))
                .into_any_element()
        } else {
            Icon::new(IconName::CircleDashed)
                .size_4()
                .text_color(cx.theme().muted_foreground)
                .into_any_element()
        });
    let text = details_text(label, fields);
    trigger
        .tooltip(move |window, cx| Tooltip::new(text.clone()).build(window, cx))
        .into_any_element()
}

pub(super) fn tokens(session: &Session, cx: &App) -> AnyElement {
    let Some(stats) = session.stats.data() else {
        return div().into_any_element();
    };
    let usage = &stats.tokens;
    let mut fields = vec![
        (t(cx, "composer-token-input"), usage.input.to_string()),
        (t(cx, "composer-token-output"), usage.output.to_string()),
        (
            t(cx, "composer-token-cache-read"),
            usage.cache_read.to_string(),
        ),
        (
            t(cx, "composer-token-cache-write"),
            usage.cache_write.to_string(),
        ),
    ];
    let latest = session
        .history()
        .path(session.history().leaf.as_deref())
        .into_iter()
        .rev()
        .find_map(|e| {
            e.data
                .get("message")
                .filter(|m| m["role"] == "assistant")
                .and_then(|m| m.get("usage"))
        });
    let hit = latest
        .and_then(cache_hit_percent)
        .map(|percent| format!("{percent:.1}%"))
        .unwrap_or_else(|| t(cx, "conversation-unknown"));
    fields.push((t(cx, "conversation-cache-hit"), hit));
    fields.push((t(cx, "conversation-cost"), format!("${:.4}", stats.cost)));
    let label = t(cx, "composer-token-usage");
    let trigger = h_flex()
        .id("token-usage")
        .gap_3()
        .h_5()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .role(Role::Image)
        .aria_label(label.clone())
        .focusable()
        .tab_stop(true)
        .children(
            [
                (IconName::ArrowUp, usage.input),
                (IconName::ArrowDown, usage.output),
                (IconName::Database, usage.cache_read),
            ]
            .into_iter()
            .map(|(icon, count)| {
                h_flex()
                    .gap_1()
                    .items_center()
                    .child(Icon::new(icon).size_3())
                    .child(compact(count))
            }),
        );
    let text = details_text(label, fields);
    trigger
        .tooltip(move |window, cx| Tooltip::new(text.clone()).build(window, cx))
        .into_any_element()
}

pub(super) fn status(
    session: &Session,
    owner: Entity<ConversationState>,
    key: String,
    cx: &App,
) -> AnyElement {
    if let Some(error) = session.stats.error() {
        return Button::new("stats-error")
            .ghost()
            .xsmall()
            .icon(IconName::CircleAlert)
            .tooltip(error.to_owned())
            .accessibility_label(t(cx, "composer-stats-retry"))
            .on_click(move |_, _, cx| owner.update(cx, |state, cx| state.refresh_stats(&key, cx)))
            .into_any_element();
    }
    if session.stats.running() {
        let label = t(cx, "composer-stats-loading");
        return div()
            .id("stats-loading")
            .role(Role::Status)
            .aria_label(label.clone())
            .tooltip(move |window, cx| Tooltip::new(label.clone()).build(window, cx))
            .child(gpui_kit::component::spinner::Spinner::new().small())
            .into_any_element();
    }
    div().into_any_element()
}

fn compact(value: u64) -> String {
    let (scale, suffix) = if value >= 1_000_000 {
        (1_000_000., "M")
    } else if value >= 1_000 {
        (1_000., "k")
    } else {
        return value.to_string();
    };
    let text = format!("{:.1}", value as f64 / scale);
    format!("{}{suffix}", text.strip_suffix(".0").unwrap_or(&text))
}
fn cache_hit_percent(usage: &serde_json::Value) -> Option<f64> {
    let read = usage["cacheRead"].as_u64()? as f64;
    let total = usage["input"].as_u64()? as f64 + read + usage["cacheWrite"].as_u64()? as f64;
    (total > 0.).then_some(100. * read / total)
}

#[cfg(test)]
mod tests {
    use super::{cache_hit_percent, compact};
    #[test]
    fn token_counts_and_cache_hit_are_not_speeds_or_context_percentages() {
        assert_eq!(compact(30_000), "30k");
        assert_eq!(compact(4400), "4.4k");
        assert_eq!(compact(1_000_000), "1M");
        assert_eq!(
            cache_hit_percent(&serde_json::json!({"input":6,"cacheRead":994,"cacheWrite":0})),
            Some(99.4)
        );
        assert_eq!(
            cache_hit_percent(&serde_json::json!({"cacheRead":994})),
            None
        );
        assert_eq!(
            cache_hit_percent(&serde_json::json!({"input":0,"cacheRead":0,"cacheWrite":0})),
            None
        );
    }
}

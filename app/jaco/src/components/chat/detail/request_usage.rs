use fluent_bundle::FluentArgs;
use gpui::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce, Role,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _, px,
};
use gpui_component::{
    ActiveTheme as _, Icon, Sizable as _, StyledExt as _, description_list::DescriptionList,
    h_flex, hover_card::HoverCard, label::Label, v_flex,
};
use jaco_core::{AgentMessageRequestUsage, ProviderUsageCoverage};

use crate::foundation::{I18n, assets::IconName};

#[derive(Debug, PartialEq, Eq)]
enum RequestUsageContent {
    Unavailable,
    Unreported,
    Fields(Vec<(String, String)>),
}

#[derive(IntoElement)]
pub(super) struct RequestUsageDisclosure {
    id: ElementId,
    reveal_group: SharedString,
    request_usage: AgentMessageRequestUsage,
}

impl RequestUsageDisclosure {
    pub(super) fn new(
        id: impl Into<ElementId>,
        reveal_group: impl Into<SharedString>,
        request_usage: AgentMessageRequestUsage,
    ) -> Self {
        Self {
            id: id.into(),
            reveal_group: reveal_group.into(),
            request_usage,
        }
    }
}

impl RenderOnce for RequestUsageDisclosure {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let i18n = cx.global::<I18n>();
        let tooltip = i18n.t("conversation-request-usage-tooltip");
        let title = i18n.t("conversation-request-usage-title");
        let step_id = self.request_usage.provider_step_id.clone();
        let compact_total = request_usage_compact_total(&self.request_usage, i18n);
        let content = request_usage_fields(&self.request_usage, i18n);
        let content_id = format!("conversation-request-usage-content-body-{step_id}");
        let content_debug_selector = format!("conversation-request-usage-content-{step_id}");
        let compact_total_debug_selector =
            format!("conversation-request-usage-compact-total-{step_id}");
        let content = v_flex()
            .id(content_id)
            .debug_selector(move || content_debug_selector.clone())
            .min_w(px(280.))
            .gap_2()
            .child(Label::new(title).font_semibold())
            .child(match content {
                RequestUsageContent::Unavailable => {
                    Label::new(i18n.t("conversation-request-usage-unavailable"))
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .into_any_element()
                }
                RequestUsageContent::Unreported => {
                    Label::new(i18n.t("conversation-request-usage-unreported"))
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .into_any_element()
                }
                RequestUsageContent::Fields(fields) => DescriptionList::horizontal()
                    .columns(1)
                    .bordered(false)
                    .small()
                    .children(fields.into_iter().map(|(label, value)| {
                        gpui_component::description_list::DescriptionItem::new(label).value(value)
                    }))
                    .into_any_element(),
            });
        let trigger_id = self.id;
        let hover_card_id: ElementId =
            format!("conversation-request-usage-hover-card-{step_id}").into();
        let trigger_debug_selector = format!("conversation-request-usage-trigger-{step_id}");
        let reveal_group = self.reveal_group;
        let trigger = div()
            .id(trigger_id)
            .debug_selector(move || trigger_debug_selector.clone())
            .role(Role::Image)
            .aria_label(tooltip)
            .h_5()
            .min_w_5()
            .flex()
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .rounded(cx.theme().radius)
            .text_color(cx.theme().secondary_foreground)
            .hover(|this| this.bg(cx.theme().tokens.secondary_hover.background))
            .child(Icon::new(IconName::ChartNoAxesColumn).xsmall());

        h_flex()
            .flex_shrink_0()
            .gap_1()
            .child(
                HoverCard::new(hover_card_id)
                    .anchor(gpui::Anchor::BottomLeft)
                    .trigger(trigger)
                    .child(content),
            )
            .when_some(compact_total, |this, total| {
                this.child(
                    div()
                        .debug_selector(move || compact_total_debug_selector.clone())
                        .opacity(0.)
                        .group_hover(reveal_group, |this| this.opacity(1.))
                        .child(
                            Label::new(total)
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .whitespace_nowrap(),
                        ),
                )
            })
    }
}

fn request_usage_fields(
    request_usage: &AgentMessageRequestUsage,
    i18n: &I18n,
) -> RequestUsageContent {
    let Some(usage) = request_usage.usage.as_ref() else {
        return RequestUsageContent::Unavailable;
    };

    let coverage = usage.coverage();
    if coverage == ProviderUsageCoverage::Unreported {
        return RequestUsageContent::Unreported;
    }

    let mut fields = vec![
        (
            i18n.t("conversation-request-usage-input-tokens"),
            format_token_count(usage.input_tokens),
        ),
        (
            i18n.t("conversation-request-usage-output-tokens"),
            format_token_count(usage.output_tokens),
        ),
    ];
    if usage.cached_input_tokens > 0 {
        fields.push((
            i18n.t("conversation-request-usage-cache-read"),
            format_token_count(usage.cached_input_tokens),
        ));
    }
    if let Some(rate) = usage.cache_hit_rate(&request_usage.provider_kind) {
        fields.push((
            i18n.t("conversation-request-usage-cache-hit-rate"),
            format_rate(rate),
        ));
    }
    if usage.cache_write_input_tokens > 0 {
        fields.push((
            i18n.t("conversation-request-usage-cache-write"),
            format_token_count(usage.cache_write_input_tokens),
        ));
    }
    if usage.reasoning_tokens > 0 {
        fields.push((
            i18n.t("conversation-request-usage-reasoning-tokens"),
            format_token_count(usage.reasoning_tokens),
        ));
    }
    fields.push((
        i18n.t("conversation-request-usage-total-tokens"),
        if coverage == ProviderUsageCoverage::Partial {
            i18n.t("conversation-request-usage-unknown-value")
        } else {
            format_token_count(usage.total_tokens)
        },
    ));

    RequestUsageContent::Fields(fields)
}

fn request_usage_compact_total(
    request_usage: &AgentMessageRequestUsage,
    i18n: &I18n,
) -> Option<String> {
    let tokens = request_usage
        .usage
        .as_ref()
        .filter(|usage| usage.coverage() == ProviderUsageCoverage::Reported)
        .map(|usage| format_compact_token_count(usage.total_tokens))?;
    let mut args = FluentArgs::new();
    args.set("tokens", tokens);
    Some(i18n.t_with_args("conversation-request-usage-compact-total", &args))
}

fn format_token_count(value: u64) -> String {
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

fn format_compact_token_count(value: u64) -> String {
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

fn format_rate(value: f64) -> String {
    debug_assert!(value.is_finite());
    format!("{:.1}%", value * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Context, Modifiers, Render, TestAppContext, point, px};
    use jaco_core::{ProviderRawPayload, ProviderUsageSnapshot};
    use std::time::Duration;
    use time::OffsetDateTime;

    fn request_usage(usage: Option<ProviderUsageSnapshot>) -> AgentMessageRequestUsage {
        AgentMessageRequestUsage {
            conversation_entry_id: "entry-final".to_string(),
            agent_run_id: "run-1".to_string(),
            provider_step_id: "step-final".to_string(),
            provider_id: "provider-1".to_string(),
            model_id: "model-1".to_string(),
            provider_kind: "openai".to_string(),
            completed_at: OffsetDateTime::UNIX_EPOCH,
            usage,
        }
    }

    fn usage(
        input_tokens: u64,
        output_tokens: u64,
        cached_input_tokens: u64,
        cache_write_input_tokens: u64,
        reasoning_tokens: u64,
        total_tokens: u64,
    ) -> ProviderUsageSnapshot {
        ProviderUsageSnapshot {
            input_tokens,
            output_tokens,
            cached_input_tokens,
            cache_write_input_tokens,
            reasoning_tokens,
            total_tokens,
            metadata: Some(ProviderRawPayload {
                provider_kind: "must-not-render".to_string(),
                value: serde_json::json!({"secret": "must-not-render"}),
            }),
        }
    }

    #[test]
    fn token_count_and_cache_rate_formatting_preserve_exact_values() {
        assert_eq!(format_token_count(0), "0");
        assert_eq!(format_token_count(999), "999");
        assert_eq!(format_token_count(1_000), "1,000");
        assert_eq!(format_token_count(24_716), "24,716");
        assert_eq!(format_token_count(u64::MAX), "18,446,744,073,709,551,615");
        assert_eq!(format_rate(0.989), "98.9%");
    }

    #[test]
    fn compact_token_count_keeps_only_the_action_row_summary_short() {
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

    #[test]
    fn request_usage_fields_render_reported_partial_unreported_and_unavailable() {
        let i18n = I18n::english_for_test();

        assert_eq!(
            request_usage_fields(&request_usage(None), &i18n),
            RequestUsageContent::Unavailable
        );
        assert_eq!(
            request_usage_fields(&request_usage(Some(usage(0, 0, 0, 0, 0, 0))), &i18n),
            RequestUsageContent::Unreported
        );

        let RequestUsageContent::Fields(partial) = request_usage_fields(
            &request_usage(Some(usage(24_716, 416, 24_448, 17, 31, 0))),
            &i18n,
        ) else {
            panic!("partial usage must render fields");
        };
        assert_eq!(partial[0].1, "24,716");
        assert_eq!(partial[1].1, "416");
        assert_eq!(partial.last().unwrap().1, "Unknown");
        assert!(partial.iter().any(|(_, value)| value == "98.9%"));

        let RequestUsageContent::Fields(reported) = request_usage_fields(
            &request_usage(Some(usage(24_716, 416, 24_448, 17, 31, 25_132))),
            &i18n,
        ) else {
            panic!("reported usage must render fields");
        };
        assert_eq!(reported.last().unwrap().1, "25,132");
        let rendered = reported
            .iter()
            .flat_map(|(label, value)| [label.as_str(), value.as_str()])
            .collect::<Vec<_>>()
            .join(" ");
        assert!(!rendered.contains("must-not-render"));
        assert!(!rendered.to_ascii_lowercase().contains("context"));
        assert!(!rendered.to_ascii_lowercase().contains("ttft"));
        assert!(!rendered.to_ascii_lowercase().contains("token/s"));
    }

    #[test]
    fn compact_total_only_renders_provider_reported_totals() {
        let i18n = I18n::english_for_test();

        assert_eq!(
            request_usage_compact_total(&request_usage(None), &i18n),
            None
        );
        assert_eq!(
            request_usage_compact_total(&request_usage(Some(usage(0, 0, 0, 0, 0, 0))), &i18n),
            None
        );
        assert_eq!(
            request_usage_compact_total(
                &request_usage(Some(usage(24_716, 416, 0, 0, 0, 0))),
                &i18n
            ),
            None
        );
        assert_eq!(
            request_usage_compact_total(
                &request_usage(Some(usage(24_716, 416, 24_448, 17, 31, 25_132))),
                &i18n
            ),
            Some("25.1k Token".to_string())
        );
        assert_eq!(
            request_usage_compact_total(
                &request_usage(Some(usage(24_716, 416, 24_448, 17, 31, 25_132))),
                &I18n::for_locale_tag("zh-CN")
            ),
            Some("25.1k Token".to_string())
        );
    }

    #[test]
    fn request_usage_localization_keys_exist_in_both_runtime_locales() {
        const KEYS: [&str; 13] = [
            "conversation-request-usage-tooltip",
            "conversation-request-usage-title",
            "conversation-request-usage-compact-total",
            "conversation-request-usage-input-tokens",
            "conversation-request-usage-output-tokens",
            "conversation-request-usage-cache-read",
            "conversation-request-usage-cache-hit-rate",
            "conversation-request-usage-cache-write",
            "conversation-request-usage-reasoning-tokens",
            "conversation-request-usage-total-tokens",
            "conversation-request-usage-unreported",
            "conversation-request-usage-unavailable",
            "conversation-request-usage-unknown-value",
        ];

        for locale in ["en-US", "zh-CN"] {
            let i18n = I18n::for_locale_tag(locale);
            for key in KEYS {
                let text = if key == "conversation-request-usage-compact-total" {
                    let mut args = FluentArgs::new();
                    args.set("tokens", "25.1k");
                    i18n.t_with_args(key, &args)
                } else {
                    i18n.t(key)
                };
                assert_ne!(text, key, "locale {locale} is missing {key}");
            }
        }
    }

    struct RequestUsageHarness;

    impl Render for RequestUsageHarness {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            RequestUsageDisclosure::new(
                "conversation-request-usage-step-final",
                "request-usage-test-row",
                request_usage(Some(usage(24_716, 416, 24_448, 17, 31, 25_132))),
            )
        }
    }

    #[gpui::test]
    fn request_usage_hover_card_uses_only_the_icon_and_component_delays(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        cx.update(crate::foundation::init_i18n);
        let (_, cx) = cx.add_window_view(|_, _| RequestUsageHarness);

        let trigger_center = cx
            .debug_bounds("conversation-request-usage-trigger-step-final")
            .expect("request usage trigger")
            .center();
        let compact_total_center = cx
            .debug_bounds("conversation-request-usage-compact-total-step-final")
            .expect("request usage compact total")
            .center();
        let outside = point(px(1_200.), px(800.));

        assert!(
            cx.debug_bounds("conversation-request-usage-content-step-final")
                .is_none()
        );
        cx.simulate_mouse_move(compact_total_center, None, Modifiers::default());
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("conversation-request-usage-content-step-final")
                .is_none()
        );
        cx.simulate_click(compact_total_center, Modifiers::default());
        assert!(
            cx.debug_bounds("conversation-request-usage-content-step-final")
                .is_none()
        );

        cx.simulate_mouse_move(trigger_center, None, Modifiers::default());
        assert!(
            cx.debug_bounds("conversation-request-usage-content-step-final")
                .is_none()
        );
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("conversation-request-usage-content-step-final")
                .is_some()
        );

        let content_center = cx
            .debug_bounds("conversation-request-usage-content-step-final")
            .expect("request usage content")
            .center();
        cx.simulate_mouse_move(content_center, None, Modifiers::default());
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("conversation-request-usage-content-step-final")
                .is_some()
        );

        cx.simulate_mouse_move(outside, None, Modifiers::default());
        assert!(
            cx.debug_bounds("conversation-request-usage-content-step-final")
                .is_some()
        );
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("conversation-request-usage-content-step-final")
                .is_none()
        );

        cx.simulate_mouse_move(trigger_center, None, Modifiers::default());
        cx.simulate_mouse_move(outside, None, Modifiers::default());
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("conversation-request-usage-content-step-final")
                .is_none()
        );
    }
}

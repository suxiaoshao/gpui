use std::num::NonZeroU64;

use fluent_bundle::FluentArgs;
use gpui::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce, Role,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use gpui_component::{
    ActiveTheme as _, Icon, Sizable as _, StyledExt as _, h_flex, hover_card::HoverCard,
    label::Label, v_flex,
};
use jaco_core::{
    ConversationContextRequestUsage, ProviderId, ProviderModelId, ProviderUsageCoverage,
};

use crate::{
    foundation::{I18n, assets::IconName, conversation_format::format_token_count},
    state::providers::{ProviderModelChoice, ProviderModelKey},
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ComposerContextProjection {
    pub(crate) current_choice: Option<ComposerContextChoice>,
    pub(crate) latest_request: Option<ConversationContextRequestUsage>,
    pub(crate) occupancy: ComposerContextOccupancy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ComposerContextChoice {
    pub(crate) provider_id: ProviderId,
    pub(crate) provider_label: SharedString,
    pub(crate) model_id: ProviderModelId,
    pub(crate) model_label: SharedString,
    pub(crate) context_window_tokens: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ComposerContextOccupancy {
    Known {
        used_tokens: u64,
        percentage_tenths: u128,
    },
    Unknown(ComposerContextUnknownReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ComposerContextUnknownReason {
    NoModelSelected,
    ContextWindowUnknown,
    NoCompletedRequest,
    LatestRequestModelMismatch,
    UsageUnavailable,
    UsageUnreported,
    UsagePartial,
}

pub(crate) fn composer_context_projection(
    selected_model: Option<&ProviderModelKey>,
    choices: &Result<Vec<ProviderModelChoice>, SharedString>,
    latest_request: Option<&ConversationContextRequestUsage>,
) -> ComposerContextProjection {
    let Some(selected_model) = selected_model else {
        return ComposerContextProjection {
            current_choice: None,
            latest_request: latest_request.cloned(),
            occupancy: ComposerContextOccupancy::Unknown(
                ComposerContextUnknownReason::NoModelSelected,
            ),
        };
    };

    let catalog_choice = choices.as_ref().ok().and_then(|choices| {
        choices.iter().find(|choice| {
            choice.provider_id == selected_model.provider_id
                && choice.model_id == selected_model.model_id
        })
    });
    let current_choice = ComposerContextChoice {
        provider_id: selected_model.provider_id.clone(),
        provider_label: catalog_choice
            .map(|choice| choice.provider_display_name.clone())
            .unwrap_or_else(|| selected_model.provider_id.clone())
            .into(),
        model_id: selected_model.model_id.clone(),
        model_label: catalog_choice
            .map(ProviderModelChoice::display_label)
            .unwrap_or_else(|| selected_model.model_id.clone())
            .into(),
        context_window_tokens: catalog_choice
            .and_then(|choice| choice.capabilities.context_window.as_ref())
            .map(|context_window| context_window.tokens.get()),
    };
    let occupancy = derive_occupancy(&current_choice, latest_request);

    ComposerContextProjection {
        current_choice: Some(current_choice),
        latest_request: latest_request.cloned(),
        occupancy,
    }
}

fn derive_occupancy(
    current_choice: &ComposerContextChoice,
    latest_request: Option<&ConversationContextRequestUsage>,
) -> ComposerContextOccupancy {
    let Some(capacity) = current_choice
        .context_window_tokens
        .and_then(NonZeroU64::new)
    else {
        return ComposerContextOccupancy::Unknown(
            ComposerContextUnknownReason::ContextWindowUnknown,
        );
    };
    let Some(latest_request) = latest_request else {
        return ComposerContextOccupancy::Unknown(ComposerContextUnknownReason::NoCompletedRequest);
    };
    if latest_request.provider_id != current_choice.provider_id
        || latest_request.model_id != current_choice.model_id
    {
        return ComposerContextOccupancy::Unknown(
            ComposerContextUnknownReason::LatestRequestModelMismatch,
        );
    }
    let Some(usage) = latest_request.usage.as_ref() else {
        return ComposerContextOccupancy::Unknown(ComposerContextUnknownReason::UsageUnavailable);
    };

    match usage.coverage() {
        ProviderUsageCoverage::Unreported => {
            ComposerContextOccupancy::Unknown(ComposerContextUnknownReason::UsageUnreported)
        }
        ProviderUsageCoverage::Partial => {
            ComposerContextOccupancy::Unknown(ComposerContextUnknownReason::UsagePartial)
        }
        ProviderUsageCoverage::Reported => ComposerContextOccupancy::Known {
            used_tokens: usage.total_tokens,
            percentage_tenths: percentage_tenths(usage.total_tokens, capacity),
        },
    }
}

fn percentage_tenths(used_tokens: u64, capacity: NonZeroU64) -> u128 {
    let capacity = u128::from(capacity.get());
    let numerator = u128::from(used_tokens) * 1_000;
    (numerator + capacity / 2) / capacity
}

fn format_percentage_number(percentage_tenths: u128) -> String {
    if percentage_tenths.is_multiple_of(10) {
        (percentage_tenths / 10).to_string()
    } else {
        format!("{}.{}", percentage_tenths / 10, percentage_tenths % 10)
    }
}

fn percentage_text(i18n: &I18n, key: &str, percentage_tenths: u128) -> String {
    let mut args = FluentArgs::new();
    args.set("percentage", format_percentage_number(percentage_tenths));
    i18n.t_with_args(key, &args)
}

fn context_occupancy_token_summary(projection: &ComposerContextProjection, i18n: &I18n) -> String {
    let unknown = || i18n.t("conversation-context-occupancy-unknown-value");
    let used = match projection.occupancy {
        ComposerContextOccupancy::Known { used_tokens, .. } => format_token_count(used_tokens),
        ComposerContextOccupancy::Unknown(_) => unknown(),
    };
    let context_window = projection
        .current_choice
        .as_ref()
        .and_then(|choice| choice.context_window_tokens)
        .map(format_token_count)
        .unwrap_or_else(unknown);

    let mut args = FluentArgs::new();
    args.set("used", used);
    args.set("context_window", context_window);
    i18n.t_with_args("conversation-context-occupancy-token-summary", &args)
}

fn context_occupancy_content_lines(
    projection: &ComposerContextProjection,
    i18n: &I18n,
) -> [String; 2] {
    [
        i18n.t("conversation-context-occupancy-title"),
        context_occupancy_token_summary(projection, i18n),
    ]
}

fn unknown_reason_key(reason: ComposerContextUnknownReason) -> &'static str {
    match reason {
        ComposerContextUnknownReason::NoModelSelected => {
            "conversation-context-occupancy-reason-no-model"
        }
        ComposerContextUnknownReason::ContextWindowUnknown => {
            "conversation-context-occupancy-reason-window-unknown"
        }
        ComposerContextUnknownReason::NoCompletedRequest => {
            "conversation-context-occupancy-reason-no-request"
        }
        ComposerContextUnknownReason::LatestRequestModelMismatch => {
            "conversation-context-occupancy-reason-model-mismatch"
        }
        ComposerContextUnknownReason::UsageUnavailable => {
            "conversation-context-occupancy-reason-usage-unavailable"
        }
        ComposerContextUnknownReason::UsageUnreported => {
            "conversation-context-occupancy-reason-usage-unreported"
        }
        ComposerContextUnknownReason::UsagePartial => {
            "conversation-context-occupancy-reason-usage-partial"
        }
    }
}

#[derive(IntoElement)]
pub(crate) struct ContextOccupancyDisclosure {
    id: ElementId,
    hover_card_id: ElementId,
    projection: ComposerContextProjection,
}

impl ContextOccupancyDisclosure {
    pub(crate) fn new(id: impl Into<SharedString>, projection: ComposerContextProjection) -> Self {
        let id = id.into();
        Self {
            id: id.clone().into(),
            hover_card_id: format!("{}-hover-card", id.as_ref()).into(),
            projection,
        }
    }
}

impl RenderOnce for ContextOccupancyDisclosure {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let i18n = cx.global::<I18n>();
        let (summary, accessible_label) = match self.projection.occupancy {
            ComposerContextOccupancy::Known {
                percentage_tenths, ..
            } => (
                percentage_text(
                    i18n,
                    "conversation-context-occupancy-summary-known",
                    percentage_tenths,
                ),
                percentage_text(
                    i18n,
                    "conversation-context-occupancy-accessible-known",
                    percentage_tenths,
                ),
            ),
            ComposerContextOccupancy::Unknown(reason) => {
                let reason = i18n.t(unknown_reason_key(reason));
                let mut args = FluentArgs::new();
                args.set("reason", reason.clone());
                (
                    i18n.t("conversation-context-occupancy-summary-unknown"),
                    i18n.t_with_args("conversation-context-occupancy-accessible-unknown", &args),
                )
            }
        };
        let [title, token_summary] = context_occupancy_content_lines(&self.projection, i18n);
        let accessible_description = token_summary.clone();
        let content = v_flex()
            .id("conversation-context-occupancy-content-body")
            .debug_selector(|| "conversation-context-occupancy-content".into())
            .min_w(px(280.))
            .gap_2()
            .child(Label::new(title).font_semibold())
            .child(
                Label::new(token_summary)
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .whitespace_nowrap(),
            );
        let trigger = h_flex()
            .id(self.id)
            .debug_selector(|| "conversation-context-occupancy-trigger".into())
            .role(Role::Image)
            .aria_label(accessible_label)
            .aria_description(accessible_description)
            .tab_index(0)
            .items_center()
            .gap_1()
            .flex_shrink_0()
            .rounded(cx.theme().radius)
            .text_color(cx.theme().muted_foreground)
            .focus_visible(|this| this.bg(cx.theme().tokens.secondary_hover.background))
            .child(Icon::new(IconName::Gauge).xsmall())
            .child(
                Label::new(summary)
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .whitespace_nowrap(),
            );

        div().mr(px(5.)).flex_shrink_0().child(
            HoverCard::new(self.hover_card_id)
                .anchor(gpui::Anchor::BottomRight)
                .trigger(trigger)
                .child(content),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Context, Modifiers, Render, TestAppContext, point, px};
    use jaco_core::{
        CapabilitySourceSnapshot, ContextWindowCapabilitySnapshot, ProviderUsageSnapshot,
        conservative_model_capabilities,
    };
    use std::time::Duration;
    use time::OffsetDateTime;

    fn selected_model() -> ProviderModelKey {
        ProviderModelKey {
            provider_id: "provider-1".to_string(),
            model_id: "model-1".to_string(),
        }
    }

    fn model_choice(context_window: Option<u64>) -> ProviderModelChoice {
        let mut capabilities = conservative_model_capabilities("openai");
        capabilities.context_window =
            context_window.map(|tokens| ContextWindowCapabilitySnapshot {
                tokens: NonZeroU64::new(tokens).expect("positive fixture context window"),
                source: CapabilitySourceSnapshot::Manual {
                    source: "test fixture".to_string(),
                },
            });
        ProviderModelChoice {
            provider_id: "provider-1".to_string(),
            provider_kind: "openai".to_string(),
            provider_display_name: "Provider One".to_string(),
            model_id: "model-1".to_string(),
            model_display_name: Some("Model One".to_string()),
            capabilities,
        }
    }

    fn usage(total_tokens: u64, detail_tokens: u64) -> ProviderUsageSnapshot {
        ProviderUsageSnapshot {
            input_tokens: detail_tokens,
            output_tokens: 0,
            cached_input_tokens: 0,
            cache_write_input_tokens: 0,
            reasoning_tokens: 0,
            total_tokens,
            metadata: None,
        }
    }

    fn request(
        provider_id: &str,
        model_id: &str,
        usage: Option<ProviderUsageSnapshot>,
    ) -> ConversationContextRequestUsage {
        ConversationContextRequestUsage {
            agent_run_id: "run-1".to_string(),
            provider_step_id: "step-1".to_string(),
            provider_step_seq: 1,
            provider_id: provider_id.to_string(),
            model_id: model_id.to_string(),
            provider_step_completed_at: OffsetDateTime::UNIX_EPOCH,
            agent_run_completed_at: OffsetDateTime::UNIX_EPOCH,
            usage,
        }
    }

    fn projection(
        selected: Option<&ProviderModelKey>,
        context_window: Option<u64>,
        request: Option<&ConversationContextRequestUsage>,
    ) -> ComposerContextProjection {
        composer_context_projection(selected, &Ok(vec![model_choice(context_window)]), request)
    }

    #[test]
    fn context_occupancy_projection_distinguishes_every_unknown_state() {
        let selected = selected_model();
        let reported = request("provider-1", "model-1", Some(usage(48_000, 48_000)));
        assert_eq!(
            projection(None, Some(128_000), Some(&reported)).occupancy,
            ComposerContextOccupancy::Unknown(ComposerContextUnknownReason::NoModelSelected)
        );
        assert_eq!(
            projection(Some(&selected), None, Some(&reported)).occupancy,
            ComposerContextOccupancy::Unknown(ComposerContextUnknownReason::ContextWindowUnknown)
        );
        assert_eq!(
            projection(Some(&selected), Some(128_000), None).occupancy,
            ComposerContextOccupancy::Unknown(ComposerContextUnknownReason::NoCompletedRequest)
        );
        let mismatch = request("provider-1", "model-2", Some(usage(48_000, 48_000)));
        assert_eq!(
            projection(Some(&selected), Some(128_000), Some(&mismatch)).occupancy,
            ComposerContextOccupancy::Unknown(
                ComposerContextUnknownReason::LatestRequestModelMismatch
            )
        );
        let unavailable = request("provider-1", "model-1", None);
        assert_eq!(
            projection(Some(&selected), Some(128_000), Some(&unavailable)).occupancy,
            ComposerContextOccupancy::Unknown(ComposerContextUnknownReason::UsageUnavailable)
        );
        let unreported = request("provider-1", "model-1", Some(usage(0, 0)));
        assert_eq!(
            projection(Some(&selected), Some(128_000), Some(&unreported)).occupancy,
            ComposerContextOccupancy::Unknown(ComposerContextUnknownReason::UsageUnreported)
        );
        let partial = request("provider-1", "model-1", Some(usage(0, 24)));
        assert_eq!(
            projection(Some(&selected), Some(128_000), Some(&partial)).occupancy,
            ComposerContextOccupancy::Unknown(ComposerContextUnknownReason::UsagePartial)
        );
        assert_eq!(
            projection(Some(&selected), Some(128_000), Some(&reported)).occupancy,
            ComposerContextOccupancy::Known {
                used_tokens: 48_000,
                percentage_tenths: 375,
            }
        );
    }

    #[test]
    fn percentage_formatting_rounds_with_integers_and_does_not_clamp() {
        let capacity = NonZeroU64::new(128_000).unwrap();
        assert_eq!(percentage_tenths(1_280, capacity), 10);
        assert_eq!(percentage_tenths(48_000, capacity), 375);
        assert_eq!(percentage_tenths(160_000, capacity), 1_250);
        assert_eq!(format_percentage_number(10), "1");
        assert_eq!(format_percentage_number(375), "37.5");
        assert_eq!(format_percentage_number(1_250), "125");
        assert!(percentage_tenths(u64::MAX, NonZeroU64::new(1).unwrap()) > 1_000);
    }

    #[test]
    fn context_occupancy_uses_catalog_labels_and_id_fallbacks() {
        let selected = selected_model();
        let known = composer_context_projection(
            Some(&selected),
            &Ok(vec![model_choice(Some(128_000))]),
            None,
        );
        let choice = known.current_choice.unwrap();
        assert_eq!(choice.provider_label.as_ref(), "Provider One");
        assert_eq!(choice.model_label.as_ref(), "Model One");

        let fallback =
            composer_context_projection(Some(&selected), &Err("catalog unavailable".into()), None);
        let choice = fallback.current_choice.unwrap();
        assert_eq!(choice.provider_label.as_ref(), "provider-1");
        assert_eq!(choice.model_label.as_ref(), "model-1");
        assert_eq!(choice.context_window_tokens, None);
    }

    #[test]
    fn context_occupancy_content_has_only_title_and_token_summary() {
        let i18n = I18n::english_for_test();
        let selected = selected_model();
        let reported_request = request("provider-1", "model-1", Some(usage(48_000, 48_000)));
        let reported_projection =
            projection(Some(&selected), Some(128_000), Some(&reported_request));
        let lines = context_occupancy_content_lines(&reported_projection, &i18n);

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "Context occupancy");
        assert_eq!(lines[1], "48,000 / 128,000 Token");
        assert_eq!(lines[1].matches("Token").count(), 1);
        let content = lines.join("\n");
        for excluded in ["Provider One", "Model One", "Request completed", "%"] {
            assert!(
                !content.contains(excluded),
                "unexpected {excluded} in {content}"
            );
        }

        let partial = request("provider-1", "model-1", Some(usage(0, 24)));
        let partial_projection = projection(Some(&selected), Some(128_000), Some(&partial));
        let lines = context_occupancy_content_lines(&partial_projection, &i18n);
        assert_eq!(lines[0], "Context occupancy");
        assert_eq!(lines[1], "— / 128,000 Token");
        assert_eq!(
            i18n.t(unknown_reason_key(
                ComposerContextUnknownReason::UsagePartial
            )),
            "The provider reported only partial usage for the latest request"
        );
    }

    #[test]
    fn context_occupancy_localization_keys_exist_in_both_runtime_locales() {
        const KEYS: [&str; 23] = [
            "conversation-context-occupancy-tooltip",
            "conversation-context-occupancy-title",
            "conversation-context-occupancy-summary-known",
            "conversation-context-occupancy-summary-unknown",
            "conversation-context-occupancy-accessible-known",
            "conversation-context-occupancy-accessible-unknown",
            "conversation-context-occupancy-used-tokens",
            "conversation-context-occupancy-context-window",
            "conversation-context-occupancy-percentage",
            "conversation-context-occupancy-provider",
            "conversation-context-occupancy-model",
            "conversation-context-occupancy-request-completed",
            "conversation-context-occupancy-token-value",
            "conversation-context-occupancy-token-summary",
            "conversation-context-occupancy-percentage-value",
            "conversation-context-occupancy-unknown-value",
            "conversation-context-occupancy-reason-no-model",
            "conversation-context-occupancy-reason-window-unknown",
            "conversation-context-occupancy-reason-no-request",
            "conversation-context-occupancy-reason-model-mismatch",
            "conversation-context-occupancy-reason-usage-unavailable",
            "conversation-context-occupancy-reason-usage-unreported",
            "conversation-context-occupancy-reason-usage-partial",
        ];

        for locale in ["en-US", "zh-CN"] {
            let i18n = I18n::for_locale_tag(locale);
            for key in KEYS {
                let text = match key {
                    "conversation-context-occupancy-summary-known"
                    | "conversation-context-occupancy-accessible-known"
                    | "conversation-context-occupancy-percentage-value" => {
                        let mut args = FluentArgs::new();
                        args.set("percentage", "37.5");
                        i18n.t_with_args(key, &args)
                    }
                    "conversation-context-occupancy-accessible-unknown" => {
                        let mut args = FluentArgs::new();
                        args.set("reason", "unknown");
                        i18n.t_with_args(key, &args)
                    }
                    "conversation-context-occupancy-token-value" => {
                        let mut args = FluentArgs::new();
                        args.set("tokens", "128,000");
                        i18n.t_with_args(key, &args)
                    }
                    "conversation-context-occupancy-token-summary" => {
                        let mut args = FluentArgs::new();
                        args.set("used", "48,000");
                        args.set("context_window", "128,000");
                        i18n.t_with_args(key, &args)
                    }
                    _ => i18n.t(key),
                };
                assert_ne!(text, key, "locale {locale} is missing {key}");
            }
        }
    }

    struct ContextOccupancyHarness;

    impl Render for ContextOccupancyHarness {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let selected = selected_model();
            let request = request("provider-1", "model-1", Some(usage(48_000, 48_000)));
            ContextOccupancyDisclosure::new(
                "conversation-context-occupancy-test",
                projection(Some(&selected), Some(128_000), Some(&request)),
            )
        }
    }

    #[gpui::test]
    fn context_occupancy_cluster_uses_native_hover_card_delays(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        cx.update(crate::foundation::init_i18n);
        let (_, cx) = cx.add_window_view(|_, _| ContextOccupancyHarness);
        let trigger_center = cx
            .debug_bounds("conversation-context-occupancy-trigger")
            .expect("context occupancy trigger")
            .center();
        let outside = point(px(1_200.), px(800.));

        assert!(
            cx.debug_bounds("conversation-context-occupancy-content")
                .is_none()
        );
        cx.simulate_click(trigger_center, Modifiers::default());
        assert!(
            cx.debug_bounds("conversation-context-occupancy-content")
                .is_none()
        );
        cx.simulate_mouse_move(trigger_center, None, Modifiers::default());
        assert!(
            cx.debug_bounds("conversation-context-occupancy-content")
                .is_none()
        );
        cx.executor().advance_clock(Duration::from_millis(599));
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("conversation-context-occupancy-content")
                .is_none()
        );
        cx.executor().advance_clock(Duration::from_millis(1));
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("conversation-context-occupancy-content")
                .is_some()
        );

        let content_center = cx
            .debug_bounds("conversation-context-occupancy-content")
            .expect("context occupancy content")
            .center();
        cx.simulate_mouse_move(content_center, None, Modifiers::default());
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("conversation-context-occupancy-content")
                .is_some()
        );
        cx.simulate_mouse_move(outside, None, Modifiers::default());
        cx.executor().advance_clock(Duration::from_millis(299));
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("conversation-context-occupancy-content")
                .is_some()
        );
        cx.executor().advance_clock(Duration::from_millis(1));
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("conversation-context-occupancy-content")
                .is_none()
        );
    }
}

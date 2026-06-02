use crate::foundation;
use ai_chat_core::ReasoningCapabilitySnapshot;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ThinkingEffort {
    None,
    Low,
    Medium,
    High,
    XHigh,
}

impl ThinkingEffort {
    pub(crate) fn label(self, i18n: &foundation::I18n) -> String {
        i18n.t(match self {
            Self::None => "chat-form-effort-none",
            Self::Low => "chat-form-effort-low",
            Self::Medium => "chat-form-effort-medium",
            Self::High => "chat-form-effort-high",
            Self::XHigh => "chat-form-effort-xhigh",
        })
    }
}

pub(crate) fn reasoning_efforts(
    reasoning: Option<&ReasoningCapabilitySnapshot>,
) -> Vec<ThinkingEffort> {
    reasoning
        .map(|reasoning| {
            reasoning
                .efforts
                .iter()
                .filter_map(|effort| ThinkingEffort::from_capability_value(effort))
                .filter(|effort| *effort != ThinkingEffort::None)
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn computed_default_reasoning_effort(
    reasoning: Option<&ReasoningCapabilitySnapshot>,
) -> Option<ThinkingEffort> {
    let reasoning = reasoning?;
    let efforts = reasoning_efforts(Some(reasoning));
    let default_effort = ThinkingEffort::from_capability_value(&reasoning.default_effort);
    computed_default_effort(&efforts, default_effort)
}

pub(crate) fn computed_default_effort(
    efforts: &[ThinkingEffort],
    default_effort: Option<ThinkingEffort>,
) -> Option<ThinkingEffort> {
    default_effort
        .filter(|effort| *effort != ThinkingEffort::None)
        .or_else(|| {
            efforts
                .contains(&ThinkingEffort::Medium)
                .then_some(ThinkingEffort::Medium)
        })
        .or_else(|| {
            efforts
                .iter()
                .copied()
                .find(|effort| *effort != ThinkingEffort::None)
        })
}

impl ThinkingEffort {
    fn from_capability_value(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "none" => Some(Self::None),
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "x_high" | "xhigh" | "extra_high" | "extra-high" | "extra high" => Some(Self::XHigh),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ThinkingEffort, computed_default_effort, computed_default_reasoning_effort,
        reasoning_efforts,
    };
    use ai_chat_core::ReasoningCapabilitySnapshot;

    #[test]
    fn default_effort_skips_none_and_prefers_medium() {
        let efforts = &[
            ThinkingEffort::None,
            ThinkingEffort::Low,
            ThinkingEffort::Medium,
            ThinkingEffort::High,
        ];

        assert_eq!(
            computed_default_effort(efforts, Some(ThinkingEffort::None)),
            Some(ThinkingEffort::Medium)
        );
    }

    #[test]
    fn default_effort_falls_back_to_first_non_none() {
        let efforts = &[ThinkingEffort::None, ThinkingEffort::High];

        assert_eq!(
            computed_default_effort(efforts, None),
            Some(ThinkingEffort::High)
        );
    }

    #[test]
    fn reasoning_efforts_map_known_capability_values() {
        let reasoning = ReasoningCapabilitySnapshot {
            default_effort: "x_high".to_string(),
            efforts: vec![
                "low".to_string(),
                "medium".to_string(),
                "extra_high".to_string(),
                "unknown".to_string(),
            ],
            summaries: false,
        };

        assert_eq!(
            reasoning_efforts(Some(&reasoning)),
            vec![
                ThinkingEffort::Low,
                ThinkingEffort::Medium,
                ThinkingEffort::XHigh
            ]
        );
        assert_eq!(
            computed_default_reasoning_effort(Some(&reasoning)),
            Some(ThinkingEffort::XHigh)
        );
    }

    #[test]
    fn reasoning_default_falls_back_to_medium_then_first_known_effort() {
        let reasoning = ReasoningCapabilitySnapshot {
            default_effort: "unknown".to_string(),
            efforts: vec!["low".to_string(), "medium".to_string()],
            summaries: false,
        };

        assert_eq!(
            computed_default_reasoning_effort(Some(&reasoning)),
            Some(ThinkingEffort::Medium)
        );
    }

    #[test]
    fn reasoning_efforts_hide_none_option() {
        let reasoning = ReasoningCapabilitySnapshot {
            default_effort: "none".to_string(),
            efforts: vec!["none".to_string(), "high".to_string()],
            summaries: false,
        };

        assert_eq!(
            reasoning_efforts(Some(&reasoning)),
            vec![ThinkingEffort::High]
        );
        assert_eq!(
            computed_default_reasoning_effort(Some(&reasoning)),
            Some(ThinkingEffort::High)
        );
    }
}

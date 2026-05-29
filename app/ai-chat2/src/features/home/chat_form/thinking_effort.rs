use crate::foundation;

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

pub(crate) fn selectable_efforts(efforts: &[ThinkingEffort]) -> Vec<ThinkingEffort> {
    efforts
        .iter()
        .copied()
        .filter(|effort| *effort != ThinkingEffort::None)
        .collect()
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

#[cfg(test)]
mod tests {
    use super::{ThinkingEffort, computed_default_effort, selectable_efforts};

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
    fn selectable_efforts_hide_none() {
        let efforts = &[
            ThinkingEffort::None,
            ThinkingEffort::Low,
            ThinkingEffort::Medium,
            ThinkingEffort::High,
            ThinkingEffort::XHigh,
        ];

        assert_eq!(
            selectable_efforts(efforts),
            vec![
                ThinkingEffort::Low,
                ThinkingEffort::Medium,
                ThinkingEffort::High,
                ThinkingEffort::XHigh,
            ]
        );
    }
}

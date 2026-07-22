use crate::foundation;
use fluent_bundle::FluentArgs;
use jaco_core::{
    ReasoningCapabilitySnapshot, ReasoningControlSnapshot, ReasoningSelectionSnapshot,
    TokenBudgetSelectionMode,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TokenBudgetBounds {
    pub(crate) min: Option<u32>,
    pub(crate) max: Option<u32>,
    pub(crate) default_value: u32,
}

impl TokenBudgetBounds {
    pub(crate) fn step(self) -> u32 {
        self.max
            .filter(|max| *max <= 256)
            .map(|_| 1)
            .unwrap_or(1024)
    }
}

pub(crate) fn reasoning_selections(
    reasoning: Option<&ReasoningCapabilitySnapshot>,
) -> Vec<ReasoningSelectionSnapshot> {
    let Some(reasoning) = reasoning else {
        return Vec::new();
    };

    if let Some(control) = reasoning.control.as_ref() {
        return selections_for_control(control);
    }

    legacy_level_selections(reasoning)
}

#[cfg(test)]
pub(crate) fn computed_default_reasoning_selection(
    reasoning: Option<&ReasoningCapabilitySnapshot>,
) -> Option<ReasoningSelectionSnapshot> {
    let reasoning = reasoning?;
    if let Some(control) = reasoning.control.as_ref() {
        return default_selection_for_control(control);
    }

    legacy_default_selection(reasoning)
}

pub(crate) fn reasoning_selection_is_valid(
    reasoning: Option<&ReasoningCapabilitySnapshot>,
    selection: &ReasoningSelectionSnapshot,
) -> bool {
    let Some(reasoning) = reasoning else {
        return false;
    };

    if let Some(control) = reasoning.control.as_ref() {
        return selection_is_valid_for_control(control, selection);
    }

    legacy_level_selections(reasoning).contains(selection)
}

pub(crate) fn token_budget_bounds(
    reasoning: Option<&ReasoningCapabilitySnapshot>,
) -> Option<TokenBudgetBounds> {
    let control = reasoning?.control.as_ref()?;
    token_budget_bounds_for_control(control)
}

pub(crate) fn custom_token_budget_value(
    selection: Option<&ReasoningSelectionSnapshot>,
) -> Option<u32> {
    match selection? {
        ReasoningSelectionSnapshot::TokenBudget {
            mode: TokenBudgetSelectionMode::Custom,
            value,
        } => *value,
        ReasoningSelectionSnapshot::Composite { selections } => selections
            .iter()
            .find_map(|selection| custom_token_budget_value(Some(selection))),
        _ => None,
    }
}

pub(crate) fn set_existing_custom_token_budget(
    selection: &mut Option<ReasoningSelectionSnapshot>,
    value: u32,
) -> bool {
    fn update(selection: &mut ReasoningSelectionSnapshot, value: u32) -> bool {
        match selection {
            ReasoningSelectionSnapshot::TokenBudget {
                mode: TokenBudgetSelectionMode::Custom,
                value: current,
            } => {
                *current = Some(value);
                true
            }
            ReasoningSelectionSnapshot::Composite { selections } => selections
                .iter_mut()
                .any(|selection| update(selection, value)),
            _ => false,
        }
    }

    selection
        .as_mut()
        .is_some_and(|selection| update(selection, value))
}

pub(crate) fn reasoning_selection_label(
    selection: &ReasoningSelectionSnapshot,
    i18n: &foundation::I18n,
) -> String {
    match selection {
        ReasoningSelectionSnapshot::Boolean { enabled } => i18n.t(if *enabled {
            "chat-form-effort-enabled"
        } else {
            "chat-form-effort-disabled"
        }),
        ReasoningSelectionSnapshot::Level { value } => reasoning_value_label(value, i18n),
        ReasoningSelectionSnapshot::TokenBudget { mode, value } => match mode {
            TokenBudgetSelectionMode::Off => i18n.t("chat-form-effort-off"),
            TokenBudgetSelectionMode::Dynamic => i18n.t("chat-form-effort-dynamic"),
            TokenBudgetSelectionMode::Custom => {
                let mut args = FluentArgs::new();
                args.set("tokens", i64::from(value.unwrap_or(0)));
                i18n.t_with_args("chat-form-effort-custom-budget", &args)
            }
        },
        ReasoningSelectionSnapshot::Composite { selections } => selections
            .iter()
            .map(|selection| reasoning_selection_label(selection, i18n))
            .collect::<Vec<_>>()
            .join(", "),
        ReasoningSelectionSnapshot::AlwaysOn => i18n.t("chat-form-effort-always-on"),
    }
}

fn token_budget_bounds_for_control(
    control: &ReasoningControlSnapshot,
) -> Option<TokenBudgetBounds> {
    match control {
        ReasoningControlSnapshot::TokenBudget {
            min,
            max,
            default_value,
            ..
        } => Some(TokenBudgetBounds {
            min: *min,
            max: *max,
            default_value: default_token_budget(*min, *max, *default_value),
        }),
        ReasoningControlSnapshot::Composite { controls } => {
            controls.iter().find_map(token_budget_bounds_for_control)
        }
        _ => None,
    }
}

fn selections_for_control(control: &ReasoningControlSnapshot) -> Vec<ReasoningSelectionSnapshot> {
    match control {
        ReasoningControlSnapshot::None => Vec::new(),
        ReasoningControlSnapshot::Boolean { .. } => vec![
            ReasoningSelectionSnapshot::Boolean { enabled: false },
            ReasoningSelectionSnapshot::Boolean { enabled: true },
        ],
        ReasoningControlSnapshot::Levels { values, .. }
        | ReasoningControlSnapshot::AdaptiveLevels { values, .. } => values
            .iter()
            .filter_map(|value| normalized_level_selection(value))
            .collect(),
        ReasoningControlSnapshot::TokenBudget {
            min,
            max,
            default_value,
            dynamic_supported,
            off_supported,
        } => {
            let mut selections = Vec::new();
            if *off_supported {
                selections.push(ReasoningSelectionSnapshot::TokenBudget {
                    mode: TokenBudgetSelectionMode::Off,
                    value: None,
                });
            }
            if *dynamic_supported {
                selections.push(ReasoningSelectionSnapshot::TokenBudget {
                    mode: TokenBudgetSelectionMode::Dynamic,
                    value: None,
                });
            }
            selections.push(ReasoningSelectionSnapshot::TokenBudget {
                mode: TokenBudgetSelectionMode::Custom,
                value: Some(default_token_budget(*min, *max, *default_value)),
            });
            selections
        }
        ReasoningControlSnapshot::AlwaysOn { .. } => vec![ReasoningSelectionSnapshot::AlwaysOn],
        ReasoningControlSnapshot::Composite { controls } => {
            let mut selections = Vec::new();
            for control in controls {
                for selection in selections_for_control(control) {
                    if !selections.contains(&selection) {
                        selections.push(selection);
                    }
                }
            }
            selections
        }
    }
}

fn selection_is_valid_for_control(
    control: &ReasoningControlSnapshot,
    selection: &ReasoningSelectionSnapshot,
) -> bool {
    match control {
        ReasoningControlSnapshot::None => false,
        ReasoningControlSnapshot::Boolean { .. } => {
            matches!(selection, ReasoningSelectionSnapshot::Boolean { .. })
        }
        ReasoningControlSnapshot::Levels { values, .. }
        | ReasoningControlSnapshot::AdaptiveLevels { values, .. } => match selection {
            ReasoningSelectionSnapshot::Level { value } => values
                .iter()
                .any(|candidate| normalized_matches(candidate, value)),
            _ => false,
        },
        ReasoningControlSnapshot::TokenBudget {
            min,
            max,
            dynamic_supported,
            off_supported,
            ..
        } => token_budget_selection_is_valid(
            *min,
            *max,
            *dynamic_supported,
            *off_supported,
            selection,
        ),
        ReasoningControlSnapshot::AlwaysOn { .. } => {
            matches!(selection, ReasoningSelectionSnapshot::AlwaysOn)
        }
        ReasoningControlSnapshot::Composite { controls } => match selection {
            ReasoningSelectionSnapshot::Composite { selections } => {
                !selections.is_empty()
                    && selections.iter().all(|selection| {
                        controls
                            .iter()
                            .any(|control| selection_is_valid_for_control(control, selection))
                    })
            }
            _ => controls
                .iter()
                .any(|control| selection_is_valid_for_control(control, selection)),
        },
    }
}

fn token_budget_selection_is_valid(
    min: Option<u32>,
    max: Option<u32>,
    dynamic_supported: bool,
    off_supported: bool,
    selection: &ReasoningSelectionSnapshot,
) -> bool {
    match selection {
        ReasoningSelectionSnapshot::TokenBudget {
            mode: TokenBudgetSelectionMode::Off,
            value: None,
        } => off_supported,
        ReasoningSelectionSnapshot::TokenBudget {
            mode: TokenBudgetSelectionMode::Dynamic,
            value: None,
        } => dynamic_supported,
        ReasoningSelectionSnapshot::TokenBudget {
            mode: TokenBudgetSelectionMode::Custom,
            value: Some(value),
        } => min.is_none_or(|min| *value >= min) && max.is_none_or(|max| *value <= max),
        _ => false,
    }
}

#[cfg(test)]
fn default_selection_for_control(
    control: &ReasoningControlSnapshot,
) -> Option<ReasoningSelectionSnapshot> {
    match control {
        ReasoningControlSnapshot::None => None,
        ReasoningControlSnapshot::Boolean { default_enabled } => {
            Some(ReasoningSelectionSnapshot::Boolean {
                enabled: default_enabled.unwrap_or(false),
            })
        }
        ReasoningControlSnapshot::Levels {
            values,
            default_value,
        }
        | ReasoningControlSnapshot::AdaptiveLevels {
            values,
            default_value,
        } => default_level_selection(values, default_value.as_deref()),
        ReasoningControlSnapshot::TokenBudget {
            min,
            max,
            default_value,
            dynamic_supported,
            off_supported,
        } => Some(default_token_budget_selection(
            *min,
            *max,
            *default_value,
            *dynamic_supported,
            *off_supported,
        )),
        ReasoningControlSnapshot::AlwaysOn { .. } => Some(ReasoningSelectionSnapshot::AlwaysOn),
        ReasoningControlSnapshot::Composite { controls } => default_composite_selection(controls),
    }
}

#[cfg(test)]
fn default_composite_selection(
    controls: &[ReasoningControlSnapshot],
) -> Option<ReasoningSelectionSnapshot> {
    controls
        .iter()
        .find_map(|control| match control {
            ReasoningControlSnapshot::Levels { .. }
            | ReasoningControlSnapshot::AdaptiveLevels { .. } => {
                default_selection_for_control(control)
            }
            _ => None,
        })
        .or_else(|| {
            controls.iter().find_map(|control| match control {
                ReasoningControlSnapshot::TokenBudget { .. } => {
                    default_selection_for_control(control)
                }
                _ => None,
            })
        })
        .or_else(|| controls.iter().find_map(default_selection_for_control))
}

#[cfg(test)]
fn default_level_selection(
    values: &[String],
    default_value: Option<&str>,
) -> Option<ReasoningSelectionSnapshot> {
    default_value
        .and_then(normalized_level_value)
        .filter(|value| {
            values
                .iter()
                .any(|candidate| normalized_matches(candidate, value))
        })
        .or_else(|| {
            values
                .iter()
                .find_map(|value| normalized_matches(value, "medium").then(|| "medium".to_string()))
        })
        .or_else(|| {
            values
                .iter()
                .find_map(|value| normalized_level_value(value))
        })
        .map(|value| ReasoningSelectionSnapshot::Level { value })
}

fn legacy_level_selections(
    reasoning: &ReasoningCapabilitySnapshot,
) -> Vec<ReasoningSelectionSnapshot> {
    let mut selections = Vec::new();
    for effort in &reasoning.efforts {
        if let Some(selection) = normalized_level_selection(effort)
            && !selections.contains(&selection)
        {
            selections.push(selection);
        }
    }
    selections
}

#[cfg(test)]
fn legacy_default_selection(
    reasoning: &ReasoningCapabilitySnapshot,
) -> Option<ReasoningSelectionSnapshot> {
    let options = legacy_level_selections(reasoning);
    normalized_level_selection(&reasoning.default_effort)
        .filter(|selection| options.contains(selection))
        .or_else(|| {
            options
                .iter()
                .find(|selection| {
                    matches!(selection, ReasoningSelectionSnapshot::Level { value } if value == "medium")
                })
                .cloned()
        })
        .or_else(|| options.first().cloned())
}

#[cfg(test)]
fn default_token_budget_selection(
    min: Option<u32>,
    max: Option<u32>,
    default_value: Option<i32>,
    dynamic_supported: bool,
    off_supported: bool,
) -> ReasoningSelectionSnapshot {
    match default_value {
        Some(value) if value <= 0 && off_supported && !dynamic_supported => {
            ReasoningSelectionSnapshot::TokenBudget {
                mode: TokenBudgetSelectionMode::Off,
                value: None,
            }
        }
        Some(value) if value < 0 && dynamic_supported => ReasoningSelectionSnapshot::TokenBudget {
            mode: TokenBudgetSelectionMode::Dynamic,
            value: None,
        },
        Some(0) if off_supported => ReasoningSelectionSnapshot::TokenBudget {
            mode: TokenBudgetSelectionMode::Off,
            value: None,
        },
        Some(value) if value > 0 => ReasoningSelectionSnapshot::TokenBudget {
            mode: TokenBudgetSelectionMode::Custom,
            value: Some(value as u32),
        },
        _ if dynamic_supported => ReasoningSelectionSnapshot::TokenBudget {
            mode: TokenBudgetSelectionMode::Dynamic,
            value: None,
        },
        _ if off_supported => ReasoningSelectionSnapshot::TokenBudget {
            mode: TokenBudgetSelectionMode::Off,
            value: None,
        },
        _ => ReasoningSelectionSnapshot::TokenBudget {
            mode: TokenBudgetSelectionMode::Custom,
            value: Some(default_token_budget(min, max, default_value)),
        },
    }
}

fn default_token_budget(min: Option<u32>, max: Option<u32>, default_value: Option<i32>) -> u32 {
    default_value
        .and_then(|value| (value > 0).then_some(value as u32))
        .or(min.filter(|value| *value > 0))
        .or(max)
        .unwrap_or(1024)
}

fn normalized_level_selection(value: &str) -> Option<ReasoningSelectionSnapshot> {
    normalized_level_value(value).map(|value| ReasoningSelectionSnapshot::Level { value })
}

fn normalized_level_value(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let normalized = match value.to_ascii_lowercase().as_str() {
        "x_high" | "extra_high" | "extra-high" | "extra high" => "xhigh".to_string(),
        other => other.to_string(),
    };
    Some(normalized)
}

fn normalized_matches(candidate: &str, value: &str) -> bool {
    normalized_level_value(candidate).as_deref() == Some(value)
}

fn reasoning_value_label(value: &str, i18n: &foundation::I18n) -> String {
    let Some(value) = normalized_level_value(value) else {
        return String::new();
    };
    match value.as_str() {
        "none" => i18n.t("chat-form-effort-none"),
        "minimal" => i18n.t("chat-form-effort-minimal"),
        "low" => i18n.t("chat-form-effort-low"),
        "medium" => i18n.t("chat-form-effort-medium"),
        "high" => i18n.t("chat-form-effort-high"),
        "xhigh" => i18n.t("chat-form-effort-xhigh"),
        "max" => i18n.t("chat-form-effort-max"),
        "disabled" => i18n.t("chat-form-effort-disabled"),
        "enabled" => i18n.t("chat-form-effort-enabled"),
        "off" => i18n.t("chat-form-effort-off"),
        "dynamic" => i18n.t("chat-form-effort-dynamic"),
        "always_on" => i18n.t("chat-form-effort-always-on"),
        _ => value,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        computed_default_reasoning_selection, reasoning_selection_is_valid, reasoning_selections,
    };
    use jaco_core::{
        CapabilitySourceSnapshot, ReasoningCapabilitySnapshot, ReasoningControlSnapshot,
        ReasoningSelectionSnapshot, TokenBudgetSelectionMode,
    };

    #[test]
    fn level_controls_preserve_provider_values() {
        let reasoning = reasoning(ReasoningControlSnapshot::Levels {
            values: vec![
                "minimal".to_string(),
                "low".to_string(),
                "medium".to_string(),
                "x_high".to_string(),
            ],
            default_value: Some("x_high".to_string()),
        });

        assert_eq!(
            reasoning_selections(Some(&reasoning)),
            vec![
                level("minimal"),
                level("low"),
                level("medium"),
                level("xhigh")
            ]
        );
        assert_eq!(
            computed_default_reasoning_selection(Some(&reasoning)),
            Some(level("xhigh"))
        );
    }

    #[test]
    fn token_budget_controls_expose_off_dynamic_and_custom_default() {
        let reasoning = reasoning(ReasoningControlSnapshot::TokenBudget {
            min: Some(0),
            max: Some(32768),
            default_value: Some(-1),
            dynamic_supported: true,
            off_supported: true,
        });

        assert_eq!(
            reasoning_selections(Some(&reasoning)),
            vec![
                ReasoningSelectionSnapshot::TokenBudget {
                    mode: TokenBudgetSelectionMode::Off,
                    value: None,
                },
                ReasoningSelectionSnapshot::TokenBudget {
                    mode: TokenBudgetSelectionMode::Dynamic,
                    value: None,
                },
                ReasoningSelectionSnapshot::TokenBudget {
                    mode: TokenBudgetSelectionMode::Custom,
                    value: Some(32768),
                },
            ]
        );
        assert_eq!(
            computed_default_reasoning_selection(Some(&reasoning)),
            Some(ReasoningSelectionSnapshot::TokenBudget {
                mode: TokenBudgetSelectionMode::Dynamic,
                value: None,
            })
        );
    }

    #[test]
    fn token_budget_custom_values_validate_by_bounds() {
        let reasoning = reasoning(ReasoningControlSnapshot::TokenBudget {
            min: Some(1024),
            max: Some(8192),
            default_value: Some(4096),
            dynamic_supported: false,
            off_supported: false,
        });

        assert!(reasoning_selection_is_valid(
            Some(&reasoning),
            &ReasoningSelectionSnapshot::TokenBudget {
                mode: TokenBudgetSelectionMode::Custom,
                value: Some(2048),
            }
        ));
        assert!(!reasoning_selection_is_valid(
            Some(&reasoning),
            &ReasoningSelectionSnapshot::TokenBudget {
                mode: TokenBudgetSelectionMode::Custom,
                value: Some(512),
            }
        ));
        assert!(!reasoning_selection_is_valid(
            Some(&reasoning),
            &ReasoningSelectionSnapshot::TokenBudget {
                mode: TokenBudgetSelectionMode::Dynamic,
                value: None,
            }
        ));
    }

    #[test]
    fn composite_controls_pick_a_level_default_without_leaking_old_models() {
        let old = reasoning(ReasoningControlSnapshot::Levels {
            values: vec!["low".to_string()],
            default_value: Some("low".to_string()),
        });
        let new = reasoning(ReasoningControlSnapshot::Boolean {
            default_enabled: Some(false),
        });
        let old_selection = computed_default_reasoning_selection(Some(&old)).unwrap();

        assert!(!reasoning_selection_is_valid(Some(&new), &old_selection));
        assert_eq!(
            computed_default_reasoning_selection(Some(&new)),
            Some(ReasoningSelectionSnapshot::Boolean { enabled: false })
        );
    }

    #[test]
    fn legacy_reasoning_payloads_still_map_to_level_selections() {
        let reasoning = ReasoningCapabilitySnapshot {
            default_effort: "extra_high".to_string(),
            efforts: vec![
                "low".to_string(),
                "medium".to_string(),
                "extra_high".to_string(),
            ],
            summaries: false,
            control: None,
            source: CapabilitySourceSnapshot::Heuristic {
                reason: "legacy".to_string(),
            },
        };

        assert_eq!(
            reasoning_selections(Some(&reasoning)),
            vec![level("low"), level("medium"), level("xhigh")]
        );
        assert_eq!(
            computed_default_reasoning_selection(Some(&reasoning)),
            Some(level("xhigh"))
        );
    }

    fn reasoning(control: ReasoningControlSnapshot) -> ReasoningCapabilitySnapshot {
        ReasoningCapabilitySnapshot {
            default_effort: String::new(),
            efforts: Vec::new(),
            summaries: false,
            control: Some(control),
            source: CapabilitySourceSnapshot::Manual {
                source: "test".to_string(),
            },
        }
    }

    fn level(value: &str) -> ReasoningSelectionSnapshot {
        ReasoningSelectionSnapshot::Level {
            value: value.to_string(),
        }
    }
}

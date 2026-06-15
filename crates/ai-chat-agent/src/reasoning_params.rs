use ai_chat_core::{ReasoningSelectionSnapshot, RunSettingsSnapshot, TokenBudgetSelectionMode};
use serde_json::{Map, Value, json};

pub(crate) fn reasoning_additional_params(settings: &RunSettingsSnapshot) -> Option<Value> {
    let selection = settings.reasoning_selection.as_ref()?;
    let provider_kind = settings.provider_settings.provider_kind.as_str();
    match provider_kind {
        "openai" | "custom_openai_compatible" | "azure_openai" => {
            level_selection(selection).map(|effort| json!({ "reasoning": { "effort": effort } }))
        }
        "ollama" => ollama_reasoning(selection),
        "gemini" => gemini_reasoning(selection),
        "anthropic" => anthropic_reasoning(selection),
        "deepseek" => deepseek_reasoning(selection),
        "mistral" => mistral_reasoning(selection),
        "openrouter" => openrouter_reasoning(selection),
        _ => None,
    }
}

pub(crate) fn merge_additional_params(left: Option<Value>, right: Option<Value>) -> Option<Value> {
    match (left, right) {
        (None, None) => None,
        (Some(value), None) | (None, Some(value)) => Some(value),
        (Some(mut left), Some(right)) => {
            merge_value(&mut left, right);
            Some(left)
        }
    }
}

fn ollama_reasoning(selection: &ReasoningSelectionSnapshot) -> Option<Value> {
    match selection {
        ReasoningSelectionSnapshot::Boolean { enabled } => Some(json!({ "think": enabled })),
        ReasoningSelectionSnapshot::Level { value } if ollama_level(value) => {
            Some(json!({ "think": value }))
        }
        _ => None,
    }
}

fn gemini_reasoning(selection: &ReasoningSelectionSnapshot) -> Option<Value> {
    match selection {
        ReasoningSelectionSnapshot::Boolean { enabled } => {
            let thinking_config = if *enabled {
                json!({ "includeThoughts": true })
            } else {
                json!({
                    "thinkingBudget": 0,
                    "includeThoughts": true
                })
            };
            Some(json!({
                "generationConfig": {
                    "thinkingConfig": thinking_config
                }
            }))
        }
        ReasoningSelectionSnapshot::Level { value } => Some(json!({
            "generationConfig": {
                "thinkingConfig": {
                    "thinkingLevel": value,
                    "includeThoughts": true
                }
            }
        })),
        ReasoningSelectionSnapshot::TokenBudget { mode, value } => {
            let budget = match mode {
                TokenBudgetSelectionMode::Off => Some(0),
                TokenBudgetSelectionMode::Dynamic => None,
                TokenBudgetSelectionMode::Custom => *value,
            };
            let mut thinking_config = Map::new();
            if let Some(budget) = budget {
                thinking_config.insert("thinkingBudget".to_string(), json!(budget));
            }
            thinking_config.insert("includeThoughts".to_string(), Value::Bool(true));
            Some(json!({ "generationConfig": { "thinkingConfig": thinking_config } }))
        }
        _ => None,
    }
}

fn anthropic_reasoning(selection: &ReasoningSelectionSnapshot) -> Option<Value> {
    match selection {
        ReasoningSelectionSnapshot::Level { value } => Some(json!({
            "thinking": { "type": "adaptive" },
            "output_config": { "effort": value },
        })),
        ReasoningSelectionSnapshot::TokenBudget {
            mode: TokenBudgetSelectionMode::Custom,
            value: Some(value),
        } => Some(json!({
            "thinking": {
                "type": "enabled",
                "budget_tokens": value
            }
        })),
        _ => None,
    }
}

fn deepseek_reasoning(selection: &ReasoningSelectionSnapshot) -> Option<Value> {
    match selection {
        ReasoningSelectionSnapshot::Level { value } if value == "disabled" => {
            Some(json!({ "thinking": { "type": "disabled" } }))
        }
        ReasoningSelectionSnapshot::Level { value } if value == "high" || value == "max" => {
            Some(json!({
                "thinking": { "type": "enabled" },
                "reasoning_effort": value,
            }))
        }
        _ => None,
    }
}

fn mistral_reasoning(selection: &ReasoningSelectionSnapshot) -> Option<Value> {
    match selection {
        ReasoningSelectionSnapshot::Level { value } if value == "none" || value == "high" => {
            Some(json!({ "reasoning_effort": value }))
        }
        ReasoningSelectionSnapshot::AlwaysOn => None,
        _ => None,
    }
}

fn openrouter_reasoning(selection: &ReasoningSelectionSnapshot) -> Option<Value> {
    let mut reasoning = Map::new();
    collect_openrouter_reasoning(selection, &mut reasoning);
    (!reasoning.is_empty()).then(|| json!({ "reasoning": reasoning }))
}

fn collect_openrouter_reasoning(
    selection: &ReasoningSelectionSnapshot,
    reasoning: &mut Map<String, Value>,
) {
    match selection {
        ReasoningSelectionSnapshot::Boolean { enabled } => {
            reasoning.insert("enabled".to_string(), Value::Bool(*enabled));
        }
        ReasoningSelectionSnapshot::Level { value } => {
            reasoning.insert("effort".to_string(), Value::String(value.clone()));
        }
        ReasoningSelectionSnapshot::TokenBudget { mode, value } => match mode {
            TokenBudgetSelectionMode::Off => {
                reasoning.insert("enabled".to_string(), Value::Bool(false));
            }
            TokenBudgetSelectionMode::Dynamic => {}
            TokenBudgetSelectionMode::Custom => {
                if let Some(value) = value {
                    reasoning.insert("max_tokens".to_string(), json!(value));
                }
            }
        },
        ReasoningSelectionSnapshot::Composite { selections } => {
            for selection in selections {
                collect_openrouter_reasoning(selection, reasoning);
            }
        }
        ReasoningSelectionSnapshot::AlwaysOn => {}
    }
}

fn level_selection(selection: &ReasoningSelectionSnapshot) -> Option<&str> {
    match selection {
        ReasoningSelectionSnapshot::Level { value } => Some(value),
        _ => None,
    }
    .map(String::as_str)
}

fn ollama_level(value: &str) -> bool {
    matches!(value, "low" | "medium" | "high")
}

fn merge_value(left: &mut Value, right: Value) {
    match (left, right) {
        (Value::Object(left), Value::Object(right)) => {
            for (key, value) in right {
                match left.get_mut(&key) {
                    Some(existing) => merge_value(existing, value),
                    None => {
                        left.insert(key, value);
                    }
                }
            }
        }
        (left, right) => *left = right,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ai_chat_core::{
        ModelCapabilitiesSnapshot, ProviderCapabilityExtensionSnapshot, ProviderSettingsPayload,
        ToolApprovalMode, ToolApprovalPolicy, ToolPolicySnapshot,
    };

    #[test]
    fn openai_level_maps_to_reasoning_effort() {
        let settings = run_settings(
            "openai",
            ReasoningSelectionSnapshot::Level {
                value: "high".to_string(),
            },
        );

        assert_eq!(
            reasoning_additional_params(&settings),
            Some(json!({ "reasoning": { "effort": "high" } }))
        );
    }

    #[test]
    fn provider_specific_reasoning_shapes_are_generated() {
        assert_eq!(
            reasoning_additional_params(&run_settings(
                "ollama",
                ReasoningSelectionSnapshot::Boolean { enabled: true }
            )),
            Some(json!({ "think": true }))
        );
        assert_eq!(
            reasoning_additional_params(&run_settings(
                "gemini",
                ReasoningSelectionSnapshot::TokenBudget {
                    mode: TokenBudgetSelectionMode::Off,
                    value: None,
                }
            )),
            Some(json!({
                "generationConfig": {
                    "thinkingConfig": {
                        "thinkingBudget": 0,
                        "includeThoughts": true
                    }
                }
            }))
        );
        assert_eq!(
            reasoning_additional_params(&run_settings(
                "gemini",
                ReasoningSelectionSnapshot::Boolean { enabled: false }
            )),
            Some(json!({
                "generationConfig": {
                    "thinkingConfig": {
                        "thinkingBudget": 0,
                        "includeThoughts": true
                    }
                }
            }))
        );
        assert_eq!(
            reasoning_additional_params(&run_settings(
                "deepseek",
                ReasoningSelectionSnapshot::Level {
                    value: "max".to_string(),
                }
            )),
            Some(json!({
                "thinking": { "type": "enabled" },
                "reasoning_effort": "max",
            }))
        );
    }

    #[test]
    fn openrouter_composite_reasoning_merges_fields() {
        let settings = run_settings(
            "openrouter",
            ReasoningSelectionSnapshot::Composite {
                selections: vec![
                    ReasoningSelectionSnapshot::Boolean { enabled: true },
                    ReasoningSelectionSnapshot::Level {
                        value: "high".to_string(),
                    },
                    ReasoningSelectionSnapshot::TokenBudget {
                        mode: TokenBudgetSelectionMode::Custom,
                        value: Some(2048),
                    },
                ],
            },
        );

        assert_eq!(
            reasoning_additional_params(&settings),
            Some(json!({
                "reasoning": {
                    "enabled": true,
                    "effort": "high",
                    "max_tokens": 2048,
                }
            }))
        );
    }

    #[test]
    fn additional_params_merge_without_dropping_tools() {
        let merged = merge_additional_params(
            Some(json!({ "reasoning": { "effort": "high" } })),
            Some(json!({ "tools": [{ "type": "web_search_preview" }] })),
        );

        assert_eq!(
            merged,
            Some(json!({
                "reasoning": { "effort": "high" },
                "tools": [{ "type": "web_search_preview" }],
            }))
        );
    }

    fn run_settings(
        provider_kind: &str,
        selection: ReasoningSelectionSnapshot,
    ) -> RunSettingsSnapshot {
        RunSettingsSnapshot {
            prompt: None,
            provider_id: "provider".to_string(),
            model_id: "model".to_string(),
            model_capabilities: ModelCapabilitiesSnapshot {
                text_input: true,
                text_output: true,
                streaming: true,
                image_input: None,
                file_input: None,
                audio_input: false,
                image_generation: false,
                tool_calling: None,
                hosted_web_search: false,
                remote_mcp: false,
                reasoning: None,
                structured_output: false,
                stateful_response_continuation: false,
                extension: ProviderCapabilityExtensionSnapshot::None,
            },
            provider_settings: ProviderSettingsPayload {
                provider_kind: provider_kind.to_string(),
                fields: Vec::new(),
            },
            reasoning_selection: Some(selection),
            tool_policy: ToolPolicySnapshot {
                approval_policy: ToolApprovalPolicy::Never,
                enabled_sources: Vec::new(),
                max_steps: 8,
                approval_mode: ToolApprovalMode::RequestApproval,
                permission_scope: None,
            },
        }
    }
}

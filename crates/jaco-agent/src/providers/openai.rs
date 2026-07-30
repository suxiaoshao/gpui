use crate::{AgentRuntimeError, Result};
use jaco_core::{ProviderRequestContextSnapshot, ReasoningSelectionSnapshot, RunSettingsSnapshot};
use rig::providers::openai::responses_api::{
    Reasoning, ReasoningContext, ReasoningEffort, ReasoningMode,
};
use serde_json::Value;

mod websocket;
pub use websocket::OpenAiResponsesSessionPool;
pub(crate) use websocket::{
    OpenAiAttemptCoordinator, OpenAiSessionKey, OpenAiWebSocketCompletionModel,
    OpenAiWebSocketModelClient, official_gpt_5_6_websocket,
};

#[derive(Clone, Debug)]
pub(crate) struct OpenAiReasoningPolicy {
    effort: Option<ReasoningEffort>,
    mode: Option<ReasoningMode>,
    context: Option<ReasoningContext>,
    store: bool,
}

impl OpenAiReasoningPolicy {
    pub(crate) fn from_run_settings(settings: &RunSettingsSnapshot) -> Result<Self> {
        let effort = match settings.reasoning_selection.as_ref() {
            None => None,
            Some(ReasoningSelectionSnapshot::Level { value }) => {
                Some(reasoning_effort(value).ok_or_else(|| {
                    AgentRuntimeError::Unsupported(format!(
                        "OpenAI reasoning effort `{value}` is not supported"
                    ))
                })?)
            }
            Some(_) => {
                return Err(AgentRuntimeError::Unsupported(
                    "OpenAI requires a reasoning level selection".to_string(),
                ));
            }
        };
        Ok(Self {
            effort,
            mode: None,
            context: None,
            store: true,
        })
    }

    pub(crate) fn for_request_context(mut self, context: ProviderRequestContextSnapshot) -> Self {
        self.context = match context {
            ProviderRequestContextSnapshot::FullHistory => None,
            ProviderRequestContextSnapshot::PreviousResponse => Some(ReasoningContext::AllTurns),
            ProviderRequestContextSnapshot::FullHistoryFallback => {
                Some(ReasoningContext::CurrentTurn)
            }
        };
        self
    }

    pub(crate) fn to_rig_reasoning(&self) -> Reasoning {
        let mut reasoning = Reasoning::new();
        if let Some(effort) = self.effort.clone() {
            reasoning = reasoning.with_effort(effort);
        }
        if let Some(mode) = self.mode.clone() {
            reasoning = reasoning.with_mode(mode);
        }
        if let Some(context) = self.context.clone() {
            reasoning = reasoning.with_context(context);
        }
        reasoning
    }

    pub(crate) fn merge_into_request_params(&self, parameters: Option<Value>) -> Result<Value> {
        let mut parameters = match parameters {
            Some(Value::Object(parameters)) => parameters,
            Some(_) => {
                return Err(AgentRuntimeError::Invariant(
                    "provider additional parameters must be a JSON object".to_string(),
                ));
            }
            None => serde_json::Map::new(),
        };
        parameters.insert(
            "reasoning".to_string(),
            serde_json::to_value(self.to_rig_reasoning())?,
        );
        parameters.insert("store".to_string(), Value::Bool(self.store));
        Ok(Value::Object(parameters))
    }

    #[allow(dead_code)]
    pub(crate) fn with_mode(mut self, mode: ReasoningMode) -> Self {
        self.mode = Some(mode);
        self
    }
}

fn reasoning_effort(value: &str) -> Option<ReasoningEffort> {
    match value {
        "none" => Some(ReasoningEffort::None),
        "minimal" => Some(ReasoningEffort::Minimal),
        "low" => Some(ReasoningEffort::Low),
        "medium" => Some(ReasoningEffort::Medium),
        "high" => Some(ReasoningEffort::High),
        "xhigh" => Some(ReasoningEffort::Xhigh),
        "max" => Some(ReasoningEffort::Max),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jaco_core::{
        ModelCapabilitiesSnapshot, ProviderCapabilityExtensionSnapshot, ProviderSettingsPayload,
        ToolApprovalMode, ToolApprovalPolicy, ToolPolicySnapshot,
    };
    use serde_json::json;

    fn settings(value: &str) -> RunSettingsSnapshot {
        RunSettingsSnapshot {
            prompt: None,
            provider_id: "provider".to_string(),
            model_id: "gpt-5.6".to_string(),
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
                structured_output: true,
                stateful_response_continuation: true,
                extension: ProviderCapabilityExtensionSnapshot::None,
            },
            provider_settings: ProviderSettingsPayload {
                provider_kind: "openai".to_string(),
                fields: Vec::new(),
            },
            reasoning_selection: Some(ReasoningSelectionSnapshot::Level {
                value: value.to_string(),
            }),
            tool_policy: ToolPolicySnapshot {
                approval_policy: ToolApprovalPolicy::Never,
                enabled_sources: Vec::new(),
                max_steps: 8,
                approval_mode: ToolApprovalMode::RequestApproval,
                permission_scope: None,
            },
        }
    }

    #[test]
    fn typed_policy_maps_gpt_5_6_effort_context_store_and_runtime_pro() {
        let policy = OpenAiReasoningPolicy::from_run_settings(&settings("max"))
            .unwrap()
            .for_request_context(ProviderRequestContextSnapshot::PreviousResponse)
            .with_mode(ReasoningMode::Pro);
        let value = policy.merge_into_request_params(None).unwrap();
        assert_eq!(
            value,
            json!({
                "reasoning": {
                    "effort": "max",
                    "mode": "pro",
                    "context": "all_turns"
                },
                "store": true
            })
        );
    }

    #[test]
    fn typed_policy_preserves_unmodeled_provider_parameters() {
        let value = OpenAiReasoningPolicy::from_run_settings(&settings("high"))
            .unwrap()
            .merge_into_request_params(Some(json!({
                "tools": [{"type": "web_search"}],
                "parallel_tool_calls": true
            })))
            .unwrap();
        assert_eq!(value["tools"][0]["type"], "web_search");
        assert_eq!(value["parallel_tool_calls"], true);
        assert_eq!(value["reasoning"]["effort"], "high");
        assert_eq!(value["store"], true);
    }

    #[test]
    fn typed_policy_preserves_minimal_for_older_gpt_5_models() {
        let mut settings = settings("minimal");
        settings.model_id = "gpt-5".to_string();

        let value = OpenAiReasoningPolicy::from_run_settings(&settings)
            .unwrap()
            .merge_into_request_params(None)
            .unwrap();

        assert_eq!(value["reasoning"]["effort"], "minimal");
        assert_eq!(value["store"], true);
    }
}

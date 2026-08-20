use super::*;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderUsageSnapshot {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_write_input_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_tokens: u64,
    pub metadata: Option<ProviderRawPayload>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderUsageCoverage {
    Unreported,
    Partial,
    Reported,
}

impl ProviderUsageSnapshot {
    pub fn coverage(&self) -> ProviderUsageCoverage {
        let has_detail = self.input_tokens > 0
            || self.output_tokens > 0
            || self.cached_input_tokens > 0
            || self.cache_write_input_tokens > 0
            || self.reasoning_tokens > 0;

        match (self.total_tokens, has_detail) {
            (0, false) => ProviderUsageCoverage::Unreported,
            (0, true) => ProviderUsageCoverage::Partial,
            (_, _) => ProviderUsageCoverage::Reported,
        }
    }

    pub fn cache_hit_rate(&self, provider_kind: &str) -> Option<f64> {
        if self.cached_input_tokens == 0 {
            return None;
        }

        let denominator = match provider_kind {
            "anthropic" => self
                .input_tokens
                .checked_add(self.cached_input_tokens)?
                .checked_add(self.cache_write_input_tokens)?,
            "openai" | "gemini" | "openrouter" | "deepseek" | "mistral" => self.input_tokens,
            _ => return None,
        };

        if denominator == 0 {
            return None;
        }

        Some(self.cached_input_tokens as f64 / denominator as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
                provider_kind: "ignored".to_string(),
                value: serde_json::json!({"ignored": true}),
            }),
        }
    }

    #[test]
    fn usage_snapshot_classifies_all_zero_as_unreported() {
        assert_eq!(
            usage(0, 0, 0, 0, 0, 0).coverage(),
            ProviderUsageCoverage::Unreported
        );
    }

    #[test]
    fn usage_snapshot_classifies_detail_without_total_as_partial() {
        assert_eq!(
            usage(24, 0, 0, 0, 0, 0).coverage(),
            ProviderUsageCoverage::Partial
        );
    }

    #[test]
    fn usage_snapshot_classifies_positive_total_as_reported() {
        assert_eq!(
            usage(24, 0, 0, 0, 0, 24).coverage(),
            ProviderUsageCoverage::Reported
        );
    }

    #[test]
    fn cache_hit_rate_uses_inclusive_input_provider_denominator() {
        let usage = usage(1_000, 100, 500, 0, 0, 1_600);

        assert_eq!(usage.cache_hit_rate("openai"), Some(0.5));
        assert_eq!(usage.cache_hit_rate("gemini"), Some(0.5));
        assert_eq!(usage.cache_hit_rate("openrouter"), Some(0.5));
        assert_eq!(usage.cache_hit_rate("deepseek"), Some(0.5));
        assert_eq!(usage.cache_hit_rate("mistral"), Some(0.5));
    }

    #[test]
    fn cache_hit_rate_uses_anthropic_total_input_denominator() {
        let usage = usage(1_000, 100, 500, 250, 0, 1_850);

        assert_eq!(usage.cache_hit_rate("anthropic"), Some(500.0 / 1_750.0));
    }

    #[test]
    fn cache_hit_rate_is_unknown_for_zero_unsupported_and_overflow() {
        assert_eq!(usage(0, 0, 0, 0, 0, 0).cache_hit_rate("openai"), None);
        assert_eq!(usage(1_000, 0, 500, 0, 0, 0).cache_hit_rate("ollama"), None);
        assert_eq!(
            usage(u64::MAX, 0, 1, u64::MAX, 0, 0).cache_hit_rate("anthropic"),
            None
        );
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum ShortcutAction {
    OpenTemporaryConversation,
    SendToConversation {
        conversation_id: Option<ConversationId>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolApprovalMode {
    AutoApprove,
    RequestApproval,
    FullAccess,
}

pub fn default_tool_approval_mode() -> ToolApprovalMode {
    ToolApprovalMode::RequestApproval
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolPermissionScopeSnapshot {
    pub project_roots: Vec<String>,
    pub external_read_requires_approval: bool,
    pub external_write_requires_approval: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolAccessKind {
    Read,
    Write,
    Execute,
    Network,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolAccessRequestPayload {
    pub kind: ToolAccessKind,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized_path: Option<String>,
    #[serde(default)]
    pub within_project: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolPolicySnapshot {
    pub approval_policy: ToolApprovalPolicy,
    pub enabled_sources: Vec<ToolSource>,
    pub max_steps: u32,
    #[serde(default = "default_tool_approval_mode")]
    pub approval_mode: ToolApprovalMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_scope: Option<ToolPermissionScopeSnapshot>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderSettingFieldValue {
    pub key: String,
    pub value: ProviderSettingValue,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum ProviderSettingValue {
    String { value: String },
    Bool { value: bool },
    Number { value: f64 },
    Object { value: ProviderRawPayload },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderSecretRef {
    pub key: String,
    pub storage: String,
    pub ref_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageInputCapabilitySnapshot {
    pub max_images: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileInputCapabilitySnapshot {
    pub max_files: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolCallingCapabilitySnapshot {
    pub parallel_tool_calls: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReasoningCapabilitySnapshot {
    pub default_effort: String,
    pub efforts: Vec<String>,
    pub summaries: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<ReasoningControlSnapshot>,
    #[serde(default = "default_capability_source")]
    pub source: CapabilitySourceSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum CapabilitySourceSnapshot {
    ApiDiscovered {
        provider: String,
        endpoint: String,
    },
    OfficialDocs {
        provider: String,
        url: String,
        checked_at: String,
    },
    Heuristic {
        reason: String,
    },
    Manual {
        source: String,
    },
    OpenRouterNormalized,
}

impl Default for CapabilitySourceSnapshot {
    fn default() -> Self {
        default_capability_source()
    }
}

fn default_capability_source() -> CapabilitySourceSnapshot {
    CapabilitySourceSnapshot::Heuristic {
        reason: "legacy capability payload".to_string(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum ReasoningControlSnapshot {
    None,
    Boolean {
        default_enabled: Option<bool>,
    },
    Levels {
        values: Vec<String>,
        default_value: Option<String>,
    },
    TokenBudget {
        min: Option<u32>,
        max: Option<u32>,
        default_value: Option<i32>,
        dynamic_supported: bool,
        off_supported: bool,
    },
    AdaptiveLevels {
        values: Vec<String>,
        default_value: Option<String>,
    },
    AlwaysOn {
        visible_summary_supported: bool,
    },
    Composite {
        controls: Vec<ReasoningControlSnapshot>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum ReasoningSelectionSnapshot {
    Boolean {
        enabled: bool,
    },
    Level {
        value: String,
    },
    TokenBudget {
        mode: TokenBudgetSelectionMode,
        value: Option<u32>,
    },
    Composite {
        selections: Vec<ReasoningSelectionSnapshot>,
    },
    AlwaysOn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenBudgetSelectionMode {
    Off,
    Dynamic,
    Custom,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "provider", rename_all = "camelCase", deny_unknown_fields)]
pub enum ProviderCapabilityExtensionSnapshot {
    None,
    OpenAi {
        responses_api: bool,
        raw: Option<ProviderRawPayload>,
    },
    Ollama {
        raw_capabilities: Vec<String>,
        family: String,
        #[serde(default)]
        families: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        thinking: Option<OllamaThinkingCapabilitySnapshot>,
        #[serde(default)]
        local_web_tools: bool,
        raw: Option<ProviderRawPayload>,
    },
    Gemini {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        thinking: Option<bool>,
        raw: Option<ProviderRawPayload>,
    },
    OpenRouter {
        #[serde(default)]
        supported_parameters: Vec<String>,
        raw: Option<ProviderRawPayload>,
    },
    Other {
        raw: ProviderRawPayload,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OllamaThinkingCapabilitySnapshot {
    Boolean,
    Levels,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderModelMetadata {
    pub display_name: Option<String>,
    pub family: Option<String>,
    pub raw: Option<ProviderRawPayload>,
}

mod preset;
mod provider;
mod run_persistence;
mod runner;
mod types;

pub(crate) use preset::{
    apply_ext_setting, build_request_template, ext_settings as preset_ext_settings,
};
#[cfg(test)]
pub(crate) use provider::ProviderModelsSuccess;
#[allow(unused_imports)]
pub(crate) use provider::{
    AvailableModelsBatch, CapabilityRequirement, ExtSettingControl, ExtSettingItem,
    ExtSettingOption, ModelCapabilities, OllamaModelCapabilities, OllamaProvider, OllamaSettings,
    OllamaThinkingCapability, OpenAIModelCapabilities, OpenAIProvider, OpenAISettings, Provider,
    ProviderCapabilityExtension, ProviderModel, ProviderModelsFailure, ProviderSettingsFieldKind,
    ProviderSettingsFieldSpec, ProviderSettingsSpec, ReasoningCapability, ReasoningEffort,
    available_models, provider_by_name, provider_is_configured, provider_names,
    provider_settings_specs,
};
pub(crate) use run_persistence::{
    ProviderRunPersistenceAccumulator, persisted_provider_settings_snapshot,
    provider_run_request_context_key,
};
pub(crate) use runner::ProviderRunRunner;
// Re-export the full provider-neutral item vocabulary for staged follow-up issues.
#[allow(unused_imports)]
pub(crate) use types::{
    LlmAttachmentRef, LlmContentPart, LlmHistoryMessage, LlmHostedToolCall, LlmInputItem,
    LlmMcpApprovalRequest, LlmOutputItem, LlmToolCall, LlmToolResult, ProviderRunEvent,
    ProviderRunRequest, ProviderRunState, ProviderUsage, build_input_items,
};

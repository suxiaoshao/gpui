#![allow(dead_code)]

use ai_chat_core::ProviderId;
use ai_chat_db::NewProviderModel;

use super::{capabilities::conservative_capabilities, catalog::ProviderKindKey};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ModelFetchSupport {
    Supported,
    ManualOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ModelFetchError {
    pub(super) message: String,
}

pub(super) fn fetch_support(kind: &ProviderKindKey) -> ModelFetchSupport {
    match kind.as_str() {
        "azure_openai"
        | "zai"
        | "xai"
        | "groq"
        | "perplexity"
        | "together"
        | "custom_openai_compatible" => ModelFetchSupport::ManualOnly,
        _ => ModelFetchSupport::Supported,
    }
}

pub(super) fn manual_model(
    provider_id: ProviderId,
    kind: &ProviderKindKey,
    model_id: String,
    display_name: Option<String>,
) -> NewProviderModel {
    NewProviderModel {
        provider_id,
        model_id,
        display_name: display_name.clone(),
        enabled: true,
        capabilities: conservative_capabilities(kind),
        metadata: ai_chat_core::ProviderModelMetadata {
            display_name,
            family: None,
            raw: None,
        },
    }
}

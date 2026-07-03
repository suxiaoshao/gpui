#![allow(dead_code)]

use ai_chat_core::ProviderId;
use ai_chat_db::NewProviderModel;

use super::{
    capabilities::conservative_capabilities,
    catalog::{ModelListingStrategy, ProviderKindKey},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ModelFetchSupport {
    Supported,
    ManualOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ModelFetchError {
    pub(super) message: String,
}

pub(super) fn fetch_support(strategy: ModelListingStrategy) -> ModelFetchSupport {
    match strategy {
        ModelListingStrategy::RigModels | ModelListingStrategy::OllamaTagsAndShow => {
            ModelFetchSupport::Supported
        }
        ModelListingStrategy::Manual => ModelFetchSupport::ManualOnly,
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

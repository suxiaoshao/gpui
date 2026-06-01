#![allow(dead_code)]

use crate::database;
use ai_chat_core::{ModelCapabilitiesSnapshot, ProviderId};
use ai_chat_db::{ProviderModelRecord, ProviderRecord};
use gpui::App;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ProviderModelChoice {
    pub(crate) provider_id: ProviderId,
    pub(crate) provider_kind: String,
    pub(crate) provider_display_name: String,
    pub(crate) model_id: String,
    pub(crate) model_display_name: Option<String>,
    pub(crate) capabilities: ModelCapabilitiesSnapshot,
}

pub(crate) fn providers_with_models(
    cx: &App,
) -> ai_chat_db::Result<Vec<(ProviderRecord, Vec<ProviderModelRecord>)>> {
    let repository = database::repository(cx);
    repository
        .list_providers()?
        .into_iter()
        .map(|provider| {
            let models = repository.list_provider_models(&provider.id)?;
            Ok((provider, models))
        })
        .collect()
}

pub(crate) fn enabled_provider_models(cx: &App) -> ai_chat_db::Result<Vec<ProviderModelChoice>> {
    Ok(providers_with_models(cx)?
        .into_iter()
        .filter(|(provider, _)| provider.enabled)
        .flat_map(|(provider, models)| {
            models
                .into_iter()
                .filter(|model| model.enabled)
                .map(move |model| ProviderModelChoice {
                    provider_id: provider.id.clone(),
                    provider_kind: provider.kind.clone(),
                    provider_display_name: provider.display_name.clone(),
                    model_id: model.model_id,
                    model_display_name: model.display_name,
                    capabilities: model.capabilities,
                })
        })
        .collect())
}

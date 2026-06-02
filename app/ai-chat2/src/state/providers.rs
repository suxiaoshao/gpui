#![allow(dead_code)]

use crate::database;
use ai_chat_core::{ModelCapabilitiesSnapshot, ProviderId, ProviderModelId};
use ai_chat_db::{ProviderModelRecord, ProviderRecord};
use gpui::App;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProviderModelKey {
    pub(crate) provider_id: ProviderId,
    pub(crate) model_id: ProviderModelId,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ProviderModelChoice {
    pub(crate) provider_id: ProviderId,
    pub(crate) provider_kind: String,
    pub(crate) provider_display_name: String,
    pub(crate) model_id: String,
    pub(crate) model_display_name: Option<String>,
    pub(crate) capabilities: ModelCapabilitiesSnapshot,
}

impl ProviderModelChoice {
    pub(crate) fn key(&self) -> ProviderModelKey {
        ProviderModelKey {
            provider_id: self.provider_id.clone(),
            model_id: self.model_id.clone(),
        }
    }

    pub(crate) fn display_label(&self) -> String {
        self.model_display_name
            .clone()
            .unwrap_or_else(|| self.model_id.clone())
    }
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

#[cfg(test)]
mod tests {
    use super::{ProviderModelChoice, ProviderModelKey};
    use ai_chat_core::conservative_model_capabilities;

    #[test]
    fn provider_model_choice_uses_provider_model_composite_key() {
        let choice = ProviderModelChoice {
            provider_id: "provider-1".to_string(),
            provider_kind: "openai".to_string(),
            provider_display_name: "OpenAI".to_string(),
            model_id: "gpt-5".to_string(),
            model_display_name: Some("GPT Five".to_string()),
            capabilities: conservative_model_capabilities("openai"),
        };

        assert_eq!(
            choice.key(),
            ProviderModelKey {
                provider_id: "provider-1".to_string(),
                model_id: "gpt-5".to_string(),
            }
        );
        assert_eq!(choice.display_label(), "GPT Five");

        let mut choice_without_display_name = choice.clone();
        choice_without_display_name.model_display_name = None;
        assert_eq!(choice_without_display_name.display_label(), "gpt-5");
    }
}

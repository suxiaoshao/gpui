#![allow(dead_code)]

use crate::database;
use ai_chat_core::{ModelCapabilitiesSnapshot, ProviderId, ProviderModelId};
use ai_chat_db::{
    NewProvider, NewProviderModel, ProviderModelRecord, ProviderRecord, UpdateProvider,
};
use gpui::{App, AppContext, Context, Entity, EventEmitter, Global};

#[derive(Clone)]
pub(crate) struct ProviderCatalogGlobal(Entity<ProviderCatalogStore>);

impl ProviderCatalogGlobal {
    pub(crate) fn entity(&self) -> Entity<ProviderCatalogStore> {
        self.0.clone()
    }
}

impl Global for ProviderCatalogGlobal {}

pub(crate) struct ProviderCatalogStore {
    revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ProviderCatalogEvent {
    Changed(ProviderCatalogChange),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ProviderCatalogChange {
    ProviderSaved {
        provider_id: ProviderId,
    },
    ModelsReplaced {
        provider_id: ProviderId,
    },
    ModelEnabledChanged {
        provider_id: ProviderId,
        model_id: ProviderModelId,
        enabled: bool,
    },
}

impl EventEmitter<ProviderCatalogEvent> for ProviderCatalogStore {}

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

impl ProviderCatalogStore {
    fn new() -> Self {
        Self { revision: 0 }
    }

    #[cfg(test)]
    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn update_provider(
        &mut self,
        provider_id: &ProviderId,
        input: UpdateProvider,
        cx: &mut Context<Self>,
    ) -> ai_chat_db::Result<ProviderRecord> {
        let provider = database::repository(cx).update_provider(provider_id, input)?;
        self.emit_changed(
            ProviderCatalogChange::ProviderSaved {
                provider_id: provider.id.clone(),
            },
            cx,
        );
        Ok(provider)
    }

    pub(crate) fn insert_provider_with_id(
        &mut self,
        provider_id: ProviderId,
        input: NewProvider,
        cx: &mut Context<Self>,
    ) -> ai_chat_db::Result<ProviderRecord> {
        let provider = database::repository(cx).insert_provider_with_id(provider_id, input)?;
        self.emit_changed(
            ProviderCatalogChange::ProviderSaved {
                provider_id: provider.id.clone(),
            },
            cx,
        );
        Ok(provider)
    }

    pub(crate) fn replace_fetched_provider_models(
        &mut self,
        provider_id: &ProviderId,
        models: Vec<NewProviderModel>,
        cx: &mut Context<Self>,
    ) -> ai_chat_db::Result<Vec<ProviderModelRecord>> {
        let models =
            database::repository(cx).replace_fetched_provider_models(provider_id, models)?;
        self.emit_changed(
            ProviderCatalogChange::ModelsReplaced {
                provider_id: provider_id.clone(),
            },
            cx,
        );
        Ok(models)
    }

    pub(crate) fn set_provider_model_enabled(
        &mut self,
        provider_id: &ProviderId,
        model_id: &ProviderModelId,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> ai_chat_db::Result<ProviderModelRecord> {
        let model =
            database::repository(cx).set_provider_model_enabled(provider_id, model_id, enabled)?;
        self.emit_changed(
            ProviderCatalogChange::ModelEnabledChanged {
                provider_id: model.provider_id.clone(),
                model_id: model.model_id.clone(),
                enabled: model.enabled,
            },
            cx,
        );
        Ok(model)
    }

    fn emit_changed(&mut self, change: ProviderCatalogChange, cx: &mut Context<Self>) {
        self.revision += 1;
        cx.emit(ProviderCatalogEvent::Changed(change));
        cx.notify();
    }
}

pub(crate) fn init(cx: &mut App) {
    let store = cx.new(|_| ProviderCatalogStore::new());
    cx.set_global(ProviderCatalogGlobal(store));
}

pub(crate) fn catalog(cx: &App) -> Entity<ProviderCatalogStore> {
    cx.global::<ProviderCatalogGlobal>().entity()
}

pub(crate) fn update_provider(
    cx: &mut App,
    provider_id: &ProviderId,
    input: UpdateProvider,
) -> ai_chat_db::Result<ProviderRecord> {
    catalog(cx).update(cx, |catalog, cx| {
        catalog.update_provider(provider_id, input, cx)
    })
}

pub(crate) fn insert_provider_with_id(
    cx: &mut App,
    provider_id: ProviderId,
    input: NewProvider,
) -> ai_chat_db::Result<ProviderRecord> {
    catalog(cx).update(cx, |catalog, cx| {
        catalog.insert_provider_with_id(provider_id, input, cx)
    })
}

pub(crate) fn replace_fetched_provider_models(
    cx: &mut App,
    provider_id: &ProviderId,
    models: Vec<NewProviderModel>,
) -> ai_chat_db::Result<Vec<ProviderModelRecord>> {
    catalog(cx).update(cx, |catalog, cx| {
        catalog.replace_fetched_provider_models(provider_id, models, cx)
    })
}

pub(crate) fn set_provider_model_enabled(
    cx: &mut App,
    provider_id: &ProviderId,
    model_id: &ProviderModelId,
    enabled: bool,
) -> ai_chat_db::Result<ProviderModelRecord> {
    catalog(cx).update(cx, |catalog, cx| {
        catalog.set_provider_model_enabled(provider_id, model_id, enabled, cx)
    })
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

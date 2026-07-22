use crate::database;
use gpui::{App, Global};
use gpui_store::{SharedStore, StoreState};
use jaco_core::{ModelCapabilitiesSnapshot, ProviderId, ProviderModelId};
use jaco_db::{NewProvider, NewProviderModel, ProviderModelRecord, ProviderRecord, UpdateProvider};

#[derive(Clone)]
pub(crate) struct ProviderCatalogGlobal(SharedStore<ProviderCatalogSnapshot>);

impl ProviderCatalogGlobal {
    pub(crate) fn store(&self) -> SharedStore<ProviderCatalogSnapshot> {
        self.0.clone()
    }
}

impl Global for ProviderCatalogGlobal {}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ProviderCatalogSnapshot {
    pub(crate) providers: Vec<(ProviderRecord, Vec<ProviderModelRecord>)>,
    pub(crate) enabled_models: Vec<ProviderModelChoice>,
}

impl StoreState for ProviderCatalogSnapshot {}

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

fn refresh_snapshot(store: &SharedStore<ProviderCatalogSnapshot>, cx: &mut App) {
    let Ok(snapshot) = load_catalog_snapshot(cx) else {
        return;
    };
    store.update(cx, |current| {
        *current = snapshot;
    });
}

fn update_provider_impl(
    provider_id: &ProviderId,
    input: UpdateProvider,
    cx: &mut App,
) -> jaco_db::Result<ProviderRecord> {
    let provider = database::repository(cx).update_provider(provider_id, input)?;
    refresh_snapshot(&catalog(cx), cx);
    Ok(provider)
}

fn insert_provider_with_id_impl(
    provider_id: ProviderId,
    input: NewProvider,
    cx: &mut App,
) -> jaco_db::Result<ProviderRecord> {
    let provider = database::repository(cx).insert_provider_with_id(provider_id, input)?;
    refresh_snapshot(&catalog(cx), cx);
    Ok(provider)
}

fn replace_fetched_provider_models_impl(
    provider_id: &ProviderId,
    models: Vec<NewProviderModel>,
    cx: &mut App,
) -> jaco_db::Result<Vec<ProviderModelRecord>> {
    let models = database::repository(cx).replace_fetched_provider_models(provider_id, models)?;
    refresh_snapshot(&catalog(cx), cx);
    Ok(models)
}

fn set_provider_model_enabled_impl(
    provider_id: &ProviderId,
    model_id: &ProviderModelId,
    enabled: bool,
    cx: &mut App,
) -> jaco_db::Result<ProviderModelRecord> {
    let model =
        database::repository(cx).set_provider_model_enabled(provider_id, model_id, enabled)?;
    refresh_snapshot(&catalog(cx), cx);
    Ok(model)
}

pub(crate) fn init(cx: &mut App) {
    let snapshot = load_catalog_snapshot(cx).unwrap_or_default();
    let store = SharedStore::new(cx, snapshot);
    cx.set_global(ProviderCatalogGlobal(store));
}

pub(crate) fn catalog(cx: &App) -> SharedStore<ProviderCatalogSnapshot> {
    cx.global::<ProviderCatalogGlobal>().store()
}

pub(crate) fn update_provider(
    cx: &mut App,
    provider_id: &ProviderId,
    input: UpdateProvider,
) -> jaco_db::Result<ProviderRecord> {
    update_provider_impl(provider_id, input, cx)
}

pub(crate) fn insert_provider_with_id(
    cx: &mut App,
    provider_id: ProviderId,
    input: NewProvider,
) -> jaco_db::Result<ProviderRecord> {
    insert_provider_with_id_impl(provider_id, input, cx)
}

pub(crate) fn replace_fetched_provider_models(
    cx: &mut App,
    provider_id: &ProviderId,
    models: Vec<NewProviderModel>,
) -> jaco_db::Result<Vec<ProviderModelRecord>> {
    replace_fetched_provider_models_impl(provider_id, models, cx)
}

pub(crate) fn set_provider_model_enabled(
    cx: &mut App,
    provider_id: &ProviderId,
    model_id: &ProviderModelId,
    enabled: bool,
) -> jaco_db::Result<ProviderModelRecord> {
    set_provider_model_enabled_impl(provider_id, model_id, enabled, cx)
}

pub(crate) fn providers_with_models(
    cx: &App,
) -> jaco_db::Result<Vec<(ProviderRecord, Vec<ProviderModelRecord>)>> {
    if cx.has_global::<ProviderCatalogGlobal>() {
        return Ok(catalog(cx).read_cloned(cx, |snapshot| &snapshot.providers));
    }
    query_providers_with_models(cx)
}

fn query_providers_with_models(
    cx: &App,
) -> jaco_db::Result<Vec<(ProviderRecord, Vec<ProviderModelRecord>)>> {
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

pub(crate) fn enabled_provider_models(cx: &App) -> jaco_db::Result<Vec<ProviderModelChoice>> {
    if cx.has_global::<ProviderCatalogGlobal>() {
        return Ok(catalog(cx).read_cloned(cx, |snapshot| &snapshot.enabled_models));
    }
    Ok(query_providers_with_models(cx)?
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

fn load_catalog_snapshot(cx: &App) -> jaco_db::Result<ProviderCatalogSnapshot> {
    let providers = query_providers_with_models(cx)?;
    let enabled_models = providers
        .iter()
        .filter(|(provider, _)| provider.enabled)
        .flat_map(|(provider, models)| {
            models
                .iter()
                .filter(|model| model.enabled)
                .map(move |model| ProviderModelChoice {
                    provider_id: provider.id.clone(),
                    provider_kind: provider.kind.clone(),
                    provider_display_name: provider.display_name.clone(),
                    model_id: model.model_id.clone(),
                    model_display_name: model.display_name.clone(),
                    capabilities: model.capabilities.clone(),
                })
        })
        .collect();
    Ok(ProviderCatalogSnapshot {
        providers,
        enabled_models,
    })
}

#[cfg(test)]
mod tests {
    use super::{ProviderModelChoice, ProviderModelKey};
    use jaco_core::conservative_model_capabilities;

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

use std::fmt;

use gpui::{App, Task};
use gpui_operation::{Complete, Load, Refresh, Retry, Transition, refresh};
use gpui_store::Store;
use jaco_core::{ModelCapabilitiesSnapshot, ProviderId, ProviderModelId};
use jaco_db::{
    DbError, NewProvider, NewProviderModel, ProviderModelRecord, ProviderRecord, UpdateProvider,
};
use tokio::sync::oneshot;

use crate::{database, state::session::CatalogMutation};

pub(crate) type ProviderOperation = refresh::Operation<ProviderData, ProviderProblem, Task<()>>;
pub(crate) type ProviderStore = Store<ProviderOperation>;

#[derive(Debug)]
pub(crate) struct ProviderProblem(DbError);

impl fmt::Display for ProviderProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for ProviderProblem {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ProviderData {
    pub(crate) providers: Vec<(ProviderRecord, Vec<ProviderModelRecord>)>,
    pub(crate) enabled_models: Vec<ProviderModelChoice>,
}

impl ProviderData {
    pub(crate) fn providers(&self) -> &[(ProviderRecord, Vec<ProviderModelRecord>)] {
        &self.providers
    }

    fn new(mut providers: Vec<(ProviderRecord, Vec<ProviderModelRecord>)>) -> Self {
        sort_providers(&mut providers);
        let mut data = Self {
            providers,
            enabled_models: Vec::new(),
        };
        data.rebuild_enabled_models();
        data
    }

    fn rebuild_enabled_models(&mut self) {
        self.enabled_models = self
            .providers
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
    }

    fn upsert_provider(&mut self, provider: ProviderRecord) {
        match self
            .providers
            .iter_mut()
            .find(|(current, _)| current.id == provider.id)
        {
            Some((current, _)) => *current = provider,
            None => self.providers.push((provider, Vec::new())),
        }
        sort_providers(&mut self.providers);
        self.rebuild_enabled_models();
    }

    fn replace_models(&mut self, provider_id: &ProviderId, mut models: Vec<ProviderModelRecord>) {
        sort_models(&mut models);
        if let Some((_, current)) = self
            .providers
            .iter_mut()
            .find(|(provider, _)| &provider.id == provider_id)
        {
            *current = models;
        }
        self.rebuild_enabled_models();
    }

    fn upsert_model(&mut self, model: ProviderModelRecord) {
        if let Some((_, models)) = self
            .providers
            .iter_mut()
            .find(|(provider, _)| provider.id == model.provider_id)
        {
            match models.iter_mut().find(|current| current.id == model.id) {
                Some(current) => *current = model,
                None => models.push(model),
            }
            sort_models(models);
        }
        self.rebuild_enabled_models();
    }
}

pub(crate) enum ProviderMessage {
    UpsertProvider(ProviderRecord),
    ReplaceModels {
        provider_id: ProviderId,
        models: Vec<ProviderModelRecord>,
    },
    UpsertModel(Box<ProviderModelRecord>),
}

impl Transition<ProviderMessage> for &mut ProviderData {
    type Output = ();

    fn transition(self, message: ProviderMessage) {
        match message {
            ProviderMessage::UpsertProvider(provider) => self.upsert_provider(provider),
            ProviderMessage::ReplaceModels {
                provider_id,
                models,
            } => self.replace_models(&provider_id, models),
            ProviderMessage::UpsertModel(model) => self.upsert_model(*model),
        }
    }
}

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

fn sort_providers(providers: &mut [(ProviderRecord, Vec<ProviderModelRecord>)]) {
    for (_, models) in providers.iter_mut() {
        sort_models(models);
    }
    providers.sort_by(|(left, _), (right, _)| {
        left.display_name
            .cmp(&right.display_name)
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.id.cmp(&right.id))
    });
}

fn sort_models(models: &mut [ProviderModelRecord]) {
    models.sort_by(|left, right| {
        left.display_name
            .cmp(&right.display_name)
            .then_with(|| left.model_id.cmp(&right.model_id))
            .then_with(|| left.id.cmp(&right.id))
    });
}

pub(crate) fn init(cx: &mut App) {
    ProviderStore::install_global(cx, ProviderOperation::new());
    let Some(binding) = database::ready_binding(cx) else {
        return;
    };
    let Ok(executor) = database::ready_executor(cx) else {
        return;
    };
    let task = cx.spawn(async move |cx| {
        let result = executor
            .execute(|repository| {
                let providers = repository
                    .list_providers()?
                    .into_iter()
                    .map(|provider| {
                        let models = repository.list_provider_models(&provider.id)?;
                        Ok((provider, models))
                    })
                    .collect::<jaco_db::Result<Vec<_>>>()?;
                Ok(ProviderData::new(providers))
            })
            .await
            .map_err(ProviderProblem);
        cx.update(|cx| {
            if database::ready_binding(cx).as_ref() != Some(&binding) {
                return;
            }
            catalog(cx).update(cx, |operation| {
                if matches!(operation, ProviderOperation::Loading(_)) {
                    operation.transition(Complete(result));
                }
            });
        });
    });
    catalog(cx).update(cx, |operation| operation.transition(Load(task)));
}

pub(crate) fn catalog(cx: &impl gpui::AppContext) -> ProviderStore {
    ProviderStore::global(cx)
}

pub(crate) fn request_refresh(cx: &mut App) {
    let Some(binding) = database::ready_binding(cx) else {
        return;
    };
    let Ok(executor) = database::ready_executor(cx) else {
        return;
    };
    if catalog(cx).read(cx, ProviderOperation::is_running) {
        return;
    }
    let task = cx.spawn(async move |cx| {
        let result = executor
            .execute(|repository| {
                let providers = repository
                    .list_providers()?
                    .into_iter()
                    .map(|provider| {
                        let models = repository.list_provider_models(&provider.id)?;
                        Ok((provider, models))
                    })
                    .collect::<jaco_db::Result<Vec<_>>>()?;
                Ok(ProviderData::new(providers))
            })
            .await
            .map_err(ProviderProblem);
        cx.update(|cx| {
            if database::ready_binding(cx).as_ref() != Some(&binding) {
                return;
            }
            catalog(cx).update(cx, |operation| {
                if operation.is_running() {
                    operation.transition(Complete(result));
                }
            });
        });
    });
    catalog(cx).update(cx, |operation| match operation {
        ProviderOperation::Idle(_) => operation.transition(Load(task)),
        ProviderOperation::Ready(_) | ProviderOperation::Degraded(_) => {
            operation.transition(Refresh(task))
        }
        ProviderOperation::Unavailable(_) => operation.transition(Retry(task)),
        ProviderOperation::Loading(_)
        | ProviderOperation::Refreshing(_)
        | ProviderOperation::Retrying(_)
        | ProviderOperation::RefreshingDegraded(_) => {}
    });
}

fn apply(message: ProviderMessage, cx: &mut App) {
    catalog(cx).update(cx, |operation| {
        let ProviderOperation::Ready(ready) = operation else {
            panic!("provider commit requires an exact Ready operation");
        };
        ready.transition(message);
    });
}

fn ensure_ready(cx: &App) -> jaco_db::Result<()> {
    catalog(cx).read(cx, |operation| {
        matches!(operation, ProviderOperation::Ready(_))
            .then_some(())
            .ok_or_else(|| DbError::Invariant("provider resource is not ready".to_string()))
    })
}

pub(crate) fn update_provider(
    cx: &mut App,
    provider_id: ProviderId,
    input: UpdateProvider,
) -> Task<jaco_db::Result<ProviderRecord>> {
    spawn_provider_mutation(
        cx,
        move |repo| repo.update_provider(&provider_id, input),
        |record| ProviderMessage::UpsertProvider(record.clone()),
    )
}

pub(crate) fn insert_provider_with_id(
    cx: &mut App,
    provider_id: ProviderId,
    input: NewProvider,
) -> Task<jaco_db::Result<ProviderRecord>> {
    spawn_provider_mutation(
        cx,
        move |repo| repo.insert_provider_with_id(provider_id, input),
        |record| ProviderMessage::UpsertProvider(record.clone()),
    )
}

pub(crate) fn replace_fetched_provider_models(
    cx: &mut App,
    provider_id: ProviderId,
    models: Vec<NewProviderModel>,
) -> Task<jaco_db::Result<Vec<ProviderModelRecord>>> {
    let message_provider_id = provider_id.clone();
    spawn_provider_mutation(
        cx,
        move |repo| repo.replace_fetched_provider_models(&provider_id, models),
        move |models| ProviderMessage::ReplaceModels {
            provider_id: message_provider_id.clone(),
            models: models.clone(),
        },
    )
}

pub(crate) fn set_provider_model_enabled(
    cx: &mut App,
    provider_id: ProviderId,
    model_id: ProviderModelId,
    enabled: bool,
) -> Task<jaco_db::Result<ProviderModelRecord>> {
    spawn_provider_mutation(
        cx,
        move |repo| repo.set_provider_model_enabled(&provider_id, &model_id, enabled),
        |record| ProviderMessage::UpsertModel(Box::new(record.clone())),
    )
}

fn spawn_provider_mutation<R>(
    cx: &mut App,
    command: impl FnOnce(&jaco_db::FreshRepository) -> jaco_db::Result<R> + Send + 'static,
    message: impl FnOnce(&R) -> ProviderMessage + Send + 'static,
) -> Task<jaco_db::Result<R>>
where
    R: Send + 'static,
{
    if let Err(error) = ensure_ready(cx) {
        return Task::ready(Err(error));
    }
    let Some(binding) = database::ready_binding(cx) else {
        return Task::ready(Err(DbError::Invariant(
            "provider mutation requires an exact Ready session".to_string(),
        )));
    };
    let executor = match database::ready_executor(cx) {
        Ok(executor) => executor,
        Err(error) => return Task::ready(Err(error)),
    };
    let (sender, receiver) = oneshot::channel();
    cx.spawn(async move |cx| {
        let result = executor.mutate(CatalogMutation::Provider, command).await;
        if let Ok(value) = &result {
            let message = message(value);
            cx.update(|cx| {
                if database::ready_binding(cx).as_ref() != Some(&binding) {
                    return;
                }
                apply(message, cx);
            });
        }
        let _ = sender.send(result);
    })
    .detach();
    cx.spawn(async move |_| {
        receiver.await.unwrap_or_else(|_| {
            Err(DbError::Invariant(
                "provider mutation driver ended without a result".to_string(),
            ))
        })
    })
}

pub(crate) fn providers_with_models(
    cx: &App,
) -> jaco_db::Result<Vec<(ProviderRecord, Vec<ProviderModelRecord>)>> {
    catalog(cx).read(cx, |operation| {
        operation
            .data()
            .map(|data| data.providers.clone())
            .ok_or_else(|| DbError::Invariant("provider resource is not ready".to_string()))
    })
}

pub(crate) fn enabled_provider_models(cx: &App) -> jaco_db::Result<Vec<ProviderModelChoice>> {
    catalog(cx).read(cx, |operation| {
        operation
            .data()
            .map(|data| data.enabled_models.clone())
            .ok_or_else(|| DbError::Invariant("provider resource is not ready".to_string()))
    })
}

pub(crate) fn ready_provider(
    provider_id: &ProviderId,
    cx: &impl gpui::AppContext,
) -> jaco_db::Result<ProviderRecord> {
    catalog(cx).read(cx, |operation| match operation {
        ProviderOperation::Ready(ready) => ready
            .data()
            .providers
            .iter()
            .find(|(provider, _)| &provider.id == provider_id)
            .map(|(provider, _)| provider.clone())
            .ok_or_else(|| DbError::Invariant(format!("provider `{provider_id}` was not found"))),
        _ => Err(DbError::Invariant(
            "provider resource must be exactly Ready".to_string(),
        )),
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

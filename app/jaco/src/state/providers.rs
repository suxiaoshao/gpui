pub(crate) mod secrets;

use std::fmt;

use gpui::{App, AppContext, Entity, Global, Subscription, Task};
use gpui_operation::{Cancel, Complete, Load, Refresh, Retry, Transition, refresh};
use gpui_store::{Select, Store};
use jaco_core::{ModelCapabilitiesSnapshot, ProviderId, ProviderModelId};
use jaco_db::{
    DbError, NewProvider, NewProviderModel, ProviderModelRecord, ProviderRecord, UpdateProvider,
};
use tokio::sync::oneshot;

use crate::database;

pub(crate) type ProviderOperation = refresh::Operation<ProviderData, ProviderProblem, Task<()>>;
pub(crate) type ProviderStore = Store<ProviderOperation>;
pub(crate) type ProviderWithModels = (ProviderRecord, Vec<ProviderModelRecord>);

struct ProviderDatabaseOwner {
    mutation_tasks: Vec<Task<()>>,
    _database_subscription: Subscription,
}

struct ProviderDatabaseOwnerGlobal(Entity<ProviderDatabaseOwner>);

impl Global for ProviderDatabaseOwnerGlobal {}

#[derive(Clone, Copy, Default)]
pub(crate) struct SelectProviderRecordsWithModels;

impl Select<ProviderOperation> for SelectProviderRecordsWithModels {
    type Output = Option<Vec<ProviderWithModels>>;

    fn select(&self, operation: &ProviderOperation) -> Self::Output {
        operation.data().map(|data| data.providers.clone())
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SelectProviderStatus;

impl Select<ProviderOperation> for SelectProviderStatus {
    type Output = (gpui_operation::refresh::Phase, Option<String>);

    fn select(&self, operation: &ProviderOperation) -> Self::Output {
        (
            operation.phase(),
            operation.problem().map(ToString::to_string),
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ProviderModelCatalogSnapshot {
    models: Option<Vec<ProviderModelChoice>>,
    phase: gpui_operation::refresh::Phase,
    problem: Option<String>,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SelectProviderModelCatalog;

impl Select<ProviderOperation> for SelectProviderModelCatalog {
    type Output = ProviderModelCatalogSnapshot;

    fn select(&self, operation: &ProviderOperation) -> Self::Output {
        ProviderModelCatalogSnapshot {
            models: operation.data().map(|data| data.enabled_models.clone()),
            phase: operation.phase(),
            problem: operation.problem().map(ToString::to_string),
        }
    }
}

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
    let owner = cx.new(|cx| {
        let subscription = database::store(cx).observe_select(
            cx,
            database::SelectDatabaseReady,
            |owner: &mut ProviderDatabaseOwner, ready, cx| owner.sync(*ready, cx),
        );
        ProviderDatabaseOwner {
            mutation_tasks: Vec::new(),
            _database_subscription: subscription,
        }
    });
    cx.set_global(ProviderDatabaseOwnerGlobal(owner.clone()));
    let ready = database::is_ready(cx);
    owner.update(cx, |owner, cx| owner.sync(ready, cx));
}

impl ProviderDatabaseOwner {
    fn sync(&mut self, ready: bool, cx: &mut App) {
        if ready {
            request_refresh(cx);
        } else {
            self.mutation_tasks.clear();
            catalog(cx).update(cx, |operation| operation.transition(Cancel));
        }
    }

    fn retain(&mut self, task: Task<()>) {
        self.mutation_tasks.retain(|task| !task.is_ready());
        self.mutation_tasks.push(task);
    }
}

fn load_task(cx: &mut App) -> Option<Task<()>> {
    let executor = database::ready_executor(cx).ok()?;
    Some(cx.spawn(async move |cx| {
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
            catalog(cx).update(cx, |operation| {
                if operation.is_running() {
                    operation.transition(Complete(result));
                }
            });
        });
    }))
}

pub(crate) fn catalog(cx: &impl gpui::AppContext) -> ProviderStore {
    ProviderStore::global(cx)
}

pub(crate) fn request_refresh(cx: &mut App) {
    if !database::is_ready(cx) {
        return;
    }
    if catalog(cx).read(cx, ProviderOperation::is_running) {
        return;
    }
    let Some(task) = load_task(cx) else {
        return;
    };
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
    let executor = match database::ready_executor(cx) {
        Ok(executor) => executor,
        Err(error) => return Task::ready(Err(error)),
    };
    let (sender, receiver) = oneshot::channel();
    let driver = cx.spawn(async move |cx| {
        let result = executor.execute(command).await;
        if let Ok(value) = &result {
            let message = message(value);
            cx.update(|cx| {
                if database::is_ready(cx) {
                    apply(message, cx);
                }
            });
        }
        let _ = sender.send(result);
    });
    if cx.has_global::<ProviderDatabaseOwnerGlobal>() {
        let owner = cx.global::<ProviderDatabaseOwnerGlobal>().0.clone();
        owner.update(cx, |owner, _| owner.retain(driver));
    } else {
        crate::app::tasks::retain_application(driver, cx);
    }
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
    use super::{
        ProviderData, ProviderDatabaseOwnerGlobal, ProviderModelChoice, ProviderModelKey,
        ProviderOperation, catalog, init,
    };
    use crate::database;
    use jaco_core::{
        CapabilitySourceSnapshot, ContextWindowCapabilitySnapshot, ProviderModelMetadata,
        ProviderSecretRefs, ProviderSettingsPayload, conservative_model_capabilities,
    };
    use jaco_db::{ProviderModelRecord, ProviderRecord};
    use std::num::NonZeroU64;
    use time::OffsetDateTime;

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

    #[test]
    fn old_cached_openai_model_stays_unknown_until_provider_refresh() {
        let capabilities = conservative_model_capabilities("openai");
        let data = ProviderData::new(vec![(
            provider_record("openai"),
            vec![provider_model_record("gpt-5.6-sol", capabilities.clone())],
        )]);

        assert_eq!(data.providers[0].1[0].capabilities, capabilities);
        assert_eq!(data.enabled_models.len(), 1);
        assert_eq!(data.enabled_models[0].capabilities, capabilities);
        assert!(data.enabled_models[0].capabilities.context_window.is_none());
    }

    #[test]
    fn enabled_choice_preserves_existing_context_window_provenance() {
        let mut capabilities = conservative_model_capabilities("openai");
        capabilities.context_window = Some(ContextWindowCapabilitySnapshot {
            tokens: NonZeroU64::new(64_000).unwrap(),
            source: CapabilitySourceSnapshot::Manual {
                source: "test".to_string(),
            },
        });
        let data = ProviderData::new(vec![(
            provider_record("openai"),
            vec![provider_model_record("gpt-5.6-sol", capabilities.clone())],
        )]);

        assert_eq!(data.providers[0].1[0].capabilities, capabilities);
        assert_eq!(
            data.enabled_models[0].capabilities.context_window,
            capabilities.context_window
        );
    }

    fn provider_record(kind: &str) -> ProviderRecord {
        ProviderRecord {
            id: "provider".to_string(),
            kind: kind.to_string(),
            display_name: kind.to_string(),
            enabled: true,
            settings: ProviderSettingsPayload {
                provider_kind: kind.to_string(),
                fields: Vec::new(),
            },
            secret_refs: ProviderSecretRefs { refs: Vec::new() },
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    fn provider_model_record(
        model_id: &str,
        capabilities: jaco_core::ModelCapabilitiesSnapshot,
    ) -> ProviderModelRecord {
        ProviderModelRecord {
            id: "model".to_string(),
            provider_id: "provider".to_string(),
            model_id: model_id.to_string(),
            display_name: None,
            enabled: true,
            capabilities,
            pricing: None,
            metadata: ProviderModelMetadata {
                display_name: None,
                family: None,
                raw: None,
            },
            fetched_at: OffsetDateTime::UNIX_EPOCH,
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    #[gpui::test]
    fn losing_database_readiness_cancels_the_provider_load(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        cx.update(|cx| {
            database::install_for_test(cx, dir.path());
            init(cx);
            assert!(matches!(
                catalog(cx).read(cx, |operation| operation.phase()),
                gpui_operation::refresh::Phase::Loading
            ));

            let owner = cx.global::<ProviderDatabaseOwnerGlobal>().0.clone();
            owner.update(cx, |owner, cx| owner.sync(false, cx));

            assert!(catalog(cx).read(cx, |operation| matches!(
                operation,
                ProviderOperation::Idle(_)
            )));
        });
    }
}

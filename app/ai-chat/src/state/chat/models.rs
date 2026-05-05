use crate::{
    llm::{AvailableModelsBatch, ProviderModel, ProviderModelsFailure, available_models},
    state::AiChatConfig,
};
use async_compat::CompatExt;
use gpui::*;
use std::{collections::HashSet, ops::Deref, time::Duration};
use tracing::{Level, event};

const MODEL_RELOAD_DEBOUNCE: Duration = Duration::from_millis(600);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ModelStoreStatus {
    Idle,
    InitialLoading,
    Refreshing,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ModelStoreSnapshot {
    pub(crate) models: Vec<ProviderModel>,
    pub(crate) status: Option<ModelStoreStatus>,
    pub(crate) failures: Vec<ProviderModelsFailure>,
}

pub(crate) struct ModelStoreState {
    models: Vec<ProviderModel>,
    status: ModelStoreStatus,
    loaded_once: bool,
    last_config_fingerprint: Option<String>,
    last_failures: Vec<ProviderModelsFailure>,
    request_version: u64,
    load_task: Option<Task<()>>,
    debounced_reload_task: Option<Task<()>>,
}

impl Default for ModelStoreState {
    fn default() -> Self {
        Self {
            models: Vec::new(),
            status: ModelStoreStatus::Idle,
            loaded_once: false,
            last_config_fingerprint: None,
            last_failures: Vec::new(),
            request_version: 0,
            load_task: None,
            debounced_reload_task: None,
        }
    }
}

impl ModelStoreState {
    pub(crate) fn snapshot(&self) -> ModelStoreSnapshot {
        ModelStoreSnapshot {
            models: self.models.clone(),
            status: Some(self.status),
            failures: self.last_failures.clone(),
        }
    }

    pub(crate) fn ensure_loaded(&mut self, cx: &mut Context<Self>) {
        self.refresh(false, cx);
    }

    pub(crate) fn reload(&mut self, cx: &mut Context<Self>) {
        self.cancel_debounced_reload();
        self.refresh(true, cx);
    }

    pub(crate) fn schedule_reload(&mut self, cx: &mut Context<Self>) {
        self.cancel_debounced_reload();
        self.debounced_reload_task = Some(cx.spawn(async move |state, cx| {
            smol::Timer::after(MODEL_RELOAD_DEBOUNCE).await;
            let _ = state.update(cx, |this, cx| {
                this.refresh(true, cx);
            });
        }));
    }

    fn cancel_debounced_reload(&mut self) -> bool {
        self.debounced_reload_task.take().is_some()
    }

    fn prepare_running_task(&mut self, force: bool) -> bool {
        if self.load_task.is_some() && !force {
            event!(
                Level::INFO,
                status = ?self.status,
                request_version = self.request_version,
                "model store load skipped: request already running"
            );
            return false;
        }
        if force && self.load_task.take().is_some() {
            event!(
                Level::INFO,
                request_version = self.request_version,
                "model store load task cancelled for forced reload"
            );
        }
        true
    }

    fn apply_load_result(
        &mut self,
        request_version: u64,
        fingerprint: String,
        batch: &AvailableModelsBatch,
    ) -> bool {
        if self.request_version != request_version {
            event!(
                Level::INFO,
                request_version,
                current_request_version = self.request_version,
                "model store load result ignored: stale request"
            );
            return false;
        }

        self.status = ModelStoreStatus::Idle;
        self.models = merge_models(&self.models, batch);
        self.loaded_once = true;
        self.last_config_fingerprint = Some(fingerprint);
        self.last_failures = batch.failures.clone();
        self.load_task = None;
        true
    }

    fn refresh(&mut self, force: bool, cx: &mut Context<Self>) {
        if !self.prepare_running_task(force) {
            return;
        }

        let config = cx.global::<AiChatConfig>().clone();
        let fingerprint = match config.model_settings_fingerprint() {
            Ok(fingerprint) => fingerprint,
            Err(err) => {
                event!(
                    Level::ERROR,
                    "build model settings fingerprint failed: {}",
                    err
                );
                return;
            }
        };

        if !force
            && self.loaded_once
            && self.last_config_fingerprint.as_deref() == Some(fingerprint.as_str())
        {
            event!(
                Level::INFO,
                loaded_once = self.loaded_once,
                models_count = self.models.len(),
                request_version = self.request_version,
                "model store load skipped: settings unchanged"
            );
            return;
        }

        let previous_status = self.status;
        let previous_models_count = self.models.len();
        self.request_version = self.request_version.saturating_add(1);
        let request_version = self.request_version;
        self.status = if self.loaded_once {
            ModelStoreStatus::Refreshing
        } else {
            ModelStoreStatus::InitialLoading
        };
        event!(
            Level::INFO,
            force,
            request_version,
            previous_status = ?previous_status,
            next_status = ?self.status,
            loaded_once = self.loaded_once,
            previous_models_count,
            "model store load started"
        );
        cx.notify();

        self.load_task = Some(cx.spawn(async move |state, cx| {
            let batch = available_models(config).compat().await;
            let _ = state.update(cx, move |this, cx| {
                if !this.apply_load_result(request_version, fingerprint, &batch) {
                    return;
                }
                event!(
                    Level::INFO,
                    request_version,
                    loaded_once = this.loaded_once,
                    success_providers = batch.successes.len(),
                    failed_providers = batch.failures.len(),
                    final_models_count = this.models.len(),
                    "model store load completed"
                );

                for failure in &batch.failures {
                    event!(
                        Level::ERROR,
                        provider = %failure.provider_name,
                        error = %failure.message,
                        "load provider models failed"
                    );
                }
                cx.notify();
            });
        }));
    }
}

pub(crate) struct ModelStore {
    data: Entity<ModelStoreState>,
}

impl Deref for ModelStore {
    type Target = Entity<ModelStoreState>;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl Global for ModelStore {}

pub(crate) fn init_global(cx: &mut App) {
    let data = cx.new(|_| ModelStoreState::default());
    cx.set_global(ModelStore { data: data.clone() });
    data.update(cx, |store, cx| store.ensure_loaded(cx));
}

pub(crate) fn reload_models(cx: &mut App) {
    if !cx.has_global::<ModelStore>() {
        return;
    }
    let data = cx.global::<ModelStore>().deref().clone();
    data.update(cx, |store, cx| store.reload(cx));
}

pub(crate) fn reload_models_debounced(cx: &mut App) {
    if !cx.has_global::<ModelStore>() {
        return;
    }
    let data = cx.global::<ModelStore>().deref().clone();
    data.update(cx, |store, cx| store.schedule_reload(cx));
}

fn merge_models(previous: &[ProviderModel], batch: &AvailableModelsBatch) -> Vec<ProviderModel> {
    let failed_providers = batch
        .failures
        .iter()
        .map(|failure| failure.provider_name.as_str())
        .collect::<HashSet<_>>();
    let mut models = previous
        .iter()
        .filter(|model| failed_providers.contains(model.provider_name.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    for success in &batch.successes {
        models.extend(success.models.clone());
    }
    models.sort_by(|left, right| {
        left.provider_name
            .cmp(&right.provider_name)
            .then_with(|| left.id.cmp(&right.id))
    });
    models
}

#[cfg(test)]
mod tests {
    use super::{ModelStoreState, merge_models};
    use crate::llm::{
        AvailableModelsBatch, ProviderModel, ProviderModelCapability, ProviderModelsFailure,
        ProviderModelsSuccess,
    };
    use gpui::Task;

    fn model(provider: &str, id: &str) -> ProviderModel {
        ProviderModel::new(provider, id, ProviderModelCapability::Streaming)
    }

    #[test]
    fn merge_models_keeps_stale_entries_for_failed_providers() {
        let previous = vec![
            model("Ollama", "qwen2"),
            model("OpenAI", "gpt-4.1"),
            model("OpenAI", "gpt-5"),
        ];
        let batch = AvailableModelsBatch {
            successes: vec![ProviderModelsSuccess {
                provider_name: "Ollama".to_string(),
                models: vec![model("Ollama", "qwen3")],
            }],
            failures: vec![ProviderModelsFailure {
                provider_name: "OpenAI".to_string(),
                message: "api key missing".to_string(),
            }],
        };

        assert_eq!(
            merge_models(&previous, &batch),
            vec![
                model("Ollama", "qwen3"),
                model("OpenAI", "gpt-4.1"),
                model("OpenAI", "gpt-5"),
            ]
        );
    }

    #[test]
    fn merge_models_drops_previous_models_for_successful_or_removed_providers() {
        let previous = vec![model("Ollama", "qwen2"), model("OpenAI", "gpt-4.1")];
        let batch = AvailableModelsBatch {
            successes: vec![ProviderModelsSuccess {
                provider_name: "OpenAI".to_string(),
                models: vec![],
            }],
            failures: vec![],
        };

        assert!(merge_models(&previous, &batch).is_empty());
    }

    #[test]
    fn running_task_blocks_non_force_load() {
        let mut store = ModelStoreState {
            load_task: Some(Task::ready(())),
            ..Default::default()
        };

        assert!(!store.prepare_running_task(false));
        assert!(store.load_task.is_some());
    }

    #[test]
    fn forced_reload_clears_running_task_before_starting_new_load() {
        let mut store = ModelStoreState {
            load_task: Some(Task::ready(())),
            ..Default::default()
        };

        assert!(store.prepare_running_task(true));
        assert!(store.load_task.is_none());
    }

    #[test]
    fn cancel_debounced_reload_drops_pending_task() {
        let mut store = ModelStoreState {
            debounced_reload_task: Some(Task::ready(())),
            ..Default::default()
        };

        assert!(store.cancel_debounced_reload());
        assert!(store.debounced_reload_task.is_none());
        assert!(!store.cancel_debounced_reload());
    }

    #[test]
    fn apply_load_result_clears_task_and_returns_to_idle() {
        let mut store = ModelStoreState {
            status: super::ModelStoreStatus::InitialLoading,
            request_version: 7,
            load_task: Some(Task::ready(())),
            ..Default::default()
        };
        let batch = AvailableModelsBatch {
            successes: vec![ProviderModelsSuccess {
                provider_name: "OpenAI".to_string(),
                models: vec![model("OpenAI", "gpt-5")],
            }],
            failures: vec![ProviderModelsFailure {
                provider_name: "Ollama".to_string(),
                message: "timeout".to_string(),
            }],
        };

        assert!(store.apply_load_result(7, "fingerprint".to_string(), &batch));
        assert!(store.load_task.is_none());
        assert_eq!(store.status, super::ModelStoreStatus::Idle);
        assert!(store.loaded_once);
        assert_eq!(
            store.last_config_fingerprint.as_deref(),
            Some("fingerprint")
        );
        assert_eq!(store.last_failures, batch.failures);
        assert_eq!(store.models, vec![model("OpenAI", "gpt-5")]);
    }

    #[test]
    fn stale_load_result_does_not_clear_current_task_or_models() {
        let mut store = ModelStoreState {
            status: super::ModelStoreStatus::Refreshing,
            request_version: 8,
            models: vec![model("OpenAI", "gpt-4.1")],
            load_task: Some(Task::ready(())),
            ..Default::default()
        };
        let batch = AvailableModelsBatch {
            successes: vec![ProviderModelsSuccess {
                provider_name: "OpenAI".to_string(),
                models: vec![model("OpenAI", "gpt-5")],
            }],
            failures: vec![],
        };

        assert!(!store.apply_load_result(7, "old".to_string(), &batch));
        assert!(store.load_task.is_some());
        assert_eq!(store.status, super::ModelStoreStatus::Refreshing);
        assert_eq!(store.models, vec![model("OpenAI", "gpt-4.1")]);
        assert!(!store.loaded_once);
    }
}

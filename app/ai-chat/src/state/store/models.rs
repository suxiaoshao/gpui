use crate::{
    i18n::I18n,
    llm::{AvailableModelsBatch, ProviderModel, ProviderModelsFailure, available_models},
    state::AiChatConfig,
};
use async_compat::CompatExt;
use gpui::*;
use gpui_component::{
    WindowExt,
    notification::{Notification, NotificationType},
};
use std::collections::HashSet;
use std::ops::Deref;
use tracing::{Level, event};

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
}

pub(crate) struct ModelStoreState {
    models: Vec<ProviderModel>,
    status: ModelStoreStatus,
    loaded_once: bool,
    last_config_fingerprint: Option<String>,
    request_version: u64,
}

impl Default for ModelStoreState {
    fn default() -> Self {
        Self {
            models: Vec::new(),
            status: ModelStoreStatus::Idle,
            loaded_once: false,
            last_config_fingerprint: None,
            request_version: 0,
        }
    }
}

impl ModelStoreState {
    pub(crate) fn snapshot(&self) -> ModelStoreSnapshot {
        ModelStoreSnapshot {
            models: self.models.clone(),
            status: Some(self.status),
        }
    }

    pub(crate) fn ensure_loaded(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.refresh(false, window, cx);
    }

    pub(crate) fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.refresh(true, window, cx);
    }

    fn refresh(&mut self, force: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.status != ModelStoreStatus::Idle {
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
            return;
        }

        self.request_version = self.request_version.saturating_add(1);
        let request_version = self.request_version;
        self.status = if self.loaded_once {
            ModelStoreStatus::Refreshing
        } else {
            ModelStoreStatus::InitialLoading
        };
        cx.notify();

        let state = cx.entity().downgrade();
        cx.spawn_in(window, async move |_, cx| {
            let batch = available_models(config).compat().await;
            let _ = state.update_in(cx, move |this, window, cx| {
                if this.request_version != request_version {
                    return;
                }

                this.status = ModelStoreStatus::Idle;
                this.models = merge_models(&this.models, &batch);
                this.loaded_once = true;
                this.last_config_fingerprint = Some(fingerprint);

                if !batch.failures.is_empty() {
                    let title = cx.global::<I18n>().t("notify-load-models-partial-failed");
                    let message = format_failure_message(&batch.failures);
                    window.push_notification(
                        Notification::new()
                            .title(title)
                            .message(message)
                            .with_type(NotificationType::Error),
                        cx,
                    );
                    for failure in &batch.failures {
                        event!(
                            Level::ERROR,
                            "load provider models failed: provider={}, error={}",
                            failure.provider_name,
                            failure.message
                        );
                    }
                }
                cx.notify();
            });
        })
        .detach();
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
    cx.set_global(ModelStore { data });
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

fn format_failure_message(failures: &[ProviderModelsFailure]) -> String {
    failures
        .iter()
        .map(|failure| format!("{}: {}", failure.provider_name, failure.message))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::{format_failure_message, merge_models};
    use crate::llm::{
        AvailableModelsBatch, ProviderModel, ProviderModelCapability, ProviderModelsFailure,
        ProviderModelsSuccess,
    };

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
    fn format_failure_message_lists_each_provider_on_its_own_line() {
        let message = format_failure_message(&[
            ProviderModelsFailure {
                provider_name: "Ollama".to_string(),
                message: "request failed".to_string(),
            },
            ProviderModelsFailure {
                provider_name: "OpenAI".to_string(),
                message: "api key missing".to_string(),
            },
        ]);

        assert_eq!(message, "Ollama: request failed\nOpenAI: api key missing");
    }
}

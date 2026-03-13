use crate::{
    config::AiChatConfig,
    llm::{ProviderModel, available_models},
};
use gpui::*;
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
            let result = available_models(config).await;
            let _ = state.update_in(cx, move |this, _, cx| {
                if this.request_version != request_version {
                    return;
                }

                this.status = ModelStoreStatus::Idle;
                match result {
                    Ok(models) => {
                        this.models = models;
                        this.loaded_once = true;
                        this.last_config_fingerprint = Some(fingerprint);
                    }
                    Err(err) => {
                        event!(Level::ERROR, "load available models failed: {}", err);
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

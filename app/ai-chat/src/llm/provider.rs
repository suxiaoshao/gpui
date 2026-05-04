use super::Message;
use crate::{
    database::Content,
    errors::{AiChatError, AiChatResult},
    state::AiChatConfig,
};
use futures::{
    future::{BoxFuture, Either, select},
    stream::{BoxStream, FuturesUnordered, StreamExt},
};
use gpui::App;
use std::time::Duration;

mod ollama;
mod openai;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProviderModelCapability {
    Streaming,
    NonStreaming,
}

impl ProviderModelCapability {
    pub(crate) fn stream_flag(self) -> bool {
        matches!(self, Self::Streaming)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProviderModel {
    pub(crate) provider_name: String,
    pub(crate) id: String,
    pub(crate) capability: ProviderModelCapability,
    pub(crate) metadata: serde_json::Value,
}

impl ProviderModel {
    pub(crate) fn new(
        provider_name: impl Into<String>,
        id: impl Into<String>,
        capability: ProviderModelCapability,
    ) -> Self {
        Self {
            provider_name: provider_name.into(),
            id: id.into(),
            capability,
            metadata: serde_json::json!({}),
        }
    }

    pub(crate) fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExtSettingOption {
    pub(crate) value: &'static str,
    pub(crate) label_key: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExtSettingItem {
    pub(crate) key: &'static str,
    pub(crate) label_key: &'static str,
    pub(crate) tooltip: Option<&'static str>,
    pub(crate) control: ExtSettingControl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProviderSettingsFieldKind {
    Text,
    SecretText,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProviderSettingsFieldSpec {
    pub(crate) key: &'static str,
    pub(crate) label_key: &'static str,
    pub(crate) kind: ProviderSettingsFieldKind,
    pub(crate) search_keywords: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProviderSettingsSpec {
    pub(crate) provider_name: &'static str,
    pub(crate) title_key: &'static str,
    pub(crate) fields: &'static [ProviderSettingsFieldSpec],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExtSettingControl {
    Select {
        value: String,
        options: Vec<ExtSettingOption>,
    },
    Boolean(bool),
}

pub(crate) trait Provider: Sync {
    fn name(&self) -> &'static str;
    fn is_configured(&self, settings: &serde_json::Value) -> bool;
    fn default_template_for_model(&self, model: &ProviderModel) -> AiChatResult<serde_json::Value>;
    fn request_body(
        &self,
        template: &serde_json::Value,
        history_messages: Vec<Message>,
    ) -> AiChatResult<serde_json::Value>;
    fn fetch_by_request_body<'a>(
        &self,
        config: AiChatConfig,
        settings: toml::Value,
        request_body: &'a serde_json::Value,
    ) -> BoxStream<'a, AiChatResult<FetchUpdate>>;
    fn list_models(
        &self,
        config: AiChatConfig,
        settings: toml::Value,
    ) -> BoxFuture<'static, AiChatResult<Vec<ProviderModel>>>;
    fn settings_spec(&self) -> ProviderSettingsSpec;
    fn read_settings_field(&self, key: &str, config: &AiChatConfig) -> Option<String>;
    fn write_settings_field(&self, key: &str, value: String, cx: &mut App) -> AiChatResult<()>;
    fn ext_settings(
        &self,
        _model: &ProviderModel,
        _template: &serde_json::Value,
    ) -> AiChatResult<Vec<ExtSettingItem>> {
        Ok(Vec::new())
    }
    fn apply_ext_setting(
        &self,
        _model: &ProviderModel,
        _template: &mut serde_json::Value,
        setting: &ExtSettingItem,
    ) -> AiChatResult<()> {
        Err(AiChatError::StreamError(format!(
            "unsupported provider setting: {}",
            setting.key
        )))
    }
}

pub(crate) fn optional_setting_value(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

pub(crate) fn normalized_or_default(
    value: &str,
    default: impl FnOnce() -> String,
    normalize: impl FnOnce(&str) -> String,
) -> String {
    if value.trim().is_empty() {
        default()
    } else {
        normalize(value)
    }
}

pub(crate) use ollama::{OllamaProvider, OllamaSettings};
pub(crate) use openai::{OpenAIProvider, OpenAISettings};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FetchUpdate {
    ThinkingStarted,
    ReasoningSummaryDelta(String),
    TextDelta(String),
    Complete(Content),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProviderModelsSuccess {
    pub(crate) provider_name: String,
    pub(crate) models: Vec<ProviderModel>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProviderModelsFailure {
    pub(crate) provider_name: String,
    pub(crate) message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct AvailableModelsBatch {
    pub(crate) successes: Vec<ProviderModelsSuccess>,
    pub(crate) failures: Vec<ProviderModelsFailure>,
}

const PROVIDERS: [&dyn Provider; 2] = [&OllamaProvider, &OpenAIProvider];
pub(crate) const MODEL_LIST_PROVIDER_TIMEOUT: Duration = Duration::from_secs(15);

pub(crate) fn provider_names() -> Vec<&'static str> {
    PROVIDERS.iter().map(|provider| provider.name()).collect()
}

pub(crate) fn provider_settings_specs() -> Vec<ProviderSettingsSpec> {
    PROVIDERS
        .iter()
        .map(|provider| provider.settings_spec())
        .collect()
}

pub(crate) fn provider_by_name(name: &str) -> AiChatResult<&'static dyn Provider> {
    PROVIDERS
        .iter()
        .copied()
        .find(|provider| provider.name() == name)
        .ok_or_else(|| AiChatError::ProviderNotFound(name.to_string()))
}

fn provider_settings_json(
    config: &AiChatConfig,
    provider: &dyn Provider,
) -> Option<(toml::Value, serde_json::Value)> {
    let settings = config.get_provider_settings(provider.name())?.clone();
    let settings_json = serde_json::to_value(&settings).ok()?;
    Some((settings, settings_json))
}

pub(crate) fn provider_is_configured(
    config: &AiChatConfig,
    provider_name: &str,
) -> AiChatResult<bool> {
    let provider = provider_by_name(provider_name)?;
    Ok(provider_settings_json(config, provider)
        .is_some_and(|(_, settings)| provider.is_configured(&settings)))
}

fn sort_models(models: &mut [ProviderModel]) {
    models.sort_by(|left, right| {
        left.provider_name
            .cmp(&right.provider_name)
            .then_with(|| left.id.cmp(&right.id))
    });
}

async fn load_provider_models(
    provider_name: String,
    list_models: BoxFuture<'static, AiChatResult<Vec<ProviderModel>>>,
    timeout: Duration,
) -> Result<ProviderModelsSuccess, ProviderModelsFailure> {
    match with_timeout(list_models, timeout).await {
        Ok(Ok(mut models)) => {
            sort_models(&mut models);
            Ok(ProviderModelsSuccess {
                provider_name,
                models,
            })
        }
        Ok(Err(err)) => Err(ProviderModelsFailure {
            provider_name,
            message: err.to_string(),
        }),
        Err(()) => Err(ProviderModelsFailure {
            provider_name,
            message: format!(
                "model list request timed out after {}",
                format_model_list_timeout(timeout)
            ),
        }),
    }
}

fn format_model_list_timeout(timeout: Duration) -> String {
    if timeout.as_secs() > 0 {
        format!("{}s", timeout.as_secs())
    } else {
        format!("{}ms", timeout.as_millis())
    }
}

async fn with_timeout<T>(future: BoxFuture<'static, T>, timeout: Duration) -> Result<T, ()> {
    let timer = futures::FutureExt::map(smol::Timer::after(timeout), |_| ());
    futures::pin_mut!(future);
    futures::pin_mut!(timer);
    match select(future, timer).await {
        Either::Left((result, _)) => Ok(result),
        Either::Right(((), _)) => Err(()),
    }
}

async fn available_models_from_providers_with_timeout(
    providers: &[&dyn Provider],
    config: AiChatConfig,
    timeout: Duration,
) -> AvailableModelsBatch {
    let mut tasks = FuturesUnordered::new();
    for provider in providers {
        let Some((settings, settings_json)) = provider_settings_json(&config, *provider) else {
            continue;
        };
        if !provider.is_configured(&settings_json) {
            continue;
        }
        let provider_name = provider.name().to_string();
        let list_models = provider.list_models(config.clone(), settings);
        tasks.push(load_provider_models(provider_name, list_models, timeout));
    }

    let mut batch = AvailableModelsBatch::default();
    while let Some(outcome) = tasks.next().await {
        match outcome {
            Ok(success) => batch.successes.push(success),
            Err(failure) => batch.failures.push(failure),
        }
    }
    batch
        .successes
        .sort_by(|left, right| left.provider_name.cmp(&right.provider_name));
    batch
        .failures
        .sort_by(|left, right| left.provider_name.cmp(&right.provider_name));
    batch
}

async fn available_models_from_providers(
    providers: &[&dyn Provider],
    config: AiChatConfig,
) -> AvailableModelsBatch {
    available_models_from_providers_with_timeout(providers, config, MODEL_LIST_PROVIDER_TIMEOUT)
        .await
}

pub(crate) async fn available_models(config: AiChatConfig) -> AvailableModelsBatch {
    available_models_from_providers(&PROVIDERS, config).await
}

#[cfg(test)]
mod tests {
    use super::{
        AvailableModelsBatch, ExtSettingItem, FetchUpdate, Message, Provider, ProviderModel,
        ProviderModelCapability, ProviderModelsFailure, ProviderSettingsFieldKind,
        ProviderSettingsSpec, available_models_from_providers,
        available_models_from_providers_with_timeout, provider_settings_specs,
    };
    use crate::{
        errors::{AiChatError, AiChatResult},
        state::AiChatConfig,
    };
    use futures::{FutureExt, StreamExt, future::BoxFuture, stream::BoxStream};
    use std::time::Duration;

    struct MockProvider {
        name: &'static str,
        configured: bool,
        models: Vec<ProviderModel>,
        error: Option<&'static str>,
        pending: bool,
    }

    impl Provider for MockProvider {
        fn name(&self) -> &'static str {
            self.name
        }

        fn is_configured(&self, _settings: &serde_json::Value) -> bool {
            self.configured
        }

        fn default_template_for_model(
            &self,
            _model: &ProviderModel,
        ) -> crate::errors::AiChatResult<serde_json::Value> {
            unreachable!()
        }

        fn request_body(
            &self,
            _template: &serde_json::Value,
            _history_messages: Vec<Message>,
        ) -> crate::errors::AiChatResult<serde_json::Value> {
            unreachable!()
        }

        fn fetch_by_request_body<'a>(
            &self,
            _config: AiChatConfig,
            _settings: toml::Value,
            _request_body: &'a serde_json::Value,
        ) -> BoxStream<'a, crate::errors::AiChatResult<FetchUpdate>> {
            futures::stream::empty().boxed()
        }

        fn list_models(
            &self,
            _config: AiChatConfig,
            _settings: toml::Value,
        ) -> BoxFuture<'static, crate::errors::AiChatResult<Vec<ProviderModel>>> {
            if self.pending {
                return futures::future::pending().boxed();
            }
            let result = match self.error {
                Some(message) => Err(AiChatError::StreamError(message.to_string())),
                None => Ok(self.models.clone()),
            };
            async move { result }.boxed()
        }

        fn settings_spec(&self) -> ProviderSettingsSpec {
            unreachable!()
        }

        fn read_settings_field(&self, _key: &str, _config: &AiChatConfig) -> Option<String> {
            unreachable!()
        }

        fn write_settings_field(
            &self,
            _key: &str,
            _value: String,
            _cx: &mut gpui::App,
        ) -> AiChatResult<()> {
            unreachable!()
        }

        fn ext_settings(
            &self,
            _model: &ProviderModel,
            _template: &serde_json::Value,
        ) -> crate::errors::AiChatResult<Vec<ExtSettingItem>> {
            unreachable!()
        }
    }

    fn configured_config(names: &[&str]) -> AiChatConfig {
        let mut config = AiChatConfig::default();
        for name in names {
            config.set_provider_settings(name, toml::Value::Table(Default::default()));
        }
        config
    }

    fn model(provider: &str, id: &str) -> ProviderModel {
        ProviderModel::new(provider, id, ProviderModelCapability::Streaming)
    }

    #[tokio::test]
    async fn available_models_collects_partial_failures_without_blocking_successes() {
        let success = MockProvider {
            name: "Provider A",
            configured: true,
            models: vec![model("Provider A", "b"), model("Provider A", "a")],
            error: None,
            pending: false,
        };
        let failure = MockProvider {
            name: "Provider B",
            configured: true,
            models: vec![],
            error: Some("boom"),
            pending: false,
        };

        let batch = available_models_from_providers(
            &[&success, &failure],
            configured_config(&["Provider A", "Provider B"]),
        )
        .await;

        assert_eq!(
            batch,
            AvailableModelsBatch {
                successes: vec![super::ProviderModelsSuccess {
                    provider_name: "Provider A".to_string(),
                    models: vec![model("Provider A", "a"), model("Provider A", "b")],
                }],
                failures: vec![ProviderModelsFailure {
                    provider_name: "Provider B".to_string(),
                    message: "stream错误:boom".to_string(),
                }],
            }
        );
    }

    #[tokio::test]
    async fn available_models_skips_unconfigured_providers() {
        let configured = MockProvider {
            name: "Provider A",
            configured: true,
            models: vec![model("Provider A", "a")],
            error: None,
            pending: false,
        };
        let unconfigured = MockProvider {
            name: "Provider B",
            configured: false,
            models: vec![model("Provider B", "b")],
            error: Some("should not run"),
            pending: false,
        };

        let batch = available_models_from_providers(
            &[&configured, &unconfigured],
            configured_config(&["Provider A", "Provider B"]),
        )
        .await;

        assert_eq!(batch.successes.len(), 1);
        assert_eq!(batch.successes[0].provider_name, "Provider A");
        assert!(batch.failures.is_empty());
    }

    #[tokio::test]
    async fn available_models_times_out_pending_provider_without_blocking_successes() {
        let success = MockProvider {
            name: "Provider A",
            configured: true,
            models: vec![model("Provider A", "a")],
            error: None,
            pending: false,
        };
        let pending = MockProvider {
            name: "Provider B",
            configured: true,
            models: vec![],
            error: None,
            pending: true,
        };

        let batch = available_models_from_providers_with_timeout(
            &[&success, &pending],
            configured_config(&["Provider A", "Provider B"]),
            Duration::from_millis(10),
        )
        .await;

        assert_eq!(batch.successes.len(), 1);
        assert_eq!(batch.successes[0].provider_name, "Provider A");
        assert_eq!(batch.failures.len(), 1);
        assert_eq!(batch.failures[0].provider_name, "Provider B");
        assert_eq!(
            batch.failures[0].message,
            "model list request timed out after 10ms"
        );
    }

    #[test]
    fn openai_api_key_settings_field_is_secret_text() {
        let specs = provider_settings_specs();
        let openai = specs
            .iter()
            .find(|spec| spec.provider_name == "OpenAI")
            .expect("OpenAI settings spec exists");
        let api_key = openai
            .fields
            .iter()
            .find(|field| field.key == "apiKey")
            .expect("OpenAI API key field exists");

        assert_eq!(api_key.kind, ProviderSettingsFieldKind::SecretText);
    }
}

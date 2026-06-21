use std::collections::BTreeMap;

use crate::{
    AgentRunHandle, AgentRunRequest, AgentRuntime, AgentRuntimeError, AgentRuntimeObserver,
    model_capabilities::{
        capabilities_for_model, capabilities_from_gemini_model, capabilities_from_ollama_show,
        capabilities_from_openrouter_model,
    },
};
use ai_chat_core::{
    ProviderModelMetadata, ProviderRawPayload, ProviderSettingValue, ProviderSettingsPayload,
};
use ai_chat_db::{NewProviderModel, ProviderRecord};
use rig_core::{
    client::{CompletionClient, ModelListingClient},
    model::{Model, ModelList, ModelListingError},
    providers::{anthropic, deepseek, gemini, mistral, ollama, openai, openrouter},
};
use serde::Deserialize;
use serde_json::json;
use thiserror::Error;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProviderSecretValues {
    pub values: BTreeMap<String, String>,
}

impl ProviderSecretValues {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProviderModelFetchRequest {
    pub provider: ProviderRecord,
    pub secrets: ProviderSecretValues,
}

#[derive(Debug, Error)]
pub enum ProviderModelFetchError {
    #[error("manual model configuration is required for provider `{provider_kind}`")]
    ManualModelsRequired { provider_kind: String },
    #[error("missing provider secret `{key}`")]
    MissingSecret { key: String },
    #[error("invalid provider configuration: {message}")]
    InvalidConfig { message: String },
    #[error("model listing failed for provider `{provider_kind}`: {message}")]
    ListingFailed {
        provider_kind: String,
        message: String,
    },
}

pub async fn fetch_provider_models(
    request: ProviderModelFetchRequest,
) -> Result<Vec<NewProviderModel>, ProviderModelFetchError> {
    let provider_kind = request.provider.kind.as_str();
    match provider_kind {
        "ollama" => return fetch_ollama_models(&request).await,
        "gemini" => return fetch_gemini_models(&request).await,
        "openrouter" => return fetch_openrouter_models(&request).await,
        _ => {}
    }

    let models = match provider_kind {
        "openai" => {
            let client = apply_base_url(
                openai::Client::builder().api_key(required_secret(&request.secrets, "api_key")?),
                &request.provider.settings,
            )?
            .build()
            .map_err(invalid_config)?;
            list_models(provider_kind, client).await?
        }
        "anthropic" => {
            let client = apply_base_url(
                anthropic::Client::builder().api_key(required_secret(&request.secrets, "api_key")?),
                &request.provider.settings,
            )?
            .build()
            .map_err(invalid_config)?;
            list_models(provider_kind, client).await?
        }
        "deepseek" => {
            let client = apply_base_url(
                deepseek::Client::builder().api_key(required_secret(&request.secrets, "api_key")?),
                &request.provider.settings,
            )?
            .build()
            .map_err(invalid_config)?;
            list_models(provider_kind, client).await?
        }
        "mistral" => {
            let client = apply_base_url(
                mistral::Client::builder().api_key(required_secret(&request.secrets, "api_key")?),
                &request.provider.settings,
            )?
            .build()
            .map_err(invalid_config)?;
            list_models(provider_kind, client).await?
        }
        _ => {
            return Err(ProviderModelFetchError::ManualModelsRequired {
                provider_kind: request.provider.kind,
            });
        }
    };

    Ok(models
        .data
        .into_iter()
        .map(|model| provider_model_from_rig_model(&request.provider, model))
        .collect())
}

pub(crate) async fn run_saved_provider_model(
    runtime: &AgentRuntime,
    request: AgentRunRequest,
    provider: ProviderRecord,
    secrets: ProviderSecretValues,
    observer: Option<AgentRuntimeObserver>,
) -> crate::Result<AgentRunHandle> {
    let model_id = request.model_id.clone();
    macro_rules! run_with_client {
        ($client:expr) => {
            match $client.map_err(runtime_config_error) {
                Ok(client) => {
                    runtime
                        .run_with_model_observed(
                            request,
                            client.completion_model(model_id),
                            observer,
                        )
                        .await
                }
                Err(error) => runtime.record_setup_failed_run(request, error, observer.as_ref()),
            }
        };
    }

    match provider.kind.as_str() {
        "openai" => run_with_client!(build_openai_client(&provider, &secrets)),
        "anthropic" => run_with_client!(build_anthropic_client(&provider, &secrets)),
        "gemini" => run_with_client!(build_gemini_client(&provider, &secrets)),
        "ollama" => run_with_client!(build_ollama_client(&provider, &secrets)),
        "openrouter" => run_with_client!(build_openrouter_client(&provider, &secrets)),
        "deepseek" => run_with_client!(build_deepseek_client(&provider, &secrets)),
        "mistral" => run_with_client!(build_mistral_client(&provider, &secrets)),
        provider_kind => runtime.record_setup_failed_run(
            request,
            AgentRuntimeError::Unsupported(format!(
                "provider `{provider_kind}` cannot run completion models"
            )),
            observer.as_ref(),
        ),
    }
}

fn runtime_config_error(err: ProviderModelFetchError) -> AgentRuntimeError {
    AgentRuntimeError::Unsupported(err.to_string())
}

pub fn build_openai_client(
    provider: &ProviderRecord,
    secrets: &ProviderSecretValues,
) -> std::result::Result<openai::Client, ProviderModelFetchError> {
    apply_base_url(
        openai::Client::builder().api_key(required_secret(secrets, "api_key")?),
        &provider.settings,
    )?
    .build()
    .map_err(invalid_config)
}

pub fn build_anthropic_client(
    provider: &ProviderRecord,
    secrets: &ProviderSecretValues,
) -> std::result::Result<anthropic::Client, ProviderModelFetchError> {
    apply_base_url(
        anthropic::Client::builder().api_key(required_secret(secrets, "api_key")?),
        &provider.settings,
    )?
    .build()
    .map_err(invalid_config)
}

pub fn build_gemini_client(
    provider: &ProviderRecord,
    secrets: &ProviderSecretValues,
) -> std::result::Result<gemini::Client, ProviderModelFetchError> {
    apply_base_url(
        gemini::Client::builder().api_key(required_secret(secrets, "api_key")?),
        &provider.settings,
    )?
    .build()
    .map_err(invalid_config)
}

pub fn build_ollama_client(
    provider: &ProviderRecord,
    secrets: &ProviderSecretValues,
) -> std::result::Result<ollama::Client, ProviderModelFetchError> {
    apply_base_url(
        ollama::Client::builder().api_key(secrets.get("bearer_token").unwrap_or_default()),
        &provider.settings,
    )?
    .build()
    .map_err(invalid_config)
}

pub fn build_openrouter_client(
    provider: &ProviderRecord,
    secrets: &ProviderSecretValues,
) -> std::result::Result<openrouter::Client, ProviderModelFetchError> {
    apply_base_url(
        openrouter::Client::builder().api_key(required_secret(secrets, "api_key")?),
        &provider.settings,
    )?
    .build()
    .map_err(invalid_config)
}

pub fn build_deepseek_client(
    provider: &ProviderRecord,
    secrets: &ProviderSecretValues,
) -> std::result::Result<deepseek::Client, ProviderModelFetchError> {
    apply_base_url(
        deepseek::Client::builder().api_key(required_secret(secrets, "api_key")?),
        &provider.settings,
    )?
    .build()
    .map_err(invalid_config)
}

pub fn build_mistral_client(
    provider: &ProviderRecord,
    secrets: &ProviderSecretValues,
) -> std::result::Result<mistral::Client, ProviderModelFetchError> {
    apply_base_url(
        mistral::Client::builder().api_key(required_secret(secrets, "api_key")?),
        &provider.settings,
    )?
    .build()
    .map_err(invalid_config)
}

pub fn provider_model_from_rig_model(provider: &ProviderRecord, model: Model) -> NewProviderModel {
    let model_id = model.id.clone();
    let raw = serde_json::to_value(&model)
        .ok()
        .map(|value| ProviderRawPayload {
            provider_kind: provider.kind.clone(),
            value,
        });
    let display_name = model
        .name
        .clone()
        .filter(|name| name != &model.id)
        .or_else(|| Some(model.display_name().to_string()));

    NewProviderModel {
        provider_id: provider.id.clone(),
        model_id: model_id.clone(),
        display_name: display_name.clone(),
        enabled: true,
        capabilities: capabilities_for_model(&provider.kind, &model_id, raw.clone()),
        metadata: ProviderModelMetadata {
            display_name,
            family: model.owned_by,
            raw,
        },
    }
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaTagModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaTagModel {
    name: String,
    model: String,
}

#[derive(Debug, Deserialize)]
struct OllamaShowResponse {
    #[serde(default)]
    details: OllamaModelDetails,
    #[serde(default, deserialize_with = "deserialize_null_default_vec")]
    capabilities: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct OllamaModelDetails {
    #[serde(default)]
    family: String,
    #[serde(default, deserialize_with = "deserialize_null_default_vec")]
    families: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiModelsResponse {
    models: Vec<GeminiModelEntry>,
    next_page_token: Option<String>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiModelEntry {
    name: String,
    base_model_id: Option<String>,
    display_name: Option<String>,
    input_token_limit: Option<u64>,
    #[serde(default)]
    supported_generation_methods: Vec<String>,
    thinking: Option<bool>,
}

impl GeminiModelEntry {
    fn supports_generate_content(&self) -> bool {
        self.supported_generation_methods
            .iter()
            .any(|method| method == "generateContent")
    }
}

#[derive(Debug, Deserialize)]
struct OpenRouterModelsResponse {
    data: Vec<OpenRouterModelEntry>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct OpenRouterModelEntry {
    id: String,
    name: String,
    created: Option<u64>,
    #[serde(rename = "context_length")]
    context_length: Option<u32>,
    architecture: Option<OpenRouterModelArchitecture>,
    #[serde(default, rename = "supported_parameters")]
    supported_parameters: Vec<String>,
}

#[derive(Debug, Default, Deserialize, serde::Serialize)]
struct OpenRouterModelArchitecture {
    #[serde(default)]
    input_modalities: Vec<String>,
}

async fn fetch_ollama_models(
    request: &ProviderModelFetchRequest,
) -> Result<Vec<NewProviderModel>, ProviderModelFetchError> {
    let base_url = provider_base_url(&request.provider.settings, "http://localhost:11434")?;
    let bearer_token = request
        .secrets
        .get("bearer_token")
        .filter(|token| !token.is_empty());
    let client = reqwest::Client::new();
    let mut tags_request = client.get(provider_url(&base_url, "/api/tags")?);
    if let Some(token) = bearer_token {
        tags_request = tags_request.bearer_auth(token);
    }

    let tags = send_json::<OllamaTagsResponse>("ollama", "/api/tags", tags_request).await?;
    let mut models = Vec::new();
    for model in tags.models {
        let model_id = if !model.model.trim().is_empty() {
            model.model.clone()
        } else {
            model.name.clone()
        };
        let mut show_request = client
            .post(provider_url(&base_url, "/api/show")?)
            .json(&json!({ "model": model_id.clone() }));
        if let Some(token) = bearer_token {
            show_request = show_request.bearer_auth(token);
        }

        let show = send_json::<OllamaShowResponse>("ollama", "/api/show", show_request).await?;
        if !show
            .capabilities
            .iter()
            .any(|capability| capability == "completion")
        {
            continue;
        }
        let display_name = Some(model.name.clone()).filter(|name| name != &model_id);
        let raw = Some(ProviderRawPayload {
            provider_kind: request.provider.kind.clone(),
            value: json!({
                "tag": {
                    "name": model.name,
                    "model": model_id.clone(),
                },
                "show": {
                    "details": {
                        "family": show.details.family.clone(),
                        "families": show.details.families.clone(),
                    },
                    "capabilities": show.capabilities.clone(),
                }
            }),
        });
        models.push(NewProviderModel {
            provider_id: request.provider.id.clone(),
            model_id,
            display_name: display_name.clone(),
            enabled: true,
            capabilities: capabilities_from_ollama_show(
                show.capabilities,
                show.details.family.clone(),
                show.details.families.clone(),
                raw.clone(),
            ),
            metadata: ProviderModelMetadata {
                display_name,
                family: Some(show.details.family),
                raw,
            },
        });
    }
    models.sort_by(|left, right| left.model_id.cmp(&right.model_id));
    Ok(models)
}

async fn fetch_gemini_models(
    request: &ProviderModelFetchRequest,
) -> Result<Vec<NewProviderModel>, ProviderModelFetchError> {
    let base_url = provider_base_url(
        &request.provider.settings,
        "https://generativelanguage.googleapis.com",
    )?;
    let api_key = required_secret(&request.secrets, "api_key")?;
    let client = reqwest::Client::new();
    let mut page_token: Option<String> = None;
    let mut models = Vec::new();

    loop {
        let url = gemini_models_url(&base_url, api_key, page_token.as_deref())?;
        let page =
            send_json::<GeminiModelsResponse>("gemini", "/v1beta/models", client.get(url)).await?;
        page_token = page.next_page_token.clone();
        for model in page.models {
            if let Some(model) = provider_model_from_gemini_model(&request.provider, model) {
                models.push(model);
            }
        }
        if page_token.is_none() {
            break;
        }
    }

    models.sort_by(|left, right| left.model_id.cmp(&right.model_id));
    Ok(models)
}

fn provider_model_from_gemini_model(
    provider: &ProviderRecord,
    model: GeminiModelEntry,
) -> Option<NewProviderModel> {
    if !model.supports_generate_content() {
        return None;
    }
    let model_id = model
        .base_model_id
        .clone()
        .filter(|id| !id.trim().is_empty())
        .or_else(|| normalize_gemini_model_name(&model.name))?;
    let raw = serde_json::to_value(&model)
        .ok()
        .map(|value| ProviderRawPayload {
            provider_kind: provider.kind.clone(),
            value,
        });
    Some(NewProviderModel {
        provider_id: provider.id.clone(),
        model_id: model_id.clone(),
        display_name: model.display_name.clone().filter(|name| name != &model_id),
        enabled: true,
        capabilities: capabilities_from_gemini_model(&model_id, model.thinking, raw.clone()),
        metadata: ProviderModelMetadata {
            display_name: model.display_name,
            family: None,
            raw,
        },
    })
}

async fn fetch_openrouter_models(
    request: &ProviderModelFetchRequest,
) -> Result<Vec<NewProviderModel>, ProviderModelFetchError> {
    let base_url = provider_base_url(&request.provider.settings, "https://openrouter.ai/api/v1")?;
    let api_key = required_secret(&request.secrets, "api_key")?;
    let client = reqwest::Client::new();
    let response = send_json::<OpenRouterModelsResponse>(
        "openrouter",
        "/models",
        client
            .get(provider_url(&base_url, "/models")?)
            .bearer_auth(api_key),
    )
    .await?;

    let mut models = response
        .data
        .into_iter()
        .map(|model| {
            let display_name = Some(model.name.clone()).filter(|name| name != &model.id);
            let raw = serde_json::to_value(&model)
                .ok()
                .map(|value| ProviderRawPayload {
                    provider_kind: request.provider.kind.clone(),
                    value,
                });
            NewProviderModel {
                provider_id: request.provider.id.clone(),
                model_id: model.id.clone(),
                display_name: display_name.clone(),
                enabled: true,
                capabilities: capabilities_from_openrouter_model(
                    model.supported_parameters,
                    model
                        .architecture
                        .as_ref()
                        .map(|architecture| architecture.input_modalities.clone())
                        .unwrap_or_default(),
                    raw.clone(),
                ),
                metadata: ProviderModelMetadata {
                    display_name,
                    family: None,
                    raw,
                },
            }
        })
        .collect::<Vec<_>>();
    models.sort_by(|left, right| left.model_id.cmp(&right.model_id));
    Ok(models)
}

fn required_secret<'a>(
    secrets: &'a ProviderSecretValues,
    key: &str,
) -> Result<&'a str, ProviderModelFetchError> {
    secrets
        .get(key)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ProviderModelFetchError::MissingSecret {
            key: key.to_string(),
        })
}

async fn list_models(
    provider_kind: &str,
    client: impl ModelListingClient,
) -> Result<ModelList, ProviderModelFetchError> {
    client
        .list_models()
        .await
        .map_err(|err| listing_failed(provider_kind, err))
}

fn listing_failed(provider_kind: &str, err: ModelListingError) -> ProviderModelFetchError {
    ProviderModelFetchError::ListingFailed {
        provider_kind: provider_kind.to_string(),
        message: err.to_string(),
    }
}

fn invalid_config(err: impl std::fmt::Display) -> ProviderModelFetchError {
    ProviderModelFetchError::InvalidConfig {
        message: err.to_string(),
    }
}

async fn send_json<T>(
    provider_kind: &str,
    path: &str,
    request: reqwest::RequestBuilder,
) -> Result<T, ProviderModelFetchError>
where
    T: for<'de> Deserialize<'de>,
{
    let response = request
        .send()
        .await
        .map_err(|err| listing_failed_message(provider_kind, err))?;
    let response = response
        .error_for_status()
        .map_err(|err| listing_failed_message(provider_kind, err))?;
    response
        .json::<T>()
        .await
        .map_err(|err| ProviderModelFetchError::ListingFailed {
            provider_kind: provider_kind.to_string(),
            message: format!("decode {path} response failed: {err}"),
        })
}

fn listing_failed_message(
    provider_kind: &str,
    err: impl std::fmt::Display,
) -> ProviderModelFetchError {
    ProviderModelFetchError::ListingFailed {
        provider_kind: provider_kind.to_string(),
        message: err.to_string(),
    }
}

fn provider_base_url(
    settings: &ProviderSettingsPayload,
    default_base_url: &str,
) -> Result<String, ProviderModelFetchError> {
    let base_url = settings_field_string(settings, "base_url")
        .map(str::trim)
        .filter(|base_url| !base_url.is_empty())
        .unwrap_or(default_base_url);
    validate_base_url(base_url)?;
    Ok(base_url.trim_end_matches('/').to_string())
}

fn provider_url(base_url: &str, path: &str) -> Result<url::Url, ProviderModelFetchError> {
    let base = url::Url::parse(&format!("{}/", base_url.trim_end_matches('/'))).map_err(|err| {
        ProviderModelFetchError::InvalidConfig {
            message: format!("invalid base URL `{base_url}`: {err}"),
        }
    })?;
    base.join(path.trim_start_matches('/'))
        .map_err(|err| ProviderModelFetchError::InvalidConfig {
            message: format!("invalid provider path `{path}`: {err}"),
        })
}

fn gemini_models_url(
    base_url: &str,
    api_key: &str,
    page_token: Option<&str>,
) -> Result<url::Url, ProviderModelFetchError> {
    let mut url = provider_url(base_url, "/v1beta/models")?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("pageSize", "1000");
        query.append_pair("key", api_key);
        if let Some(page_token) = page_token {
            query.append_pair("pageToken", page_token);
        }
    }
    Ok(url)
}

fn normalize_gemini_model_name(name: &str) -> Option<String> {
    let trimmed = name.trim();
    let trimmed = trimmed.strip_prefix("models/").unwrap_or(trimmed);
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn apply_base_url<Builder>(
    builder: Builder,
    settings: &ProviderSettingsPayload,
) -> Result<Builder, ProviderModelFetchError>
where
    Builder: BaseUrlBuilder,
{
    match settings_field_string(settings, "base_url") {
        Some(base_url) if !base_url.trim().is_empty() => {
            validate_base_url(base_url.trim())?;
            Ok(builder.with_base_url(base_url.trim()))
        }
        _ => Ok(builder),
    }
}

fn validate_base_url(base_url: &str) -> Result<(), ProviderModelFetchError> {
    let url = url::Url::parse(base_url).map_err(|err| ProviderModelFetchError::InvalidConfig {
        message: format!("invalid base URL `{base_url}`: {err}"),
    })?;
    match url.scheme() {
        "http" | "https" => Ok(()),
        scheme => Err(ProviderModelFetchError::InvalidConfig {
            message: format!("base URL must use http or https, got `{scheme}`"),
        }),
    }
}

trait BaseUrlBuilder: Sized {
    fn with_base_url(self, base_url: &str) -> Self;
}

impl<Ext, Key, H> BaseUrlBuilder for rig_core::client::ClientBuilder<Ext, Key, H>
where
    Ext: Clone,
{
    fn with_base_url(self, base_url: &str) -> Self {
        self.base_url(base_url)
    }
}

fn settings_field_string<'a>(settings: &'a ProviderSettingsPayload, key: &str) -> Option<&'a str> {
    settings
        .fields
        .iter()
        .find(|field| field.key == key)
        .and_then(|field| match &field.value {
            ProviderSettingValue::String { value } => Some(value.as_str()),
            _ => None,
        })
}

fn deserialize_null_default_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(deserializer)?.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ai_chat_core::{
        ProviderSecretRefs, ProviderSettingFieldValue, ProviderSettingValue,
        ProviderSettingsPayload,
    };
    use ai_chat_db::{FreshStore, NewProvider};
    use rig_core::providers::{gemini, ollama, openrouter};
    use tempfile::tempdir;

    #[test]
    fn missing_api_key_returns_typed_error() {
        let err = build_openai_client_for_test(
            &provider_record("openai", Some("https://api.openai.com/v1")),
            &ProviderSecretValues::default(),
        )
        .expect_err("api key is required");

        assert!(matches!(
            err,
            ProviderModelFetchError::MissingSecret { key } if key == "api_key"
        ));
    }

    #[test]
    fn bad_base_url_returns_invalid_config() {
        let provider = provider_record("openai", Some("not a url"));
        let secrets = ProviderSecretValues {
            values: BTreeMap::from([("api_key".to_string(), "sk-test".to_string())]),
        };

        let err = build_openai_client_for_test(&provider, &secrets)
            .expect_err("invalid base url should fail during client construction");

        assert!(matches!(err, ProviderModelFetchError::InvalidConfig { .. }));
    }

    #[test]
    fn supported_provider_configs_can_build_listing_clients() {
        let api_secret = ProviderSecretValues {
            values: BTreeMap::from([("api_key".to_string(), "sk-test".to_string())]),
        };
        let ollama_secret = ProviderSecretValues::default();

        build_openai_client_for_test(&provider_record("openai", None), &api_secret).unwrap();
        build_anthropic_client_for_test(&provider_record("anthropic", None), &api_secret).unwrap();
        build_gemini_client_for_test(&provider_record("gemini", None), &api_secret).unwrap();
        build_ollama_client_for_test(&provider_record("ollama", None), &ollama_secret).unwrap();
        build_openrouter_client_for_test(&provider_record("openrouter", None), &api_secret)
            .unwrap();
        build_deepseek_client_for_test(&provider_record("deepseek", None), &api_secret).unwrap();
        build_mistral_client_for_test(&provider_record("mistral", None), &api_secret).unwrap();
    }

    #[test]
    fn rig_model_maps_to_provider_model_record_payload() {
        let provider = provider_record("openai", None);
        let model = Model {
            id: "gpt-5".to_string(),
            name: Some("GPT-5".to_string()),
            description: Some("flagship".to_string()),
            r#type: Some("chat".to_string()),
            created_at: Some(1),
            owned_by: Some("openai".to_string()),
            context_length: Some(272_000),
        };

        let mapped = provider_model_from_rig_model(&provider, model);

        assert_eq!(mapped.provider_id, provider.id);
        assert_eq!(mapped.model_id, "gpt-5");
        assert_eq!(mapped.display_name.as_deref(), Some("GPT-5"));
        assert!(mapped.enabled);
        assert!(mapped.capabilities.reasoning.is_some());
        assert!(mapped.metadata.raw.is_some());
    }

    #[test]
    fn native_openrouter_payload_keeps_supported_parameters() {
        let payload: OpenRouterModelsResponse = serde_json::from_value(json!({
            "data": [{
                "id": "openai/gpt-5",
                "name": "GPT-5",
                "created": 1,
                "context_length": 272000,
                "architecture": {
                    "input_modalities": ["text", "image", "file"]
                },
                "supported_parameters": ["tools", "reasoning"]
            }]
        }))
        .unwrap();

        let model = &payload.data[0];
        assert_eq!(model.context_length, Some(272000));
        assert_eq!(
            model.supported_parameters,
            vec!["tools".to_string(), "reasoning".to_string()]
        );
        assert_eq!(
            model
                .architecture
                .as_ref()
                .map(|architecture| architecture.input_modalities.as_slice()),
            Some(["text".to_string(), "image".to_string(), "file".to_string()].as_slice())
        );
    }

    #[test]
    fn native_gemini_payload_keeps_thinking_signal() {
        let payload: GeminiModelsResponse = serde_json::from_value(json!({
            "models": [{
                "name": "models/gemini-2.5-flash",
                "baseModelId": "gemini-2.5-flash",
                "displayName": "Gemini 2.5 Flash",
                "inputTokenLimit": 1048576,
                "supportedGenerationMethods": ["generateContent", "countTokens"],
                "thinking": true
            }]
        }))
        .unwrap();

        let model = &payload.models[0];
        assert_eq!(model.base_model_id.as_deref(), Some("gemini-2.5-flash"));
        assert!(model.supports_generate_content());
        assert_eq!(model.thinking, Some(true));
    }

    #[test]
    fn native_gemini_model_listing_keeps_only_generate_content_models() {
        let payload: GeminiModelsResponse = serde_json::from_value(json!({
            "models": [
                {
                    "name": "models/gemini-2.5-flash",
                    "baseModelId": "gemini-2.5-flash",
                    "displayName": "Gemini 2.5 Flash",
                    "supportedGenerationMethods": ["generateContent", "countTokens"],
                    "thinking": true
                },
                {
                    "name": "models/text-embedding-004",
                    "baseModelId": "text-embedding-004",
                    "displayName": "Text Embedding 004",
                    "supportedGenerationMethods": ["embedContent"],
                    "thinking": false
                }
            ]
        }))
        .unwrap();
        let provider = provider_record("gemini", None);
        let models = payload
            .models
            .into_iter()
            .filter_map(|model| provider_model_from_gemini_model(&provider, model))
            .collect::<Vec<_>>();

        assert_eq!(models.len(), 1);
        assert_eq!(models[0].model_id, "gemini-2.5-flash");
        assert_eq!(models[0].display_name.as_deref(), Some("Gemini 2.5 Flash"));
    }

    #[test]
    fn provider_url_joins_paths_without_losing_base_path() {
        let url = provider_url("https://example.com/openai/v1", "/models").unwrap();

        assert_eq!(url.as_str(), "https://example.com/openai/v1/models");
    }

    #[tokio::test]
    async fn no_listing_provider_returns_manual_required() {
        let provider = provider_record("moonshot", None);
        let err = fetch_provider_models(ProviderModelFetchRequest {
            provider,
            secrets: ProviderSecretValues::default(),
        })
        .await
        .expect_err("moonshot does not expose Rig model listing");

        assert!(matches!(
            err,
            ProviderModelFetchError::ManualModelsRequired { provider_kind }
                if provider_kind == "moonshot"
        ));
    }

    fn build_openai_client_for_test(
        provider: &ProviderRecord,
        secrets: &ProviderSecretValues,
    ) -> Result<openai::Client, ProviderModelFetchError> {
        apply_base_url(
            openai::Client::builder().api_key(required_secret(secrets, "api_key")?),
            &provider.settings,
        )?
        .build()
        .map_err(invalid_config)
    }

    fn build_anthropic_client_for_test(
        provider: &ProviderRecord,
        secrets: &ProviderSecretValues,
    ) -> Result<anthropic::Client, ProviderModelFetchError> {
        apply_base_url(
            anthropic::Client::builder().api_key(required_secret(secrets, "api_key")?),
            &provider.settings,
        )?
        .build()
        .map_err(invalid_config)
    }

    fn build_gemini_client_for_test(
        provider: &ProviderRecord,
        secrets: &ProviderSecretValues,
    ) -> Result<gemini::Client, ProviderModelFetchError> {
        apply_base_url(
            gemini::Client::builder().api_key(required_secret(secrets, "api_key")?),
            &provider.settings,
        )?
        .build()
        .map_err(invalid_config)
    }

    fn build_ollama_client_for_test(
        provider: &ProviderRecord,
        secrets: &ProviderSecretValues,
    ) -> Result<ollama::Client, ProviderModelFetchError> {
        apply_base_url(
            ollama::Client::builder().api_key(secrets.get("bearer_token").unwrap_or_default()),
            &provider.settings,
        )?
        .build()
        .map_err(invalid_config)
    }

    fn build_openrouter_client_for_test(
        provider: &ProviderRecord,
        secrets: &ProviderSecretValues,
    ) -> Result<openrouter::Client, ProviderModelFetchError> {
        apply_base_url(
            openrouter::Client::builder().api_key(required_secret(secrets, "api_key")?),
            &provider.settings,
        )?
        .build()
        .map_err(invalid_config)
    }

    fn build_deepseek_client_for_test(
        provider: &ProviderRecord,
        secrets: &ProviderSecretValues,
    ) -> Result<deepseek::Client, ProviderModelFetchError> {
        apply_base_url(
            deepseek::Client::builder().api_key(required_secret(secrets, "api_key")?),
            &provider.settings,
        )?
        .build()
        .map_err(invalid_config)
    }

    fn build_mistral_client_for_test(
        provider: &ProviderRecord,
        secrets: &ProviderSecretValues,
    ) -> Result<mistral::Client, ProviderModelFetchError> {
        apply_base_url(
            mistral::Client::builder().api_key(required_secret(secrets, "api_key")?),
            &provider.settings,
        )?
        .build()
        .map_err(invalid_config)
    }

    fn provider_record(kind: &str, base_url: Option<&str>) -> ProviderRecord {
        let dir = tempdir().unwrap();
        let store = FreshStore::open_in_dir(dir.path()).unwrap();
        store
            .repository()
            .insert_provider(NewProvider {
                kind: kind.to_string(),
                display_name: kind.to_string(),
                enabled: true,
                settings: ProviderSettingsPayload {
                    provider_kind: kind.to_string(),
                    fields: base_url
                        .map(|value| {
                            vec![ProviderSettingFieldValue {
                                key: "base_url".to_string(),
                                value: ProviderSettingValue::String {
                                    value: value.to_string(),
                                },
                            }]
                        })
                        .unwrap_or_default(),
                },
                secret_refs: ProviderSecretRefs { refs: Vec::new() },
            })
            .unwrap()
    }
}

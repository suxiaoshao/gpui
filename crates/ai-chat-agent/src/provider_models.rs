use std::collections::BTreeMap;

use ai_chat_core::{
    ProviderModelMetadata, ProviderRawPayload, ProviderSettingValue, ProviderSettingsPayload,
    conservative_model_capabilities,
};
use ai_chat_db::{NewProviderModel, ProviderRecord};
use rig_core::{
    client::ModelListingClient,
    model::{Model, ModelList, ModelListingError},
    providers::{anthropic, deepseek, gemini, mistral, ollama, openai, openrouter},
};
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
        "gemini" => {
            let client = apply_base_url(
                gemini::Client::builder().api_key(required_secret(&request.secrets, "api_key")?),
                &request.provider.settings,
            )?
            .build()
            .map_err(invalid_config)?;
            list_models(provider_kind, client).await?
        }
        "ollama" => {
            let token = request.secrets.get("bearer_token").unwrap_or_default();
            let client = apply_base_url(
                ollama::Client::builder().api_key(token),
                &request.provider.settings,
            )?
            .build()
            .map_err(invalid_config)?;
            list_models(provider_kind, client).await?
        }
        "openrouter" => {
            let client = apply_base_url(
                openrouter::Client::builder()
                    .api_key(required_secret(&request.secrets, "api_key")?),
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

pub fn provider_model_from_rig_model(provider: &ProviderRecord, model: Model) -> NewProviderModel {
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
        model_id: model.id,
        display_name: display_name.clone(),
        enabled: true,
        capabilities: conservative_model_capabilities(&provider.kind),
        metadata: ProviderModelMetadata {
            display_name,
            family: model.owned_by,
            raw,
        },
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use ai_chat_core::{
        ProviderSecretRefs, ProviderSettingFieldValue, ProviderSettingValue,
        ProviderSettingsPayload,
    };
    use ai_chat_db::{FreshStore, NewProvider};
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

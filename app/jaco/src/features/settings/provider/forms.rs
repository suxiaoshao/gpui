use std::collections::BTreeMap;

use gpui::{App, AppContext as _, Context, Entity, EntityId, Window};
use gpui_form::{
    ErrorParamValue, Form, FormRevision, FormVersion, GardeValidator, PrepareError as SubmitError,
    ValidationReport, ValidationTrigger,
};
use jaco_core::{
    ProviderSecretRefs, ProviderSettingFieldValue, ProviderSettingValue, ProviderSettingsPayload,
};

use crate::features::settings::form_validation::{
    JacoGardeMessageProvider, JacoValidationContext, garde_message,
};

use super::{catalog::ProviderFormKind, draft::ProviderFormSeed};

mod api_key;
mod custom_openai;
mod ollama;
mod secret;

pub(super) use api_key::ApiKeyProviderFormInput;
pub(super) use custom_openai::{
    ApiModeChoice, CustomOpenAiProviderFormInput, ProviderApiMode, localized_api_mode_choices,
};
pub(super) use ollama::OllamaProviderFormInput;
pub(super) use secret::{ProviderSecretInput, ProviderSecretValue};

const FIELD_NAME: &str = "name";
const FIELD_API_KEY: &str = "api_key";
const FIELD_BASE_URL: &str = "base_url";
const FIELD_BEARER_TOKEN: &str = "bearer_token";
const FIELD_API_MODE: &str = "api_mode";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum ProviderFormField {
    Name,
    ApiKey,
    BaseUrl,
    BearerToken,
    ApiMode,
}

impl ProviderFormField {
    pub(super) const fn key(self) -> &'static str {
        match self {
            Self::Name => FIELD_NAME,
            Self::ApiKey => FIELD_API_KEY,
            Self::BaseUrl => FIELD_BASE_URL,
            Self::BearerToken => FIELD_BEARER_TOKEN,
            Self::ApiMode => FIELD_API_MODE,
        }
    }

    pub(super) const fn label_key(self) -> &'static str {
        match self {
            Self::Name => "provider-field-name",
            Self::ApiKey => "provider-field-api-key",
            Self::BaseUrl => "provider-field-base-url",
            Self::BearerToken => "provider-field-bearer-token",
            Self::ApiMode => "provider-field-api-mode",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProviderValidationKind {
    Required,
    UrlInvalid,
    UrlScheme,
}

impl ProviderValidationKind {
    const fn message_key(self) -> &'static str {
        match self {
            Self::Required => "provider-validation-required",
            Self::UrlInvalid => "provider-validation-url-invalid",
            Self::UrlScheme => "provider-validation-url-scheme",
        }
    }
}

#[derive(Clone)]
pub(super) enum ProviderSettingsForm {
    ApiKey(Entity<Form<ApiKeyProviderFormInput>>),
    Ollama(Entity<Form<OllamaProviderFormInput>>),
    CustomOpenAi(Entity<Form<CustomOpenAiProviderFormInput>>),
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ProviderValidationDependencies {
    pub(super) secret_refs: ProviderSecretRefs,
}

impl Default for ProviderValidationDependencies {
    fn default() -> Self {
        Self {
            secret_refs: ProviderSecretRefs { refs: Vec::new() },
        }
    }
}

pub(super) type ProviderValidationContext = JacoValidationContext<ProviderValidationDependencies>;

fn provider_validation_context(secret_refs: ProviderSecretRefs) -> ProviderValidationContext {
    ProviderValidationContext::new(ProviderValidationDependencies { secret_refs })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ProviderSettingsFormOutput {
    ApiKey {
        enabled: bool,
        api_key: ProviderSecretValue,
        base_url: String,
    },
    Ollama {
        enabled: bool,
        base_url: String,
        bearer_token: ProviderSecretValue,
    },
    CustomOpenAi {
        enabled: bool,
        name: String,
        api_key: ProviderSecretValue,
        base_url: String,
        api_mode: ProviderApiMode,
    },
}

pub(super) struct ProviderPreparedSubmit {
    pub(super) version: FormVersion,
    pub(super) output: ProviderSettingsFormOutput,
}

impl ProviderSettingsFormOutput {
    pub(super) fn enabled(&self) -> bool {
        match self {
            Self::ApiKey { enabled, .. }
            | Self::Ollama { enabled, .. }
            | Self::CustomOpenAi { enabled, .. } => *enabled,
        }
    }

    pub(super) fn persistent_fields(&self) -> BTreeMap<String, ProviderSettingValue> {
        match self {
            Self::ApiKey { base_url, .. } | Self::Ollama { base_url, .. } => BTreeMap::from([(
                FIELD_BASE_URL.to_string(),
                ProviderSettingValue::String {
                    value: base_url.trim().to_string(),
                },
            )]),
            Self::CustomOpenAi {
                name,
                base_url,
                api_mode,
                ..
            } => BTreeMap::from([
                (
                    FIELD_NAME.to_string(),
                    ProviderSettingValue::String {
                        value: name.trim().to_string(),
                    },
                ),
                (
                    FIELD_BASE_URL.to_string(),
                    ProviderSettingValue::String {
                        value: base_url.trim().to_string(),
                    },
                ),
                (
                    FIELD_API_MODE.to_string(),
                    ProviderSettingValue::String {
                        value: api_mode.key().to_string(),
                    },
                ),
            ]),
        }
    }

    pub(super) fn settings_payload(&self, provider_kind: &str) -> ProviderSettingsPayload {
        ProviderSettingsPayload {
            provider_kind: provider_kind.to_string(),
            fields: self
                .persistent_fields()
                .into_iter()
                .map(|(key, value)| ProviderSettingFieldValue { key, value })
                .collect(),
        }
    }

    pub(super) fn display_name(&self, fallback: &str) -> String {
        match self {
            Self::CustomOpenAi { name, .. } => name.trim().to_string(),
            Self::ApiKey { .. } | Self::Ollama { .. } => fallback.to_string(),
        }
    }

    pub(super) fn secret_fields(&self) -> Vec<ProviderSecretValue> {
        match self {
            Self::ApiKey { api_key, .. } | Self::CustomOpenAi { api_key, .. } => {
                vec![api_key.clone()]
            }
            Self::Ollama { bearer_token, .. } => vec![bearer_token.clone()],
        }
    }
}

impl ProviderSettingsForm {
    pub(super) fn new<T>(
        form_kind: ProviderFormKind,
        seed: &ProviderFormSeed,
        _window: &mut Window,
        cx: &mut Context<T>,
    ) -> Self
    where
        T: 'static,
    {
        match form_kind {
            ProviderFormKind::ApiKey => Self::ApiKey(cx.new(|_| {
                Form::new(ApiKeyProviderFormInput::from_seed(seed)).with_validator(
                    GardeValidator::<ApiKeyProviderFormInput, JacoGardeMessageProvider>::new(
                        provider_validation_context(seed.existing_secret_refs.clone()),
                    ),
                )
            })),
            ProviderFormKind::Ollama => Self::Ollama(cx.new(|_| {
                Form::new(OllamaProviderFormInput::from_seed(seed)).with_validator(
                    GardeValidator::<OllamaProviderFormInput, JacoGardeMessageProvider>::new(
                        provider_validation_context(seed.existing_secret_refs.clone()),
                    ),
                )
            })),
            ProviderFormKind::CustomOpenAiCompatible => Self::CustomOpenAi(cx.new(|_| {
                Form::new(CustomOpenAiProviderFormInput::from_seed(seed)).with_validator(
                    GardeValidator::<CustomOpenAiProviderFormInput, JacoGardeMessageProvider>::new(
                        provider_validation_context(seed.existing_secret_refs.clone()),
                    ),
                )
            })),
        }
    }

    pub(super) fn entity_id(&self) -> EntityId {
        match self {
            Self::ApiKey(form) => form.entity_id(),
            Self::Ollama(form) => form.entity_id(),
            Self::CustomOpenAi(form) => form.entity_id(),
        }
    }

    pub(super) fn enabled(&self, cx: &App) -> bool {
        match self {
            Self::ApiKey(form) => ApiKeyProviderFormInput::ENABLED.get(form, cx),
            Self::Ollama(form) => OllamaProviderFormInput::ENABLED.get(form, cx),
            Self::CustomOpenAi(form) => CustomOpenAiProviderFormInput::ENABLED.get(form, cx),
        }
    }

    pub(super) fn is_dirty(&self, cx: &App) -> bool {
        match self {
            Self::ApiKey(form) => form.read(cx).is_dirty(),
            Self::Ollama(form) => form.read(cx).is_dirty(),
            Self::CustomOpenAi(form) => form.read(cx).is_dirty(),
        }
    }

    pub(super) fn rebase_if_current(
        &self,
        version: FormVersion,
        output: &ProviderSettingsFormOutput,
        cx: &mut App,
    ) -> bool {
        match (self, output) {
            (
                Self::ApiKey(form),
                ProviderSettingsFormOutput::ApiKey {
                    enabled, base_url, ..
                },
            ) => form.update(cx, |form, cx| {
                form.rebase_if_current(
                    version,
                    ApiKeyProviderFormInput {
                        enabled: *enabled,
                        api_key: ProviderSecretValue::new(
                            ProviderFormField::ApiKey,
                            String::new(),
                            false,
                        ),
                        base_url: base_url.clone(),
                    },
                    cx,
                )
            }),
            (
                Self::Ollama(form),
                ProviderSettingsFormOutput::Ollama {
                    enabled, base_url, ..
                },
            ) => form.update(cx, |form, cx| {
                form.rebase_if_current(
                    version,
                    OllamaProviderFormInput {
                        enabled: *enabled,
                        base_url: base_url.clone(),
                        bearer_token: ProviderSecretValue::new(
                            ProviderFormField::BearerToken,
                            String::new(),
                            false,
                        ),
                    },
                    cx,
                )
            }),
            (
                Self::CustomOpenAi(form),
                ProviderSettingsFormOutput::CustomOpenAi {
                    enabled,
                    name,
                    base_url,
                    api_mode,
                    ..
                },
            ) => form.update(cx, |form, cx| {
                form.rebase_if_current(
                    version,
                    CustomOpenAiProviderFormInput {
                        enabled: *enabled,
                        name: name.clone(),
                        api_key: ProviderSecretValue::new(
                            ProviderFormField::ApiKey,
                            String::new(),
                            false,
                        ),
                        base_url: base_url.clone(),
                        api_mode: Some(*api_mode),
                    },
                    cx,
                )
            }),
            _ => false,
        }
    }

    pub(super) fn prepare_submit(
        &self,
        secret_refs: ProviderSecretRefs,
        cx: &mut App,
    ) -> Result<ProviderPreparedSubmit, SubmitError> {
        match self {
            Self::ApiKey(form) => {
                let prepared = form.update(cx, |form, cx| {
                    form.replace_validator(
                        GardeValidator::<ApiKeyProviderFormInput, JacoGardeMessageProvider>::new(
                            provider_validation_context(secret_refs.clone()),
                        ),
                        cx,
                    );
                    form.prepare(cx)
                })?;
                let (version, output) = prepared
                    .map(|output| ProviderSettingsFormOutput::ApiKey {
                        enabled: output.enabled,
                        api_key: output.api_key,
                        base_url: output.base_url.trim().to_string(),
                    })
                    .into_parts();
                Ok(ProviderPreparedSubmit { version, output })
            }
            Self::Ollama(form) => {
                let prepared = form.update(cx, |form, cx| {
                    form.replace_validator(
                        GardeValidator::<OllamaProviderFormInput, JacoGardeMessageProvider>::new(
                            provider_validation_context(secret_refs.clone()),
                        ),
                        cx,
                    );
                    form.prepare(cx)
                })?;
                let (version, output) = prepared
                    .map(|output| ProviderSettingsFormOutput::Ollama {
                        enabled: output.enabled,
                        base_url: output.base_url.trim().to_string(),
                        bearer_token: output.bearer_token,
                    })
                    .into_parts();
                Ok(ProviderPreparedSubmit { version, output })
            }
            Self::CustomOpenAi(form) => {
                let prepared = form.update(cx, |form, cx| {
                    form.replace_validator(
                        GardeValidator::<
                            CustomOpenAiProviderFormInput,
                            JacoGardeMessageProvider,
                        >::new(provider_validation_context(secret_refs.clone())),
                        cx,
                    );
                    form.prepare(cx)
                })?;
                let (version, output) = prepared
                    .map(|output| ProviderSettingsFormOutput::CustomOpenAi {
                        enabled: output.enabled,
                        name: output.name.trim().to_string(),
                        api_key: output.api_key,
                        base_url: output.base_url.trim().to_string(),
                        api_mode: output.api_mode.unwrap_or_default(),
                    })
                    .into_parts();
                Ok(ProviderPreparedSubmit { version, output })
            }
        }
    }

    pub(super) fn set_enabled(&self, enabled: bool, _window: &mut Window, cx: &mut App) {
        match self {
            Self::ApiKey(form) => {
                ApiKeyProviderFormInput::ENABLED.set(form, enabled, cx);
            }
            Self::Ollama(form) => {
                OllamaProviderFormInput::ENABLED.set(form, enabled, cx);
            }
            Self::CustomOpenAi(form) => {
                CustomOpenAiProviderFormInput::ENABLED.set(form, enabled, cx);
            }
        }
    }

    pub(super) fn revision(&self, cx: &App) -> FormRevision {
        match self {
            Self::ApiKey(form) => form.read(cx).revision(),
            Self::Ollama(form) => form.read(cx).revision(),
            Self::CustomOpenAi(form) => form.read(cx).revision(),
        }
    }

    pub(super) fn validation_report(&self, cx: &App) -> ValidationReport {
        match self {
            Self::ApiKey(form) => form.read(cx).validation_report(),
            Self::Ollama(form) => form.read(cx).validation_report(),
            Self::CustomOpenAi(form) => form.read(cx).validation_report(),
        }
    }

    #[cfg(test)]
    pub(super) fn current_output(&self, cx: &App) -> ProviderSettingsFormOutput {
        match self {
            Self::ApiKey(form) => ProviderSettingsFormOutput::ApiKey {
                enabled: ApiKeyProviderFormInput::ENABLED.get(form, cx),
                api_key: ApiKeyProviderFormInput::API_KEY.get(form, cx),
                base_url: ApiKeyProviderFormInput::BASE_URL.get(form, cx),
            },
            Self::Ollama(form) => ProviderSettingsFormOutput::Ollama {
                enabled: OllamaProviderFormInput::ENABLED.get(form, cx),
                base_url: OllamaProviderFormInput::BASE_URL.get(form, cx),
                bearer_token: OllamaProviderFormInput::BEARER_TOKEN.get(form, cx),
            },
            Self::CustomOpenAi(form) => ProviderSettingsFormOutput::CustomOpenAi {
                enabled: CustomOpenAiProviderFormInput::ENABLED.get(form, cx),
                name: CustomOpenAiProviderFormInput::NAME.get(form, cx),
                api_key: CustomOpenAiProviderFormInput::API_KEY.get(form, cx),
                base_url: CustomOpenAiProviderFormInput::BASE_URL.get(form, cx),
                api_mode: CustomOpenAiProviderFormInput::API_MODE
                    .get(form, cx)
                    .unwrap_or_default(),
            },
        }
    }

    pub(super) fn validate_current(
        &self,
        secret_refs: ProviderSecretRefs,
        _window: &mut Window,
        cx: &mut App,
    ) -> ValidationReport {
        match self {
            Self::ApiKey(form) => form.update(cx, |form, cx| {
                form.replace_validator(
                    GardeValidator::<ApiKeyProviderFormInput, JacoGardeMessageProvider>::new(
                        provider_validation_context(secret_refs.clone()),
                    ),
                    cx,
                );
                form.validate(ValidationTrigger::Submit, cx);
                form.validation_report()
            }),
            Self::Ollama(form) => form.update(cx, |form, cx| {
                form.replace_validator(
                    GardeValidator::<OllamaProviderFormInput, JacoGardeMessageProvider>::new(
                        provider_validation_context(secret_refs.clone()),
                    ),
                    cx,
                );
                form.validate(ValidationTrigger::Submit, cx);
                form.validation_report()
            }),
            Self::CustomOpenAi(form) => form.update(cx, |form, cx| {
                form.replace_validator(
                    GardeValidator::<CustomOpenAiProviderFormInput, JacoGardeMessageProvider>::new(
                        provider_validation_context(secret_refs.clone()),
                    ),
                    cx,
                );
                form.validate(ValidationTrigger::Submit, cx);
                form.validation_report()
            }),
        }
    }
}

impl ApiKeyProviderFormInput {
    fn from_seed(seed: &ProviderFormSeed) -> Self {
        Self {
            enabled: seed.enabled,
            api_key: ProviderSecretValue::new(ProviderFormField::ApiKey, String::new(), false),
            base_url: seed.field_string(FIELD_BASE_URL),
        }
    }
}

impl OllamaProviderFormInput {
    fn from_seed(seed: &ProviderFormSeed) -> Self {
        let base_url = seed.field_string(FIELD_BASE_URL);
        Self {
            enabled: seed.enabled,
            base_url: if base_url.is_empty() {
                "http://localhost:11434".to_string()
            } else {
                base_url
            },
            bearer_token: ProviderSecretValue::new(
                ProviderFormField::BearerToken,
                String::new(),
                false,
            ),
        }
    }
}

impl CustomOpenAiProviderFormInput {
    fn from_seed(seed: &ProviderFormSeed) -> Self {
        Self {
            enabled: seed.enabled,
            name: seed.field_string(FIELD_NAME),
            api_key: ProviderSecretValue::new(ProviderFormField::ApiKey, String::new(), false),
            base_url: seed.field_string(FIELD_BASE_URL),
            api_mode: Some(ProviderApiMode::from_key(
                &seed.field_string(FIELD_API_MODE),
            )),
        }
    }
}

fn provider_validation_error(
    _context: &ProviderValidationContext,
    field: ProviderFormField,
    kind: ProviderValidationKind,
) -> garde::Error {
    garde_message(
        kind.message_key(),
        [("field", ErrorParamValue::from(field.label_key()))],
    )
}

pub(super) fn validate_required_provider_text(
    value: &str,
    context: &ProviderValidationContext,
) -> garde::Result {
    if value.trim().is_empty() {
        Err(provider_validation_error(
            context,
            ProviderFormField::Name,
            ProviderValidationKind::Required,
        ))
    } else {
        Ok(())
    }
}

pub(super) fn validate_provider_secret(
    secret: &ProviderSecretValue,
    context: &ProviderValidationContext,
) -> garde::Result {
    let has_saved_secret = !secret.changed
        && context
            .dependencies
            .secret_refs
            .refs
            .iter()
            .any(|saved| saved.key == secret.key());
    if !has_saved_secret && secret.value.trim().is_empty() {
        Err(provider_validation_error(
            context,
            secret.field,
            ProviderValidationKind::Required,
        ))
    } else {
        Ok(())
    }
}

pub(super) fn validate_required_provider_url(
    value: &str,
    context: &ProviderValidationContext,
) -> garde::Result {
    let value = value.trim();
    if value.is_empty() {
        return Err(provider_validation_error(
            context,
            ProviderFormField::BaseUrl,
            ProviderValidationKind::Required,
        ));
    }
    validate_provider_url(value, context)
}

pub(super) fn validate_optional_provider_url(
    value: &str,
    context: &ProviderValidationContext,
) -> garde::Result {
    let value = value.trim();
    if value.is_empty() {
        Ok(())
    } else {
        validate_provider_url(value, context)
    }
}

fn validate_provider_url(value: &str, context: &ProviderValidationContext) -> garde::Result {
    match url::Url::parse(value) {
        Ok(url) if matches!(url.scheme(), "http" | "https") => Ok(()),
        Ok(_) => Err(provider_validation_error(
            context,
            ProviderFormField::BaseUrl,
            ProviderValidationKind::UrlScheme,
        )),
        Err(_) => Err(provider_validation_error(
            context,
            ProviderFormField::BaseUrl,
            ProviderValidationKind::UrlInvalid,
        )),
    }
}

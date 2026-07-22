use std::collections::BTreeMap;

use gpui::{App, AppContext as _, Context, Entity, EntityId, Window};
use gpui_form::typed::{
    ErrorParamValue, FormRevision, FormStore as _, SubmitError, SubmitTransform, TransformReport,
    ValidationReport, ValidationScope, ValidationTrigger,
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

pub(super) use api_key::{
    ApiKeyProviderFormInput, ApiKeyProviderFormInputField, ApiKeyProviderFormStore,
};
pub(super) use custom_openai::{
    ApiModeChoice, CustomOpenAiProviderFormInput, CustomOpenAiProviderFormInputField,
    CustomOpenAiProviderFormStore, ProviderApiMode, localized_api_mode_choices,
};
pub(super) use ollama::{
    OllamaProviderFormInput, OllamaProviderFormInputField, OllamaProviderFormStore,
};
pub(super) use secret::{ProviderSecretInputState, ProviderSecretValue};

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
    ApiKey(Entity<ApiKeyProviderFormStore>),
    Ollama(Entity<OllamaProviderFormStore>),
    CustomOpenAi(Entity<CustomOpenAiProviderFormStore>),
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

#[derive(Clone, Debug, Default)]
pub(super) struct ApiKeyProviderTransform;

#[derive(Clone, Debug, Default)]
pub(super) struct OllamaProviderTransform;

#[derive(Clone, Debug, Default)]
pub(super) struct CustomOpenAiProviderTransform;

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

impl SubmitTransform<ApiKeyProviderFormInput> for ApiKeyProviderTransform {
    type Output = ApiKeyProviderFormInput;

    fn transform(&self, draft: &ApiKeyProviderFormInput) -> Result<Self::Output, TransformReport> {
        Ok(ApiKeyProviderFormInput {
            enabled: draft.enabled,
            api_key: draft.api_key.clone(),
            base_url: draft.base_url.trim().to_string(),
        })
    }
}

impl SubmitTransform<OllamaProviderFormInput> for OllamaProviderTransform {
    type Output = OllamaProviderFormInput;

    fn transform(&self, draft: &OllamaProviderFormInput) -> Result<Self::Output, TransformReport> {
        Ok(OllamaProviderFormInput {
            enabled: draft.enabled,
            base_url: draft.base_url.trim().to_string(),
            bearer_token: draft.bearer_token.clone(),
        })
    }
}

impl SubmitTransform<CustomOpenAiProviderFormInput> for CustomOpenAiProviderTransform {
    type Output = CustomOpenAiProviderFormInput;

    fn transform(
        &self,
        draft: &CustomOpenAiProviderFormInput,
    ) -> Result<Self::Output, TransformReport> {
        Ok(CustomOpenAiProviderFormInput {
            enabled: draft.enabled,
            name: draft.name.trim().to_string(),
            api_key: draft.api_key.clone(),
            base_url: draft.base_url.trim().to_string(),
            api_mode: draft.api_mode,
        })
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
            ProviderFormKind::ApiKey => Self::ApiKey(cx.new(|cx| {
                ApiKeyProviderFormStore::from_value_with_validation_context(
                    ApiKeyProviderFormInput::from_seed(seed),
                    provider_validation_context(seed.existing_secret_refs.clone()),
                    cx,
                )
            })),
            ProviderFormKind::Ollama => Self::Ollama(cx.new(|cx| {
                OllamaProviderFormStore::from_value_with_validation_context(
                    OllamaProviderFormInput::from_seed(seed),
                    provider_validation_context(seed.existing_secret_refs.clone()),
                    cx,
                )
            })),
            ProviderFormKind::CustomOpenAiCompatible => Self::CustomOpenAi(cx.new(|cx| {
                CustomOpenAiProviderFormStore::from_value_with_validation_context(
                    CustomOpenAiProviderFormInput::from_seed(seed),
                    provider_validation_context(seed.existing_secret_refs.clone()),
                    cx,
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
            Self::ApiKey(form) => ApiKeyProviderFormStore::enabled_field(form)
                .value(cx)
                .expect("provider enabled field is available"),
            Self::Ollama(form) => OllamaProviderFormStore::enabled_field(form)
                .value(cx)
                .expect("provider enabled field is available"),
            Self::CustomOpenAi(form) => CustomOpenAiProviderFormStore::enabled_field(form)
                .value(cx)
                .expect("provider enabled field is available"),
        }
    }

    pub(super) fn is_dirty(&self, cx: &App) -> bool {
        match self {
            Self::ApiKey(form) => form.read(cx).is_dirty(),
            Self::Ollama(form) => form.read(cx).is_dirty(),
            Self::CustomOpenAi(form) => form.read(cx).is_dirty(),
        }
    }

    pub(super) fn revision(&self, cx: &App) -> FormRevision {
        match self {
            Self::ApiKey(form) => form.read(cx).revision(),
            Self::Ollama(form) => form.read(cx).revision(),
            Self::CustomOpenAi(form) => form.read(cx).revision(),
        }
    }

    pub(super) fn rebase_if_revision(
        &self,
        revision: FormRevision,
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
                form.rebase_if_revision(
                    revision,
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
                form.rebase_if_revision(
                    revision,
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
                form.rebase_if_revision(
                    revision,
                    CustomOpenAiProviderFormInput {
                        enabled: *enabled,
                        name: name.clone(),
                        api_key: ProviderSecretValue::new(
                            ProviderFormField::ApiKey,
                            String::new(),
                            false,
                        ),
                        base_url: base_url.clone(),
                        api_mode: *api_mode,
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
    ) -> Result<ProviderSettingsFormOutput, SubmitError> {
        match self {
            Self::ApiKey(form) => {
                let output = form.update(cx, |form, cx| {
                    form.set_validation_context(
                        provider_validation_context(secret_refs.clone()),
                        cx,
                    );
                    form.prepare_submit(cx)
                })?;
                let output = ProviderSettingsFormOutput::ApiKey {
                    enabled: output.enabled,
                    api_key: output.api_key,
                    base_url: output.base_url,
                };
                Ok(output)
            }
            Self::Ollama(form) => {
                let output = form.update(cx, |form, cx| {
                    form.set_validation_context(
                        provider_validation_context(secret_refs.clone()),
                        cx,
                    );
                    form.prepare_submit(cx)
                })?;
                let output = ProviderSettingsFormOutput::Ollama {
                    enabled: output.enabled,
                    base_url: output.base_url,
                    bearer_token: output.bearer_token,
                };
                Ok(output)
            }
            Self::CustomOpenAi(form) => {
                let output = form.update(cx, |form, cx| {
                    form.set_validation_context(
                        provider_validation_context(secret_refs.clone()),
                        cx,
                    );
                    form.prepare_submit(cx)
                })?;
                let output = ProviderSettingsFormOutput::CustomOpenAi {
                    enabled: output.enabled,
                    name: output.name,
                    api_key: output.api_key,
                    base_url: output.base_url,
                    api_mode: output.api_mode,
                };
                Ok(output)
            }
        }
    }

    pub(super) fn set_enabled(&self, enabled: bool, _window: &mut Window, cx: &mut App) {
        match self {
            Self::ApiKey(form) => {
                let _ = ApiKeyProviderFormStore::enabled_field(form).set_user_value(enabled, cx);
            }
            Self::Ollama(form) => {
                let _ = OllamaProviderFormStore::enabled_field(form).set_user_value(enabled, cx);
            }
            Self::CustomOpenAi(form) => {
                let _ =
                    CustomOpenAiProviderFormStore::enabled_field(form).set_user_value(enabled, cx);
            }
        }
    }

    #[cfg(test)]
    pub(super) fn current_output(&self, cx: &App) -> ProviderSettingsFormOutput {
        match self {
            Self::ApiKey(form) => ProviderSettingsFormOutput::ApiKey {
                enabled: ApiKeyProviderFormStore::enabled_field(form)
                    .value(cx)
                    .unwrap(),
                api_key: ApiKeyProviderFormStore::api_key_field(form)
                    .value(cx)
                    .unwrap(),
                base_url: ApiKeyProviderFormStore::base_url_field(form)
                    .value(cx)
                    .unwrap(),
            },
            Self::Ollama(form) => ProviderSettingsFormOutput::Ollama {
                enabled: OllamaProviderFormStore::enabled_field(form)
                    .value(cx)
                    .unwrap(),
                base_url: OllamaProviderFormStore::base_url_field(form)
                    .value(cx)
                    .unwrap(),
                bearer_token: OllamaProviderFormStore::bearer_token_field(form)
                    .value(cx)
                    .unwrap(),
            },
            Self::CustomOpenAi(form) => ProviderSettingsFormOutput::CustomOpenAi {
                enabled: CustomOpenAiProviderFormStore::enabled_field(form)
                    .value(cx)
                    .unwrap(),
                name: CustomOpenAiProviderFormStore::name_field(form)
                    .value(cx)
                    .unwrap(),
                api_key: CustomOpenAiProviderFormStore::api_key_field(form)
                    .value(cx)
                    .unwrap(),
                base_url: CustomOpenAiProviderFormStore::base_url_field(form)
                    .value(cx)
                    .unwrap(),
                api_mode: CustomOpenAiProviderFormStore::api_mode_field(form)
                    .value(cx)
                    .unwrap(),
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
                form.set_validation_context(provider_validation_context(secret_refs.clone()), cx);
                form.validate(ValidationTrigger::Submit, ValidationScope::Form, cx);
                form.validation_report()
            }),
            Self::Ollama(form) => form.update(cx, |form, cx| {
                form.set_validation_context(provider_validation_context(secret_refs.clone()), cx);
                form.validate(ValidationTrigger::Submit, ValidationScope::Form, cx);
                form.validation_report()
            }),
            Self::CustomOpenAi(form) => form.update(cx, |form, cx| {
                form.set_validation_context(provider_validation_context(secret_refs.clone()), cx);
                form.validate(ValidationTrigger::Submit, ValidationScope::Form, cx);
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
            api_mode: ProviderApiMode::from_key(&seed.field_string(FIELD_API_MODE)),
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

use std::collections::BTreeMap;

use ai_chat_core::{
    ProviderSecretRefs, ProviderSettingFieldValue, ProviderSettingValue, ProviderSettingsPayload,
};
use fluent_bundle::FluentArgs;
use gpui::{App, AppContext as _, Context, Entity, EntityId, SharedString, Task, Window};
use gpui_form::{
    FieldChangeCause, FieldError, FormMeta, FormStore as _, SubmitError, SubmitStart,
    ValidationSource, ValidationTrigger,
};

use crate::foundation::I18n;

use super::{
    catalog::ProviderFormKind,
    draft::{ProviderDraft, ProviderDraftValue},
};

mod api_key;
mod custom_openai;
mod ollama;
mod secret;

pub(super) use api_key::{
    ApiKeyProviderFormEvent, ApiKeyProviderFormField, ApiKeyProviderFormInput,
    ApiKeyProviderFormStore,
};
pub(super) use custom_openai::{
    ApiModeChoice, CustomOpenAiProviderFormEvent, CustomOpenAiProviderFormField,
    CustomOpenAiProviderFormInput, CustomOpenAiProviderFormStore, ProviderApiMode,
};
pub(super) use ollama::{
    OllamaProviderFormEvent, OllamaProviderFormField, OllamaProviderFormInput,
    OllamaProviderFormStore,
};
pub(super) use secret::{ProviderSecretInputBinding, ProviderSecretValue};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProviderValidationIssue {
    pub(super) field: ProviderFormField,
    pub(super) kind: ProviderValidationKind,
    pub(super) field_label: SharedString,
    pub(super) message: SharedString,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ProviderValidationKind {
    Required,
    UrlInvalid,
    UrlScheme,
}

impl ProviderValidationKind {
    const fn code(self) -> &'static str {
        match self {
            Self::Required => "required",
            Self::UrlInvalid => "url_invalid",
            Self::UrlScheme => "url_scheme",
        }
    }

    const fn message_key(self) -> &'static str {
        match self {
            Self::Required => "provider-validation-required",
            Self::UrlInvalid => "provider-validation-url-invalid",
            Self::UrlScheme => "provider-validation-url-scheme",
        }
    }
}

pub(super) enum ProviderSettingsForm {
    ApiKey(Entity<ApiKeyProviderFormStore>),
    Ollama(Entity<OllamaProviderFormStore>),
    CustomOpenAi(Entity<CustomOpenAiProviderFormStore>),
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

impl ProviderSettingsFormOutput {
    pub(super) fn enabled(&self) -> bool {
        match self {
            Self::ApiKey { enabled, .. }
            | Self::Ollama { enabled, .. }
            | Self::CustomOpenAi { enabled, .. } => *enabled,
        }
    }

    pub(super) fn persistent_fields(&self) -> BTreeMap<String, ProviderDraftValue> {
        match self {
            Self::ApiKey { base_url, .. } | Self::Ollama { base_url, .. } => BTreeMap::from([(
                FIELD_BASE_URL.to_string(),
                ProviderDraftValue::String(base_url.trim().to_string()),
            )]),
            Self::CustomOpenAi {
                name,
                base_url,
                api_mode,
                ..
            } => BTreeMap::from([
                (
                    FIELD_NAME.to_string(),
                    ProviderDraftValue::String(name.trim().to_string()),
                ),
                (
                    FIELD_BASE_URL.to_string(),
                    ProviderDraftValue::String(base_url.trim().to_string()),
                ),
                (
                    FIELD_API_MODE.to_string(),
                    ProviderDraftValue::String(api_mode.key().to_string()),
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
                .map(|(key, value)| ProviderSettingFieldValue {
                    key,
                    value: match value {
                        ProviderDraftValue::String(value) => ProviderSettingValue::String { value },
                        ProviderDraftValue::Bool(value) => ProviderSettingValue::Bool { value },
                        ProviderDraftValue::Number(value) => ProviderSettingValue::Number { value },
                    },
                })
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

    pub(super) fn dirty_secret_keys(&self) -> Vec<&'static str> {
        self.secret_fields()
            .iter()
            .filter_map(|secret| secret.changed.then_some(secret.key()))
            .collect()
    }

    pub(super) fn validate(
        &self,
        secret_refs: &ProviderSecretRefs,
        i18n: &I18n,
    ) -> Result<(), ProviderValidationIssue> {
        match self {
            Self::ApiKey {
                api_key, base_url, ..
            } => {
                require_secret(api_key, secret_refs, i18n)?;
                validate_optional_base_url(ProviderFormField::BaseUrl, base_url, i18n)?;
                Ok(())
            }
            Self::Ollama { base_url, .. } => {
                require_base_url(ProviderFormField::BaseUrl, base_url, i18n)?;
                Ok(())
            }
            Self::CustomOpenAi {
                name,
                api_key,
                base_url,
                ..
            } => {
                require_text(ProviderFormField::Name, name, i18n)?;
                require_secret(api_key, secret_refs, i18n)?;
                require_base_url(ProviderFormField::BaseUrl, base_url, i18n)?;
                Ok(())
            }
        }
    }
}

impl ProviderSettingsForm {
    pub(super) fn new<T>(
        form_kind: ProviderFormKind,
        draft: &ProviderDraft,
        window: &mut Window,
        cx: &mut Context<T>,
    ) -> Self
    where
        T: 'static,
    {
        match form_kind {
            ProviderFormKind::ApiKey => Self::ApiKey(cx.new(|cx| {
                ApiKeyProviderFormStore::from_value(
                    ApiKeyProviderFormInput::from_draft(draft),
                    window,
                    cx,
                )
            })),
            ProviderFormKind::Ollama => Self::Ollama(cx.new(|cx| {
                OllamaProviderFormStore::from_value(
                    OllamaProviderFormInput::from_draft(draft),
                    window,
                    cx,
                )
            })),
            ProviderFormKind::CustomOpenAiCompatible => Self::CustomOpenAi(cx.new(|cx| {
                CustomOpenAiProviderFormStore::from_value(
                    CustomOpenAiProviderFormInput::from_draft(draft),
                    window,
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
            Self::ApiKey(form) => form.read(cx).enabled_value(),
            Self::Ollama(form) => form.read(cx).enabled_value(),
            Self::CustomOpenAi(form) => form.read(cx).enabled_value(),
        }
    }

    pub(super) fn is_submitting(&self, cx: &App) -> bool {
        match self {
            Self::ApiKey(form) => form.read(cx).is_submitting(),
            Self::Ollama(form) => form.read(cx).is_submitting(),
            Self::CustomOpenAi(form) => form.read(cx).is_submitting(),
        }
    }

    pub(super) fn submit_async_save<H>(
        &self,
        secret_refs: ProviderSecretRefs,
        handler: H,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<SubmitStart, SubmitError<ProviderValidationIssue>>
    where
        H: FnOnce(ProviderSettingsFormOutput, &mut Window, &mut App) -> Task<Result<(), String>>
            + 'static,
    {
        match self {
            Self::ApiKey(form) => form.update(cx, |form, cx| {
                form.submit_async(
                    move |output, window, cx| {
                        let output = ProviderSettingsFormOutput::ApiKey {
                            enabled: output.enabled,
                            api_key: output.api_key,
                            base_url: output.base_url,
                        };
                        output.validate(&secret_refs, cx.global::<I18n>())?;
                        Ok(handler(output, window, cx))
                    },
                    window,
                    cx,
                )
            }),
            Self::Ollama(form) => form.update(cx, |form, cx| {
                form.submit_async(
                    move |output, window, cx| {
                        let output = ProviderSettingsFormOutput::Ollama {
                            enabled: output.enabled,
                            base_url: output.base_url,
                            bearer_token: output.bearer_token,
                        };
                        output.validate(&secret_refs, cx.global::<I18n>())?;
                        Ok(handler(output, window, cx))
                    },
                    window,
                    cx,
                )
            }),
            Self::CustomOpenAi(form) => form.update(cx, |form, cx| {
                form.submit_async(
                    move |output, window, cx| {
                        let output = ProviderSettingsFormOutput::CustomOpenAi {
                            enabled: output.enabled,
                            name: output.name,
                            api_key: output.api_key,
                            base_url: output.base_url,
                            api_mode: output.api_mode,
                        };
                        output.validate(&secret_refs, cx.global::<I18n>())?;
                        Ok(handler(output, window, cx))
                    },
                    window,
                    cx,
                )
            }),
        }
    }

    pub(super) fn set_enabled(&self, enabled: bool, window: &mut Window, cx: &mut App) {
        match self {
            Self::ApiKey(form) => form.update(cx, |form, cx| {
                form.set_enabled_value(enabled, FieldChangeCause::UserInput, window, cx);
            }),
            Self::Ollama(form) => form.update(cx, |form, cx| {
                form.set_enabled_value(enabled, FieldChangeCause::UserInput, window, cx);
            }),
            Self::CustomOpenAi(form) => form.update(cx, |form, cx| {
                form.set_enabled_value(enabled, FieldChangeCause::UserInput, window, cx);
            }),
        }
    }

    pub(super) fn current_output(&self, cx: &App) -> ProviderSettingsFormOutput {
        match self {
            Self::ApiKey(form) => {
                let form = form.read(cx);
                let input = form.draft();
                ProviderSettingsFormOutput::ApiKey {
                    enabled: input.enabled,
                    api_key: input.api_key,
                    base_url: input.base_url,
                }
            }
            Self::Ollama(form) => {
                let form = form.read(cx);
                let input = form.draft();
                ProviderSettingsFormOutput::Ollama {
                    enabled: input.enabled,
                    base_url: input.base_url,
                    bearer_token: input.bearer_token,
                }
            }
            Self::CustomOpenAi(form) => {
                let form = form.read(cx);
                let input = form.draft();
                ProviderSettingsFormOutput::CustomOpenAi {
                    enabled: input.enabled,
                    name: input.name,
                    api_key: input.api_key,
                    base_url: input.base_url,
                    api_mode: input.api_mode,
                }
            }
        }
    }

    pub(super) fn validate_current(
        &self,
        secret_refs: &ProviderSecretRefs,
        i18n: &I18n,
        cx: &App,
    ) -> Result<(), ProviderValidationIssue> {
        self.current_output(cx).validate(secret_refs, i18n)
    }

    pub(super) fn clear_validation_errors(&self, cx: &mut App) {
        match self {
            Self::ApiKey(form) => form.update(cx, |form, cx| form.clear_all_errors(cx)),
            Self::Ollama(form) => form.update(cx, |form, cx| form.clear_all_errors(cx)),
            Self::CustomOpenAi(form) => form.update(cx, |form, cx| form.clear_all_errors(cx)),
        }
    }

    pub(super) fn apply_validation_issue(
        &self,
        issue: Option<&ProviderValidationIssue>,
        cx: &mut App,
    ) {
        self.clear_validation_errors(cx);
        let Some(issue) = issue else {
            return;
        };
        let error = provider_field_error(issue);
        match self {
            Self::ApiKey(form) => form.update(cx, |form, cx| {
                if let Some(field) = api_key_generated_field(issue.field) {
                    form.apply_field_error(field, error, cx);
                }
            }),
            Self::Ollama(form) => form.update(cx, |form, cx| {
                if let Some(field) = ollama_generated_field(issue.field) {
                    form.apply_field_error(field, error, cx);
                }
            }),
            Self::CustomOpenAi(form) => form.update(cx, |form, cx| {
                if let Some(field) = custom_openai_generated_field(issue.field) {
                    form.apply_field_error(field, error, cx);
                }
            }),
        }
    }
}

fn api_key_generated_field(field: ProviderFormField) -> Option<ApiKeyProviderFormField> {
    match field {
        ProviderFormField::ApiKey => Some(ApiKeyProviderFormField::ApiKey),
        ProviderFormField::BaseUrl => Some(ApiKeyProviderFormField::BaseUrl),
        ProviderFormField::Name | ProviderFormField::BearerToken | ProviderFormField::ApiMode => {
            None
        }
    }
}

fn ollama_generated_field(field: ProviderFormField) -> Option<OllamaProviderFormField> {
    match field {
        ProviderFormField::BaseUrl => Some(OllamaProviderFormField::BaseUrl),
        ProviderFormField::BearerToken => Some(OllamaProviderFormField::BearerToken),
        ProviderFormField::Name | ProviderFormField::ApiKey | ProviderFormField::ApiMode => None,
    }
}

fn custom_openai_generated_field(
    field: ProviderFormField,
) -> Option<CustomOpenAiProviderFormField> {
    match field {
        ProviderFormField::Name => Some(CustomOpenAiProviderFormField::Name),
        ProviderFormField::ApiKey => Some(CustomOpenAiProviderFormField::ApiKey),
        ProviderFormField::BaseUrl => Some(CustomOpenAiProviderFormField::BaseUrl),
        ProviderFormField::ApiMode => Some(CustomOpenAiProviderFormField::ApiMode),
        ProviderFormField::BearerToken => None,
    }
}

impl ApiKeyProviderFormInput {
    fn from_draft(draft: &ProviderDraft) -> Self {
        Self {
            enabled: draft.enabled,
            api_key: ProviderSecretValue::new(ProviderFormField::ApiKey, String::new(), false),
            base_url: draft.field_string(FIELD_BASE_URL),
        }
    }
}

impl OllamaProviderFormInput {
    fn from_draft(draft: &ProviderDraft) -> Self {
        let base_url = draft.field_string(FIELD_BASE_URL);
        Self {
            enabled: draft.enabled,
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
    fn from_draft(draft: &ProviderDraft) -> Self {
        Self {
            enabled: draft.enabled,
            name: draft.field_string(FIELD_NAME),
            api_key: ProviderSecretValue::new(ProviderFormField::ApiKey, String::new(), false),
            base_url: draft.field_string(FIELD_BASE_URL),
            api_mode: ProviderApiMode::from_key(&draft.field_string(FIELD_API_MODE)),
        }
    }
}

fn require_text(
    field: ProviderFormField,
    value: &str,
    i18n: &I18n,
) -> Result<(), ProviderValidationIssue> {
    if value.trim().is_empty() {
        Err(required_field_issue(field, i18n))
    } else {
        Ok(())
    }
}

fn require_secret(
    secret: &ProviderSecretValue,
    secret_refs: &ProviderSecretRefs,
    i18n: &I18n,
) -> Result<(), ProviderValidationIssue> {
    let has_saved_secret = !secret.changed
        && secret_refs
            .refs
            .iter()
            .any(|saved| saved.key == secret.key());
    if has_saved_secret || !secret.value.is_empty() {
        Ok(())
    } else {
        Err(required_field_issue(secret.field, i18n))
    }
}

fn required_field_issue(field: ProviderFormField, i18n: &I18n) -> ProviderValidationIssue {
    provider_field_issue(field, ProviderValidationKind::Required, i18n)
}

fn require_base_url(
    field: ProviderFormField,
    value: &str,
    i18n: &I18n,
) -> Result<(), ProviderValidationIssue> {
    require_text(field, value, i18n)?;
    validate_base_url(field, value, i18n)
}

fn validate_optional_base_url(
    field: ProviderFormField,
    value: &str,
    i18n: &I18n,
) -> Result<(), ProviderValidationIssue> {
    if value.trim().is_empty() {
        return Ok(());
    }
    validate_base_url(field, value, i18n)
}

fn validate_base_url(
    field: ProviderFormField,
    value: &str,
    i18n: &I18n,
) -> Result<(), ProviderValidationIssue> {
    match url::Url::parse(value.trim()) {
        Ok(url) if matches!(url.scheme(), "http" | "https") => Ok(()),
        Ok(_) => Err(provider_field_issue(
            field,
            ProviderValidationKind::UrlScheme,
            i18n,
        )),
        Err(_) => Err(provider_field_issue(
            field,
            ProviderValidationKind::UrlInvalid,
            i18n,
        )),
    }
}

fn provider_field_issue(
    field: ProviderFormField,
    kind: ProviderValidationKind,
    i18n: &I18n,
) -> ProviderValidationIssue {
    ProviderValidationIssue {
        field,
        kind,
        field_label: i18n.t(field.label_key()).into(),
        message: provider_field_message(field.label_key(), kind.message_key(), i18n),
    }
}

fn provider_field_message(field_label_key: &str, message_key: &str, i18n: &I18n) -> SharedString {
    let mut args = FluentArgs::new();
    args.set("field", i18n.t(field_label_key));
    i18n.t_with_args(message_key, &args).into()
}

fn provider_field_error(issue: &ProviderValidationIssue) -> FieldError {
    FieldError::new_for_field(
        issue.field.key(),
        ValidationTrigger::Submit,
        ValidationSource::App("ai-chat2-provider".into()),
        issue.kind.code(),
        issue.kind.message_key(),
    )
    .with_param("field", issue.field_label.to_string())
}

pub(super) fn field_errors<Field>(field: &Field) -> Vec<FieldError>
where
    Field: gpui_form::FormField,
{
    field
        .visible_errors(&FormMeta::default())
        .into_iter()
        .cloned()
        .collect()
}

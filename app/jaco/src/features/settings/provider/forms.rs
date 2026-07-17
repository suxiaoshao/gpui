use std::collections::BTreeMap;

use gpui::{App, AppContext as _, Context, Entity, EntityId, Task, Window};
use gpui_form::{
    FieldChangeCause, FieldError, FormMeta, FormStore as _, FormValidationReport, SubmitError,
    SubmitTransform, TransformContext, TransformReport, ValidationAdapter, ValidationAdapterReport,
    ValidationContext, ValidationIssue, ValidationScope, ValidationSource, ValidationTrigger,
};
use jaco_core::{
    ProviderSecretRefs, ProviderSettingFieldValue, ProviderSettingValue, ProviderSettingsPayload,
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
    ApiKeyProviderFormField, ApiKeyProviderFormInput, ApiKeyProviderFormStore,
};
pub(super) use custom_openai::{
    ApiModeChoice, CustomOpenAiProviderFormField, CustomOpenAiProviderFormInput,
    CustomOpenAiProviderFormStore, ProviderApiMode, localized_api_mode_choices,
};
pub(super) use ollama::{
    OllamaProviderFormField, OllamaProviderFormInput, OllamaProviderFormStore,
};
pub(super) use secret::{ProviderSecretCodec, ProviderSecretValue, bind_provider_secret};

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

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ProviderValidationContext {
    pub(super) secret_refs: ProviderSecretRefs,
}

impl Default for ProviderValidationContext {
    fn default() -> Self {
        Self {
            secret_refs: ProviderSecretRefs { refs: Vec::new() },
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct ApiKeyProviderValidator;

#[derive(Clone, Debug, Default)]
pub(super) struct OllamaProviderValidator;

#[derive(Clone, Debug, Default)]
pub(super) struct CustomOpenAiProviderValidator;

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
}

impl ValidationAdapter<ApiKeyProviderFormInput> for ApiKeyProviderValidator {
    type Context = ProviderValidationContext;

    fn validate(
        &self,
        draft: &ApiKeyProviderFormInput,
        trigger: ValidationTrigger,
        scope: ValidationScope,
        context: ValidationContext<'_, Self::Context>,
        cx: &App,
    ) -> ValidationAdapterReport {
        let mut issues = Vec::new();
        push_secret_required_issue(
            &mut issues,
            ProviderFormField::ApiKey,
            &draft.api_key,
            &context.external.secret_refs,
            trigger,
            &scope,
            cx,
        );
        push_optional_url_issue(
            &mut issues,
            ProviderFormField::BaseUrl,
            &draft.base_url,
            trigger,
            &scope,
            cx,
        );
        ValidationAdapterReport::new(issues)
    }
}

impl ValidationAdapter<OllamaProviderFormInput> for OllamaProviderValidator {
    type Context = ProviderValidationContext;

    fn validate(
        &self,
        draft: &OllamaProviderFormInput,
        trigger: ValidationTrigger,
        scope: ValidationScope,
        _context: ValidationContext<'_, Self::Context>,
        cx: &App,
    ) -> ValidationAdapterReport {
        let mut issues = Vec::new();
        push_required_url_issue(
            &mut issues,
            ProviderFormField::BaseUrl,
            &draft.base_url,
            trigger,
            &scope,
            cx,
        );
        ValidationAdapterReport::new(issues)
    }
}

impl ValidationAdapter<CustomOpenAiProviderFormInput> for CustomOpenAiProviderValidator {
    type Context = ProviderValidationContext;

    fn validate(
        &self,
        draft: &CustomOpenAiProviderFormInput,
        trigger: ValidationTrigger,
        scope: ValidationScope,
        context: ValidationContext<'_, Self::Context>,
        cx: &App,
    ) -> ValidationAdapterReport {
        let mut issues = Vec::new();
        push_required_text_issue(
            &mut issues,
            ProviderFormField::Name,
            &draft.name,
            trigger,
            &scope,
            cx,
        );
        push_secret_required_issue(
            &mut issues,
            ProviderFormField::ApiKey,
            &draft.api_key,
            &context.external.secret_refs,
            trigger,
            &scope,
            cx,
        );
        push_required_url_issue(
            &mut issues,
            ProviderFormField::BaseUrl,
            &draft.base_url,
            trigger,
            &scope,
            cx,
        );
        ValidationAdapterReport::new(issues)
    }
}

impl SubmitTransform<ApiKeyProviderFormInput, ApiKeyProviderFormInput> for ApiKeyProviderTransform {
    fn preview(
        &self,
        draft: &ApiKeyProviderFormInput,
        _context: &TransformContext,
    ) -> Result<ApiKeyProviderFormInput, TransformReport> {
        Ok(ApiKeyProviderFormInput {
            enabled: draft.enabled,
            api_key: draft.api_key.clone(),
            base_url: draft.base_url.trim().to_string(),
        })
    }

    fn transform_on_submit(
        &self,
        draft: &ApiKeyProviderFormInput,
        context: &TransformContext,
    ) -> Result<ApiKeyProviderFormInput, TransformReport> {
        self.preview(draft, context)
    }
}

impl SubmitTransform<OllamaProviderFormInput, OllamaProviderFormInput> for OllamaProviderTransform {
    fn preview(
        &self,
        draft: &OllamaProviderFormInput,
        _context: &TransformContext,
    ) -> Result<OllamaProviderFormInput, TransformReport> {
        Ok(OllamaProviderFormInput {
            enabled: draft.enabled,
            base_url: draft.base_url.trim().to_string(),
            bearer_token: draft.bearer_token.clone(),
        })
    }

    fn transform_on_submit(
        &self,
        draft: &OllamaProviderFormInput,
        context: &TransformContext,
    ) -> Result<OllamaProviderFormInput, TransformReport> {
        self.preview(draft, context)
    }
}

impl SubmitTransform<CustomOpenAiProviderFormInput, CustomOpenAiProviderFormInput>
    for CustomOpenAiProviderTransform
{
    fn preview(
        &self,
        draft: &CustomOpenAiProviderFormInput,
        _context: &TransformContext,
    ) -> Result<CustomOpenAiProviderFormInput, TransformReport> {
        Ok(CustomOpenAiProviderFormInput {
            enabled: draft.enabled,
            name: draft.name.trim().to_string(),
            api_key: draft.api_key.clone(),
            base_url: draft.base_url.trim().to_string(),
            api_mode: draft.api_mode,
        })
    }

    fn transform_on_submit(
        &self,
        draft: &CustomOpenAiProviderFormInput,
        context: &TransformContext,
    ) -> Result<CustomOpenAiProviderFormInput, TransformReport> {
        self.preview(draft, context)
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
                ApiKeyProviderFormStore::from_value_with_validation_context(
                    ApiKeyProviderFormInput::from_draft(draft),
                    ProviderValidationContext {
                        secret_refs: draft.existing_secret_refs.clone(),
                    },
                    window,
                    cx,
                )
            })),
            ProviderFormKind::Ollama => Self::Ollama(cx.new(|cx| {
                OllamaProviderFormStore::from_value_with_validation_context(
                    OllamaProviderFormInput::from_draft(draft),
                    ProviderValidationContext {
                        secret_refs: draft.existing_secret_refs.clone(),
                    },
                    window,
                    cx,
                )
            })),
            ProviderFormKind::CustomOpenAiCompatible => Self::CustomOpenAi(cx.new(|cx| {
                CustomOpenAiProviderFormStore::from_value_with_validation_context(
                    CustomOpenAiProviderFormInput::from_draft(draft),
                    ProviderValidationContext {
                        secret_refs: draft.existing_secret_refs.clone(),
                    },
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
    ) -> Result<(), SubmitError<()>>
    where
        H: FnOnce(ProviderSettingsFormOutput, &mut Window, &mut App) -> Task<Result<(), String>>
            + 'static,
    {
        match self {
            Self::ApiKey(form) => form.update(cx, |form, cx| {
                form.set_validation_context(
                    ProviderValidationContext {
                        secret_refs: secret_refs.clone(),
                    },
                    cx,
                );
                form.submit_async(
                    move |output, window, cx| {
                        let output = ProviderSettingsFormOutput::ApiKey {
                            enabled: output.enabled,
                            api_key: output.api_key,
                            base_url: output.base_url,
                        };
                        Ok::<_, ()>(handler(output, window, cx))
                    },
                    window,
                    cx,
                )
            }),
            Self::Ollama(form) => form.update(cx, |form, cx| {
                form.set_validation_context(
                    ProviderValidationContext {
                        secret_refs: secret_refs.clone(),
                    },
                    cx,
                );
                form.submit_async(
                    move |output, window, cx| {
                        let output = ProviderSettingsFormOutput::Ollama {
                            enabled: output.enabled,
                            base_url: output.base_url,
                            bearer_token: output.bearer_token,
                        };
                        Ok::<_, ()>(handler(output, window, cx))
                    },
                    window,
                    cx,
                )
            }),
            Self::CustomOpenAi(form) => form.update(cx, |form, cx| {
                form.set_validation_context(
                    ProviderValidationContext {
                        secret_refs: secret_refs.clone(),
                    },
                    cx,
                );
                form.submit_async(
                    move |output, window, cx| {
                        let output = ProviderSettingsFormOutput::CustomOpenAi {
                            enabled: output.enabled,
                            name: output.name,
                            api_key: output.api_key,
                            base_url: output.base_url,
                            api_mode: output.api_mode,
                        };
                        Ok::<_, ()>(handler(output, window, cx))
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
        secret_refs: ProviderSecretRefs,
        window: &mut Window,
        cx: &mut App,
    ) -> FormValidationReport {
        match self {
            Self::ApiKey(form) => form.update(cx, |form, cx| {
                form.set_validation_context(
                    ProviderValidationContext {
                        secret_refs: secret_refs.clone(),
                    },
                    cx,
                );
                form.validate(ValidationTrigger::Submit, window, cx)
            }),
            Self::Ollama(form) => form.update(cx, |form, cx| {
                form.set_validation_context(
                    ProviderValidationContext {
                        secret_refs: secret_refs.clone(),
                    },
                    cx,
                );
                form.validate(ValidationTrigger::Submit, window, cx)
            }),
            Self::CustomOpenAi(form) => form.update(cx, |form, cx| {
                form.set_validation_context(
                    ProviderValidationContext {
                        secret_refs: secret_refs.clone(),
                    },
                    cx,
                );
                form.validate(ValidationTrigger::Submit, window, cx)
            }),
        }
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

fn push_required_text_issue(
    issues: &mut Vec<ValidationIssue>,
    field: ProviderFormField,
    value: &str,
    trigger: ValidationTrigger,
    scope: &ValidationScope,
    cx: &App,
) {
    if value.trim().is_empty() && provider_scope_includes_field(scope, field) {
        issues.push(provider_validation_issue(
            field,
            ProviderValidationKind::Required,
            trigger,
            cx,
        ));
    }
}

fn push_secret_required_issue(
    issues: &mut Vec<ValidationIssue>,
    field: ProviderFormField,
    secret: &ProviderSecretValue,
    secret_refs: &ProviderSecretRefs,
    trigger: ValidationTrigger,
    scope: &ValidationScope,
    cx: &App,
) {
    if !provider_scope_includes_field(scope, field) {
        return;
    }
    let has_saved_secret = !secret.changed
        && secret_refs
            .refs
            .iter()
            .any(|saved| saved.key == secret.key());
    if !has_saved_secret && secret.value.trim().is_empty() {
        issues.push(provider_validation_issue(
            field,
            ProviderValidationKind::Required,
            trigger,
            cx,
        ));
    }
}

fn push_required_url_issue(
    issues: &mut Vec<ValidationIssue>,
    field: ProviderFormField,
    value: &str,
    trigger: ValidationTrigger,
    scope: &ValidationScope,
    cx: &App,
) {
    if !provider_scope_includes_field(scope, field) {
        return;
    }
    let value = value.trim();
    if value.is_empty() {
        issues.push(provider_validation_issue(
            field,
            ProviderValidationKind::Required,
            trigger,
            cx,
        ));
        return;
    }
    push_base_url_issue(issues, field, value, trigger, cx);
}

fn push_optional_url_issue(
    issues: &mut Vec<ValidationIssue>,
    field: ProviderFormField,
    value: &str,
    trigger: ValidationTrigger,
    scope: &ValidationScope,
    cx: &App,
) {
    if !provider_scope_includes_field(scope, field) {
        return;
    }
    let value = value.trim();
    if !value.is_empty() {
        push_base_url_issue(issues, field, value, trigger, cx);
    }
}

fn push_base_url_issue(
    issues: &mut Vec<ValidationIssue>,
    field: ProviderFormField,
    value: &str,
    trigger: ValidationTrigger,
    cx: &App,
) {
    match url::Url::parse(value) {
        Ok(url) if matches!(url.scheme(), "http" | "https") => {}
        Ok(_) => issues.push(provider_validation_issue(
            field,
            ProviderValidationKind::UrlScheme,
            trigger,
            cx,
        )),
        Err(_) => issues.push(provider_validation_issue(
            field,
            ProviderValidationKind::UrlInvalid,
            trigger,
            cx,
        )),
    }
}

fn provider_validation_issue(
    field: ProviderFormField,
    kind: ProviderValidationKind,
    trigger: ValidationTrigger,
    cx: &App,
) -> ValidationIssue {
    ValidationIssue::field(
        gpui_form::FieldPath::from_static(field.key()),
        trigger,
        ValidationSource::App("jaco-provider".into()),
        kind.code(),
        kind.message_key(),
    )
    .with_param(
        "field",
        cx.global::<I18n>().t(field.label_key()).to_string(),
    )
}

fn provider_scope_includes_field(scope: &ValidationScope, field: ProviderFormField) -> bool {
    let path = gpui_form::FieldPath::from_static(field.key());
    match scope {
        ValidationScope::Form => true,
        ValidationScope::Field(field_path) => field_path == &path,
        ValidationScope::Group(group_path) => path.starts_with(group_path),
        ValidationScope::ArrayItem {
            path: array_path, ..
        } => path.starts_with(array_path),
    }
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

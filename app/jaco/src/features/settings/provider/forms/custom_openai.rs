use gpui::{IntoElement as _, SharedString};
use gpui_component::select::SelectItem;

use crate::foundation::I18n;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::features::settings::provider) enum ProviderApiMode {
    #[default]
    Responses,
    ChatCompletions,
}

impl ProviderApiMode {
    pub(in crate::features::settings::provider) fn from_key(value: &str) -> Self {
        match value {
            "chat_completions" => Self::ChatCompletions,
            _ => Self::Responses,
        }
    }

    pub(in crate::features::settings::provider) const fn key(self) -> &'static str {
        match self {
            Self::Responses => "responses",
            Self::ChatCompletions => "chat_completions",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::features::settings::provider) struct ApiModeChoice {
    value: ProviderApiMode,
    label: SharedString,
}

impl ApiModeChoice {
    fn new(value: ProviderApiMode, label: impl Into<SharedString>) -> Self {
        Self {
            value,
            label: label.into(),
        }
    }
}

impl SelectItem for ApiModeChoice {
    type Value = ProviderApiMode;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn display_title(&self) -> Option<gpui::AnyElement> {
        Some(self.label.clone().into_any_element())
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

pub(in crate::features::settings::provider) fn localized_api_mode_choices(
    i18n: &I18n,
) -> Vec<ApiModeChoice> {
    vec![
        ApiModeChoice::new(
            ProviderApiMode::Responses,
            i18n.t("provider-api-mode-responses"),
        ),
        ApiModeChoice::new(
            ProviderApiMode::ChatCompletions,
            i18n.t("provider-api-mode-chat-completions"),
        ),
    ]
}

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore, garde::Validate)]
#[garde(context(super::ProviderValidationContext))]
#[form(
    store = CustomOpenAiProviderFormStore,
    validation(adapter = "garde", i18n = super::JacoGardeI18nProvider),
    transform(adapter = super::CustomOpenAiProviderTransform)
)]
pub(in crate::features::settings::provider) struct CustomOpenAiProviderFormInput {
    #[garde(skip)]
    pub(super) enabled: bool,
    #[form(required, validate(on_change, on_blur, on_submit))]
    #[garde(custom(super::validate_required_provider_text))]
    pub(super) name: String,
    #[form(validate(on_change, on_blur, on_submit))]
    #[garde(custom(super::validate_provider_secret))]
    pub(super) api_key: super::ProviderSecretValue,
    #[form(required, validate(on_change, on_blur, on_submit))]
    #[garde(custom(super::validate_required_provider_url))]
    pub(super) base_url: String,
    #[form(validate(on_change, on_submit))]
    #[garde(skip)]
    pub(super) api_mode: ProviderApiMode,
}

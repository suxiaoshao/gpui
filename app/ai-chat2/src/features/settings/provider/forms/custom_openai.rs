use gpui::{IntoElement, SharedString};
use gpui_component::select::SelectItem;

use crate::foundation::I18n;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::features::settings::provider) enum ProviderApiMode {
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

impl gpui_form::SelectFieldValue for ProviderApiMode {
    type Selected = ProviderApiMode;

    fn to_selected_value(&self) -> Option<Self::Selected> {
        Some(*self)
    }

    fn from_selected_value(selected: Option<Self::Selected>, previous: &Self) -> Self {
        selected.unwrap_or(*previous)
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

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = CustomOpenAiProviderFormStore)]
pub(in crate::features::settings::provider) struct CustomOpenAiProviderFormInput {
    #[form(component = "bool")]
    pub(super) enabled: bool,
    #[form(
        component = "input",
        label = "provider-field-name",
        placeholder = "provider-placeholder-provider-name",
        required,
        validate(on_change, on_blur, on_submit)
    )]
    pub(super) name: String,
    #[form(
        component = "input",
        label = "provider-field-api-key",
        placeholder = "provider-placeholder-api-key",
        required,
        mask,
        validate(on_change, on_blur, on_submit)
    )]
    pub(super) api_key: String,
    #[form(
        component = "input",
        label = "provider-field-base-url",
        placeholder = "provider-placeholder-custom-base-url",
        required,
        validate(on_change, on_blur, on_submit)
    )]
    pub(super) base_url: String,
    #[form(
        component = "select",
        delegate = "Vec<ApiModeChoice>",
        options = "localized_api_mode_choices(cx.global::<crate::foundation::I18n>())",
        label = "provider-field-api-mode",
        placeholder = "provider-placeholder-api-mode",
        validate(on_change, on_submit)
    )]
    pub(super) api_mode: ProviderApiMode,
}

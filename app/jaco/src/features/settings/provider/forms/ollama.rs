#[derive(Clone, Debug, PartialEq, gpui_form::FormStore, garde::Validate)]
#[garde(context(super::ProviderValidationContext))]
#[form(
    store = OllamaProviderFormStore,
    validation(adapter = "garde", messages = super::JacoGardeMessageProvider),
    transform(adapter = super::OllamaProviderTransform)
)]
pub(in crate::features::settings::provider) struct OllamaProviderFormInput {
    #[garde(skip)]
    pub(super) enabled: bool,
    #[form(required, validate(on_change, on_blur, on_submit))]
    #[garde(custom(super::validate_required_provider_url))]
    pub(super) base_url: String,
    #[form(validate(on_change, on_blur, on_submit))]
    #[garde(skip)]
    pub(super) bearer_token: super::ProviderSecretValue,
}

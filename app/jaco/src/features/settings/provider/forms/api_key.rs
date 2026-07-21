#[derive(Clone, Debug, PartialEq, gpui_form::FormStore, garde::Validate)]
#[garde(context(super::ProviderValidationContext))]
#[form(
    store = ApiKeyProviderFormStore,
    validation(adapter = "garde", i18n = super::JacoGardeI18nProvider),
    transform(adapter = super::ApiKeyProviderTransform)
)]
pub(in crate::features::settings::provider) struct ApiKeyProviderFormInput {
    #[garde(skip)]
    pub(super) enabled: bool,
    #[form(validate(on_change, on_blur, on_submit))]
    #[garde(custom(super::validate_provider_secret))]
    pub(super) api_key: super::ProviderSecretValue,
    #[form(validate(on_change, on_blur, on_submit))]
    #[garde(custom(super::validate_optional_provider_url))]
    pub(super) base_url: String,
}

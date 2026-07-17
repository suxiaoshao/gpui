#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(
    store = ApiKeyProviderFormStore,
    validation(adapter = super::ApiKeyProviderValidator, context = super::ProviderValidationContext),
    transform(adapter = super::ApiKeyProviderTransform)
)]
pub(in crate::features::settings::provider) struct ApiKeyProviderFormInput {
    #[form(component = "value")]
    pub(super) enabled: bool,
    #[form(
        codec = "super::ProviderSecretCodec",
        required,
        validate(on_change, on_blur, on_submit)
    )]
    pub(super) api_key: super::ProviderSecretValue,
    #[form(component = "value", validate(on_change, on_blur, on_submit))]
    pub(super) base_url: String,
}

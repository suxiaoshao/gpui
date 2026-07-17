#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(
    store = OllamaProviderFormStore,
    validation(adapter = super::OllamaProviderValidator, context = super::ProviderValidationContext),
    transform(adapter = super::OllamaProviderTransform)
)]
pub(in crate::features::settings::provider) struct OllamaProviderFormInput {
    #[form(component = "value")]
    pub(super) enabled: bool,
    #[form(component = "value", required, validate(on_change, on_blur, on_submit))]
    pub(super) base_url: String,
    #[form(
        codec = "super::ProviderSecretCodec",
        validate(on_change, on_blur, on_submit)
    )]
    pub(super) bearer_token: super::ProviderSecretValue,
}

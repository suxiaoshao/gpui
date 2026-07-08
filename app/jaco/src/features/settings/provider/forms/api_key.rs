type StringInputBinding = gpui_form_gpui_component::TextInputBinding<String>;
type BoolInputBinding = gpui_form_gpui_component::BoolBinding;
type SecretInputBinding = super::ProviderSecretInputBinding;

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(
    store = ApiKeyProviderFormStore,
    validation(adapter = super::ApiKeyProviderValidator, context = super::ProviderValidationContext),
    transform(adapter = super::ApiKeyProviderTransform)
)]
pub(in crate::features::settings::provider) struct ApiKeyProviderFormInput {
    #[form(binding = "BoolInputBinding")]
    pub(super) enabled: bool,
    #[form(
        binding = "SecretInputBinding",
        label = "provider-field-api-key",
        placeholder = "provider-placeholder-api-key",
        required,
        mask,
        validate(on_change, on_blur, on_submit)
    )]
    pub(super) api_key: super::ProviderSecretValue,
    #[form(
        binding = "StringInputBinding",
        label = "provider-field-base-url",
        placeholder = "provider-placeholder-base-url-default",
        validate(on_change, on_blur, on_submit)
    )]
    pub(super) base_url: String,
}

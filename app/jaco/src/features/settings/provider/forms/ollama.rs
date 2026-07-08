type StringInputBinding = gpui_form_gpui_component::TextInputBinding<String>;
type BoolInputBinding = gpui_form_gpui_component::BoolBinding;
type SecretInputBinding = super::ProviderSecretInputBinding;

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(
    store = OllamaProviderFormStore,
    validation(adapter = super::OllamaProviderValidator, context = super::ProviderValidationContext),
    transform(adapter = super::OllamaProviderTransform)
)]
pub(in crate::features::settings::provider) struct OllamaProviderFormInput {
    #[form(binding = "BoolInputBinding")]
    pub(super) enabled: bool,
    #[form(
        binding = "StringInputBinding",
        label = "provider-field-base-url",
        placeholder = "provider-placeholder-ollama-base-url",
        required,
        validate(on_change, on_blur, on_submit)
    )]
    pub(super) base_url: String,
    #[form(
        binding = "SecretInputBinding",
        label = "provider-field-bearer-token",
        placeholder = "provider-placeholder-bearer-token",
        mask,
        validate(on_change, on_blur, on_submit)
    )]
    pub(super) bearer_token: super::ProviderSecretValue,
}

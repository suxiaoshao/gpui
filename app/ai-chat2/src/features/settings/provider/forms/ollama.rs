#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = OllamaProviderFormStore)]
pub(in crate::features::settings::provider) struct OllamaProviderFormInput {
    #[form(component = "bool")]
    pub(super) enabled: bool,
    #[form(
        component = "input",
        label = "provider-field-base-url",
        placeholder = "provider-placeholder-ollama-base-url",
        validate(on_change, on_blur, on_submit)
    )]
    pub(super) base_url: String,
    #[form(
        component = "input",
        label = "provider-field-bearer-token",
        placeholder = "provider-placeholder-bearer-token",
        mask,
        validate(on_change, on_blur, on_submit)
    )]
    pub(super) bearer_token: String,
}

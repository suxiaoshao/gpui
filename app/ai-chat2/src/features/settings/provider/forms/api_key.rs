#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = ApiKeyProviderFormStore)]
pub(in crate::features::settings::provider) struct ApiKeyProviderFormInput {
    #[form(component = "bool")]
    pub(super) enabled: bool,
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
        placeholder = "provider-placeholder-base-url-default",
        validate(on_change, on_blur, on_submit)
    )]
    pub(super) base_url: String,
}

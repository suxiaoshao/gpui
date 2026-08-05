use proc_macro2::TokenStream;
use quote::quote;

#[allow(clippy::too_many_arguments)]
pub(super) fn field_schema(
    name: &str,
    required: bool,
    mount: bool,
    change: bool,
    blur: bool,
    dynamic: bool,
    submit: bool,
) -> TokenStream {
    quote! {
        ::gpui_form::FieldSchema::new(
            #name,
            #required,
            ::gpui_form::ValidationTriggers {
                mount: #mount,
                change: #change,
                blur: #blur,
                dynamic: #dynamic,
                submit: #submit,
            },
        )
    }
}

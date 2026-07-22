use proc_macro::TokenStream;

mod attributes;
mod expand;

#[proc_macro_derive(FormStore, attributes(form))]
pub fn derive_form_store(input: TokenStream) -> TokenStream {
    expand::derive_form_store(input.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

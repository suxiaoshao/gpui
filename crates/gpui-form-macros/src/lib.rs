use proc_macro::TokenStream;

mod derive;

#[proc_macro_derive(FormSchema, attributes(form))]
pub fn derive_form_schema(input: TokenStream) -> TokenStream {
    derive::expand(input.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

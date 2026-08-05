use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

pub(super) fn schema_driver(name: &Ident, body: TokenStream) -> TokenStream {
    quote! {
        impl ::gpui_form::FormSchema for #name {
            fn __visit(&self, visitor: &mut dyn ::gpui_form::__private::SchemaVisitor) {
                #body
            }
        }
    }
}

use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

pub(super) fn model_definition(
    name: &Ident,
    functions: &[TokenStream],
    constants: &[TokenStream],
) -> TokenStream {
    quote! {
        impl #name {
            pub const ROOT: ::gpui_form::RootDef<Self> = ::gpui_form::RootDef::__new();
            #(#functions)*
            #(#constants)*
        }
    }
}

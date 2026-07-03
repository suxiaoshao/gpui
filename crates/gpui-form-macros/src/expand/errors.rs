use proc_macro2::TokenStream;
use quote::quote;
use syn::Result;

use crate::field_kind::FieldKind;

use super::{FieldModel, arrays::vec_inner_type, field_variant_ident};

pub(super) fn clear_all_error_statement(model: &FieldModel<'_>) -> Result<TokenStream> {
    let ident = model.ident;
    Ok(match model.attrs.component {
        FieldKind::Array => {
            let item_ty = vec_inner_type(model)?;
            let store = model.attrs.store.as_ref().expect("checked");
            quote! {
                self.#ident.clear_errors();
                for __gpui_form_item in self.#ident.items_mut() {
                    let __gpui_form_child_store = __gpui_form_item.item.store();
                    __gpui_form_child_store.update(cx, |child, cx| {
                        <#store as ::gpui_form::macro_support::GeneratedFormStore<#item_ty>>::clear_all_errors(
                            child,
                            cx,
                        );
                    });
                    let __gpui_form_child = __gpui_form_child_store.read(cx);
                    __gpui_form_item.item.sync_from_child(
                        <#store as ::gpui_form::macro_support::GeneratedFormStore<#item_ty>>::draft(__gpui_form_child),
                        <#store as ::gpui_form::macro_support::GeneratedFormStore<#item_ty>>::meta(__gpui_form_child).clone(),
                    );
                }
            }
        }
        FieldKind::Group => {
            let ty = model.ty;
            let store = model.attrs.store.as_ref().expect("checked");
            quote! {
                ::gpui_form::FormField::clear_errors(&mut self.#ident);
                let __gpui_form_child_store = self.#ident.store();
                __gpui_form_child_store.update(cx, |child, cx| {
                    <#store as ::gpui_form::macro_support::GeneratedFormStore<#ty>>::clear_all_errors(
                        child,
                        cx,
                    );
                });
                let __gpui_form_child = __gpui_form_child_store.read(cx);
                self.#ident.sync_from_child(
                    <#store as ::gpui_form::macro_support::GeneratedFormStore<#ty>>::draft(__gpui_form_child),
                    <#store as ::gpui_form::macro_support::GeneratedFormStore<#ty>>::meta(__gpui_form_child).clone(),
                );
            }
        }
        _ => quote! {
            ::gpui_form::FormField::clear_errors(&mut self.#ident);
        },
    })
}

pub(super) fn clear_field_error_arm(
    model: &FieldModel<'_>,
    field_enum_ident: &syn::Ident,
) -> Result<TokenStream> {
    let ident = model.ident;
    let variant_ident = field_variant_ident(&model.name);
    Ok(match model.attrs.component {
        FieldKind::Array => quote! {
            #field_enum_ident::#variant_ident => {
                self.#ident.clear_errors();
            }
        },
        _ => quote! {
            #field_enum_ident::#variant_ident => {
                ::gpui_form::FormField::clear_errors(&mut self.#ident);
            }
        },
    })
}

pub(super) fn apply_field_error_arm(
    model: &FieldModel<'_>,
    field_enum_ident: &syn::Ident,
) -> Result<TokenStream> {
    let ident = model.ident;
    let variant_ident = field_variant_ident(&model.name);
    Ok(match model.attrs.component {
        FieldKind::Array => quote! {
            #field_enum_ident::#variant_ident => {
                self.#ident.set_errors(::std::vec![error]);
            }
        },
        _ => quote! {
            #field_enum_ident::#variant_ident => {
                ::gpui_form::FormField::mark_touched(&mut self.#ident);
                ::gpui_form::FormField::set_errors(&mut self.#ident, ::std::vec![error]);
            }
        },
    })
}

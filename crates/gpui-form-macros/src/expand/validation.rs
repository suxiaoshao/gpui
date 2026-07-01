use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Result;

use crate::field_kind::FieldKind;

use super::{FieldModel, arrays::vec_inner_type};

pub(super) fn prepare_submit_statement(model: &FieldModel<'_>) -> Result<TokenStream> {
    let ident = model.ident;
    let name = &model.name;
    Ok(match model.attrs.component {
        FieldKind::Number => {
            quote! {
                if let Err(__gpui_form_error) = self.#ident.parse_raw_for_submit(
                    ::gpui_form::macro_support::field_path(#name),
                    cx,
                ) {
                    report.push_field_error(__gpui_form_error);
                }
            }
        }
        FieldKind::Group => {
            let ty = model.ty;
            let store = model.attrs.store.as_ref().expect("checked");
            quote! {
                let __gpui_form_child_store = self.#ident.store();
                let __gpui_form_child_report = __gpui_form_child_store.update(cx, |child, cx| {
                    <#store as ::gpui_form::macro_support::GeneratedFormStore<#ty>>::prepare_submit(
                        child,
                        cx,
                    )
                });
                let __gpui_form_child = __gpui_form_child_store.read(cx);
                self.#ident.sync_from_child(
                    <#store as ::gpui_form::macro_support::GeneratedFormStore<#ty>>::draft(__gpui_form_child),
                    <#store as ::gpui_form::macro_support::GeneratedFormStore<#ty>>::meta(__gpui_form_child).clone(),
                );
                report.merge(__gpui_form_child_report.with_field_prefix(self.#ident.path()));
            }
        }
        FieldKind::Array => {
            let item_ty = vec_inner_type(model)?;
            let store = model.attrs.store.as_ref().expect("checked");
            let refresh_meta_ident = format_ident!("{}_refresh_meta", ident);
            quote! {
                let __gpui_form_array_path = self.#ident.path().clone();
                for __gpui_form_item in self.#ident.items_mut() {
                    let __gpui_form_child_store = __gpui_form_item.item.store();
                    let __gpui_form_child_report = __gpui_form_child_store.update(cx, |child, cx| {
                        <#store as ::gpui_form::macro_support::GeneratedFormStore<#item_ty>>::prepare_submit(
                            child,
                            cx,
                        )
                    });
                    let __gpui_form_child = __gpui_form_child_store.read(cx);
                    __gpui_form_item.item.sync_from_child(
                        <#store as ::gpui_form::macro_support::GeneratedFormStore<#item_ty>>::draft(__gpui_form_child),
                        <#store as ::gpui_form::macro_support::GeneratedFormStore<#item_ty>>::meta(__gpui_form_child).clone(),
                    );
                    report.merge(
                        __gpui_form_child_report.with_field_prefix(
                            &__gpui_form_array_path.join_index(__gpui_form_item.index),
                        ),
                    );
                }
                self.#refresh_meta_ident();
            }
        }
        _ => quote! {},
    })
}

pub(super) fn current_validation_report_statement(model: &FieldModel<'_>) -> Result<TokenStream> {
    let ident = model.ident;
    Ok(match model.attrs.component {
        FieldKind::Array => {
            let item_ty = vec_inner_type(model)?;
            let store = model.attrs.store.as_ref().expect("checked");
            quote! {
                for __gpui_form_error in self.#ident.errors() {
                    report.push_field_error(__gpui_form_error.clone());
                }
                let __gpui_form_array_path = self.#ident.path().clone();
                for __gpui_form_item in self.#ident.items() {
                    let __gpui_form_child_store = __gpui_form_item.item.store();
                    let __gpui_form_child = __gpui_form_child_store.read(cx);
                    let __gpui_form_child_report =
                        <#store as ::gpui_form::macro_support::GeneratedFormStore<#item_ty>>::current_validation_report(
                            __gpui_form_child,
                            cx,
                        );
                    report.merge(
                        __gpui_form_child_report.with_field_prefix(
                            &__gpui_form_array_path.join_index(__gpui_form_item.index),
                        ),
                    );
                }
            }
        }
        FieldKind::Group => {
            let ty = model.ty;
            let store = model.attrs.store.as_ref().expect("checked");
            quote! {
                let __gpui_form_child_store = self.#ident.store();
                let __gpui_form_child = __gpui_form_child_store.read(cx);
                let __gpui_form_child_report =
                    <#store as ::gpui_form::macro_support::GeneratedFormStore<#ty>>::current_validation_report(
                        __gpui_form_child,
                        cx,
                    );
                report.merge(__gpui_form_child_report.with_field_prefix(self.#ident.path()));
            }
        }
        _ => quote! {
            for __gpui_form_error in ::gpui_form::FormField::errors(&self.#ident) {
                report.push_field_error(__gpui_form_error.clone());
            }
        },
    })
}

pub(super) fn apply_validation_statement(model: &FieldModel<'_>) -> Result<TokenStream> {
    let ident = model.ident;
    let name = &model.name;
    Ok(match model.attrs.component {
        FieldKind::Array => {
            let refresh_meta_ident = format_ident!("{}_refresh_meta", ident);
            let item_ty = vec_inner_type(model)?;
            let store = model.attrs.store.as_ref().expect("checked");
            quote! {
                if ::gpui_form::macro_support::scope_contains_path(scope, self.#ident.path()) {
                    let __gpui_form_array_path = self.#ident.path().clone();
                    let __gpui_form_array_errors = report
                        .field_errors()
                        .iter()
                        .filter(|error| error.path == __gpui_form_array_path)
                        .cloned()
                        .collect::<::std::vec::Vec<_>>();
                    self.#ident.set_errors(__gpui_form_array_errors);

                    for __gpui_form_item in self.#ident.items_mut() {
                        let __gpui_form_item_prefix =
                            __gpui_form_array_path.join_index(__gpui_form_item.index);
                        let __gpui_form_child_report =
                            report.strip_field_prefix(&__gpui_form_item_prefix);
                        let __gpui_form_child_scope = ::gpui_form::ValidationScope::Form;
                        let __gpui_form_child_store = __gpui_form_item.item.store();
                        __gpui_form_child_store.update(cx, |child, cx| {
                            <#store as ::gpui_form::macro_support::GeneratedFormStore<#item_ty>>::apply_validation_report(
                                child,
                                &__gpui_form_child_report,
                                &__gpui_form_child_scope,
                                cx,
                            );
                        });
                        let __gpui_form_child = __gpui_form_child_store.read(cx);
                        __gpui_form_item.item.sync_from_child(
                            <#store as ::gpui_form::macro_support::GeneratedFormStore<#item_ty>>::draft(__gpui_form_child),
                            <#store as ::gpui_form::macro_support::GeneratedFormStore<#item_ty>>::meta(__gpui_form_child).clone(),
                        );
                    }
                    self.#refresh_meta_ident();
                }
            }
        }
        FieldKind::Group => group_apply_validation_statement(model),
        _ => quote! {
            let __gpui_form_field_path = ::gpui_form::macro_support::field_path(#name);
            if ::gpui_form::macro_support::scope_contains_path(
                scope,
                &__gpui_form_field_path,
            ) {
                let mut __gpui_form_errors = ::std::vec::Vec::new();
                __gpui_form_errors.extend(
                    ::gpui_form::FormField::errors(&self.#ident)
                        .iter()
                        .filter(|error| {
                            error.source == ::gpui_form::ValidationSource::Internal
                        })
                        .cloned(),
                );
                __gpui_form_errors.extend(
                    report
                        .field_errors()
                        .iter()
                        .filter(|error| {
                            error.path == __gpui_form_field_path
                        })
                        .cloned(),
                );
                ::gpui_form::FormField::set_errors(
                    &mut self.#ident,
                    __gpui_form_errors,
                );
            }
        },
    })
}

fn group_apply_validation_statement(model: &FieldModel<'_>) -> TokenStream {
    let ident = model.ident;
    let ty = model.ty;
    let store = model.attrs.store.as_ref().expect("checked");
    quote! {
        if ::gpui_form::macro_support::scope_contains_path(scope, self.#ident.path()) {
            let __gpui_form_child_store = self.#ident.store();
            let __gpui_form_child_report =
                report.strip_field_prefix(self.#ident.path());
            let __gpui_form_child_scope = ::gpui_form::ValidationScope::Form;
            __gpui_form_child_store.update(cx, |child, cx| {
                <#store as ::gpui_form::macro_support::GeneratedFormStore<#ty>>::apply_validation_report(
                    child,
                    &__gpui_form_child_report,
                    &__gpui_form_child_scope,
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
}

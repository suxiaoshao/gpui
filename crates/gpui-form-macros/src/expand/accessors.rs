use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Result;

use crate::field_kind::FieldKind;

use super::{FieldModel, arrays::vec_inner_type, field_variant_ident};

pub(super) fn field_draft_type(model: &FieldModel<'_>) -> TokenStream {
    let ty = model.ty;
    match model.attrs.component {
        FieldKind::Value => {
            let codec = model
                .attrs
                .codec
                .as_ref()
                .map(|codec| quote!(#codec))
                .unwrap_or_else(|| quote!(::gpui_form::IdentityCodec<#ty>));
            quote!(<#codec as ::gpui_form::FieldCodec<#ty>>::Draft)
        }
        _ => quote!(#ty),
    }
}

pub(super) fn draft_field_value(model: &FieldModel<'_>) -> TokenStream {
    let ident = model.ident;
    match model.attrs.component {
        FieldKind::Array => quote! {
            self.#ident
                .items()
                .iter()
                .map(|item| item.item.value().clone())
                .collect()
        },
        FieldKind::Value if model.attrs.codec.is_some() => {
            // The generated form snapshot keeps the domain shape used by
            // validation/transform/group stores. Raw drafts are exposed via
            // `<field>_draft()` and `FormFieldHandle`; an invalid raw draft
            // therefore does not make the domain snapshot unconstructible.
            quote!(::gpui_form::FormField::value(&self.#ident).clone())
        }
        FieldKind::Value => quote!(self.#ident.draft().clone()),
        _ => quote!(::gpui_form::FormField::value(&self.#ident).clone()),
    }
}

pub(super) fn replace_field_statement(
    model: &FieldModel<'_>,
    field_enum_ident: &syn::Ident,
) -> TokenStream {
    let ident = model.ident;
    let value_ident = &model.value_ident;
    let name = &model.name;
    let variant_ident = field_variant_ident(name);
    match model.attrs.component {
        FieldKind::Value => quote! {
            let __gpui_form_field_changed = self.#ident.replace_baseline(#value_ident);
            if __gpui_form_field_changed {
                cx.emit(::gpui_form::FormDraftEvent::new(
                    ::gpui_form::FieldPath::field(#name),
                    self.#ident.draft().clone(),
                    ::gpui_form::FieldChangeCause::External,
                ));
                cx.emit(::gpui_form::FormStoreEvent::FieldChanged {
                    field: #field_enum_ident::#variant_ident,
                    cause: ::gpui_form::FieldChangeCause::External,
                });
            }
        },
        FieldKind::Group => quote! {
            self.#ident.replace_baseline(#value_ident, cx);
        },
        FieldKind::Array => quote! {
            let __gpui_form_array_values = #value_ident;
            if self.#ident.len() != __gpui_form_array_values.len() {
                self.#ident.set_errors(::std::vec![::gpui_form::FieldError::new(
                    ::gpui_form::macro_support::field_path(#name),
                    ::gpui_form::ValidationTrigger::Submit,
                    ::gpui_form::ValidationSource::Internal,
                    "array_length_changed",
                    "gpui-form-error-array-length-changed",
                )]);
            } else {
                self.#ident.clear_errors();
                for (__gpui_form_item, __gpui_form_value) in self
                    .#ident
                    .items_mut()
                    .iter_mut()
                    .zip(__gpui_form_array_values.iter().cloned())
                {
                    __gpui_form_item
                        .item
                        .replace_baseline(__gpui_form_value, cx);
                }
                self.#ident
                    .rebase_default_values(__gpui_form_array_values);
            }
        },
    }
}

pub(super) fn field_meta_value(model: &FieldModel<'_>) -> TokenStream {
    let ident = model.ident;
    match model.attrs.component {
        FieldKind::Array => quote!(self.#ident.meta()),
        _ => quote!(::gpui_form::FormField::meta(&self.#ident)),
    }
}

pub(super) fn field_accessor_methods(model: &FieldModel<'_>) -> Result<TokenStream> {
    let ident = model.ident;
    let ty = model.ty;
    let name = &model.name;
    let value_ident = format_ident!("{}_value", ident);
    let draft_ident = format_ident!("{}_draft", ident);
    let handle_ident = format_ident!("{}_handle", ident);
    let read_handle_ident = format_ident!("__read_{}_handle", ident);
    let write_handle_ident = format_ident!("__write_{}_handle", ident);
    let set_draft_ident = format_ident!("set_{}_draft", ident);

    Ok(match model.attrs.component {
        FieldKind::Value => {
            let draft_ty = field_draft_type(model);
            quote! {
                pub fn #value_ident(&self) -> #ty {
                    ::gpui_form::FormField::value(&self.#ident).clone()
                }

                pub fn #draft_ident(&self) -> #draft_ty {
                    self.#ident.draft().clone()
                }

                pub fn #handle_ident(
                    form: &::gpui_form::__private::gpui::Entity<Self>,
                ) -> ::gpui_form::FormFieldHandle<Self, #draft_ty> {
                    ::gpui_form::FormFieldHandle::new(
                        form.downgrade(),
                        ::gpui_form::FieldPath::field(#name),
                        Self::#read_handle_ident,
                        Self::#write_handle_ident,
                    )
                }

                fn #read_handle_ident(form: &Self) -> #draft_ty {
                    form.#ident.draft().clone()
                }

                fn #write_handle_ident(
                    form: &mut Self,
                    draft: #draft_ty,
                    cause: ::gpui_form::FieldChangeCause,
                    cx: &mut ::gpui_form::__private::gpui::Context<Self>,
                ) {
                    form.#set_draft_ident(draft, cause, cx);
                }
            }
        }
        FieldKind::Group => {
            let store = model.attrs.store.as_ref().expect("checked");
            let store_ident = format_ident!("{}_store", ident);
            quote! {
                pub fn #value_ident(&self) -> #ty {
                    ::gpui_form::FormField::value(&self.#ident).clone()
                }

                pub fn #store_ident(
                    &self,
                ) -> ::gpui_form::__private::gpui::Entity<#store> {
                    self.#ident.store()
                }
            }
        }
        FieldKind::Array => {
            let item_ty = vec_inner_type(model)?;
            let store = model.attrs.store.as_ref().expect("checked");
            let items_ident = format_ident!("{}_items", ident);
            quote! {
                pub fn #value_ident(&self) -> #ty {
                    self.#ident
                        .items()
                        .iter()
                        .map(|item| item.item.value().clone())
                        .collect()
                }

                pub fn #items_ident(
                    &self,
                ) -> &[::gpui_form::FieldArrayItem<::gpui_form::FieldGroupStore<#item_ty, #store>>] {
                    self.#ident.items()
                }
            }
        }
    })
}

pub(super) fn field_required_methods(model: &FieldModel<'_>) -> Result<TokenStream> {
    let ident = model.ident;
    let name = &model.name;
    let required_ident = format_ident!("{}_required", ident);
    let set_required_ident = format_ident!("set_{}_required", ident);

    let required_expr = match model.attrs.component {
        FieldKind::Array => quote!(self.#ident.is_required()),
        _ => quote!(::gpui_form::FormField::is_required(&self.#ident)),
    };

    let set_required_body = match model.attrs.component {
        FieldKind::Array => quote! {
            if self.#ident.is_required() == required {
                return;
            }
            self.#ident.set_required(required);
        },
        FieldKind::Group => quote! {
            if ::gpui_form::FormField::is_required(&self.#ident) == required {
                return;
            }
            self.#ident.set_required(required);
        },
        FieldKind::Value => quote! {
            if ::gpui_form::FormField::is_required(&self.#ident) == required {
                return;
            }
            self.#ident.core_mut().set_required(required);
            self.#ident.core_mut().remove_internal_error("required");
            if self.#ident.core().validation_triggers().contains(
                ::gpui_form::ValidationTrigger::Dynamic,
            ) {
                self.apply_validation_for_scope(
                    ::gpui_form::ValidationTrigger::Dynamic,
                    ::gpui_form::ValidationScope::Field(
                        ::gpui_form::macro_support::field_path(#name),
                    ),
                    cx,
                );
            }
        },
    };

    Ok(quote! {
        pub fn #required_ident(&self) -> bool {
            #required_expr
        }

        pub fn #set_required_ident(
            &mut self,
            required: bool,
            cx: &mut ::gpui_form::__private::gpui::Context<Self>,
        ) {
            #set_required_body
            cx.notify();
        }
    })
}

pub(super) fn field_setter_methods(
    model: &FieldModel<'_>,
    field_enum_ident: &syn::Ident,
) -> Result<TokenStream> {
    let ident = model.ident;
    let ty = model.ty;
    let set_value_ident = format_ident!("set_{}_value", ident);
    let name = &model.name;
    let variant_ident = field_variant_ident(name);
    let field_variant = quote!(#field_enum_ident::#variant_ident);
    let set_draft_ident = format_ident!("set_{}_draft", ident);
    let draft_ty = field_draft_type(model);
    let write_value = match model.attrs.component {
        FieldKind::Value => quote! {
            ::gpui_form::FormField::set_value(&mut self.#ident, value, cause);
        },
        FieldKind::Group => {
            return Ok(quote! {
            pub fn #set_value_ident(
                    &mut self,
                    value: #ty,
                    cause: ::gpui_form::FieldChangeCause,
                    window: &mut ::gpui_form::__private::gpui::Window,
                    cx: &mut ::gpui_form::__private::gpui::Context<Self>,
                ) {
                    self.#ident.write_child_value(value, cause, window, cx);
                    self.refresh_meta();
                    cx.notify();
                }
            });
        }
        FieldKind::Array => return Ok(quote!()),
    };

    let draft_setter = match model.attrs.component {
        FieldKind::Value => quote! {
            pub fn #set_draft_ident(
                &mut self,
                draft: #draft_ty,
                cause: ::gpui_form::FieldChangeCause,
                cx: &mut ::gpui_form::__private::gpui::Context<Self>,
            ) {
                if !self.#ident.set_draft(draft, cause) {
                    return;
                }
                self.refresh_meta();
                cx.emit(::gpui_form::FormDraftEvent::new(
                    ::gpui_form::FieldPath::field(#name),
                    self.#ident.draft().clone(),
                    cause,
                ));
                cx.emit(::gpui_form::FormStoreEvent::FieldChanged {
                    field: #field_variant,
                    cause,
                });
                cx.notify();
            }
        },
        _ => quote!(),
    };

    Ok(quote! {
        #draft_setter

        pub fn #set_value_ident(
            &mut self,
            value: #ty,
            cause: ::gpui_form::FieldChangeCause,
            window: &mut ::gpui_form::__private::gpui::Window,
            cx: &mut ::gpui_form::__private::gpui::Context<Self>,
        ) {
            #write_value
            if cause.triggers_change_validation()
                && self.#ident.core().validation_triggers().contains(
                    ::gpui_form::ValidationTrigger::Change,
                )
            {
                self.apply_validation_for_scope(
                    ::gpui_form::ValidationTrigger::Change,
                    ::gpui_form::ValidationScope::Field(
                        ::gpui_form::macro_support::field_path(#name),
                    ),
                    cx,
                );
            }
            if cause == ::gpui_form::FieldChangeCause::Blur
                && self.#ident.core().validation_triggers().contains(
                    ::gpui_form::ValidationTrigger::Blur,
                )
            {
                self.apply_validation_for_scope(
                    ::gpui_form::ValidationTrigger::Blur,
                    ::gpui_form::ValidationScope::Field(
                        ::gpui_form::macro_support::field_path(#name),
                    ),
                    cx,
                );
            }
            self.refresh_meta();
            cx.emit(::gpui_form::FormDraftEvent::new(
                ::gpui_form::FieldPath::field(#name),
                self.#ident.draft().clone(),
                cause,
            ));
            cx.emit(::gpui_form::FormStoreEvent::FieldChanged {
                field: #field_variant,
                cause,
            });
            cx.notify();
        }
    })
}

pub(super) fn reset_field_statement(model: &FieldModel<'_>) -> TokenStream {
    let ident = model.ident;
    match model.attrs.component {
        FieldKind::Array => {
            let reset_ident = format_ident!("{}_reset_items", ident);
            quote! {
                let __gpui_form_default_items =
                    self.#ident.default_values().to_vec();
                self.#reset_ident(__gpui_form_default_items, window, cx);
            }
        }
        _ => quote! {
            ::gpui_form::FormField::reset(&mut self.#ident, window, cx);
        },
    }
}

pub(super) fn focus_error_statement(model: &FieldModel<'_>) -> TokenStream {
    let ident = model.ident;
    match model.attrs.component {
        FieldKind::Group => quote! {
            if ::gpui_form::FormField::focus(&mut self.#ident, window, cx) {
                return true;
            }
        },
        FieldKind::Array => quote! {
            for item in self.#ident.items_mut() {
                if ::gpui_form::FormField::focus(&mut item.item, window, cx) {
                    return true;
                }
            }
        },
        _ => quote! {
            if !::gpui_form::FormField::errors(&self.#ident).is_empty()
                && ::gpui_form::FormField::focus(&mut self.#ident, window, cx)
            {
                return true;
            }
        },
    }
}

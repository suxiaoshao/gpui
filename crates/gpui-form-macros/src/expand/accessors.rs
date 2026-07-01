use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Result;

use crate::field_kind::FieldKind;

use super::{FieldModel, arrays::vec_inner_type, field_variant_ident};

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
        _ => quote!(::gpui_form::FormField::value(&self.#ident).clone()),
    }
}

pub(super) fn field_meta_value(model: &FieldModel<'_>) -> TokenStream {
    let ident = model.ident;
    match model.attrs.component {
        FieldKind::Array => quote!(self.#ident.meta()),
        _ => quote!(::gpui_form::FormField::meta(&self.#ident)),
    }
}

pub(super) fn input_state_lookup_arm(
    model: &FieldModel<'_>,
    field_enum_ident: &syn::Ident,
) -> TokenStream {
    let ident = model.ident;
    let variant_ident = field_variant_ident(&model.name);
    match model.attrs.component {
        FieldKind::Input | FieldKind::Number => {
            quote!(#field_enum_ident::#variant_ident => Some(self.#ident.input_state()),)
        }
        _ => quote!(#field_enum_ident::#variant_ident => None,),
    }
}

pub(super) fn field_accessor_methods(model: &FieldModel<'_>) -> Result<TokenStream> {
    let ident = model.ident;
    let ty = model.ty;
    let value_ident = format_ident!("{}_value", ident);

    Ok(match model.attrs.component {
        FieldKind::Value | FieldKind::Bool => quote! {
            pub fn #value_ident(&self) -> #ty {
                ::gpui_form::FormField::value(&self.#ident).clone()
            }
        },
        FieldKind::Input => {
            let input_state_ident = format_ident!("{}_input_state", ident);
            quote! {
                pub fn #value_ident(&self) -> #ty {
                    ::gpui_form::FormField::value(&self.#ident).clone()
                }

                pub fn #input_state_ident(
                    &self,
                ) -> ::gpui_form::__private::gpui::Entity<::gpui_component::input::InputState> {
                    self.#ident.input_state()
                }
            }
        }
        FieldKind::Number => {
            let input_state_ident = format_ident!("{}_input_state", ident);
            let number_input_ident = format_ident!("{}_number_input", ident);
            quote! {
                pub fn #value_ident(&self) -> #ty {
                    ::gpui_form::FormField::value(&self.#ident).clone()
                }

                pub fn #input_state_ident(
                    &self,
                ) -> ::gpui_form::__private::gpui::Entity<::gpui_component::input::InputState> {
                    self.#ident.input_state()
                }

                pub fn #number_input_ident(&self) -> ::gpui_component::input::NumberInput {
                    self.#ident.number_input()
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
        FieldKind::Binding => {
            let binding = model.attrs.binding.as_ref().expect("checked");
            let state_ident = format_ident!("{}_state", ident);
            quote! {
                pub fn #value_ident(&self) -> #ty {
                    ::gpui_form::FormField::value(&self.#ident).clone()
                }

                pub fn #state_ident(
                    &self,
                ) -> ::gpui_form::__private::gpui::Entity<
                    <#binding as ::gpui_form::FormComponentBinding<#ty>>::State,
                > {
                    self.#ident.state()
                }
            }
        }
        FieldKind::Select => {
            let delegate = model.attrs.delegate.as_ref().expect("checked");
            let select_state_ident = format_ident!("{}_select_state", ident);
            quote! {
                pub fn #value_ident(&self) -> #ty {
                    ::gpui_form::FormField::value(&self.#ident).clone()
                }

                pub fn #select_state_ident(
                    &self,
                ) -> ::gpui_form::__private::gpui::Entity<
                    ::gpui_component::select::SelectState<#delegate>,
                > {
                    self.#ident.select_state()
                }
            }
        }
        FieldKind::Combobox => {
            let delegate = model.attrs.delegate.as_ref().expect("checked");
            let combobox_state_ident = format_ident!("{}_combobox_state", ident);
            quote! {
                pub fn #value_ident(&self) -> #ty {
                    ::gpui_form::FormField::value(&self.#ident).clone()
                }

                pub fn #combobox_state_ident(
                    &self,
                ) -> ::gpui_form::__private::gpui::Entity<
                    ::gpui_component::combobox::ComboboxState<#delegate>,
                > {
                    self.#ident.combobox_state()
                }
            }
        }
    })
}

pub(super) fn field_required_methods(model: &FieldModel<'_>) -> Result<TokenStream> {
    let ident = model.ident;
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
        FieldKind::Binding => quote! {
            if ::gpui_form::FormField::is_required(&self.#ident) == required {
                return;
            }
            self.#ident.set_required(required, window, cx);
        },
        _ => quote! {
            if ::gpui_form::FormField::is_required(&self.#ident) == required {
                return;
            }
            self.#ident.core_mut().set_required(required);
        },
    };

    let window_arg = match model.attrs.component {
        FieldKind::Binding => quote!(window),
        _ => quote!(_window),
    };

    Ok(quote! {
        pub fn #required_ident(&self) -> bool {
            #required_expr
        }

        pub fn #set_required_ident(
            &mut self,
            required: bool,
            #window_arg: &mut ::gpui_form::__private::gpui::Window,
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
    event_ident: &syn::Ident,
) -> Result<TokenStream> {
    let ident = model.ident;
    let ty = model.ty;
    let set_value_ident = format_ident!("set_{}_value", ident);
    let name = &model.name;
    let variant_ident = field_variant_ident(name);
    let field_variant = quote!(#field_enum_ident::#variant_ident);
    let component_write = match model.attrs.component {
        FieldKind::Input
        | FieldKind::Number
        | FieldKind::Bool
        | FieldKind::Binding
        | FieldKind::Select
        | FieldKind::Combobox => quote! {
            let __gpui_form_component_value =
                ::gpui_form::FormField::value(&self.#ident).clone();
            self.#ident.write_component_value(&__gpui_form_component_value, cause, window, cx);
        },
        FieldKind::Value => quote!(),
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
                    cx.emit(#event_ident::FieldChanged(#field_variant));
                    cx.notify();
                }
            });
        }
        FieldKind::Array => return Ok(quote!()),
    };

    Ok(quote! {
        pub fn #set_value_ident(
            &mut self,
            value: #ty,
            cause: ::gpui_form::FieldChangeCause,
            window: &mut ::gpui_form::__private::gpui::Window,
            cx: &mut ::gpui_form::__private::gpui::Context<Self>,
        ) {
            ::gpui_form::FormField::set_value(&mut self.#ident, value, cause);
            #component_write
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
            cx.emit(#event_ident::FieldChanged(#field_variant));
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

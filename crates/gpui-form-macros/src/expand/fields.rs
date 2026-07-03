use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{LitStr, Result};

use crate::{attributes::FieldAttributes, field_kind::FieldKind};

use super::{FieldModel, arrays::vec_inner_type, field_variant_ident};

pub(super) fn store_field_type(model: &FieldModel<'_>) -> Result<TokenStream> {
    let ty = model.ty;
    Ok(match model.attrs.component {
        FieldKind::Value => quote!(::gpui_form::ValueFieldStore<#ty>),
        FieldKind::Group => {
            let store = model.attrs.store.as_ref().expect("checked");
            quote!(::gpui_form::FieldGroupStore<#ty, #store>)
        }
        FieldKind::Array => {
            let store = model.attrs.store.as_ref().expect("checked");
            let item_ty = vec_inner_type(model)?;
            quote!(
                ::gpui_form::FieldArrayStore<
                    ::gpui_form::FieldGroupStore<#item_ty, #store>,
                    #item_ty,
                >
            )
        }
        FieldKind::Binding => {
            let binding = model.attrs.binding.as_ref().expect("checked");
            quote!(::gpui_form::ComponentFieldStore<#ty, #binding>)
        }
    })
}

pub(super) fn field_initializer(
    model: &FieldModel<'_>,
    field_enum_ident: &syn::Ident,
) -> Result<TokenStream> {
    let ident = model.ident;
    let value_ident = &model.value_ident;
    let state_ident = &model.state_ident;
    let name = &model.name;
    let ty = model.ty;
    let triggers = validation_triggers(&model.attrs);
    let required = model.attrs.required;
    let field_variant_ident = field_variant_ident(name);
    let field_variant = quote!(#field_enum_ident::#field_variant_ident);

    Ok(match model.attrs.component {
        FieldKind::Value => quote! {
            let mut #ident = ::gpui_form::macro_support::value_field(#name, #value_ident);
            #ident.core_mut().set_validation_triggers(#triggers);
            #ident.core_mut().set_required(#required);
        },
        FieldKind::Group => {
            let store = model.attrs.store.as_ref().expect("checked");
            quote! {
                let #state_ident = cx.new(|cx| {
                    <#store as ::gpui_form::macro_support::GeneratedFormStore<#ty>>::from_value(
                        #value_ident.clone(),
                        window,
                        cx,
                    )
                });
                let mut #ident = ::gpui_form::FieldGroupStore::new(
                    ::gpui_form::macro_support::field_path(#name),
                    #value_ident,
                    #state_ident.clone(),
                );
                #ident.set_required(#required);
                #ident.subscriptions_mut().push(
                    cx.observe(
                        &#state_ident,
                        |this, child, cx| {
                            let child = child.read(cx);
                            this.#ident.sync_from_child(
                                <#store as ::gpui_form::macro_support::GeneratedFormStore<#ty>>::draft(child),
                                <#store as ::gpui_form::macro_support::GeneratedFormStore<#ty>>::meta(child).clone(),
                            );
                            this.refresh_meta();
                            cx.notify();
                        },
                    )
                );
            }
        }
        FieldKind::Array => {
            let store = model.attrs.store.as_ref().expect("checked");
            let item_ty = vec_inner_type(model)?;
            let refresh_meta_ident = format_ident!("{}_refresh_meta", ident);
            quote! {
                let mut #ident = ::gpui_form::FieldArrayStore::<
                    ::gpui_form::FieldGroupStore<#item_ty, #store>,
                    #item_ty,
                >::empty(::gpui_form::macro_support::field_path(#name));
                let mut __gpui_form_default_values = ::std::vec::Vec::new();
                for __gpui_form_item_value in #value_ident {
                    __gpui_form_default_values.push(__gpui_form_item_value.clone());
                    let __gpui_form_child = cx.new(|cx| {
                        <#store as ::gpui_form::macro_support::GeneratedFormStore<#item_ty>>::from_value(
                            __gpui_form_item_value.clone(),
                            window,
                            cx,
                        )
                    });
                    let __gpui_form_item_index = #ident.len();
                    let __gpui_form_group = ::gpui_form::FieldGroupStore::new(
                        ::gpui_form::macro_support::field_path(#name).join_index(__gpui_form_item_index),
                        __gpui_form_item_value,
                        __gpui_form_child.clone(),
                    );
                    let __gpui_form_item_id = #ident.append_initial(__gpui_form_group);
                    let __gpui_form_subscription = cx.observe(
                        &__gpui_form_child,
                        move |this, child, cx| {
                            let child = child.read(cx);
                            if let Some(item) = this.#ident.item_mut(__gpui_form_item_id) {
                                item.item.sync_from_child(
                                    <#store as ::gpui_form::macro_support::GeneratedFormStore<#item_ty>>::draft(child),
                                    <#store as ::gpui_form::macro_support::GeneratedFormStore<#item_ty>>::meta(child).clone(),
                                );
                            }
                            this.#refresh_meta_ident();
                            this.refresh_meta();
                            cx.notify();
                        },
                    );
                    if let Some(item) = #ident.item_mut(__gpui_form_item_id) {
                        item.subscriptions_mut().push(__gpui_form_subscription);
                    }
                }
                #ident.set_default_values(__gpui_form_default_values);
                let __gpui_form_current_values = #ident
                    .items()
                    .iter()
                    .map(|item| item.item.value().clone())
                    .collect::<::std::vec::Vec<_>>();
                let __gpui_form_child_metas = #ident
                    .items()
                    .iter()
                    .map(|item| item.item.field_meta().clone())
                    .collect::<::std::vec::Vec<_>>();
                #ident.refresh_meta_from_values(
                    __gpui_form_current_values,
                    __gpui_form_child_metas,
                );
                #ident.set_required(#required);
            }
        }
        FieldKind::Binding => {
            let binding = model.attrs.binding.as_ref().expect("checked");
            let options = component_state_options(&model.attrs);
            quote! {
                let #state_ident = <#binding as ::gpui_form::FormComponentBinding<#ty>>::new_state(
                    &#value_ident,
                    #options,
                    window,
                    cx,
                );
                let mut #ident = ::gpui_form::macro_support::component_field::<#ty, #binding>(
                    #value_ident,
                    #state_ident.clone(),
                    #triggers,
                    #required,
                );
                let __gpui_form_component_event_sink =
                    ::gpui_form::FormComponentEventSink::new(
                        |this: &mut Self,
                         __gpui_form_event: ::gpui_form::FormComponentEvent,
                         _window: &mut ::gpui_form::__private::gpui::Window,
                         cx: &mut ::gpui_form::__private::gpui::Context<Self>| {
                            let __gpui_form_outcome = this.#ident.apply_component_event(
                                ::gpui_form::macro_support::field_path(#name),
                                __gpui_form_event,
                                cx,
                            );
                            let __gpui_form_field_allows_validation_trigger =
                                if let Some(__gpui_form_trigger) =
                                    ::gpui_form::macro_support::component_field_event_trigger(
                                        __gpui_form_outcome,
                                    )
                                {
                                    this.#ident
                                        .core()
                                        .validation_triggers()
                                        .contains(__gpui_form_trigger)
                                } else {
                                    false
                                };
                            this.finish_component_field_event(
                                #field_variant,
                                ::gpui_form::macro_support::field_path(#name),
                                __gpui_form_outcome,
                                __gpui_form_field_allows_validation_trigger,
                                cx,
                            );
                        },
                    );
                let __gpui_form_component_subscriptions =
                    <#binding as ::gpui_form::FormComponentBinding<#ty>>::install_subscriptions(
                        #state_ident.clone(),
                        __gpui_form_component_event_sink,
                        window,
                        cx,
                    );
                #ident.core_mut()
                    .subscriptions_mut()
                    .extend(__gpui_form_component_subscriptions);
            }
        }
    })
}

pub(super) fn write_field_statement(model: &FieldModel<'_>) -> Result<TokenStream> {
    let ident = model.ident;
    let value_ident = &model.value_ident;
    Ok(match model.attrs.component {
        FieldKind::Binding => quote! {
            self.#ident.write_component_value(&#value_ident, cause, window, cx);
        },
        FieldKind::Group => quote! {
            self.#ident.write_child_value(#value_ident, cause, window, cx);
        },
        FieldKind::Array => {
            let name = &model.name;
            let refresh_meta_ident = format_ident!("{}_refresh_meta", ident);
            quote! {
                let mut __gpui_form_values = #value_ident.into_iter();
                let mut __gpui_form_seen = 0usize;
                for __gpui_form_item in self.#ident.items_mut() {
                    if let Some(__gpui_form_value) = __gpui_form_values.next() {
                        __gpui_form_item.item.write_child_value(
                            __gpui_form_value,
                            cause,
                            window,
                            cx,
                        );
                        __gpui_form_seen += 1;
                    }
                }
                if __gpui_form_values.next().is_some() || __gpui_form_seen != self.#ident.len() {
                    self.#ident.set_errors(::std::vec![
                        ::gpui_form::FieldError::new(
                            ::gpui_form::macro_support::field_path(#name),
                            ::gpui_form::ValidationTrigger::Submit,
                            ::gpui_form::ValidationSource::Internal,
                            "array_length_changed",
                            "gpui-form-error-array-length-changed",
                        )
                    ]);
                }
                self.#refresh_meta_ident();
            }
        }
        FieldKind::Value => quote! {
            ::gpui_form::FormField::set_value(&mut self.#ident, #value_ident, cause);
        },
    })
}

fn validation_triggers(attrs: &FieldAttributes) -> TokenStream {
    if !attrs.validate_on_mount
        && !attrs.validate_on_change
        && !attrs.validate_on_blur
        && !attrs.validate_on_submit
        && !attrs.validate_on_dynamic
    {
        return quote!(::gpui_form::ValidationTriggers::default());
    }

    let on_mount = attrs.validate_on_mount;
    let on_change = attrs.validate_on_change;
    let on_blur = attrs.validate_on_blur;
    let on_submit = attrs.validate_on_submit;
    let on_dynamic = attrs.validate_on_dynamic;

    quote! {
        ::gpui_form::ValidationTriggers {
            on_mount: #on_mount,
            on_change: #on_change,
            on_blur: #on_blur,
            on_submit: #on_submit,
            on_dynamic: #on_dynamic,
        }
    }
}

fn component_state_options(attrs: &FieldAttributes) -> TokenStream {
    let label = option_lit_str(&attrs.label);
    let description = option_lit_str(&attrs.description);
    let placeholder = option_lit_str(&attrs.placeholder);
    let masked = attrs.masked;
    let required = attrs.required;

    quote! {
        ::gpui_form::ComponentStateOptions {
            label_key: #label,
            description_key: #description,
            placeholder_key: #placeholder,
            masked: #masked,
            disabled: false,
            required: #required,
        }
    }
}

fn option_lit_str(value: &Option<LitStr>) -> TokenStream {
    match value {
        Some(value) => quote!(Some(#value)),
        None => quote!(None),
    }
}

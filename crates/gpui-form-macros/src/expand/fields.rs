use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{LitStr, Result};

use crate::{attributes::FieldAttributes, field_kind::FieldKind};

use super::{FieldModel, arrays::vec_inner_type, field_variant_ident};

pub(super) fn store_field_type(model: &FieldModel<'_>) -> Result<TokenStream> {
    let ty = model.ty;
    Ok(match model.attrs.component {
        FieldKind::Value => quote!(::gpui_form::ValueFieldStore<#ty>),
        FieldKind::Input => quote!(::gpui_form::TextFieldStore<#ty>),
        FieldKind::Number => quote!(::gpui_form::NumberFieldStore<#ty>),
        FieldKind::Bool => quote!(::gpui_form::BoolFieldStore),
        FieldKind::Group => {
            let store = model.attrs.store.as_ref().expect("checked");
            quote!(::gpui_form::FieldGroupStore<#ty, #store>)
        }
        FieldKind::Array => {
            let store = model.attrs.store.as_ref().expect("checked");
            let item_ty = vec_inner_type(model)?;
            quote!(::gpui_form::FieldArrayStore<::gpui_form::FieldGroupStore<#item_ty, #store>>)
        }
        FieldKind::Binding => {
            let binding = model.attrs.binding.as_ref().expect("checked");
            quote!(::gpui_form::ComponentFieldStore<#ty, #binding>)
        }
        FieldKind::Select => {
            let delegate = model.attrs.delegate.as_ref().expect("checked");
            quote!(::gpui_form::SelectFieldStore<#ty, #delegate>)
        }
        FieldKind::Combobox => {
            let delegate = model.attrs.delegate.as_ref().expect("checked");
            quote!(::gpui_form::ComboboxFieldStore<#ty, #delegate>)
        }
    })
}

pub(super) fn field_initializer(
    model: &FieldModel<'_>,
    field_enum_ident: &syn::Ident,
    event_ident: &syn::Ident,
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
        FieldKind::Input => {
            let options = component_state_options(&model.attrs);
            quote! {
            let #state_ident =
                <::gpui_form::TextInputBinding<#ty> as ::gpui_form::FormComponentBinding<#ty>>::new_state(
                    &#value_ident,
                    #options,
                    window,
                    cx,
                );
            let mut #ident = ::gpui_form::TextFieldStore::new(
                #value_ident,
                #state_ident.clone(),
            );
            #ident.core_mut().set_validation_triggers(#triggers);
            #ident.core_mut().set_required(#required);
            #ident.core_mut().subscriptions_mut().push(
                cx.subscribe_in(
                    &#state_ident,
                    window,
                    |this, state, event: &::gpui_component::input::InputEvent, _window, cx| {
                        match event {
                            ::gpui_component::input::InputEvent::Change => {
                                if this.is_normalizing_on_submit {
                                    return;
                                }
                                let text = state.read(cx).value().to_string();
                                ::gpui_form::FormField::set_value(
                                    &mut this.#ident,
                                    <#ty as ::gpui_form::TextFieldValue>::from_text(text),
                                    ::gpui_form::FieldChangeCause::UserInput,
                                );
                                if this.#ident.core().validation_triggers().contains(
                                    ::gpui_form::ValidationTrigger::Change,
                                ) {
                                    this.apply_validation_for_scope(
                                        ::gpui_form::ValidationTrigger::Change,
                                        ::gpui_form::ValidationScope::Field(
                                            ::gpui_form::macro_support::field_path(#name),
                                        ),
                                        cx,
                                    );
                                }
                                this.refresh_meta();
                                cx.emit(#event_ident::FieldChanged(#field_variant));
                                cx.notify();
                            }
                            ::gpui_component::input::InputEvent::Focus => {
                                ::gpui_form::FormField::mark_touched(&mut this.#ident);
                                this.refresh_meta();
                                cx.emit(#event_ident::FieldFocused(#field_variant));
                                cx.notify();
                            }
                            ::gpui_component::input::InputEvent::Blur => {
                                let text = state.read(cx).value().to_string();
                                ::gpui_form::FormField::set_value(
                                    &mut this.#ident,
                                    <#ty as ::gpui_form::TextFieldValue>::from_text(text),
                                    ::gpui_form::FieldChangeCause::Blur,
                                );
                                if this.#ident.core().validation_triggers().contains(
                                    ::gpui_form::ValidationTrigger::Blur,
                                ) {
                                    this.apply_validation_for_scope(
                                        ::gpui_form::ValidationTrigger::Blur,
                                        ::gpui_form::ValidationScope::Field(
                                            ::gpui_form::macro_support::field_path(#name),
                                        ),
                                        cx,
                                    );
                                }
                                this.refresh_meta();
                                cx.emit(#event_ident::FieldBlurred(#field_variant));
                                cx.notify();
                            }
                            ::gpui_component::input::InputEvent::PressEnter { .. } => {}
                        }
                    },
                )
            );
            }
        }
        FieldKind::Number => {
            let options = component_state_options(&model.attrs);
            quote! {
            let #state_ident =
                <::gpui_form::NumberInputBinding<#ty> as ::gpui_form::FormComponentBinding<#ty>>::new_state(
                    &#value_ident,
                    #options,
                    window,
                    cx,
                );
            let mut #ident = ::gpui_form::NumberFieldStore::new(
                #value_ident,
                #state_ident.clone(),
            );
            #ident.core_mut().set_validation_triggers(#triggers);
            #ident.core_mut().set_required(#required);
            #ident.core_mut().subscriptions_mut().push(
                cx.subscribe_in(
                    &#state_ident,
                    window,
                    |this, state, event: &::gpui_component::input::InputEvent, _window, cx| {
                        match event {
                            ::gpui_component::input::InputEvent::Change => {
                                if this.is_normalizing_on_submit {
                                    return;
                                }
                                let text = state.read(cx).value().to_string();
                                match text.parse::<#ty>() {
                                    Ok(value) => {
                                        this.#ident.set_parse_error(None);
                                        ::gpui_form::FormField::set_value(
                                            &mut this.#ident,
                                            value,
                                            ::gpui_form::FieldChangeCause::UserInput,
                                        );
                                        if this.#ident.core().validation_triggers().contains(
                                            ::gpui_form::ValidationTrigger::Change,
                                        ) {
                                            this.apply_validation_for_scope(
                                                ::gpui_form::ValidationTrigger::Change,
                                                ::gpui_form::ValidationScope::Field(
                                                    ::gpui_form::macro_support::field_path(#name),
                                            ),
                                            cx,
                                        );
                                        }
                                    }
                                    Err(_) => {
                                        let error = ::gpui_form::FieldError::new(
                                            ::gpui_form::macro_support::field_path(#name),
                                            ::gpui_form::ValidationTrigger::Change,
                                            ::gpui_form::ValidationSource::Internal,
                                            "parse",
                                            "gpui-form-error-number-parse",
                                        )
                                        .with_param("value", text);
                                        this.#ident.set_parse_error(Some(error));
                                    }
                                }
                                this.refresh_meta();
                                cx.emit(#event_ident::FieldChanged(#field_variant));
                                cx.notify();
                            }
                            ::gpui_component::input::InputEvent::Focus => {
                                ::gpui_form::FormField::mark_touched(&mut this.#ident);
                                this.refresh_meta();
                                cx.emit(#event_ident::FieldFocused(#field_variant));
                                cx.notify();
                            }
                            ::gpui_component::input::InputEvent::Blur => {
                                ::gpui_form::FormField::mark_blurred(&mut this.#ident);
                                if this.#ident.core().validation_triggers().contains(
                                    ::gpui_form::ValidationTrigger::Blur,
                                ) {
                                    this.apply_validation_for_scope(
                                        ::gpui_form::ValidationTrigger::Blur,
                                        ::gpui_form::ValidationScope::Field(
                                            ::gpui_form::macro_support::field_path(#name),
                                        ),
                                        cx,
                                    );
                                }
                                this.refresh_meta();
                                cx.emit(#event_ident::FieldBlurred(#field_variant));
                                cx.notify();
                            }
                            ::gpui_component::input::InputEvent::PressEnter { .. } => {}
                        }
                    },
                )
            );
            }
        }
        FieldKind::Bool => {
            let options = component_state_options(&model.attrs);
            quote! {
            let #state_ident =
                <::gpui_form::BoolBinding as ::gpui_form::FormComponentBinding<bool>>::new_state(
                    &#value_ident,
                    #options,
                    window,
                    cx,
                );
            let mut #ident = ::gpui_form::BoolFieldStore::new(
                #value_ident,
                #state_ident.clone(),
            );
            #ident.core_mut().set_validation_triggers(#triggers);
            #ident.core_mut().set_required(#required);
            }
        }
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
                    ::gpui_form::FieldGroupStore<#item_ty, #store>
                >::new(::gpui_form::macro_support::field_path(#name), ::std::iter::empty());
                for __gpui_form_item_value in #value_ident {
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
                    let __gpui_form_item_id = #ident.append(__gpui_form_group);
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
                #ident.set_meta(::gpui_form::FieldMeta::default());
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
                let mut #ident = ::gpui_form::ComponentFieldStore::<#ty, #binding>::new(
                    #value_ident,
                    #state_ident.clone(),
                );
                #ident.core_mut().set_validation_triggers(#triggers);
                #ident.core_mut().set_required(#required);
                let __gpui_form_component_subscriptions =
                    <#binding as ::gpui_form::FormComponentBinding<#ty>>::install_subscriptions(
                        #state_ident.clone(),
                        cx.entity(),
                        window,
                        cx,
                    );
                #ident.core_mut()
                    .subscriptions_mut()
                    .extend(__gpui_form_component_subscriptions);
                #ident.core_mut().subscriptions_mut().push(
                    cx.subscribe_in(
                        &#state_ident,
                        window,
                        |this,
                         state,
                         event: &<#binding as ::gpui_form::FormComponentBinding<#ty>>::Event,
                         _window,
                         cx| {
                            let Some(__gpui_form_event) =
                                <#binding as ::gpui_form::FormComponentBinding<#ty>>::event_kind(event)
                            else {
                                return;
                            };
                            match __gpui_form_event {
                                ::gpui_form::FormComponentEvent::Change(cause) => {
                                    if this.is_normalizing_on_submit {
                                        return;
                                    }
                                    let value =
                                        <#binding as ::gpui_form::FormComponentBinding<#ty>>::read_value(
                                            state,
                                            cx,
                                        );
                                    ::gpui_form::FormField::set_value(
                                        &mut this.#ident,
                                        value,
                                        cause,
                                    );
                                    if cause.triggers_change_validation()
                                        && this.#ident.core().validation_triggers().contains(
                                            ::gpui_form::ValidationTrigger::Change,
                                        )
                                    {
                                        this.apply_validation_for_scope(
                                            ::gpui_form::ValidationTrigger::Change,
                                            ::gpui_form::ValidationScope::Field(
                                                ::gpui_form::macro_support::field_path(#name),
                                            ),
                                            cx,
                                        );
                                    }
                                    this.refresh_meta();
                                    cx.emit(#event_ident::FieldChanged(#field_variant));
                                    cx.notify();
                                }
                                ::gpui_form::FormComponentEvent::Focus => {
                                    ::gpui_form::FormField::mark_touched(&mut this.#ident);
                                    this.refresh_meta();
                                    cx.emit(#event_ident::FieldFocused(#field_variant));
                                    cx.notify();
                                }
                                ::gpui_form::FormComponentEvent::Blur => {
                                    let value =
                                        <#binding as ::gpui_form::FormComponentBinding<#ty>>::read_value(
                                            state,
                                            cx,
                                        );
                                    ::gpui_form::FormField::set_value(
                                        &mut this.#ident,
                                        value,
                                        ::gpui_form::FieldChangeCause::Blur,
                                    );
                                    if this.#ident.core().validation_triggers().contains(
                                        ::gpui_form::ValidationTrigger::Blur,
                                    ) {
                                        this.apply_validation_for_scope(
                                            ::gpui_form::ValidationTrigger::Blur,
                                            ::gpui_form::ValidationScope::Field(
                                                ::gpui_form::macro_support::field_path(#name),
                                            ),
                                            cx,
                                        );
                                    }
                                    this.refresh_meta();
                                    cx.emit(#event_ident::FieldBlurred(#field_variant));
                                    cx.notify();
                                }
                            }
                        },
                    )
                );
            }
        }
        FieldKind::Select => {
            let delegate = model.attrs.delegate.as_ref().expect("checked");
            let delegate_initializer = delegate_initializer(model);
            let searchable = model.attrs.searchable;
            let options = component_state_options(&model.attrs);
            quote! {
                let __gpui_form_delegate: #delegate = #delegate_initializer;
                let #state_ident =
                    ::gpui_form::SelectBinding::<#ty, #delegate>::new_state_with_delegate(
                        &#value_ident,
                        __gpui_form_delegate,
                        #searchable,
                        #options,
                        window,
                        cx,
                    );
                let mut #ident = ::gpui_form::SelectFieldStore::new(
                    #value_ident,
                    #state_ident.clone(),
                );
                #ident.core_mut().set_validation_triggers(#triggers);
                #ident.core_mut().set_required(#required);
                #ident.core_mut().subscriptions_mut().push(
                    cx.subscribe_in(
                        &#state_ident,
                        window,
                        |this, _state, event: &::gpui_component::select::SelectEvent<#delegate>, _window, cx| {
                            let ::gpui_component::select::SelectEvent::Confirm(selected_value) = event;
                            if this.is_normalizing_on_submit {
                                return;
                            }
                            let previous =
                                ::gpui_form::FormField::value(&this.#ident).clone();
                            let next_value =
                                <#ty as ::gpui_form::SelectFieldValue>::from_selected_value(
                                    selected_value.clone(),
                                    &previous,
                                );
                            ::gpui_form::FormField::set_value(
                                &mut this.#ident,
                                next_value,
                                ::gpui_form::FieldChangeCause::UserInput,
                            );
                            if this.#ident.core().validation_triggers().contains(
                                ::gpui_form::ValidationTrigger::Change,
                            ) {
                                this.apply_validation_for_scope(
                                    ::gpui_form::ValidationTrigger::Change,
                                    ::gpui_form::ValidationScope::Field(
                                        ::gpui_form::macro_support::field_path(#name),
                                    ),
                                    cx,
                                );
                            }
                            this.refresh_meta();
                            cx.emit(#event_ident::FieldChanged(#field_variant));
                            cx.notify();
                        },
                    )
                );
            }
        }
        FieldKind::Combobox => {
            let delegate = model.attrs.delegate.as_ref().expect("checked");
            let delegate_initializer = delegate_initializer(model);
            let searchable = model.attrs.searchable;
            let multiple = model.attrs.multiple;
            let options = component_state_options(&model.attrs);
            quote! {
                let __gpui_form_delegate: #delegate = #delegate_initializer;
                let __gpui_form_delegate_for_state = __gpui_form_delegate.clone();
                let #state_ident =
                    ::gpui_form::ComboboxBinding::<#ty, #delegate>::new_state_with_delegate(
                        &#value_ident,
                        __gpui_form_delegate_for_state,
                        #multiple,
                        #searchable,
                        #options,
                        window,
                        cx,
                    );
                let mut #ident = ::gpui_form::ComboboxFieldStore::new(
                    #value_ident,
                    #state_ident.clone(),
                    __gpui_form_delegate,
                );
                #ident.core_mut().set_validation_triggers(#triggers);
                #ident.core_mut().set_required(#required);
                #ident.core_mut().subscriptions_mut().push(
                    cx.subscribe_in(
                        &#state_ident,
                        window,
                        |this, _state, event: &::gpui_component::combobox::ComboboxEvent<#delegate>, _window, cx| {
                            if this.is_normalizing_on_submit {
                                return;
                            }
                            match event {
                                ::gpui_component::combobox::ComboboxEvent::Change(selected_values) => {
                                    let previous =
                                        ::gpui_form::FormField::value(&this.#ident).clone();
                                    let next_value =
                                        <#ty as ::gpui_form::ComboboxFieldValue>::from_selected_values(
                                            selected_values.clone(),
                                            &previous,
                                        );
                                    ::gpui_form::FormField::set_value(
                                        &mut this.#ident,
                                        next_value,
                                        ::gpui_form::FieldChangeCause::UserInput,
                                    );
                                    if this.#ident.core().validation_triggers().contains(
                                        ::gpui_form::ValidationTrigger::Change,
                                    ) {
                                        this.apply_validation_for_scope(
                                            ::gpui_form::ValidationTrigger::Change,
                                            ::gpui_form::ValidationScope::Field(
                                                ::gpui_form::macro_support::field_path(#name),
                                            ),
                                            cx,
                                        );
                                    }
                                    this.refresh_meta();
                                    cx.emit(#event_ident::FieldChanged(#field_variant));
                                }
                                ::gpui_component::combobox::ComboboxEvent::Confirm(selected_values) => {
                                    let previous =
                                        ::gpui_form::FormField::value(&this.#ident).clone();
                                    let next_value =
                                        <#ty as ::gpui_form::ComboboxFieldValue>::from_selected_values(
                                            selected_values.clone(),
                                            &previous,
                                        );
                                    ::gpui_form::FormField::set_value(
                                        &mut this.#ident,
                                        next_value,
                                        ::gpui_form::FieldChangeCause::Blur,
                                    );
                                    if this.#ident.core().validation_triggers().contains(
                                        ::gpui_form::ValidationTrigger::Blur,
                                    ) {
                                        this.apply_validation_for_scope(
                                            ::gpui_form::ValidationTrigger::Blur,
                                            ::gpui_form::ValidationScope::Field(
                                                ::gpui_form::macro_support::field_path(#name),
                                            ),
                                            cx,
                                        );
                                    }
                                    this.refresh_meta();
                                    cx.emit(#event_ident::FieldBlurred(#field_variant));
                                }
                            }
                            cx.notify();
                        },
                    )
                );
            }
        }
    })
}

pub(super) fn write_field_statement(model: &FieldModel<'_>) -> Result<TokenStream> {
    let ident = model.ident;
    let value_ident = &model.value_ident;
    Ok(match model.attrs.component {
        FieldKind::Input
        | FieldKind::Number
        | FieldKind::Bool
        | FieldKind::Select
        | FieldKind::Combobox => quote! {
            ::gpui_form::FormField::set_value(&mut self.#ident, #value_ident, cause);
            let __gpui_form_component_value =
                ::gpui_form::FormField::value(&self.#ident).clone();
            self.#ident.write_component_value(&__gpui_form_component_value, cause, window, cx);
        },
        FieldKind::Binding => quote! {
            ::gpui_form::FormField::set_value(&mut self.#ident, #value_ident, cause);
            let __gpui_form_component_value =
                ::gpui_form::FormField::value(&self.#ident).clone();
            self.#ident.write_component_value(&__gpui_form_component_value, cause, window, cx);
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

fn delegate_initializer(model: &FieldModel<'_>) -> TokenStream {
    match &model.attrs.options {
        Some(options) => quote!(#options),
        None => quote!(::std::default::Default::default()),
    }
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

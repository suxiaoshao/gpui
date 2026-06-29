use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{GenericArgument, PathArguments, Result, Type};

use super::FieldModel;

pub(super) fn array_helper_methods(model: &FieldModel<'_>) -> Result<TokenStream> {
    let ident = model.ident;
    let name = &model.name;
    let item_ty = vec_inner_type(model)?;
    let store = model.attrs.store.as_ref().expect("checked");
    let append_ident = format_ident!("{}_append", ident);
    let insert_ident = format_ident!("{}_insert", ident);
    let remove_ident = format_ident!("{}_remove", ident);
    let remove_id_ident = format_ident!("{}_remove_id", ident);
    let move_ident = format_ident!("{}_move", ident);
    let swap_ident = format_ident!("{}_swap", ident);
    let replace_ident = format_ident!("{}_replace", ident);
    let reset_ident = format_ident!("{}_reset_items", ident);
    let values_with_id_ident = format_ident!("{}_values_with_id", ident);
    let refresh_paths_ident = format_ident!("{}_refresh_paths", ident);
    let refresh_meta_ident = format_ident!("{}_refresh_meta", ident);

    Ok(quote! {
        pub fn #append_ident(
            &mut self,
            value: #item_ty,
            window: &mut ::gpui_form::__private::gpui::Window,
            cx: &mut ::gpui_form::__private::gpui::Context<Self>,
        ) -> ::gpui_form::FormItemId {
            let __gpui_form_child = cx.new(|cx| {
                <#store as ::gpui_form::macro_support::GeneratedFormStore<#item_ty>>::from_value(
                    value.clone(),
                    window,
                    cx,
                )
            });
            let __gpui_form_item_index = self.#ident.len();
            let __gpui_form_group = ::gpui_form::FieldGroupStore::new(
                ::gpui_form::macro_support::field_path(#name).join_index(__gpui_form_item_index),
                value,
                __gpui_form_child.clone(),
            );
            let __gpui_form_item_id = self.#ident.append(__gpui_form_group);
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
            if let Some(item) = self.#ident.item_mut(__gpui_form_item_id) {
                item.subscriptions_mut().push(__gpui_form_subscription);
            }
            self.#refresh_paths_ident();
            self.#refresh_meta_ident();
            self.refresh_meta();
            cx.notify();
            __gpui_form_item_id
        }

        pub fn #insert_ident(
            &mut self,
            index: usize,
            value: #item_ty,
            window: &mut ::gpui_form::__private::gpui::Window,
            cx: &mut ::gpui_form::__private::gpui::Context<Self>,
        ) -> Result<::gpui_form::FormItemId, ::gpui_form::ArrayIndexError> {
            if index > self.#ident.len() {
                return Err(::gpui_form::ArrayIndexError {
                    index,
                    len: self.#ident.len(),
                });
            }

            let __gpui_form_child = cx.new(|cx| {
                <#store as ::gpui_form::macro_support::GeneratedFormStore<#item_ty>>::from_value(
                    value.clone(),
                    window,
                    cx,
                )
            });
            let __gpui_form_group = ::gpui_form::FieldGroupStore::new(
                ::gpui_form::macro_support::field_path(#name).join_index(index),
                value,
                __gpui_form_child.clone(),
            );
            let __gpui_form_item_id = self.#ident.insert(index, __gpui_form_group)?;
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
            if let Some(item) = self.#ident.item_mut(__gpui_form_item_id) {
                item.subscriptions_mut().push(__gpui_form_subscription);
            }
            self.#refresh_paths_ident();
            self.#refresh_meta_ident();
            self.refresh_meta();
            cx.notify();
            Ok(__gpui_form_item_id)
        }

        pub fn #remove_ident(
            &mut self,
            index: usize,
            cx: &mut ::gpui_form::__private::gpui::Context<Self>,
        ) -> Result<::gpui_form::FieldArrayItem<::gpui_form::FieldGroupStore<#item_ty, #store>>, ::gpui_form::ArrayIndexError> {
            let removed = self.#ident.remove(index)?;
            self.#refresh_paths_ident();
            self.#refresh_meta_ident();
            self.refresh_meta();
            cx.notify();
            Ok(removed)
        }

        pub fn #remove_id_ident(
            &mut self,
            id: ::gpui_form::FormItemId,
            cx: &mut ::gpui_form::__private::gpui::Context<Self>,
        ) -> Option<::gpui_form::FieldArrayItem<::gpui_form::FieldGroupStore<#item_ty, #store>>> {
            let removed = self.#ident.remove_id(id)?;
            self.#refresh_paths_ident();
            self.#refresh_meta_ident();
            self.refresh_meta();
            cx.notify();
            Some(removed)
        }

        pub fn #move_ident(
            &mut self,
            from: usize,
            to: usize,
            cx: &mut ::gpui_form::__private::gpui::Context<Self>,
        ) -> Result<(), ::gpui_form::ArrayIndexError> {
            self.#ident.move_item(from, to)?;
            self.#refresh_paths_ident();
            self.#refresh_meta_ident();
            self.refresh_meta();
            cx.notify();
            Ok(())
        }

        pub fn #swap_ident(
            &mut self,
            a: usize,
            b: usize,
            cx: &mut ::gpui_form::__private::gpui::Context<Self>,
        ) -> Result<(), ::gpui_form::ArrayIndexError> {
            self.#ident.swap(a, b)?;
            self.#refresh_paths_ident();
            self.#refresh_meta_ident();
            self.refresh_meta();
            cx.notify();
            Ok(())
        }

        pub fn #replace_ident(
            &mut self,
            values: impl IntoIterator<Item = #item_ty>,
            window: &mut ::gpui_form::__private::gpui::Window,
            cx: &mut ::gpui_form::__private::gpui::Context<Self>,
        ) {
            self.#ident = ::gpui_form::FieldArrayStore::<
                ::gpui_form::FieldGroupStore<#item_ty, #store>
            >::new(::gpui_form::macro_support::field_path(#name), ::std::iter::empty());
            for value in values {
                self.#append_ident(value, window, cx);
            }
            self.#refresh_paths_ident();
            self.#refresh_meta_ident();
            self.refresh_meta();
            cx.notify();
        }

        pub fn #reset_ident(
            &mut self,
            values: impl IntoIterator<Item = #item_ty>,
            window: &mut ::gpui_form::__private::gpui::Window,
            cx: &mut ::gpui_form::__private::gpui::Context<Self>,
        ) {
            self.#replace_ident(values, window, cx);
            self.#ident.set_meta(::gpui_form::FieldMeta::default());
            self.refresh_meta();
            cx.notify();
        }

        pub fn #values_with_id_ident(
            &self,
        ) -> ::std::vec::Vec<::gpui_form::FormRowValue<#item_ty>> {
            self.#ident
                .items()
                .iter()
                .map(|item| ::gpui_form::FormRowValue {
                    id: item.id,
                    value: item.item.value().clone(),
                })
                .collect()
        }

        fn #refresh_paths_ident(&mut self) {
            for item in self.#ident.items_mut() {
                item.item.set_path(
                    ::gpui_form::macro_support::field_path(#name).join_index(item.index),
                );
            }
        }

        fn #refresh_meta_ident(&mut self) {
            let mut __gpui_form_meta = ::gpui_form::FieldMeta::default();
            for item in self.#ident.items() {
                let item_meta = item.item.field_meta();
                __gpui_form_meta.is_dirty |= item_meta.is_dirty;
                __gpui_form_meta.is_touched |= item_meta.is_touched;
                __gpui_form_meta.is_blurred |= item_meta.is_blurred;
                __gpui_form_meta.is_validating |= item_meta.is_validating;
                __gpui_form_meta.is_valid &= item_meta.is_valid;
            }
            __gpui_form_meta.is_pristine = !__gpui_form_meta.is_dirty;
            __gpui_form_meta.is_default_value = __gpui_form_meta.is_pristine;
            if !self.#ident.errors().is_empty() {
                __gpui_form_meta.set_valid(false);
            }
            self.#ident.set_meta(__gpui_form_meta);
        }
    })
}

pub(super) fn vec_inner_type<'a>(model: &'a FieldModel<'_>) -> Result<&'a Type> {
    let Type::Path(path) = model.ty else {
        return Err(syn::Error::new_spanned(
            model.field,
            "array form components require a Vec<T> field",
        ));
    };

    let Some(segment) = path.path.segments.last() else {
        return Err(syn::Error::new_spanned(
            model.field,
            "array form components require a Vec<T> field",
        ));
    };

    if segment.ident != "Vec" {
        return Err(syn::Error::new_spanned(
            model.field,
            "array form components require a Vec<T> field",
        ));
    }

    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return Err(syn::Error::new_spanned(
            model.field,
            "array form components require a Vec<T> field",
        ));
    };

    match args.args.first() {
        Some(GenericArgument::Type(ty)) => Ok(ty),
        _ => Err(syn::Error::new_spanned(
            model.field,
            "array form components require a Vec<T> field",
        )),
    }
}

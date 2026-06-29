use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Field, Fields, GenericParam, Result, Type, parse_quote, parse2};

use crate::{
    attributes::{FieldAttributes, FormAttributes},
    field_kind::FieldKind,
};

mod accessors;
mod arrays;
mod errors;
mod fields;
mod pipeline;
mod validation;

use accessors::{
    draft_field_value, field_accessor_methods, field_meta_value, field_setter_methods,
    focus_error_statement, input_state_lookup_arm, reset_field_statement,
};
use arrays::{array_helper_methods, vec_inner_type};
use errors::{apply_field_error_arm, clear_all_error_statement, clear_field_error_arm};
use fields::{field_initializer, store_field_type, write_field_statement};
use pipeline::{
    submit_transform, submit_validation, transform_field, transform_init, validate_method_body,
    validation_field, validation_init,
};
use validation::apply_validation_statement;

struct FieldModel<'a> {
    field: &'a Field,
    ident: &'a syn::Ident,
    ty: &'a Type,
    name: String,
    value_ident: syn::Ident,
    state_ident: syn::Ident,
    attrs: FieldAttributes,
}

fn related_store_ident(store_ident: &syn::Ident, suffix: &str) -> syn::Ident {
    let store_name = store_ident.to_string();
    let base = store_name.strip_suffix("Store").unwrap_or(&store_name);
    format_ident!("{base}{suffix}")
}

fn field_variant_ident(name: &str) -> syn::Ident {
    let name = name.strip_prefix("r#").unwrap_or(name);
    let mut variant = String::new();
    let mut uppercase_next = true;

    for ch in name.chars() {
        if ch == '_' || ch == '-' {
            uppercase_next = true;
            continue;
        }

        if uppercase_next {
            variant.extend(ch.to_uppercase());
            uppercase_next = false;
        } else {
            variant.push(ch);
        }
    }

    if variant.is_empty() {
        variant.push_str("Field");
    }

    format_ident!("{variant}")
}

pub(crate) fn derive_form_store(input: TokenStream) -> Result<TokenStream> {
    let input = parse2::<DeriveInput>(input)?;
    let input_ident = &input.ident;
    let input_vis = &input.vis;
    let attrs = FormAttributes::parse(&input.attrs)?;
    let store_ident = attrs
        .store
        .unwrap_or_else(|| format_ident!("{}FormStore", input_ident));

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => fields,
            _ => {
                return Err(syn::Error::new_spanned(
                    &input.ident,
                    "FormStore can only be derived for structs with named fields",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "FormStore can only be derived for structs",
            ));
        }
    };

    let field_models = fields
        .named
        .iter()
        .map(|field| {
            let ident = field.ident.as_ref().expect("named field");
            Ok(FieldModel {
                field,
                ident,
                ty: &field.ty,
                name: ident.to_string(),
                value_ident: format_ident!("__gpui_form_{}_value", ident),
                state_ident: format_ident!("__gpui_form_{}_state", ident),
                attrs: FieldAttributes::parse(&field.attrs)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    for model in &field_models {
        if model.attrs.component == FieldKind::Binding && model.attrs.binding.is_none() {
            return Err(syn::Error::new_spanned(
                model.field,
                "component bindings require #[form(binding = \"TypeName\")]",
            ));
        }
        if matches!(model.attrs.component, FieldKind::Group | FieldKind::Array)
            && model.attrs.store.is_none()
        {
            return Err(syn::Error::new_spanned(
                model.field,
                "group and array form components require #[form(store = \"ChildFormStore\")]",
            ));
        }
        if matches!(
            model.attrs.component,
            FieldKind::Select | FieldKind::Combobox
        ) && model.attrs.delegate.is_none()
        {
            return Err(syn::Error::new_spanned(
                model.field,
                "select and combobox form components require #[form(delegate = \"DelegateType\")]",
            ));
        }
    }

    let mut store_generics = input.generics.clone();
    for param in store_generics.params.iter_mut() {
        if let GenericParam::Type(type_param) = param {
            type_param.default = None;
        }
    }
    let store_where = store_generics.make_where_clause();
    for model in &field_models {
        let ty = model.ty;
        match model.attrs.component {
            FieldKind::Value => {
                store_where
                    .predicates
                    .push(parse_quote!(#ty: Clone + PartialEq + 'static));
            }
            FieldKind::Input => {
                store_where
                    .predicates
                    .push(parse_quote!(#ty: ::gpui_form::TextFieldValue));
            }
            FieldKind::Number => {
                store_where
                    .predicates
                    .push(parse_quote!(#ty: ::gpui_form::NumberFieldValue));
            }
            FieldKind::Bool => {}
            FieldKind::Group => {
                let store = model.attrs.store.as_ref().expect("checked");
                store_where
                    .predicates
                    .push(parse_quote!(#ty: Clone + PartialEq + 'static));
                store_where.predicates.push(parse_quote!(
                    #store: ::gpui_form::macro_support::GeneratedFormStore<#ty>
                        + ::gpui_form::FormStore<Output = #ty>
                ));
            }
            FieldKind::Array => {
                let store = model.attrs.store.as_ref().expect("checked");
                let item_ty = vec_inner_type(model)?;
                store_where
                    .predicates
                    .push(parse_quote!(#item_ty: Clone + PartialEq + 'static));
                store_where.predicates.push(parse_quote!(
                    #store: ::gpui_form::macro_support::GeneratedFormStore<#item_ty>
                        + ::gpui_form::FormStore<Output = #item_ty>
                ));
            }
            FieldKind::Binding => {
                let binding = model.attrs.binding.as_ref().expect("checked");
                store_where
                    .predicates
                    .push(parse_quote!(#ty: Clone + PartialEq + 'static));
                store_where
                    .predicates
                    .push(parse_quote!(#binding: ::gpui_form::FormComponentBinding<#ty>));
            }
            FieldKind::Select => {
                let delegate = model.attrs.delegate.as_ref().expect("checked");
                store_where
                    .predicates
                    .push(parse_quote!(#ty: ::gpui_form::SelectFieldValue));
                store_where.predicates.push(parse_quote!(
                    #delegate: ::gpui_component::searchable_list::SearchableListDelegate + 'static
                ));
                store_where.predicates.push(parse_quote!(
                    <#delegate as ::gpui_component::searchable_list::SearchableListDelegate>::Item:
                        ::gpui_component::searchable_list::SearchableListItem<
                            Value = <#ty as ::gpui_form::SelectFieldValue>::Selected
                        >
                ));
                if model.attrs.options.is_none() {
                    store_where
                        .predicates
                        .push(parse_quote!(#delegate: Default));
                }
            }
            FieldKind::Combobox => {
                let delegate = model.attrs.delegate.as_ref().expect("checked");
                store_where
                    .predicates
                    .push(parse_quote!(#ty: ::gpui_form::ComboboxFieldValue));
                store_where.predicates.push(parse_quote!(
                    #delegate: ::gpui_component::searchable_list::SearchableListDelegate
                        + Clone
                        + 'static
                ));
                store_where.predicates.push(parse_quote!(
                    <#delegate as ::gpui_component::searchable_list::SearchableListDelegate>::Item:
                        ::gpui_component::searchable_list::SearchableListItem<
                            Value = <#ty as ::gpui_form::ComboboxFieldValue>::Selected
                        >
                ));
                if model.attrs.options.is_none() {
                    store_where
                        .predicates
                        .push(parse_quote!(#delegate: Default));
                }
            }
        }
    }

    let (impl_generics, ty_generics, where_clause) = store_generics.split_for_impl();
    let field_enum_ident = related_store_ident(&store_ident, "Field");
    let event_ident = related_store_ident(&store_ident, "Event");
    let validation_field = validation_field(attrs.validation, input_ident, &ty_generics);
    let validation_init = validation_init(attrs.validation);
    let transform_field = transform_field(attrs.transform, input_ident, &ty_generics);
    let transform_init = transform_init(attrs.transform);
    let validate_method_body =
        validate_method_body(attrs.validation, attrs.transform, input_ident, &ty_generics);
    let submit_transform = submit_transform(attrs.transform, input_ident, &ty_generics);
    let submit_validation = submit_validation(attrs.validation, input_ident, &ty_generics);

    let field_idents = field_models
        .iter()
        .map(|model| model.ident)
        .collect::<Vec<_>>();
    let value_idents = field_models
        .iter()
        .map(|model| &model.value_ident)
        .collect::<Vec<_>>();
    let field_names = field_models
        .iter()
        .map(|model| &model.name)
        .collect::<Vec<_>>();
    let field_variant_idents = field_models
        .iter()
        .map(|model| field_variant_ident(&model.name))
        .collect::<Vec<_>>();
    let store_field_types = field_models
        .iter()
        .map(store_field_type)
        .collect::<Result<Vec<_>>>()?;
    let field_initializers = field_models
        .iter()
        .map(|model| field_initializer(model, &field_enum_ident, &event_ident))
        .collect::<Result<Vec<_>>>()?;
    let write_field_statements = field_models
        .iter()
        .map(write_field_statement)
        .collect::<Result<Vec<_>>>()?;
    let draft_field_values = field_models
        .iter()
        .map(draft_field_value)
        .collect::<Vec<_>>();
    let field_meta_values = field_models
        .iter()
        .map(field_meta_value)
        .collect::<Vec<_>>();
    let reset_field_statements = field_models
        .iter()
        .map(reset_field_statement)
        .collect::<Vec<_>>();
    let apply_validation_statements = field_models
        .iter()
        .map(apply_validation_statement)
        .collect::<Result<Vec<_>>>()?;
    let focus_error_statements = field_models
        .iter()
        .map(focus_error_statement)
        .collect::<Vec<_>>();
    let field_accessor_methods = field_models
        .iter()
        .map(field_accessor_methods)
        .collect::<Result<Vec<_>>>()?;
    let field_setter_methods = field_models
        .iter()
        .map(|model| field_setter_methods(model, &field_enum_ident, &event_ident))
        .collect::<Result<Vec<_>>>()?;
    let clear_all_error_statements = field_models
        .iter()
        .map(clear_all_error_statement)
        .collect::<Result<Vec<_>>>()?;
    let clear_field_error_arms = field_models
        .iter()
        .map(|model| clear_field_error_arm(model, &field_enum_ident))
        .collect::<Result<Vec<_>>>()?;
    let apply_field_error_arms = field_models
        .iter()
        .map(|model| apply_field_error_arm(model, &field_enum_ident))
        .collect::<Result<Vec<_>>>()?;
    let input_state_lookup_arms = field_models
        .iter()
        .map(|model| input_state_lookup_arm(model, &field_enum_ident))
        .collect::<Vec<_>>();
    let array_helper_methods = field_models
        .iter()
        .filter(|model| model.attrs.component == FieldKind::Array)
        .map(array_helper_methods)
        .collect::<Result<Vec<_>>>()?;

    let expanded = quote! {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #input_vis enum #field_enum_ident {
            #(#field_variant_idents,)*
        }

        impl #field_enum_ident {
            pub const fn key(self) -> &'static str {
                match self {
                    #(Self::#field_variant_idents => #field_names,)*
                }
            }

            pub fn from_key(key: &str) -> Option<Self> {
                match key {
                    #(#field_names => Some(Self::#field_variant_idents),)*
                    _ => None,
                }
            }
        }

        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #input_vis enum #event_ident {
            FieldChanged(#field_enum_ident),
            FieldFocused(#field_enum_ident),
            FieldBlurred(#field_enum_ident),
        }

        impl #event_ident {
            pub const fn field(self) -> #field_enum_ident {
                match self {
                    Self::FieldChanged(field)
                    | Self::FieldFocused(field)
                    | Self::FieldBlurred(field) => field,
                }
            }
        }

        #input_vis struct #store_ident #store_generics {
            #(
                pub #field_idents: #store_field_types,
            )*
            meta: ::gpui_form::FormMeta,
            form_errors: ::std::vec::Vec<::gpui_form::FormError>,
            field_paths: ::std::vec::Vec<::gpui_form::FieldPath>,
            is_normalizing_on_submit: bool,
            #validation_field
            #transform_field
        }

        impl #impl_generics #store_ident #ty_generics #where_clause {
            pub fn from_value(
                value: #input_ident #ty_generics,
                window: &mut ::gpui_form::__private::gpui::Window,
                cx: &mut ::gpui_form::__private::gpui::Context<Self>,
            ) -> Self {
                let #input_ident { #(#field_idents: #value_idents),* } = value;

                #(#field_initializers)*

                Self {
                    #(#field_idents,)*
                    meta: ::gpui_form::FormMeta::default(),
                    form_errors: ::std::vec::Vec::new(),
                    field_paths: ::std::vec![
                        #(::gpui_form::macro_support::field_path(#field_names),)*
                    ],
                    is_normalizing_on_submit: false,
                    #validation_init
                    #transform_init
                }
            }

            pub fn draft(&self) -> #input_ident #ty_generics {
                #input_ident {
                    #(
                        #field_idents: #draft_field_values,
                    )*
                }
            }

            pub fn write_draft(
                &mut self,
                value: #input_ident #ty_generics,
                cause: ::gpui_form::FieldChangeCause,
                window: &mut ::gpui_form::__private::gpui::Window,
                cx: &mut ::gpui_form::__private::gpui::App,
            ) {
                let #input_ident { #(#field_idents: #value_idents),* } = value;
                let previous_normalizing = self.is_normalizing_on_submit;
                self.is_normalizing_on_submit =
                    cause == ::gpui_form::FieldChangeCause::NormalizeOnSubmit;

                #(#write_field_statements)*

                self.is_normalizing_on_submit = previous_normalizing;
                self.refresh_meta();
            }

            pub fn field_paths(&self) -> &[::gpui_form::FieldPath] {
                &self.field_paths
            }

            pub fn meta(&self) -> &::gpui_form::FormMeta {
                &self.meta
            }

            pub fn form_errors(&self) -> &[::gpui_form::FormError] {
                &self.form_errors
            }

            pub fn input_state_for_field(
                &self,
                field: #field_enum_ident,
            ) -> Option<
                ::gpui_form::__private::gpui::Entity<
                    ::gpui_component::input::InputState,
                >,
            > {
                match field {
                    #(#input_state_lookup_arms)*
                }
            }

            #(#field_accessor_methods)*

            #(#field_setter_methods)*

            #(#array_helper_methods)*

            pub fn clear_all_errors(
                &mut self,
                cx: &mut ::gpui_form::__private::gpui::Context<Self>,
            ) {
                self.clear_all_errors_with_app(cx);
                cx.notify();
            }

            pub fn clear_field_errors(
                &mut self,
                field: #field_enum_ident,
                cx: &mut ::gpui_form::__private::gpui::Context<Self>,
            ) {
                match field {
                    #(#clear_field_error_arms)*
                }
                self.refresh_meta();
                cx.notify();
            }

            pub fn apply_field_error(
                &mut self,
                field: #field_enum_ident,
                error: ::gpui_form::FieldError,
                cx: &mut ::gpui_form::__private::gpui::Context<Self>,
            ) {
                match field {
                    #(#apply_field_error_arms)*
                }
                self.refresh_meta();
                cx.notify();
            }

            fn refresh_meta(&mut self) {
                let was_submitting = self.meta.is_submitting;
                let was_submitted = self.meta.is_submitted;
                let was_submit_successful = self.meta.is_submit_successful;
                let submission_attempts = self.meta.submission_attempts;
                let field_meta = [
                    #(#field_meta_values,)*
                ];
                let mut meta = ::gpui_form::FormMeta::aggregate(field_meta);
                meta.is_submitting = was_submitting;
                meta.is_submitted = was_submitted;
                meta.is_submit_successful = was_submit_successful;
                meta.submission_attempts = submission_attempts;
                if self.form_errors.iter().any(|error| {
                    error.severity == ::gpui_form::ValidationSeverity::Error
                }) {
                    meta.is_valid = false;
                }
                meta.can_submit = !meta.is_submitting && !meta.is_validating;
                self.meta = meta;
            }

            fn clear_all_errors_with_app(
                &mut self,
                cx: &mut ::gpui_form::__private::gpui::App,
            ) {
                #(#clear_all_error_statements)*
                self.form_errors.clear();
                self.refresh_meta();
            }

            fn validation_report_for_scope(
                &self,
                trigger: ::gpui_form::ValidationTrigger,
                scope: ::gpui_form::ValidationScope,
            ) -> ::gpui_form::FormValidationReport {
                #validate_method_body
            }

            fn apply_validation_for_scope(
                &mut self,
                trigger: ::gpui_form::ValidationTrigger,
                scope: ::gpui_form::ValidationScope,
                cx: &mut ::gpui_form::__private::gpui::App,
            ) -> ::gpui_form::FormValidationReport {
                let report = self.validation_report_for_scope(trigger, scope.clone());
                self.apply_validation_report(&report, &scope, cx);
                self.refresh_meta();
                report
            }

            fn apply_validation_report(
                &mut self,
                report: &::gpui_form::FormValidationReport,
                scope: &::gpui_form::ValidationScope,
                cx: &mut ::gpui_form::__private::gpui::App,
            ) {
                #(#apply_validation_statements)*

                if matches!(scope, &::gpui_form::ValidationScope::Form) {
                    self.form_errors = report.form_errors().to_vec();
                }
            }
        }

        impl #impl_generics ::gpui_form::macro_support::GeneratedFormStore<#input_ident #ty_generics>
            for #store_ident #ty_generics
            #where_clause
        {
            fn from_value(
                value: #input_ident #ty_generics,
                window: &mut ::gpui_form::__private::gpui::Window,
                cx: &mut ::gpui_form::__private::gpui::Context<Self>,
            ) -> Self {
                Self::from_value(value, window, cx)
            }

            fn draft(&self) -> #input_ident #ty_generics {
                self.draft()
            }

            fn field_paths(&self) -> &[::gpui_form::FieldPath] {
                &self.field_paths
            }

            fn meta(&self) -> &::gpui_form::FormMeta {
                &self.meta
            }

            fn write_draft(
                &mut self,
                value: #input_ident #ty_generics,
                cause: ::gpui_form::FieldChangeCause,
                window: &mut ::gpui_form::__private::gpui::Window,
                cx: &mut ::gpui_form::__private::gpui::App,
            ) {
                Self::write_draft(self, value, cause, window, cx);
            }

            fn apply_validation_report(
                &mut self,
                report: &::gpui_form::FormValidationReport,
                scope: &::gpui_form::ValidationScope,
                cx: &mut ::gpui_form::__private::gpui::App,
            ) {
                Self::apply_validation_report(self, report, scope, cx);
                self.refresh_meta();
            }

            fn clear_all_errors(&mut self, cx: &mut ::gpui_form::__private::gpui::App) {
                Self::clear_all_errors_with_app(self, cx);
            }
        }

        impl #impl_generics ::gpui_form::FormStore for #store_ident #ty_generics #where_clause {
            type Output = #input_ident #ty_generics;

            fn meta(&self) -> &::gpui_form::FormMeta {
                &self.meta
            }

            fn reset(
                &mut self,
                window: &mut ::gpui_form::__private::gpui::Window,
                cx: &mut ::gpui_form::__private::gpui::Context<Self>,
            ) {
                #(#reset_field_statements)*
                self.meta = ::gpui_form::FormMeta::default();
            }

            fn validate(
                &mut self,
                trigger: ::gpui_form::ValidationTrigger,
                _window: &mut ::gpui_form::__private::gpui::Window,
                cx: &mut ::gpui_form::__private::gpui::App,
            ) -> ::gpui_form::FormValidationReport {
                self.apply_validation_for_scope(trigger, ::gpui_form::ValidationScope::Form, cx)
            }

            fn submit(
                &mut self,
                window: &mut ::gpui_form::__private::gpui::Window,
                cx: &mut ::gpui_form::__private::gpui::App,
            ) -> Result<Self::Output, ::gpui_form::FormValidationReport> {
                self.meta.begin_submit();
                #submit_transform
                self.write_draft(
                    normalized.clone(),
                    ::gpui_form::FieldChangeCause::NormalizeOnSubmit,
                    window,
                    cx,
                );
                #submit_validation
                self.apply_validation_report(&report, &::gpui_form::ValidationScope::Form, cx);
                self.refresh_meta();
                if report.is_valid() {
                    self.meta.finish_submit_success();
                    Ok(normalized)
                } else {
                    self.meta.finish_submit_failure();
                    Err(report)
                }
            }

            fn focus_first_error(
                &mut self,
                window: &mut ::gpui_form::__private::gpui::Window,
                cx: &mut ::gpui_form::__private::gpui::App,
            ) -> bool {
                #(#focus_error_statements)*
                false
            }
        }

        impl #impl_generics ::gpui_form::__private::gpui::EventEmitter<#event_ident>
            for #store_ident #ty_generics
            #where_clause
        {
        }
    };

    Ok(expanded)
}

use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use std::collections::HashSet;
use syn::{Data, DeriveInput, Field, Fields, GenericParam, Result, Type, parse_quote, parse2};

use crate::{
    attributes::{FieldAttributes, FormAttributes, ValidationAdapterKind},
    field_kind::FieldKind,
};

mod accessors;
mod arrays;
mod errors;
mod fields;
mod pipeline;
mod validation;

use accessors::{
    draft_field_value, field_accessor_methods, field_meta_value, field_required_methods,
    field_setter_methods, focus_error_statement, replace_field_statement, reset_field_statement,
};
use arrays::{array_helper_methods, vec_inner_type};
use errors::{apply_field_error_arm, clear_all_error_statement, clear_field_error_arm};
use fields::{field_initializer, store_field_type, write_field_statement};
use pipeline::{
    submit_transform, submit_validation, transform_field, transform_init, validate_method_body,
    validation_context_type, validation_field, validation_init,
};
use validation::{
    apply_validation_statement, current_validation_report_statement, prepare_submit_statement,
    required_validation_statement,
};

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

fn dedupe_where_predicates(where_clause: &mut syn::WhereClause) {
    let mut seen = HashSet::new();
    let mut predicates = syn::punctuated::Punctuated::new();

    for predicate in where_clause.predicates.iter().cloned() {
        if seen.insert(predicate.to_token_stream().to_string()) {
            predicates.push(predicate);
        }
    }

    where_clause.predicates = predicates;
}

pub(crate) fn derive_form_store(input: TokenStream) -> Result<TokenStream> {
    let input = parse2::<DeriveInput>(input)?;
    let input_ident = &input.ident;
    let input_vis = &input.vis;
    let attrs = FormAttributes::parse(&input.attrs)?;
    let form_uses_custom_validation =
        matches!(attrs.validation, ValidationAdapterKind::Custom { .. });
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
        if matches!(model.attrs.component, FieldKind::Group | FieldKind::Array)
            && model.attrs.store.is_none()
        {
            return Err(syn::Error::new_spanned(
                model.field,
                "group and array form components require #[form(store = \"ChildFormStore\")]",
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
                let codec = model
                    .attrs
                    .codec
                    .as_ref()
                    .map(|codec| quote!(#codec))
                    .unwrap_or_else(|| quote!(::gpui_form::IdentityCodec<#ty>));
                store_where
                    .predicates
                    .push(parse_quote!(#codec: ::gpui_form::FieldCodec<#ty>));
            }
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
        }
        if model.attrs.required
            && !form_uses_custom_validation
            && matches!(model.attrs.component, FieldKind::Value | FieldKind::Array)
        {
            let ty = model.ty;
            store_where
                .predicates
                .push(parse_quote!(#ty: ::gpui_form::RequiredValue));
        }
    }
    dedupe_where_predicates(store_where);

    let (impl_generics, ty_generics, where_clause) = store_generics.split_for_impl();
    let field_enum_ident = related_store_ident(&store_ident, "Field");
    let validation_context_ty = validation_context_type(&attrs.validation);
    let validation_field = validation_field(&attrs.validation, input_ident, &ty_generics);
    let validation_init = validation_init(&attrs.validation);
    let transform_field = transform_field(&attrs.transform, input_ident, &ty_generics);
    let transform_init = transform_init(&attrs.transform);
    let validate_method_body = validate_method_body(
        &attrs.validation,
        &attrs.transform,
        input_ident,
        &ty_generics,
    );
    let submit_transform = submit_transform(&attrs.transform, input_ident, &ty_generics);
    let submit_validation = submit_validation(&attrs.validation, input_ident, &ty_generics);

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
        .map(field_initializer)
        .collect::<Result<Vec<_>>>()?;
    let write_field_statements = field_models
        .iter()
        .map(write_field_statement)
        .collect::<Result<Vec<_>>>()?;
    let draft_field_values = field_models
        .iter()
        .map(draft_field_value)
        .collect::<Vec<_>>();
    let replace_field_statements = field_models
        .iter()
        .map(|model| replace_field_statement(model, &field_enum_ident))
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
    let prepare_submit_statements = field_models
        .iter()
        .map(prepare_submit_statement)
        .collect::<Result<Vec<_>>>()?;
    let current_validation_report_statements = field_models
        .iter()
        .map(current_validation_report_statement)
        .collect::<Result<Vec<_>>>()?;
    let required_validation_statements = if form_uses_custom_validation {
        Vec::new()
    } else {
        field_models
            .iter()
            .map(required_validation_statement)
            .collect::<Result<Vec<_>>>()?
    };
    let focus_error_statements = field_models
        .iter()
        .map(focus_error_statement)
        .collect::<Vec<_>>();
    let field_accessor_methods = field_models
        .iter()
        .map(field_accessor_methods)
        .collect::<Result<Vec<_>>>()?;
    let field_required_methods = field_models
        .iter()
        .map(field_required_methods)
        .collect::<Result<Vec<_>>>()?;
    let field_setter_methods = field_models
        .iter()
        .map(|model| field_setter_methods(model, &field_enum_ident))
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

        #input_vis struct #store_ident #store_generics {
            #(
                pub #field_idents: #store_field_types,
            )*
            meta: ::gpui_form::FormMeta,
            submit_runtime: ::gpui_form::SubmitRuntime,
            form_errors: ::std::vec::Vec<::gpui_form::FormError>,
            field_paths: ::std::vec::Vec<::gpui_form::FieldPath>,
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
                    submit_runtime: ::gpui_form::SubmitRuntime::default(),
                    form_errors: ::std::vec::Vec::new(),
                    field_paths: ::std::vec![
                        #(::gpui_form::macro_support::field_path(#field_names),)*
                    ],
                    #validation_init
                    #transform_init
                }
            }

            pub fn from_value_with_validation_context(
                value: #input_ident #ty_generics,
                validation_context: #validation_context_ty,
                window: &mut ::gpui_form::__private::gpui::Window,
                cx: &mut ::gpui_form::__private::gpui::Context<Self>,
            ) -> Self {
                let mut form = Self::from_value(value, window, cx);
                form.validation_context = validation_context;
                form
            }

            pub fn validation_context(&self) -> &#validation_context_ty {
                &self.validation_context
            }

            pub fn set_validation_context(
                &mut self,
                validation_context: #validation_context_ty,
                cx: &mut ::gpui_form::__private::gpui::Context<Self>,
            ) {
                self.validation_context = validation_context;
                cx.notify();
            }

            pub fn draft(&self) -> #input_ident #ty_generics {
                #input_ident {
                    #(
                        #field_idents: #draft_field_values,
                    )*
                }
            }

            pub fn replace_from_value(
                &mut self,
                value: #input_ident #ty_generics,
                cx: &mut ::gpui_form::__private::gpui::Context<Self>,
            ) {
                let #input_ident { #(#field_idents: #value_idents),* } = value;

                #(#replace_field_statements)*

                self.form_errors.clear();
                self.submit_runtime = ::gpui_form::SubmitRuntime::default();
                self.refresh_meta();
                cx.notify();
            }

            pub fn write_draft(
                &mut self,
                value: #input_ident #ty_generics,
                cause: ::gpui_form::FieldChangeCause,
                window: &mut ::gpui_form::__private::gpui::Window,
                cx: &mut ::gpui_form::__private::gpui::App,
            ) {
                let #input_ident { #(#field_idents: #value_idents),* } = value;

                #(#write_field_statements)*

                self.refresh_meta();
            }

            pub fn field_paths(&self) -> &[::gpui_form::FieldPath] {
                &self.field_paths
            }

            pub fn meta(&self) -> &::gpui_form::FormMeta {
                &self.meta
            }

            pub fn is_submitting(&self) -> bool {
                self.submit_runtime.is_submitting()
            }

            pub fn is_submitted(&self) -> bool {
                self.submit_runtime.submission_attempts() > 0 && !self.is_submitting()
            }

            pub fn can_attempt_submit(&self) -> bool {
                !self.is_submitting() && !self.meta.is_validating
            }

            pub fn form_errors(&self) -> &[::gpui_form::FormError] {
                &self.form_errors
            }

            #(#field_accessor_methods)*

            #(#field_required_methods)*

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
                let field_meta = [
                    #(#field_meta_values,)*
                ];
                self.meta = ::gpui_form::FormMeta::aggregate(field_meta)
                    .with_submit_runtime(&self.submit_runtime);
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
                cx: &::gpui_form::__private::gpui::App,
            ) -> ::gpui_form::FormValidationReport {
                #validate_method_body
                let __gpui_form_required_trigger = trigger;
                #(#required_validation_statements)*
                report
            }

            fn apply_validation_for_scope(
                &mut self,
                trigger: ::gpui_form::ValidationTrigger,
                scope: ::gpui_form::ValidationScope,
                cx: &mut ::gpui_form::__private::gpui::App,
            ) -> ::gpui_form::FormValidationReport {
                let report = self.validation_report_for_scope(trigger, scope.clone(), cx);
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

            fn prepare_submit_report(
                &mut self,
                cx: &mut ::gpui_form::__private::gpui::App,
            ) -> ::gpui_form::FormValidationReport {
                let mut report = ::gpui_form::FormValidationReport::empty();
                #(#prepare_submit_statements)*
                self.refresh_meta();
                report
            }

            fn prepare_submit_output(
                &mut self,
                window: &mut ::gpui_form::__private::gpui::Window,
                cx: &mut ::gpui_form::__private::gpui::App,
            ) -> Result<#input_ident #ty_generics, ::gpui_form::FormValidationReport> {
                let preflight_report = self.prepare_submit_report(cx);
                if !preflight_report.is_valid() {
                    return Err(preflight_report);
                }
                #submit_transform
                self.write_draft(
                    normalized.clone(),
                    ::gpui_form::FieldChangeCause::NormalizeOnSubmit,
                    window,
                    cx,
                );
                #submit_validation
                let scope = ::gpui_form::ValidationScope::Form;
                let validation_input = normalized.clone();
                let __gpui_form_required_trigger = ::gpui_form::ValidationTrigger::Submit;
                #(#required_validation_statements)*
                self.apply_validation_report(&report, &::gpui_form::ValidationScope::Form, cx);
                self.refresh_meta();
                let final_report = self.current_validation_report(cx);
                if final_report.is_valid() {
                    Ok(normalized)
                } else {
                    Err(final_report)
                }
            }

            fn current_validation_report(
                &self,
                cx: &::gpui_form::__private::gpui::App,
            ) -> ::gpui_form::FormValidationReport {
                let mut report = ::gpui_form::FormValidationReport::new(
                    ::std::vec::Vec::new(),
                    self.form_errors.clone(),
                );
                #(#current_validation_report_statements)*
                report
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

            fn replace_from_value(
                &mut self,
                value: #input_ident #ty_generics,
                cx: &mut ::gpui_form::__private::gpui::Context<Self>,
            ) {
                Self::replace_from_value(self, value, cx);
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

            fn prepare_submit(
                &mut self,
                cx: &mut ::gpui_form::__private::gpui::App,
            ) -> ::gpui_form::FormValidationReport {
                Self::prepare_submit_report(self, cx)
            }

            fn current_validation_report(
                &self,
                cx: &::gpui_form::__private::gpui::App,
            ) -> ::gpui_form::FormValidationReport {
                Self::current_validation_report(self, cx)
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

            fn is_submitting(&self) -> bool {
                self.submit_runtime.is_submitting()
            }

            fn reset(
                &mut self,
                window: &mut ::gpui_form::__private::gpui::Window,
                cx: &mut ::gpui_form::__private::gpui::Context<Self>,
            ) {
                #(#reset_field_statements)*
                self.form_errors.clear();
                self.submit_runtime = ::gpui_form::SubmitRuntime::default();
                self.refresh_meta();
                cx.notify();
            }

            fn validate(
                &mut self,
                trigger: ::gpui_form::ValidationTrigger,
                _window: &mut ::gpui_form::__private::gpui::Window,
                cx: &mut ::gpui_form::__private::gpui::App,
            ) -> ::gpui_form::FormValidationReport {
                self.apply_validation_for_scope(trigger, ::gpui_form::ValidationScope::Form, cx)
            }

            fn prepare_submit(
                &mut self,
                window: &mut ::gpui_form::__private::gpui::Window,
                cx: &mut ::gpui_form::__private::gpui::Context<'_, Self>,
            ) -> Result<Self::Output, ::gpui_form::FormValidationReport> {
                self.submit_runtime.begin_submit();
                let result = self.prepare_submit_output(window, cx);
                if result.is_ok() {
                    self.submit_runtime.finish_success();
                } else {
                    self.submit_runtime.finish_failure();
                }
                self.refresh_meta();
                cx.notify();
                result
            }

            fn submit_sync<H, Success, Error>(
                &mut self,
                handler: H,
                window: &mut ::gpui_form::__private::gpui::Window,
                cx: &mut ::gpui_form::__private::gpui::Context<'_, Self>,
            ) -> Result<Success, ::gpui_form::SubmitError<Error>>
            where
                H: FnOnce(
                    Self::Output,
                    &mut ::gpui_form::__private::gpui::Window,
                    &mut ::gpui_form::__private::gpui::App,
                ) -> Result<Success, Error>,
            {
                if self.submit_runtime.is_submitting() {
                    return Err(::gpui_form::SubmitError::Busy);
                }
                self.submit_runtime.begin_submit();
                let output = match self.prepare_submit_output(window, cx) {
                    Ok(output) => output,
                    Err(report) => {
                        self.submit_runtime.finish_failure();
                        self.refresh_meta();
                        cx.notify();
                        return Err(::gpui_form::SubmitError::Invalid(report));
                    }
                };
                let result = handler(output, window, cx);
                if result.is_ok() {
                    self.submit_runtime.finish_success();
                } else {
                    self.submit_runtime.finish_failure();
                }
                self.refresh_meta();
                cx.notify();
                result.map_err(::gpui_form::SubmitError::Handler)
            }

            fn submit_async<H, Success, TaskError, StartError>(
                &mut self,
                handler: H,
                window: &mut ::gpui_form::__private::gpui::Window,
                cx: &mut ::gpui_form::__private::gpui::Context<'_, Self>,
            ) -> Result<(), ::gpui_form::SubmitError<StartError>>
            where
                Success: 'static,
                TaskError: 'static,
                H: FnOnce(
                    Self::Output,
                    &mut ::gpui_form::__private::gpui::Window,
                    &mut ::gpui_form::__private::gpui::App,
                ) -> Result<
                    ::gpui_form::__private::gpui::Task<Result<Success, TaskError>>,
                    StartError,
                >,
            {
                if self.submit_runtime.is_submitting() {
                    return Err(::gpui_form::SubmitError::Busy);
                }
                self.submit_runtime.begin_submit();
                let output = match self.prepare_submit_output(window, cx) {
                    Ok(output) => output,
                    Err(report) => {
                        self.submit_runtime.finish_failure();
                        self.refresh_meta();
                        cx.notify();
                        return Err(::gpui_form::SubmitError::Invalid(report));
                    }
                };
                let handler_task = match handler(output, window, cx) {
                    Ok(task) => task,
                    Err(error) => {
                        self.submit_runtime.finish_failure();
                        self.refresh_meta();
                        cx.notify();
                        return Err(::gpui_form::SubmitError::Handler(error));
                    }
                };
                let task = cx.spawn(async move |this, cx| {
                    let result = handler_task.await;
                    let _ = this.update(cx, |this, cx| {
                        if result.is_ok() {
                            this.submit_runtime.finish_success();
                        } else {
                            this.submit_runtime.finish_failure();
                        }
                        this.refresh_meta();
                        cx.notify();
                    });
                });
                self.submit_runtime.set_task(task);
                self.refresh_meta();
                cx.notify();
                Ok(())
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

        impl #impl_generics ::gpui_form::__private::gpui::EventEmitter<::gpui_form::FormDraftEvent>
            for #store_ident #ty_generics
            #where_clause
        {
        }

        impl #impl_generics ::gpui_form::__private::gpui::EventEmitter<
            ::gpui_form::FormStoreEvent<#field_enum_ident>,
        > for #store_ident #ty_generics
            #where_clause
        {
        }
    };

    Ok(expanded)
}

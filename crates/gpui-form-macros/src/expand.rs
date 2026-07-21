use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, GenericParam, Result, Type, parse2};

use crate::attributes::{
    FieldAttributes, FieldShape, FormAttributes, TransformAdapterKind, ValidationAdapterKind,
};

struct FieldModel<'a> {
    ident: &'a syn::Ident,
    ty: &'a Type,
    name: String,
    variant: syn::Ident,
    attrs: FieldAttributes,
}

pub(crate) fn derive_form_store(input: TokenStream) -> Result<TokenStream> {
    let input = parse2::<DeriveInput>(input)?;
    let model_ident = &input.ident;
    let visibility = &input.vis;
    let attrs = FormAttributes::parse(&input.attrs)?;
    let store_ident = attrs
        .store
        .clone()
        .unwrap_or_else(|| format_ident!("{}FormStore", model_ident));
    let field_ident = format_ident!("{}Field", model_ident);

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
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

    let fields = fields
        .iter()
        .map(|field| {
            let ident = field.ident.as_ref().expect("named field");
            let name = ident.to_string().trim_start_matches("r#").to_string();
            Ok(FieldModel {
                ident,
                ty: &field.ty,
                variant: variant_ident(&name),
                name,
                attrs: FieldAttributes::parse(&field.attrs)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    for field in &fields {
        if matches!(field.attrs.shape, FieldShape::Array { .. }) {
            vec_inner(field.ty).ok_or_else(|| {
                syn::Error::new_spanned(field.ty, "#[form(array(...))] requires a Vec<T> field")
            })?;
        }
    }

    let mut generics = input.generics.clone();
    for parameter in &mut generics.params {
        if let GenericParam::Type(parameter) = parameter {
            parameter.default = None;
        }
    }
    let where_clause = generics.make_where_clause();
    for field in &fields {
        let ty = field.ty;
        where_clause
            .predicates
            .push(syn::parse_quote!(#ty: Clone + PartialEq + 'static));
        if field.attrs.required {
            where_clause
                .predicates
                .push(syn::parse_quote!(#ty: ::gpui_form::typed::RequiredValue));
        }
        match &field.attrs.shape {
            FieldShape::Group => where_clause
                .predicates
                .push(syn::parse_quote!(#ty: ::gpui_form::typed::StructuralValidate)),
            FieldShape::Array { .. } => {
                let item = vec_inner(ty).expect("identified array type was validated");
                where_clause
                    .predicates
                    .push(syn::parse_quote!(#item: ::gpui_form::typed::StructuralValidate));
            }
            FieldShape::Value => {}
        }
    }

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let model_ty = quote!(#model_ident #ty_generics);
    let (validation_context_ty, validation_adapter_ty) =
        validation_parts(&attrs.validation, &model_ty);
    let transform_ty = transform_parts(&attrs.transform, &model_ty);
    let default_constructor = matches!(attrs.validation, ValidationAdapterKind::None).then(|| {
        quote! {
            pub fn from_value(
                value: #model_ty,
                cx: &mut ::gpui_form::__private::gpui::Context<Self>,
            ) -> Self {
                <Self as ::gpui_form::typed::FormStore>::from_value(value, cx)
            }
        }
    });

    let variants = fields
        .iter()
        .map(|field| &field.variant)
        .collect::<Vec<_>>();
    let names = fields
        .iter()
        .map(|field| field.name.as_str())
        .collect::<Vec<_>>();
    let required = fields
        .iter()
        .map(|field| field.attrs.required)
        .collect::<Vec<_>>();
    let trigger_values = fields
        .iter()
        .map(|field| trigger_tokens(field.attrs.triggers))
        .collect::<Vec<_>>();
    let schema_consts = fields
        .iter()
        .map(|field| format_ident!("{}_SCHEMA", field.name.to_uppercase()))
        .collect::<Vec<_>>();

    let accessors = fields
        .iter()
        .map(|field| field_accessor(&field_ident, &model_ty, field))
        .collect::<Vec<_>>();
    let array_accessors = fields
        .iter()
        .filter_map(|field| array_item_accessor(&model_ty, field))
        .collect::<Result<Vec<_>>>()?;
    let structural_required_checks = fields.iter().filter(|field| field.attrs.required).map(|field| {
        let ident = field.ident;
        let variant = &field.variant;
        let name = field.name.as_str();
        quote! {
            let path = base.join_field(#name);
            if scope.includes(Some(&path))
                && (trigger == ::gpui_form::typed::ValidationTrigger::Submit
                    || <#field_ident as ::gpui_form::typed::FormFieldId>::schema(#field_ident::#variant)
                        .triggers()
                        .includes(trigger))
                && ::gpui_form::typed::RequiredValue::is_missing(&self.#ident)
            {
                issues.push(::gpui_form::typed::required_issue(path, trigger));
            }
        }
    });
    let structural_statements = fields
        .iter()
        .filter_map(structural_validation_statement)
        .collect::<Vec<_>>();
    let mapper_arms = fields
        .iter()
        .map(garde_mapper_statement)
        .collect::<Result<Vec<_>>>()?;

    Ok(quote! {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #visibility enum #field_ident {
            #(#variants,)*
        }

        impl #field_ident {
            #(pub const #schema_consts: ::gpui_form::typed::FieldSchema =
                ::gpui_form::typed::FieldSchema::new(
                    #names,
                    #required,
                    #trigger_values,
                );)*

            pub const ALL: &'static [Self] = &[#(Self::#variants,)*];

            pub const fn key(self) -> &'static str {
                match self {
                    #(Self::#variants => #names,)*
                }
            }
        }

        impl ::gpui_form::typed::FormFieldId for #field_ident {
            fn path(self) -> ::gpui_form::typed::FieldPath {
                ::gpui_form::typed::FieldPath::field(self.key())
            }

            fn schema(self) -> &'static ::gpui_form::typed::FieldSchema {
                match self {
                    #(Self::#variants => &Self::#schema_consts,)*
                }
            }
        }

        #visibility struct #store_ident #generics #where_clause {
            runtime: ::gpui_form::__private::FormRuntime<#model_ty, #validation_context_ty>,
        }

        impl #impl_generics #store_ident #ty_generics #where_clause {
            #default_constructor

            pub fn from_value_with_validation_context(
                value: #model_ty,
                validation_context: #validation_context_ty,
                cx: &mut ::gpui_form::__private::gpui::Context<Self>,
            ) -> Self {
                <Self as ::gpui_form::typed::FormStore>::from_value_with_validation_context(
                    value,
                    validation_context,
                    cx,
                )
            }

            pub fn validation_context(&self) -> &#validation_context_ty {
                <Self as ::gpui_form::typed::FormStore>::validation_context(self)
            }

            pub fn set_validation_context(
                &mut self,
                validation_context: #validation_context_ty,
                cx: &mut ::gpui_form::__private::gpui::Context<Self>,
            ) {
                <Self as ::gpui_form::typed::FormStore>::set_validation_context(
                    self, validation_context, cx,
                );
            }

            #(#accessors)*
            #(#array_accessors)*

            fn __validate_snapshot(
                &mut self,
                snapshot: &#model_ty,
                trigger: ::gpui_form::typed::ValidationTrigger,
                scope: ::gpui_form::typed::ValidationScope,
                cx: &mut ::gpui_form::__private::gpui::Context<Self>,
            ) {
                let mut generated_issues = ::std::vec::Vec::new();
                ::gpui_form::typed::StructuralValidate::structural_issues(
                    snapshot,
                    &::gpui_form::typed::FieldPath::root(),
                    trigger,
                    &scope,
                    &mut generated_issues,
                );
                let adapter_report = ::gpui_form::typed::ValidationAdapter::validate(
                    &<#validation_adapter_ty as Default>::default(),
                    snapshot,
                    trigger,
                    scope.clone(),
                    ::gpui_form::typed::ValidationContext {
                        external: self.runtime.validation_context(),
                    },
                    cx,
                );
                let adapter_issues = adapter_report
                    .into_issues()
                    .into_iter()
                    .filter(|issue| {
                        issue.path.as_ref().is_none_or(|path| {
                            #field_ident::ALL.iter().any(|field| {
                                path.starts_with(&::gpui_form::typed::FormFieldId::path(*field))
                                    && ::gpui_form::typed::FormFieldId::schema(*field)
                                        .triggers()
                                        .includes(trigger)
                            })
                        })
                    })
                    .collect();
                self.runtime
                    .validation_mut()
                    .replace_generated(&scope, generated_issues);
                self.runtime
                    .validation_mut()
                    .replace_adapter(adapter_issues);
                cx.emit(::gpui_form::typed::FormEvent::RuntimeChanged);
                cx.notify();
            }
        }

        impl #impl_generics ::gpui_form::__private::gpui::EventEmitter<
            ::gpui_form::typed::FormEvent<#field_ident>
        > for #store_ident #ty_generics #where_clause {}

        impl #impl_generics ::gpui_form::typed::FormStore for #store_ident #ty_generics #where_clause {
            type Model = #model_ty;
            type Output = <#transform_ty as ::gpui_form::typed::SubmitTransform<#model_ty>>::Output;
            type Field = #field_ident;
            type ValidationContext = #validation_context_ty;
            type ValidationAdapter = #validation_adapter_ty;
            type SubmitTransform = #transform_ty;

            fn from_value_with_validation_context(
                value: Self::Model,
                validation_context: Self::ValidationContext,
                cx: &mut ::gpui_form::__private::gpui::Context<Self>,
            ) -> Self {
                let mut form = Self {
                    runtime: ::gpui_form::__private::FormRuntime::new(value, validation_context),
                };
                <Self as ::gpui_form::typed::FormStore>::validate(
                    &mut form,
                    ::gpui_form::typed::ValidationTrigger::Mount,
                    ::gpui_form::typed::ValidationScope::Form,
                    cx,
                );
                form
            }

            fn __runtime(&self) -> &::gpui_form::__private::FormRuntime<Self::Model, Self::ValidationContext> {
                &self.runtime
            }

            fn __runtime_mut(&mut self) -> &mut ::gpui_form::__private::FormRuntime<Self::Model, Self::ValidationContext> {
                &mut self.runtime
            }

            fn validate(
                &mut self,
                trigger: ::gpui_form::typed::ValidationTrigger,
                scope: ::gpui_form::typed::ValidationScope,
                cx: &mut ::gpui_form::__private::gpui::Context<Self>,
            ) {
                let snapshot = self.runtime.value().clone();
                self.__validate_snapshot(&snapshot, trigger, scope, cx);
            }

            fn prepare_submit(
                &mut self,
                cx: &mut ::gpui_form::__private::gpui::Context<Self>,
            ) -> Result<Self::Output, ::gpui_form::typed::SubmitError> {
                let snapshot = self.runtime.value().clone();
                self.__validate_snapshot(
                    &snapshot,
                    ::gpui_form::typed::ValidationTrigger::Submit,
                    ::gpui_form::typed::ValidationScope::Form,
                    cx,
                );
                let report = self.runtime.validation().report();
                if !report.is_valid() {
                    return Err(::gpui_form::typed::SubmitError::Validation(report));
                }
                if self.runtime.validation().is_validating() {
                    return Err(::gpui_form::typed::SubmitError::ValidationPending);
                }
                ::gpui_form::typed::SubmitTransform::transform(
                    &<#transform_ty as Default>::default(),
                    &snapshot,
                ).map_err(::gpui_form::typed::SubmitError::Transform)
            }
        }

        impl #impl_generics ::gpui_form::typed::StructuralValidate for #model_ident #ty_generics #where_clause {
            fn structural_issues(
                &self,
                base: &::gpui_form::typed::FieldPath,
                trigger: ::gpui_form::typed::ValidationTrigger,
                scope: &::gpui_form::typed::ValidationScope,
                issues: &mut ::std::vec::Vec<::gpui_form::typed::ValidationIssue>,
            ) {
                #(#structural_required_checks)*
                #(#structural_statements)*
            }
        }

        impl #impl_generics ::gpui_form::typed::GardePathMapper for #model_ident #ty_generics #where_clause {
            fn map_garde_path(
                &self,
                path: &str,
            ) -> Result<::gpui_form::typed::FieldPath, ::gpui_form::typed::GardePathError> {
                if path.is_empty() {
                    return Ok(::gpui_form::typed::FieldPath::root());
                }
                #(#mapper_arms)*
                Err(::gpui_form::typed::GardePathError::UnknownField {
                    path: path.to_owned(),
                })
            }
        }
    })
}

fn variant_ident(name: &str) -> syn::Ident {
    let mut result = String::new();
    let mut uppercase = true;
    for character in name.chars() {
        if character == '_' || character == '-' {
            uppercase = true;
        } else if uppercase {
            result.extend(character.to_uppercase());
            uppercase = false;
        } else {
            result.push(character);
        }
    }
    format_ident!("{result}")
}

fn trigger_tokens(triggers: crate::attributes::ValidationTriggers) -> TokenStream {
    let mount = triggers.mount;
    let change = triggers.change;
    let blur = triggers.blur;
    let dynamic = triggers.dynamic;
    let submit = triggers.submit;
    quote!(::gpui_form::typed::ValidationTriggers {
        mount: #mount,
        change: #change,
        blur: #blur,
        dynamic: #dynamic,
        submit: #submit,
    })
}

fn validation_parts(
    validation: &ValidationAdapterKind,
    model: &TokenStream,
) -> (TokenStream, TokenStream) {
    match validation {
        ValidationAdapterKind::None => (
            quote!(::gpui_form::typed::NoValidationContext),
            quote!(::gpui_form::typed::NoopValidationAdapter),
        ),
        ValidationAdapterKind::Garde { i18n } => {
            let provider = i18n
                .as_ref()
                .map(|provider| quote!(#provider))
                .unwrap_or_else(|| quote!(::gpui_form::typed::DefaultGardeI18nProvider));
            (
                quote!(<#model as ::garde::Validate>::Context),
                quote!(::gpui_form::typed::GardeAdapter<#model, #provider>),
            )
        }
        ValidationAdapterKind::Custom { adapter, context } => {
            let context = context
                .as_ref()
                .map(|context| quote!(#context))
                .unwrap_or_else(
                    || quote!(<#adapter as ::gpui_form::typed::ValidationAdapter<#model>>::Context),
                );
            (context, quote!(#adapter))
        }
    }
}

fn transform_parts(transform: &TransformAdapterKind, model: &TokenStream) -> TokenStream {
    match transform {
        TransformAdapterKind::Identity => quote!(::gpui_form::typed::IdentityTransform<#model>),
        TransformAdapterKind::Validify => quote!(::gpui_form::typed::ValidifyTransform<#model>),
        TransformAdapterKind::Custom { adapter } => quote!(#adapter),
    }
}

fn field_accessor(
    field_enum: &syn::Ident,
    model: &TokenStream,
    field: &FieldModel<'_>,
) -> TokenStream {
    let ident = field.ident;
    let ty = field.ty;
    let variant = &field.variant;
    let name = field.name.as_str();
    let method = format_ident!("{}_field", field.name);
    let nested_method = format_ident!("{}_in", field.name);
    let read = format_ident!("__read_{}", field.name);
    let write = format_ident!("__write_{}", field.name);
    let validate_change = field.attrs.triggers.change;
    quote! {
        pub fn #method(
            form: &::gpui_form::__private::gpui::Entity<Self>,
        ) -> ::gpui_form::typed::FormField<Self, #ty> {
            ::gpui_form::typed::FormField::new(
                form.downgrade(),
                #field_enum::#variant,
                ::gpui_form::typed::FieldPath::field(#name),
                Self::#read,
                Self::#write,
            )
        }

        pub fn #nested_method<ParentForm>(
            parent: ::gpui_form::typed::FormField<ParentForm, #model>,
        ) -> ::gpui_form::typed::FormField<ParentForm, #ty>
        where
            ParentForm: ::gpui_form::typed::FormStore,
        {
            parent.project(
                #name,
                |model| &model.#ident,
                |model, value| model.#ident = value,
            )
        }

        fn #read(&self) -> &#ty {
            &self.runtime.value().#ident
        }

        fn #write(
            &mut self,
            value: #ty,
            cx: &mut ::gpui_form::__private::gpui::Context<Self>,
        ) -> Result<bool, ::gpui_form::typed::FormFieldError> {
            if self.runtime.value().#ident == value {
                return Ok(false);
            }
            let mut next = self.runtime.value().clone();
            next.#ident = value;
            let revision = self.runtime.commit_field_value(next)
                .expect("unequal field value must advance form revision");
            self.runtime.validation_mut().invalidate_path(
                &::gpui_form::typed::FieldPath::field(#name),
            );
            if #validate_change {
                <Self as ::gpui_form::typed::FormStore>::validate(
                    self,
                    ::gpui_form::typed::ValidationTrigger::Change,
                    ::gpui_form::typed::ValidationScope::Field(
                        ::gpui_form::typed::FieldPath::field(#name),
                    ),
                    cx,
                );
            }
            cx.emit(::gpui_form::typed::FormEvent::FieldChanged {
                field: #field_enum::#variant,
                path: ::gpui_form::typed::FieldPath::field(#name),
                revision,
            });
            cx.notify();
            Ok(true)
        }
    }
}

fn array_item_accessor(
    model_ty: &TokenStream,
    field: &FieldModel<'_>,
) -> Option<Result<TokenStream>> {
    let FieldShape::Array { id } = &field.attrs.shape else {
        return None;
    };
    let item_ty = match vec_inner(field.ty) {
        Some(item) => item,
        None => {
            return Some(Err(syn::Error::new_spanned(
                field.ty,
                "identified array requires Vec<T>",
            )));
        }
    };
    let field_method = format_ident!("{}_field", field.name);
    let nested_field_method = format_ident!("{}_in", field.name);
    let item_method = format_ident!("{}_item", field.name);
    let nested_item_method = format_ident!("{}_item_in", field.name);
    Some(Ok(quote! {
        pub fn #item_method(
            form: &::gpui_form::__private::gpui::Entity<Self>,
            id: ::gpui_form::typed::FormItemId,
        ) -> ::gpui_form::typed::FormField<Self, #item_ty> {
            Self::#field_method(form).identified_item(id, |item| &item.#id)
        }

        pub fn #nested_item_method<ParentForm>(
            parent: ::gpui_form::typed::FormField<ParentForm, #model_ty>,
            id: ::gpui_form::typed::FormItemId,
        ) -> ::gpui_form::typed::FormField<ParentForm, #item_ty>
        where
            ParentForm: ::gpui_form::typed::FormStore,
        {
            Self::#nested_field_method(parent).identified_item(id, |item| &item.#id)
        }
    }))
}

fn structural_validation_statement(field: &FieldModel<'_>) -> Option<TokenStream> {
    let ident = field.ident;
    let name = field.name.as_str();
    match &field.attrs.shape {
        FieldShape::Value => None,
        FieldShape::Group => Some(quote! {
            let field_path = base.join_field(#name);
            if scope.includes(Some(&field_path)) {
                ::gpui_form::typed::StructuralValidate::structural_issues(
                    &self.#ident,
                    &field_path,
                    trigger,
                    scope,
                    issues,
                );
            }
        }),
        FieldShape::Array { id } => Some(quote! {
            let array_path = base.join_field(#name);
            if scope.includes(Some(&array_path)) {
                let mut id_counts = ::std::collections::BTreeMap::new();
                for (index, item) in self.#ident.iter().enumerate() {
                    match ::gpui_form::typed::ToFormItemId::to_form_item_id(&item.#id) {
                        Some(item_id) => {
                            *id_counts.entry(item_id).or_insert(0usize) += 1;
                        }
                        None => issues.push(
                            ::gpui_form::typed::ValidationIssue::field(
                                array_path.clone(),
                                trigger,
                                ::gpui_form::typed::ValidationSource::Internal,
                                "invalid_item_id",
                                ::gpui_form::typed::ValidationMessage::key(
                                    "gpui-form-error-internal",
                                ),
                            )
                            .with_param("path", array_path.to_string())
                            .with_param("reason", format!("array item {index} has no valid stable id")),
                        ),
                    }
                }
                for (item_id, count) in &id_counts {
                    if *count > 1 {
                        let item_path = array_path.join_item(*item_id);
                        issues.push(
                            ::gpui_form::typed::ValidationIssue::field(
                                item_path.clone(),
                                trigger,
                                ::gpui_form::typed::ValidationSource::Internal,
                                "duplicate_item_id",
                                ::gpui_form::typed::ValidationMessage::key(
                                    "gpui-form-error-internal",
                                ),
                            )
                            .with_param("path", item_path.to_string())
                            .with_param("reason", "array item stable id is duplicated"),
                        );
                    }
                }
                for item in &self.#ident {
                    let Some(item_id) =
                        ::gpui_form::typed::ToFormItemId::to_form_item_id(&item.#id)
                    else {
                        continue;
                    };
                    if id_counts.get(&item_id) != Some(&1usize) {
                        continue;
                    }
                    let item_path = array_path.join_item(item_id);
                    ::gpui_form::typed::StructuralValidate::structural_issues(
                        item,
                        &item_path,
                        trigger,
                        scope,
                        issues,
                    );
                }
            }
        }),
    }
}

fn garde_mapper_statement(field: &FieldModel<'_>) -> Result<TokenStream> {
    let ident = field.ident;
    let name = field.name.as_str();
    Ok(match &field.attrs.shape {
        FieldShape::Value => quote! {
            if path == #name {
                return Ok(::gpui_form::typed::FieldPath::field(#name));
            }
        },
        FieldShape::Group => quote! {
            if path == #name {
                return Ok(::gpui_form::typed::FieldPath::field(#name));
            }
            if let Some(child_path) = path.strip_prefix(concat!(#name, ".")) {
                let child = ::gpui_form::typed::GardePathMapper::map_garde_path(
                    &self.#ident,
                    child_path,
                )?;
                return Ok(::gpui_form::typed::FieldPath::field(#name).join_path(&child));
            }
        },
        FieldShape::Array { id } => {
            let item_ty = vec_inner(field.ty).expect("validated Vec");
            quote! {
                if let Some(index_and_suffix) = path.strip_prefix(concat!(#name, "[")) {
                    let Some(close) = index_and_suffix.find(']') else {
                        return Err(::gpui_form::typed::GardePathError::InvalidIndex {
                            path: path.to_owned(),
                            value: index_and_suffix.to_owned(),
                        });
                    };
                    let index_value = &index_and_suffix[..close];
                    let index = index_value.parse::<usize>().map_err(|_| {
                        ::gpui_form::typed::GardePathError::InvalidIndex {
                            path: path.to_owned(),
                            value: index_value.to_owned(),
                        }
                    })?;
                    let item: &#item_ty = self.#ident.get(index).ok_or_else(|| {
                        ::gpui_form::typed::GardePathError::IndexOutOfBounds {
                            path: path.to_owned(),
                            index,
                            len: self.#ident.len(),
                        }
                    })?;
                    let item_id = ::gpui_form::typed::ToFormItemId::to_form_item_id(&item.#id)
                        .ok_or_else(|| ::gpui_form::typed::GardePathError::InvalidItemId {
                            path: path.to_owned(),
                            index,
                        })?;
                    if self.#ident.iter().filter_map(|candidate| {
                        ::gpui_form::typed::ToFormItemId::to_form_item_id(&candidate.#id)
                    }).filter(|candidate| *candidate == item_id).count() != 1 {
                        return Err(::gpui_form::typed::GardePathError::DuplicateItemId {
                            path: path.to_owned(),
                            index,
                        });
                    }
                    let base = ::gpui_form::typed::FieldPath::field(#name).join_item(item_id);
                    let suffix = &index_and_suffix[close + 1..];
                    if suffix.is_empty() {
                        return Ok(base);
                    }
                    let Some(child_path) = suffix.strip_prefix('.') else {
                        return Err(::gpui_form::typed::GardePathError::UnknownField {
                            path: path.to_owned(),
                        });
                    };
                    let child = ::gpui_form::typed::GardePathMapper::map_garde_path(
                        item,
                        child_path,
                    )?;
                    return Ok(base.join_path(&child));
                }
            }
        }
    })
}

fn vec_inner(ty: &Type) -> Option<&Type> {
    let Type::Path(path) = ty else { return None };
    let segment = path.path.segments.last()?;
    if segment.ident != "Vec" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    arguments.args.iter().find_map(|argument| match argument {
        syn::GenericArgument::Type(ty) => Some(ty),
        _ => None,
    })
}

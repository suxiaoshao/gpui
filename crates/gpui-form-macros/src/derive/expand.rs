use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use std::collections::HashSet;

use syn::{Data, DeriveInput, Fields, GenericParam, Result, Type, parse2, spanned::Spanned as _};

use super::attributes::{
    FieldAttributes, FieldShape, FormAttributes, TransformAdapterKind, ValidationAdapterKind,
};

struct FieldModel<'a> {
    ident: &'a syn::Ident,
    ty: &'a Type,
    name: String,
    attrs: FieldAttributes,
}

pub(crate) fn derive_form_model(input: TokenStream) -> Result<TokenStream> {
    let input = parse2::<DeriveInput>(input)?;
    let model_ident = &input.ident;
    let visibility = &input.vis;
    let attrs = FormAttributes::parse(&input.attrs)?;
    let state_ident = attrs
        .state
        .clone()
        .unwrap_or_else(|| format_ident!("{}Form", model_ident));

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    &input.ident,
                    "FormModel can only be derived for structs with named fields",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "FormModel can only be derived for structs",
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
                name,
                attrs: FieldAttributes::parse(&field.attrs)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let generic_type_parameters = input
        .generics
        .type_params()
        .map(|parameter| parameter.ident.to_string())
        .collect::<HashSet<_>>();
    for field in &fields {
        if matches!(field.attrs.shape, FieldShape::Array { .. }) {
            let item = vec_inner(field.ty).ok_or_else(|| {
                syn::Error::new_spanned(field.ty, "#[form(array(...))] requires a Vec<T> field")
            })?;
            validate_array_item_type(item, &generic_type_parameters)?;
        }
    }

    let mut base_generics = input.generics.clone();
    for parameter in &mut base_generics.params {
        if let GenericParam::Type(parameter) = parameter {
            parameter.default = None;
        }
    }
    let mut structural_generics = base_generics.clone();
    let structural_where_clause = structural_generics.make_where_clause();
    for field in &fields {
        let span = field.ty.span();
        if field.attrs.required {
            let ty = field.ty;
            structural_where_clause
                .predicates
                .push(syn::parse_quote_spanned!(span=>
                    #ty: ::gpui_form::typed::RequiredValue
                ));
        }
        match &field.attrs.shape {
            FieldShape::Group => {
                let ty = field.ty;
                structural_where_clause.predicates.push(
                    syn::parse_quote_spanned!(span=> #ty: ::gpui_form::typed::StructuralValidate),
                );
            }
            FieldShape::Array { .. } => {
                let item = vec_inner(field.ty).expect("identified array type was validated");
                structural_where_clause.predicates.push(
                    syn::parse_quote_spanned!(span=> #item: ::gpui_form::typed::StructuralValidate),
                );
            }
            FieldShape::Value => {}
        }
    }

    let mut schema_generics = base_generics.clone();
    let schema_where_clause = schema_generics.make_where_clause();
    for field in &fields {
        let span = field.ty.span();
        match &field.attrs.shape {
            FieldShape::Group => {
                let ty = field.ty;
                schema_where_clause.predicates.push(
                    syn::parse_quote_spanned!(span=> #ty: ::gpui_form::typed::FormModelSchema),
                );
            }
            FieldShape::Array { .. } => {
                let item = vec_inner(field.ty).expect("identified array type was validated");
                schema_where_clause.predicates.push(
                    syn::parse_quote_spanned!(span=> #item: ::gpui_form::typed::FormModelSchema),
                );
            }
            FieldShape::Value => {}
        }
    }

    let mut mapper_generics = base_generics.clone();
    let mapper_where_clause = mapper_generics.make_where_clause();
    for field in &fields {
        let span = field.ty.span();
        match &field.attrs.shape {
            FieldShape::Group => {
                let ty = field.ty;
                mapper_where_clause.predicates.push(
                    syn::parse_quote_spanned!(span=> #ty: ::gpui_form::typed::GardePathMapper),
                );
            }
            FieldShape::Array { .. } => {
                let item = vec_inner(field.ty).expect("identified array type was validated");
                mapper_where_clause.predicates.push(
                    syn::parse_quote_spanned!(span=> #item: ::gpui_form::typed::GardePathMapper),
                );
            }
            FieldShape::Value => {}
        }
    }

    let mut store_generics = base_generics.clone();
    let store_where_clause = store_generics.make_where_clause();
    for field in &fields {
        let span = field.ty.span();
        let ty = field.ty;
        store_where_clause
            .predicates
            .push(syn::parse_quote_spanned!(span=>
                #ty: Clone + PartialEq + 'static
            ));
        if field.attrs.required {
            store_where_clause
                .predicates
                .push(syn::parse_quote_spanned!(span=>
                    #ty: ::gpui_form::typed::RequiredValue
                ));
        }
        match &field.attrs.shape {
            FieldShape::Group => {
                store_where_clause
                    .predicates
                    .push(syn::parse_quote_spanned!(span=>
                        #ty: ::gpui_form::typed::StructuralValidate
                            + ::gpui_form::typed::FormModelSchema
                    ));
            }
            FieldShape::Array { .. } => {
                let item = vec_inner(field.ty).expect("identified array type was validated");
                store_where_clause
                    .predicates
                    .push(syn::parse_quote_spanned!(span=>
                        #item: ::gpui_form::typed::StructuralValidate
                            + ::gpui_form::typed::FormModelSchema
                    ));
            }
            FieldShape::Value => {}
        }
    }
    if matches!(attrs.validation, ValidationAdapterKind::Garde { .. }) {
        for field in &fields {
            let span = field.ty.span();
            match &field.attrs.shape {
                FieldShape::Group => {
                    let ty = field.ty;
                    store_where_clause.predicates.push(
                        syn::parse_quote_spanned!(span=> #ty: ::gpui_form::typed::GardePathMapper),
                    );
                }
                FieldShape::Array { .. } => {
                    let item = vec_inner(field.ty).expect("identified array type was validated");
                    store_where_clause.predicates.push(
                        syn::parse_quote_spanned!(span=> #item: ::gpui_form::typed::GardePathMapper),
                    );
                }
                FieldShape::Value => {}
            }
        }
    }

    let (impl_generics, ty_generics, where_clause) = store_generics.split_for_impl();
    let (structural_impl_generics, structural_ty_generics, structural_where_clause) =
        structural_generics.split_for_impl();
    let (schema_impl_generics, schema_ty_generics, schema_where_clause) =
        schema_generics.split_for_impl();
    let (mapper_impl_generics, mapper_ty_generics, mapper_where_clause) =
        mapper_generics.split_for_impl();
    let model_ty = quote!(#model_ident #ty_generics);
    let state_ty = quote!(#state_ident #ty_generics);
    let (validation_context_ty, validation_adapter_ty) =
        validation_parts(&attrs.validation, &model_ty);
    let transform_ty = transform_parts(&attrs.transform, &model_ty);
    let default_constructor = match &attrs.validation {
        ValidationAdapterKind::Garde { .. }
        | ValidationAdapterKind::Custom {
            context: Some(_), ..
        } => quote! {},
        ValidationAdapterKind::None | ValidationAdapterKind::Custom { context: None, .. } => {
            quote! {
                pub fn from_value(
                    value: #model_ty,
                    cx: &mut ::gpui_form::__private::gpui::Context<Self>,
                ) -> Self {
                    <Self as ::gpui_form::typed::FormState>::from_value(value, cx)
                }
            }
        }
    };

    let descriptors = fields
        .iter()
        .map(|field| field_descriptor(&model_ty, field))
        .collect::<Result<Vec<_>>>()?;
    let structural_required_checks =
        fields
            .iter()
            .filter(|field| field.attrs.required)
            .map(|field| {
                let ident = field.ident;
                let descriptor = format_ident!("{}", field.name.to_uppercase());
                let name = field.name.as_str();
                quote! {
                    let path = base.join_field(#name);
                    if scope.includes(Some(&path))
                        && (trigger == ::gpui_form::typed::ValidationTrigger::Submit
                            || <#state_ty>::#descriptor.schema()
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
    let schema_arms = fields
        .iter()
        .map(|field| schema_resolver_statement(&state_ty, field))
        .collect::<Vec<_>>();

    Ok(quote! {
        #visibility struct #state_ident #store_generics #where_clause {
            runtime: ::gpui_form::__private::FormRuntime<#model_ty, #validation_context_ty>,
        }

        impl #impl_generics #state_ident #ty_generics #where_clause {
            #default_constructor

            pub fn from_value_with_validation_context(
                value: #model_ty,
                validation_context: #validation_context_ty,
                cx: &mut ::gpui_form::__private::gpui::Context<Self>,
            ) -> Self {
                <Self as ::gpui_form::typed::FormState>::from_value_with_validation_context(
                    value,
                    validation_context,
                    cx,
                )
            }

            pub fn validation_context(&self) -> &#validation_context_ty {
                <Self as ::gpui_form::typed::FormState>::validation_context(self)
            }

            pub fn set_validation_context(
                &mut self,
                validation_context: #validation_context_ty,
                cx: &mut ::gpui_form::__private::gpui::Context<Self>,
            ) {
                <Self as ::gpui_form::typed::FormState>::set_validation_context(
                    self, validation_context, cx,
                );
            }

            #(#descriptors)*
        }

        impl #impl_generics ::gpui_form::__private::gpui::EventEmitter<
            ::gpui_form::typed::FormEvent
        > for #state_ident #ty_generics #where_clause {}

        impl #impl_generics ::gpui_form::typed::FormState for #state_ident #ty_generics #where_clause {
            type Model = #model_ty;
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
                <Self as ::gpui_form::typed::FormState>::validate(
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

            fn __validation_snapshot(
                &self,
                snapshot: &Self::Model,
                trigger: ::gpui_form::typed::ValidationTrigger,
                scope: ::gpui_form::typed::ValidationScope,
                cx: &::gpui_form::__private::gpui::App,
            ) -> ::gpui_form::__private::ValidationSnapshot {
                let mut generated_issues = ::std::vec::Vec::new();
                ::gpui_form::typed::StructuralValidate::structural_issues(
                    snapshot,
                    &::gpui_form::typed::FieldPath::root(),
                    trigger,
                    &scope,
                    &mut generated_issues,
                );
                let adapter_report = <#validation_adapter_ty as ::gpui_form::typed::ValidationAdapter<#model_ty>>::validate(
                    snapshot,
                    trigger,
                    &scope,
                    self.runtime.validation_context(),
                    cx,
                );
                let adapter_issues = ::gpui_form::typed::normalize_adapter_report(
                    snapshot,
                    trigger,
                    &scope,
                    adapter_report,
                );
                ::gpui_form::__private::ValidationSnapshot::new(
                    scope,
                    generated_issues,
                    adapter_issues,
                )
            }
        }

        impl #structural_impl_generics ::gpui_form::typed::StructuralValidate for #model_ident #structural_ty_generics #structural_where_clause {
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

        impl #schema_impl_generics ::gpui_form::typed::FormModelSchema for #model_ident #schema_ty_generics #schema_where_clause {
            fn schema_at_path(
                &self,
                segments: &[::gpui_form::typed::FieldPathSegment],
            ) -> Result<
                ::gpui_form::typed::FieldSchema,
                ::gpui_form::typed::FormSchemaPathError,
            > {
                let Some((segment, remaining)) = segments.split_first() else {
                    return Err(::gpui_form::typed::FormSchemaPathError::EmptyPath);
                };
                match segment {
                    ::gpui_form::typed::FieldPathSegment::Field(name) => {
                        #(#schema_arms)*
                        Err(::gpui_form::typed::FormSchemaPathError::UnknownField)
                    }
                    ::gpui_form::typed::FieldPathSegment::Item(_) => {
                        Err(::gpui_form::typed::FormSchemaPathError::UnexpectedItem)
                    }
                    ::gpui_form::typed::FieldPathSegment::Projection(_) => {
                        Err(::gpui_form::typed::FormSchemaPathError::Projection)
                    }
                }
            }
        }

        impl #mapper_impl_generics ::gpui_form::typed::GardePathMapper for #model_ident #mapper_ty_generics #mapper_where_clause {
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

fn trigger_tokens(triggers: super::attributes::ValidationTriggers) -> TokenStream {
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
        ValidationAdapterKind::Garde { messages } => {
            let provider = messages
                .as_ref()
                .map(|provider| quote!(#provider))
                .unwrap_or_else(|| quote!(::gpui_form::typed::DefaultGardeMessageProvider));
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

fn field_descriptor(model: &TokenStream, field: &FieldModel<'_>) -> Result<TokenStream> {
    let ident = field.ident;
    let ty = field.ty;
    let name = field.name.as_str();
    let descriptor = format_ident!("{}", field.name.to_uppercase());
    let schema = trigger_tokens(field.attrs.triggers);
    let required = field.attrs.required;
    let (helper, item_id_at) = match &field.attrs.shape {
        FieldShape::Array { id } => {
            let item_ty = vec_inner(ty)
                .ok_or_else(|| syn::Error::new_spanned(ty, "identified array requires Vec<T>"))?;
            let helper = format_ident!("__{}_item_id_at", field.name);
            let to_item_id = quote_spanned!(id.span()=>
                ::gpui_form::typed::ToFormItemId::to_form_item_id(&item.#id)
            );
            (
                quote! {
                    #[allow(clippy::ptr_arg)]
                    fn #helper(items: &#ty, index: usize) -> Option<::gpui_form::typed::FormItemId> {
                        let item: &#item_ty = items.get(index)?;
                        #to_item_id
                    }
                },
                quote!(Some(Self::#helper)),
            )
        }
        FieldShape::Value | FieldShape::Group => (TokenStream::new(), quote!(None)),
    };
    Ok(quote! {
        #helper

        pub const #descriptor: ::gpui_form::typed::FormField<Self, #ty> =
            ::gpui_form::typed::FormField::__new(
                #name,
                ::gpui_form::typed::FieldSchema::new(#name, #required, #schema),
                |model: &#model| &model.#ident,
                |model: &mut #model, value: #ty| model.#ident = value,
                #item_id_at,
            );
    })
}

fn schema_resolver_statement(state: &TokenStream, field: &FieldModel<'_>) -> TokenStream {
    let ident = field.ident;
    let name = field.name.as_str();
    let descriptor = format_ident!("{}", field.name.to_uppercase());
    let schema = quote!(<#state>::#descriptor.schema());
    match &field.attrs.shape {
        FieldShape::Value => quote! {
            if name.as_ref() == #name {
                return if remaining.is_empty() {
                    Ok(#schema)
                } else {
                    match &remaining[0] {
                        ::gpui_form::typed::FieldPathSegment::Item(_) => {
                            Err(::gpui_form::typed::FormSchemaPathError::UnexpectedItem)
                        }
                        ::gpui_form::typed::FieldPathSegment::Projection(_) => {
                            Err(::gpui_form::typed::FormSchemaPathError::Projection)
                        }
                        ::gpui_form::typed::FieldPathSegment::Field(_) => {
                            Err(::gpui_form::typed::FormSchemaPathError::TrailingSegments)
                        }
                    }
                };
            }
        },
        FieldShape::Group => quote! {
            if name.as_ref() == #name {
                if remaining.is_empty() {
                    return Ok(#schema);
                }
                return ::gpui_form::typed::FormModelSchema::schema_at_path(
                    &self.#ident,
                    remaining,
                );
            }
        },
        FieldShape::Array { id } => {
            let to_item_id = quote_spanned!(id.span()=>
                ::gpui_form::typed::ToFormItemId::to_form_item_id(&item.#id)
            );
            quote! {
                if name.as_ref() == #name {
                    if remaining.is_empty() {
                        return Ok(#schema);
                    }
                    let (::gpui_form::typed::FieldPathSegment::Item(item_id), child_segments) =
                        (&remaining[0], &remaining[1..])
                    else {
                        return match &remaining[0] {
                            ::gpui_form::typed::FieldPathSegment::Projection(_) => {
                                Err(::gpui_form::typed::FormSchemaPathError::Projection)
                            }
                            _ => Err(::gpui_form::typed::FormSchemaPathError::UnexpectedItem),
                        };
                    };
                    let mut matches = self.#ident.iter().filter(|item| {
                        #to_item_id == Some(*item_id)
                    });
                    let Some(item) = matches.next() else {
                        return Err(::gpui_form::typed::FormSchemaPathError::MissingItem(*item_id));
                    };
                    if matches.next().is_some() {
                        return Err(::gpui_form::typed::FormSchemaPathError::DuplicateItem(*item_id));
                    }
                    if child_segments.is_empty() {
                        return Ok(#schema);
                    }
                    return ::gpui_form::typed::FormModelSchema::schema_at_path(
                        item,
                        child_segments,
                    );
                }
            }
        }
    }
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
        FieldShape::Array { id } => {
            let to_item_id = quote_spanned!(id.span()=>
                ::gpui_form::typed::ToFormItemId::to_form_item_id(&item.#id)
            );
            Some(quote! {
                let array_path = base.join_field(#name);
                if scope.includes(Some(&array_path)) {
                    let mut id_counts = ::std::collections::BTreeMap::new();
                    for (index, item) in self.#ident.iter().enumerate() {
                        match #to_item_id {
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
                            if scope.includes(Some(&item_path)) {
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
                    }
                    for item in &self.#ident {
                        let Some(item_id) = #to_item_id else {
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
            })
        }
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
            let item_to_id = quote_spanned!(id.span()=>
                ::gpui_form::typed::ToFormItemId::to_form_item_id(&item.#id)
            );
            let candidate_to_id = quote_spanned!(id.span()=>
                ::gpui_form::typed::ToFormItemId::to_form_item_id(&candidate.#id)
            );
            quote! {
                if path == #name {
                    return Ok(::gpui_form::typed::FieldPath::field(#name));
                }
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
                    let item_id = #item_to_id
                        .ok_or_else(|| ::gpui_form::typed::GardePathError::InvalidItemId {
                            path: path.to_owned(),
                            index,
                    })?;
                    if self.#ident.iter().filter_map(|candidate| {
                        #candidate_to_id
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

fn validate_array_item_type(item: &Type, generic_type_parameters: &HashSet<String>) -> Result<()> {
    let Type::Path(path) = item else {
        return Err(syn::Error::new_spanned(
            item,
            "identified array items must use a nominal type path with a named stable-id field",
        ));
    };
    if path.qself.is_some() {
        return Err(syn::Error::new_spanned(
            item,
            "identified array items cannot use an associated type; use a nominal item type with a named stable-id field",
        ));
    }
    let Some(first) = path.path.segments.first() else {
        return Err(syn::Error::new_spanned(
            item,
            "identified array items require a nominal item type",
        ));
    };
    if first.ident == "Self" || generic_type_parameters.contains(&first.ident.to_string()) {
        return Err(syn::Error::new_spanned(
            item,
            "identified array items cannot use a bare type parameter or associated type; use a nominal item type such as Row<T>",
        ));
    }
    Ok(())
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

#[cfg(test)]
mod expansion_tests {
    #[test]
    fn generated_tokens_parse_as_rust() {
        let input = quote::quote! {
            #[form(state = ExampleForm)]
            struct ExampleInput { value: String }
        };
        let output = super::derive_form_model(input).expect("derive succeeds");
        syn::parse2::<syn::File>(output).expect("generated tokens parse");
    }
}

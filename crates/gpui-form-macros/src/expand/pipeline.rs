use proc_macro2::TokenStream;
use quote::quote;

use crate::attributes::{TransformAdapterKind, ValidationAdapterKind};

pub(super) fn validation_field(
    kind: &ValidationAdapterKind,
    input_ident: &syn::Ident,
    ty_generics: &syn::TypeGenerics<'_>,
) -> TokenStream {
    let context_ty = validation_context_type(kind);
    match kind {
        ValidationAdapterKind::None => quote! {
            validation_context: #context_ty,
        },
        ValidationAdapterKind::Garde => quote! {
            validation: ::gpui_form::GardeAdapter<#input_ident #ty_generics>,
            validation_context: #context_ty,
        },
        ValidationAdapterKind::Custom { adapter, .. } => quote! {
            validation: #adapter,
            validation_context: #context_ty,
        },
    }
}

pub(super) fn validation_init(kind: &ValidationAdapterKind) -> TokenStream {
    let context_ty = validation_context_type(kind);
    match kind {
        ValidationAdapterKind::None => quote! {
            validation_context: <#context_ty as ::std::default::Default>::default(),
        },
        ValidationAdapterKind::Garde => quote! {
            validation: ::gpui_form::GardeAdapter::new(),
            validation_context: <#context_ty as ::std::default::Default>::default(),
        },
        ValidationAdapterKind::Custom { adapter, .. } => quote! {
            validation: <#adapter as ::std::default::Default>::default(),
            validation_context: <#context_ty as ::std::default::Default>::default(),
        },
    }
}

pub(super) fn validation_context_type(kind: &ValidationAdapterKind) -> TokenStream {
    match kind {
        ValidationAdapterKind::Custom {
            context: Some(context),
            ..
        } => quote!(#context),
        ValidationAdapterKind::None
        | ValidationAdapterKind::Garde
        | ValidationAdapterKind::Custom { .. } => {
            quote!(::gpui_form::NoValidationContext)
        }
    }
}

pub(super) fn transform_field(
    kind: &TransformAdapterKind,
    input_ident: &syn::Ident,
    ty_generics: &syn::TypeGenerics<'_>,
) -> TokenStream {
    match kind {
        TransformAdapterKind::Identity => quote!(),
        TransformAdapterKind::Validify => quote! {
            transform: ::gpui_form::ValidifyTransform<#input_ident #ty_generics>,
        },
        TransformAdapterKind::Custom { adapter } => quote! {
            transform: #adapter,
        },
    }
}

pub(super) fn transform_init(kind: &TransformAdapterKind) -> TokenStream {
    match kind {
        TransformAdapterKind::Identity => quote!(),
        TransformAdapterKind::Validify => quote! {
            transform: ::gpui_form::ValidifyTransform::new(),
        },
        TransformAdapterKind::Custom { adapter } => quote! {
            transform: <#adapter as ::std::default::Default>::default(),
        },
    }
}

pub(super) fn validate_method_body(
    validation: &ValidationAdapterKind,
    transform: &TransformAdapterKind,
    input_ident: &syn::Ident,
    ty_generics: &syn::TypeGenerics<'_>,
) -> TokenStream {
    let preview = match transform {
        TransformAdapterKind::Identity => quote! {
            let validation_input = self.draft();
        },
        TransformAdapterKind::Validify => quote! {
            let draft = self.draft();
            let validation_input =
                match <::gpui_form::ValidifyTransform<#input_ident #ty_generics> as ::gpui_form::SubmitTransform<
                    #input_ident #ty_generics,
                    #input_ident #ty_generics,
                >>::preview(
                    &self.transform,
                    &draft,
                    &::gpui_form::TransformContext { submitted: false },
                ) {
                    Ok(validation_input) => validation_input,
                    Err(report) => return report.into_form_report(),
                };
        },
        TransformAdapterKind::Custom { adapter } => quote! {
            let draft = self.draft();
            let validation_input =
                match <#adapter as ::gpui_form::SubmitTransform<
                    #input_ident #ty_generics,
                    #input_ident #ty_generics,
                >>::preview(
                    &self.transform,
                    &draft,
                    &::gpui_form::TransformContext { submitted: false },
                ) {
                    Ok(validation_input) => validation_input,
                    Err(report) => return report.into_form_report(),
                };
        },
    };

    let validate = match validation {
        ValidationAdapterKind::None => quote! {
            ::gpui_form::FormValidationReport::empty()
        },
        ValidationAdapterKind::Garde => quote! {
            <::gpui_form::GardeAdapter<#input_ident #ty_generics> as ::gpui_form::ValidationAdapter<
                #input_ident #ty_generics,
            >>::validate(
                    &self.validation,
                    &validation_input,
                    trigger,
                    scope.clone(),
                    ::gpui_form::ValidationContext {
                        submitted: false,
                        external: &self.validation_context,
                    },
                    cx,
                )
            .into_form_report()
        },
        ValidationAdapterKind::Custom { adapter, .. } => quote! {
            <#adapter as ::gpui_form::ValidationAdapter<
                #input_ident #ty_generics,
            >>::validate(
                    &self.validation,
                    &validation_input,
                    trigger,
                    scope.clone(),
                    ::gpui_form::ValidationContext {
                        submitted: false,
                        external: &self.validation_context,
                    },
                    cx,
                )
            .into_form_report()
        },
    };

    quote! {
        #preview
        let mut report = #validate;
    }
}

pub(super) fn submit_transform(
    kind: &TransformAdapterKind,
    input_ident: &syn::Ident,
    ty_generics: &syn::TypeGenerics<'_>,
) -> TokenStream {
    match kind {
        TransformAdapterKind::Identity => quote! {
            let normalized = self.draft();
        },
        TransformAdapterKind::Validify => quote! {
            let draft = self.draft();
            let normalized =
                <::gpui_form::ValidifyTransform<#input_ident #ty_generics> as ::gpui_form::SubmitTransform<
                    #input_ident #ty_generics,
                    #input_ident #ty_generics,
                >>::transform_on_submit(
                    &self.transform,
                    &draft,
                    &::gpui_form::TransformContext { submitted: true },
                )
                .map_err(|report| report.into_form_report())?;
        },
        TransformAdapterKind::Custom { adapter } => quote! {
            let draft = self.draft();
            let normalized =
                <#adapter as ::gpui_form::SubmitTransform<
                    #input_ident #ty_generics,
                    #input_ident #ty_generics,
                >>::transform_on_submit(
                    &self.transform,
                    &draft,
                    &::gpui_form::TransformContext { submitted: true },
                )
                .map_err(|report| report.into_form_report())?;
        },
    }
}

pub(super) fn submit_validation(
    kind: &ValidationAdapterKind,
    input_ident: &syn::Ident,
    ty_generics: &syn::TypeGenerics<'_>,
) -> TokenStream {
    match kind {
        ValidationAdapterKind::None => quote! {
            let mut report = ::gpui_form::FormValidationReport::empty();
        },
        ValidationAdapterKind::Garde => quote! {
            let mut report =
                <::gpui_form::GardeAdapter<#input_ident #ty_generics> as ::gpui_form::ValidationAdapter<
                    #input_ident #ty_generics,
                >>::validate(
                    &self.validation,
                    &normalized,
                    ::gpui_form::ValidationTrigger::Submit,
                    ::gpui_form::ValidationScope::Form,
                    ::gpui_form::ValidationContext {
                        submitted: true,
                        external: &self.validation_context,
                    },
                    cx,
                )
                .into_form_report();
        },
        ValidationAdapterKind::Custom { adapter, .. } => quote! {
            let mut report =
                <#adapter as ::gpui_form::ValidationAdapter<
                    #input_ident #ty_generics,
                >>::validate(
                    &self.validation,
                    &normalized,
                    ::gpui_form::ValidationTrigger::Submit,
                    ::gpui_form::ValidationScope::Form,
                    ::gpui_form::ValidationContext {
                        submitted: true,
                        external: &self.validation_context,
                    },
                    cx,
                )
                .into_form_report();
        },
    }
}

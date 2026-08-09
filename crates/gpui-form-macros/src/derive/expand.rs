use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, Ident, parse2};

use super::model::{
    ChildKind, DeriveModel, ModelKind, SchemaField, SchemaFieldKind, SchemaVariant, VariantKind,
    screaming_snake, snake_case,
};

mod definition;
mod driver;
mod validation;

pub(crate) fn expand(input: TokenStream) -> syn::Result<TokenStream> {
    let input: DeriveInput = parse2(input)?;
    let model = DeriveModel::parse(input)?;
    match model.kind {
        ModelKind::Struct { fields } => expand_struct(&model.ident, &fields),
        ModelKind::Enum { variants } => expand_enum(&model.ident, &variants),
    }
}

fn expand_struct(name: &Ident, fields: &[SchemaField]) -> syn::Result<TokenStream> {
    let mut functions = Vec::new();
    let mut constants = Vec::new();
    let mut visits = Vec::new();

    for field in fields {
        let field_name = &field.ident;
        let field_type = &field.ty;
        let constant = format_ident!("{}", screaming_snake(&field_name.to_string()));
        let read = format_ident!("__form_read_{}", field_name);
        let read_mut = format_ident!("__form_read_{}_mut", field_name);
        let literal = field_name.to_string();
        functions.push(quote! {
            fn #read(value: &Self) -> &#field_type { &value.#field_name }
            fn #read_mut(value: &mut Self) -> &mut #field_type { &mut value.#field_name }
        });

        match &field.kind {
            SchemaFieldKind::Leaf => {
                let required = field.validation.required;
                let mount = field.validation.triggers.mount;
                let change = field.validation.triggers.change;
                let blur = field.validation.triggers.blur;
                let external = field.validation.triggers.external;
                let submit = field.validation.triggers.submit;
                let schema = validation::field_schema(
                    &literal, required, mount, change, blur, external, submit,
                );
                constants.push(quote! {
                    pub const #constant: ::gpui_form::FieldDef<Self, #field_type> =
                        ::gpui_form::FieldDef::__new(
                            #schema,
                            Self::#read,
                            Self::#read_mut,
                        );
                });
                let missing = if required {
                    quote!(::gpui_form::RequiredValue::is_missing(&self.#field_name))
                } else {
                    quote!(false)
                };
                visits.push(quote! {
                    visitor.field(Self::#constant.schema(), #missing);
                });
            }
            SchemaFieldKind::Child(child) => {
                constants.push(quote! {
                    pub const #constant: ::gpui_form::ChildDef<Self, #field_type> =
                        ::gpui_form::ChildDef::__new(#literal, Self::#read, Self::#read_mut);
                });
                match child {
                    ChildKind::Optional { inner } => visits.push(quote! {
                        visitor.optional(
                            #literal,
                            self.#field_name.is_some(),
                            &mut |visitor| {
                                if let Some(value) = self.#field_name.as_ref() {
                                    <#inner as ::gpui_form::FormSchema>::__visit(value, visitor);
                                }
                            },
                        );
                    }),
                    ChildKind::Direct => visits.push(quote! {
                        visitor.child(#literal, &mut |visitor| {
                            <#field_type as ::gpui_form::FormSchema>::__visit(
                                &self.#field_name,
                                visitor,
                            );
                        });
                    }),
                }
            }
            SchemaFieldKind::Items { item } => {
                constants.push(quote! {
                    pub const #constant: ::gpui_form::ItemsDef<Self, #item> =
                        ::gpui_form::ItemsDef::__new(#literal, Self::#read, Self::#read_mut);
                });
                visits.push(quote! {
                    visitor.items(
                        #literal,
                        self.#field_name.len(),
                        &mut |index, visitor| {
                            <#item as ::gpui_form::FormSchema>::__visit(
                                &self.#field_name[index],
                                visitor,
                            );
                        },
                    );
                });
            }
        }
    }

    let definition = definition::model_definition(name, &functions, &constants);
    let driver = driver::schema_driver(name, quote! { #(#visits)* });
    Ok(quote! { #definition #driver })
}

fn expand_enum(name: &Ident, variants: &[SchemaVariant]) -> syn::Result<TokenStream> {
    let mut functions = Vec::new();
    let mut constants = Vec::new();
    let mut arms = Vec::new();

    for variant in variants {
        let variant_name = &variant.ident;
        let literal = snake_case(&variant_name.to_string());
        match &variant.kind {
            VariantKind::Unit => {
                arms.push(quote! {
                    Self::#variant_name => visitor.unit_case(#literal),
                });
            }
            VariantKind::Payload(payload) => {
                let constant = format_ident!("{}", screaming_snake(&variant_name.to_string()));
                let read = format_ident!("__form_case_{}", literal);
                let read_mut = format_ident!("__form_case_{}_mut", literal);
                functions.push(quote! {
                    fn #read(value: &Self) -> Option<&#payload> {
                        match value {
                            Self::#variant_name(payload) => Some(payload),
                            _ => None,
                        }
                    }
                    fn #read_mut(value: &mut Self) -> Option<&mut #payload> {
                        match value {
                            Self::#variant_name(payload) => Some(payload),
                            _ => None,
                        }
                    }
                });
                constants.push(quote! {
                    pub const #constant: ::gpui_form::CaseDef<Self, #payload> =
                        ::gpui_form::CaseDef::__new(#literal, Self::#read, Self::#read_mut);
                });
                arms.push(quote! {
                    Self::#variant_name(payload) => visitor.case(#literal, &mut |visitor| {
                        <#payload as ::gpui_form::FormSchema>::__visit(payload, visitor);
                    }),
                });
            }
        }
    }

    let definition = definition::model_definition(name, &functions, &constants);
    let driver = driver::schema_driver(name, quote! { match self { #(#arms)* } });
    Ok(quote! { #definition #driver })
}

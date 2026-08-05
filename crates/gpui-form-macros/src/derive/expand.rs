use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DataEnum, DataStruct, DeriveInput, Fields, Ident, parse2};

use super::{
    attributes::{FieldKind, parse_field_options, reject_container_attributes},
    model::{screaming_snake, snake_case, type_argument},
};

mod definition;
mod driver;
mod validation;

pub(crate) fn expand(input: TokenStream) -> syn::Result<TokenStream> {
    let input: DeriveInput = parse2(input)?;
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "FormSchema does not support generic models",
        ));
    }
    reject_container_attributes(&input.attrs)?;
    match &input.data {
        Data::Struct(data) => expand_struct(&input.ident, data),
        Data::Enum(data) => expand_enum(&input.ident, data),
        Data::Union(data) => Err(syn::Error::new_spanned(
            data.union_token,
            "FormSchema cannot be derived for unions",
        )),
    }
}

fn expand_struct(name: &Ident, data: &DataStruct) -> syn::Result<TokenStream> {
    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new_spanned(
            &data.fields,
            "FormSchema structs must use named fields",
        ));
    };

    let mut functions = Vec::new();
    let mut constants = Vec::new();
    let mut visits = Vec::new();

    for field in &fields.named {
        let field_name = field.ident.as_ref().expect("named field");
        let field_type = &field.ty;
        let constant = format_ident!("{}", screaming_snake(&field_name.to_string()));
        let read = format_ident!("__form_read_{}", field_name);
        let read_mut = format_ident!("__form_read_{}_mut", field_name);
        let literal = field_name.to_string();
        let options = parse_field_options(&field.attrs)?;

        functions.push(quote! {
            fn #read(value: &Self) -> &#field_type { &value.#field_name }
            fn #read_mut(value: &mut Self) -> &mut #field_type { &mut value.#field_name }
        });

        match options.kind.unwrap_or(FieldKind::Leaf) {
            FieldKind::Leaf => {
                let required = options.required;
                let mount = options.mount;
                let change = options.change;
                let blur = options.blur;
                let dynamic = options.dynamic;
                let submit = options.submit;
                let schema = validation::field_schema(
                    &literal, required, mount, change, blur, dynamic, submit,
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
            FieldKind::Child => {
                constants.push(quote! {
                    pub const #constant: ::gpui_form::ChildDef<Self, #field_type> =
                        ::gpui_form::ChildDef::__new(#literal, Self::#read, Self::#read_mut);
                });
                if let Some(inner) = type_argument(field_type, "Option") {
                    visits.push(quote! {
                        visitor.optional(
                            #literal,
                            self.#field_name.is_some(),
                            &mut |visitor| {
                                if let Some(value) = self.#field_name.as_ref() {
                                    <#inner as ::gpui_form::FormSchema>::__visit(value, visitor);
                                }
                            },
                        );
                    });
                } else {
                    visits.push(quote! {
                        visitor.child(#literal, &mut |visitor| {
                            <#field_type as ::gpui_form::FormSchema>::__visit(
                                &self.#field_name,
                                visitor,
                            );
                        });
                    });
                }
            }
            FieldKind::Items => {
                let Some(item) = type_argument(field_type, "Vec") else {
                    return Err(syn::Error::new_spanned(
                        field_type,
                        "#[form(items)] requires Vec<Item>",
                    ));
                };
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

fn expand_enum(name: &Ident, data: &DataEnum) -> syn::Result<TokenStream> {
    let mut functions = Vec::new();
    let mut constants = Vec::new();
    let mut arms = Vec::new();

    for variant in &data.variants {
        if let Some(attribute) = variant
            .attrs
            .iter()
            .find(|attribute| attribute.path().is_ident("form"))
        {
            return Err(syn::Error::new_spanned(
                attribute,
                "FormSchema enum variants have no #[form(...)] options",
            ));
        }
        let variant_name = &variant.ident;
        let literal = snake_case(&variant_name.to_string());
        match &variant.fields {
            Fields::Unit => {
                arms.push(quote! {
                    Self::#variant_name => visitor.unit_case(#literal),
                });
            }
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                let payload = &fields.unnamed.first().expect("one field").ty;
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
            _ => {
                return Err(syn::Error::new_spanned(
                    &variant.fields,
                    "FormSchema enums support only unit and single-payload tuple variants",
                ));
            }
        }
    }

    let definition = definition::model_definition(name, &functions, &constants);
    let driver = driver::schema_driver(name, quote! { match self { #(#arms)* } });
    Ok(quote! { #definition #driver })
}

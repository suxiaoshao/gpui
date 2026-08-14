use syn::{
    Data, DataEnum, DataStruct, DeriveInput, Fields, GenericArgument, Ident, PathArguments, Type,
};

use super::attributes::{
    FieldKind, TriggerSelection, parse_field_options, reject_container_attributes,
};

pub(super) struct DeriveModel {
    pub(super) ident: Ident,
    pub(super) kind: ModelKind,
}

pub(super) enum ModelKind {
    Struct { fields: Vec<SchemaField> },
    Enum { variants: Vec<SchemaVariant> },
}

pub(super) struct SchemaField {
    pub(super) ident: Ident,
    pub(super) ty: Type,
    pub(super) kind: SchemaFieldKind,
    pub(super) validation: LeafValidation,
}

pub(super) enum SchemaFieldKind {
    Leaf,
    Child(ChildKind),
    Items { item: Box<Type> },
}

pub(super) enum ChildKind {
    Direct,
    Optional { inner: Box<Type> },
}

#[derive(Clone, Copy, Debug)]
pub(super) struct LeafValidation {
    pub(super) required: bool,
    pub(super) triggers: TriggerSelection,
}

pub(super) struct SchemaVariant {
    pub(super) ident: Ident,
    pub(super) kind: VariantKind,
}

pub(super) enum VariantKind {
    Unit,
    Payload(Box<Type>),
}

impl DeriveModel {
    pub(super) fn parse(input: DeriveInput) -> syn::Result<Self> {
        if !input.generics.params.is_empty() {
            return Err(syn::Error::new_spanned(
                &input.generics,
                "FormSchema does not support generic models",
            ));
        }
        reject_container_attributes(&input.attrs)?;

        let kind = match input.data {
            Data::Struct(data) => ModelKind::Struct {
                fields: parse_struct_fields(&data)?,
            },
            Data::Enum(data) => ModelKind::Enum {
                variants: parse_enum_variants(&data)?,
            },
            Data::Union(data) => {
                return Err(syn::Error::new_spanned(
                    data.union_token,
                    "FormSchema cannot be derived for unions",
                ));
            }
        };

        Ok(Self {
            ident: input.ident,
            kind,
        })
    }
}

fn parse_struct_fields(data: &DataStruct) -> syn::Result<Vec<SchemaField>> {
    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new_spanned(
            &data.fields,
            "FormSchema structs must use named fields",
        ));
    };

    fields
        .named
        .iter()
        .map(|field| {
            let options = parse_field_options(&field.attrs)?;
            if options.kind != FieldKind::Leaf {
                if let Some(span) = options.required_span {
                    return Err(syn::Error::new(
                        span,
                        "`required` is only supported on leaf fields",
                    ));
                }
                if let Some(span) = options.validation_span {
                    return Err(syn::Error::new(
                        span,
                        "`validate(...)` is only supported on leaf fields",
                    ));
                }
            }

            let kind = match options.kind {
                FieldKind::Leaf => SchemaFieldKind::Leaf,
                FieldKind::Child => match option_inner(&field.ty)? {
                    Some(inner) => SchemaFieldKind::Child(ChildKind::Optional {
                        inner: Box::new(inner.clone()),
                    }),
                    None => SchemaFieldKind::Child(ChildKind::Direct),
                },
                FieldKind::Items => SchemaFieldKind::Items {
                    item: Box::new(vec_item(&field.ty)?.clone()),
                },
            };

            let ident = field.ident.clone().expect("named field");
            Ok(SchemaField {
                ident,
                ty: field.ty.clone(),
                kind,
                validation: LeafValidation {
                    required: options.required,
                    triggers: options.triggers,
                },
            })
        })
        .collect()
}

fn parse_enum_variants(data: &DataEnum) -> syn::Result<Vec<SchemaVariant>> {
    data.variants
        .iter()
        .map(|variant| {
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

            let kind = match &variant.fields {
                Fields::Unit => VariantKind::Unit,
                Fields::Unnamed(fields) if fields.unnamed.len() == 1 => VariantKind::Payload(
                    Box::new(fields.unnamed.first().expect("one field").ty.clone()),
                ),
                _ => {
                    return Err(syn::Error::new_spanned(
                        &variant.fields,
                        "FormSchema enums support only unit and single-payload tuple variants",
                    ));
                }
            };

            Ok(SchemaVariant {
                ident: variant.ident.clone(),
                kind,
            })
        })
        .collect()
}

pub(super) fn option_inner(ty: &Type) -> syn::Result<Option<&Type>> {
    type_argument(
        ty,
        "Option",
        "#[form(child)] requires Child or Option<Child>",
    )
}

pub(super) fn vec_item(ty: &Type) -> syn::Result<&Type> {
    type_argument(ty, "Vec", "#[form(items)] requires Vec<Item>")?
        .ok_or_else(|| syn::Error::new_spanned(ty, "#[form(items)] requires Vec<Item>"))
}

fn type_argument<'a>(ty: &'a Type, expected: &str, error: &str) -> syn::Result<Option<&'a Type>> {
    let Type::Path(path) = ty else {
        return Ok(None);
    };
    let Some(segment) = path.path.segments.last() else {
        return Ok(None);
    };
    if segment.ident != expected {
        return Ok(None);
    }
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Err(syn::Error::new_spanned(ty, error));
    };
    if arguments.args.len() != 1 {
        return Err(syn::Error::new_spanned(&arguments.args, error));
    }
    match arguments.args.first().expect("one argument") {
        GenericArgument::Type(ty) => Ok(Some(ty)),
        argument => Err(syn::Error::new_spanned(argument, error)),
    }
}

pub(super) fn screaming_snake(value: &str) -> String {
    snake_case(value).to_ascii_uppercase()
}

pub(super) fn snake_case(value: &str) -> String {
    let mut output = String::new();
    for (index, character) in value.chars().enumerate() {
        if character.is_uppercase() && index != 0 {
            output.push('_');
        }
        output.extend(character.to_lowercase());
    }
    output
}

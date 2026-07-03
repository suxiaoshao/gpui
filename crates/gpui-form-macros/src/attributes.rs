use syn::{
    Attribute, Ident, LitBool, LitStr, Path, Result, Token, Type,
    parse::{Parse, ParseStream},
};

use crate::field_kind::FieldKind;

#[derive(Default)]
pub(crate) struct FormAttributes {
    pub(crate) store: Option<Ident>,
    pub(crate) validation: ValidationAdapterKind,
    pub(crate) transform: TransformAdapterKind,
}

impl FormAttributes {
    pub(crate) fn parse(attrs: &[Attribute]) -> Result<Self> {
        let mut parsed = Self::default();

        for attr in attrs {
            if !attr.path().is_ident("form") {
                continue;
            }

            let args = attr.parse_args::<FormArgs>()?;
            if args.store.is_some() {
                parsed.store = args.store;
            }
            if args.validation != ValidationAdapterKind::None {
                parsed.validation = args.validation;
            }
            if args.transform != TransformAdapterKind::Identity {
                parsed.transform = args.transform;
            }
        }

        Ok(parsed)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum ValidationAdapterKind {
    #[default]
    None,
    Garde,
    Custom {
        adapter: Path,
        context: Option<Path>,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum TransformAdapterKind {
    #[default]
    Identity,
    Validify,
    Custom {
        adapter: Path,
    },
}

#[derive(Default)]
pub(crate) struct FieldAttributes {
    pub(crate) component: FieldKind,
    pub(crate) binding: Option<Path>,
    pub(crate) store: Option<Type>,
    pub(crate) label: Option<LitStr>,
    pub(crate) description: Option<LitStr>,
    pub(crate) placeholder: Option<LitStr>,
    pub(crate) masked: bool,
    pub(crate) required: bool,
    pub(crate) validate_on_mount: bool,
    pub(crate) validate_on_change: bool,
    pub(crate) validate_on_blur: bool,
    pub(crate) validate_on_submit: bool,
    pub(crate) validate_on_dynamic: bool,
}

impl FieldAttributes {
    pub(crate) fn parse(attrs: &[Attribute]) -> Result<Self> {
        let mut parsed = Self::default();

        for attr in attrs {
            if !attr.path().is_ident("form") {
                continue;
            }

            let args = attr.parse_args::<FieldArgs>()?;
            if let Some(component) = args.component {
                parsed.component = component;
            }
            if args.binding.is_some() {
                parsed.binding = args.binding;
                parsed.component = FieldKind::Binding;
            }
            if args.store.is_some() {
                parsed.store = args.store;
            }
            if args.label.is_some() {
                parsed.label = args.label;
            }
            if args.description.is_some() {
                parsed.description = args.description;
            }
            if args.placeholder.is_some() {
                parsed.placeholder = args.placeholder;
            }
            parsed.masked |= args.masked;
            parsed.required |= args.required;
            parsed.validate_on_mount |= args.validate_on_mount;
            parsed.validate_on_change |= args.validate_on_change;
            parsed.validate_on_blur |= args.validate_on_blur;
            parsed.validate_on_submit |= args.validate_on_submit;
            parsed.validate_on_dynamic |= args.validate_on_dynamic;
        }

        Ok(parsed)
    }
}

struct FormArgs {
    store: Option<Ident>,
    validation: ValidationAdapterKind,
    transform: TransformAdapterKind,
}

impl Parse for FormArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut store = None;
        let mut validation = ValidationAdapterKind::None;
        let mut transform = TransformAdapterKind::Identity;

        while !input.is_empty() {
            let key: Ident = input.parse()?;

            if key == "store" {
                input.parse::<Token![=]>()?;
                store = Some(input.parse()?);
            } else if key == "validation" {
                let content;
                syn::parenthesized!(content in input);
                validation = parse_validation_adapter(&content)?;
            } else if key == "transform" {
                let content;
                syn::parenthesized!(content in input);
                transform = parse_transform_adapter(&content)?;
            } else if input.peek(Token![=]) {
                input.parse::<Token![=]>()?;
                if input.peek(LitStr) {
                    let _: LitStr = input.parse()?;
                } else {
                    let _: proc_macro2::TokenTree = input.parse()?;
                }
            } else if input.peek(syn::token::Paren) {
                let content;
                syn::parenthesized!(content in input);
                let _: proc_macro2::TokenStream = content.parse()?;
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(Self {
            store,
            validation,
            transform,
        })
    }
}

#[derive(Default)]
struct FieldArgs {
    component: Option<FieldKind>,
    binding: Option<Path>,
    store: Option<Type>,
    label: Option<LitStr>,
    description: Option<LitStr>,
    placeholder: Option<LitStr>,
    masked: bool,
    required: bool,
    validate_on_mount: bool,
    validate_on_change: bool,
    validate_on_blur: bool,
    validate_on_submit: bool,
    validate_on_dynamic: bool,
}

impl Parse for FieldArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut args = Self::default();

        while !input.is_empty() {
            let key: Ident = input.parse()?;

            if key == "component" {
                input.parse::<Token![=]>()?;
                let value: LitStr = input.parse()?;
                let component = value.value();
                if FieldKind::is_removed_alias(&component) {
                    return Err(syn::Error::new(
                        value.span(),
                        "built-in gpui-form component aliases were removed; use #[form(binding = \"TypeName\")] with an app or adapter binding",
                    ));
                }
                args.component =
                    Some(FieldKind::parse(&component).ok_or_else(|| {
                        syn::Error::new(value.span(), "unsupported form component")
                    })?);
            } else if key == "binding" {
                input.parse::<Token![=]>()?;
                args.binding = Some(parse_path_value(input)?);
            } else if key == "state" {
                return Err(syn::Error::new(
                    key.span(),
                    "use #[form(binding = \"TypeName\")] for app component bindings",
                ));
            } else if key == "store" {
                input.parse::<Token![=]>()?;
                args.store = Some(parse_type_value(input)?);
            } else if key == "label" {
                input.parse::<Token![=]>()?;
                args.label = Some(input.parse()?);
            } else if key == "description" {
                input.parse::<Token![=]>()?;
                args.description = Some(input.parse()?);
            } else if key == "placeholder" {
                input.parse::<Token![=]>()?;
                args.placeholder = Some(input.parse()?);
            } else if key == "mask" || key == "masked" {
                args.masked = parse_optional_bool_value(input)?;
            } else if key == "required" {
                args.required = parse_optional_bool_value(input)?;
            } else if key == "validate" {
                let content;
                syn::parenthesized!(content in input);
                while !content.is_empty() {
                    let trigger: Ident = content.parse()?;
                    if trigger == "on_mount" {
                        args.validate_on_mount = true;
                    } else if trigger == "on_change" {
                        args.validate_on_change = true;
                    } else if trigger == "on_blur" {
                        args.validate_on_blur = true;
                    } else if trigger == "on_submit" {
                        args.validate_on_submit = true;
                    } else if trigger == "on_dynamic" {
                        args.validate_on_dynamic = true;
                    } else {
                        return Err(syn::Error::new(
                            trigger.span(),
                            "unsupported validate trigger",
                        ));
                    }

                    if content.peek(Token![,]) {
                        content.parse::<Token![,]>()?;
                    }
                }
            } else {
                return Err(syn::Error::new(
                    key.span(),
                    format!("unsupported form field option `{key}`"),
                ));
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(args)
    }
}

fn parse_path_value(input: ParseStream<'_>) -> Result<Path> {
    if input.peek(LitStr) {
        input.parse::<LitStr>()?.parse()
    } else {
        input.parse()
    }
}

fn parse_type_value(input: ParseStream<'_>) -> Result<Type> {
    if input.peek(LitStr) {
        input.parse::<LitStr>()?.parse()
    } else {
        input.parse()
    }
}

fn parse_optional_bool_value(input: ParseStream<'_>) -> Result<bool> {
    if input.peek(Token![=]) {
        input.parse::<Token![=]>()?;
        Ok(input.parse::<LitBool>()?.value())
    } else {
        Ok(true)
    }
}

fn parse_validation_adapter(input: ParseStream<'_>) -> Result<ValidationAdapterKind> {
    let mut adapter = ValidationAdapterKind::None;
    let mut context = None;

    while !input.is_empty() {
        let key: Ident = input.parse()?;
        if key == "adapter" {
            input.parse::<Token![=]>()?;
            if input.peek(LitStr) {
                let value: LitStr = input.parse()?;
                adapter = match value.value().as_str() {
                    "garde" => ValidationAdapterKind::Garde,
                    other => {
                        return Err(syn::Error::new(
                            value.span(),
                            format!("unsupported validation adapter `{other}`"),
                        ));
                    }
                };
            } else {
                adapter = ValidationAdapterKind::Custom {
                    adapter: input.parse()?,
                    context: None,
                }
            }
        } else if key == "context" {
            input.parse::<Token![=]>()?;
            context = Some(parse_path_value(input)?);
        } else {
            return Err(syn::Error::new(key.span(), "unsupported validation option"));
        }

        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        }
    }

    Ok(match adapter {
        ValidationAdapterKind::Custom {
            adapter,
            context: existing_context,
        } => ValidationAdapterKind::Custom {
            adapter,
            context: context.or(existing_context),
        },
        ValidationAdapterKind::None | ValidationAdapterKind::Garde => adapter,
    })
}

fn parse_transform_adapter(input: ParseStream<'_>) -> Result<TransformAdapterKind> {
    let mut adapter = TransformAdapterKind::Identity;

    while !input.is_empty() {
        let key: Ident = input.parse()?;
        if key == "adapter" {
            input.parse::<Token![=]>()?;
            if input.peek(LitStr) {
                let value: LitStr = input.parse()?;
                adapter = match value.value().as_str() {
                    "validify" => TransformAdapterKind::Validify,
                    other => {
                        return Err(syn::Error::new(
                            value.span(),
                            format!("unsupported transform adapter `{other}`"),
                        ));
                    }
                };
            } else {
                adapter = TransformAdapterKind::Custom {
                    adapter: input.parse()?,
                }
            }
        } else {
            return Err(syn::Error::new(key.span(), "unsupported transform option"));
        }

        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        }
    }

    Ok(adapter)
}

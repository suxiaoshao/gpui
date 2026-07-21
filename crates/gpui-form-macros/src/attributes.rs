use syn::{
    Attribute, Ident, LitStr, Result, Token, TypePath,
    parse::{Parse, ParseStream},
};

#[derive(Default)]
pub(crate) struct FormAttributes {
    pub(crate) store: Option<Ident>,
    pub(crate) validation: ValidationAdapterKind,
    pub(crate) transform: TransformAdapterKind,
}

impl FormAttributes {
    pub(crate) fn parse(attrs: &[Attribute]) -> Result<Self> {
        let mut parsed = Self::default();
        let mut helpers = attrs.iter().filter(|attr| attr.path().is_ident("form"));
        let Some(attr) = helpers.next() else {
            return Ok(parsed);
        };
        if let Some(duplicate) = helpers.next() {
            return Err(syn::Error::new_spanned(
                duplicate,
                "duplicate #[form(...)] attribute",
            ));
        }
        let args = attr.parse_args::<FormArgs>()?;
        if let Some(store) = args.store
            && parsed.store.replace(store).is_some()
        {
            return Err(syn::Error::new_spanned(attr, "duplicate form store name"));
        }
        if !matches!(args.validation, ValidationAdapterKind::None) {
            if !matches!(parsed.validation, ValidationAdapterKind::None) {
                return Err(syn::Error::new_spanned(
                    attr,
                    "duplicate validation configuration",
                ));
            }
            parsed.validation = args.validation;
        }
        if !matches!(args.transform, TransformAdapterKind::Identity) {
            if !matches!(parsed.transform, TransformAdapterKind::Identity) {
                return Err(syn::Error::new_spanned(
                    attr,
                    "duplicate transform configuration",
                ));
            }
            parsed.transform = args.transform;
        }
        Ok(parsed)
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) enum ValidationAdapterKind {
    #[default]
    None,
    Garde {
        i18n: Option<Box<TypePath>>,
    },
    Custom {
        adapter: Box<TypePath>,
        context: Option<Box<TypePath>>,
    },
}

#[derive(Clone, Debug, Default)]
pub(crate) enum TransformAdapterKind {
    #[default]
    Identity,
    Validify,
    Custom {
        adapter: Box<TypePath>,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum FieldShape {
    #[default]
    Value,
    Group,
    Array {
        id: Ident,
    },
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ValidationTriggers {
    pub(crate) mount: bool,
    pub(crate) change: bool,
    pub(crate) blur: bool,
    pub(crate) dynamic: bool,
    pub(crate) submit: bool,
}

#[derive(Default)]
pub(crate) struct FieldAttributes {
    pub(crate) shape: FieldShape,
    pub(crate) required: bool,
    pub(crate) triggers: ValidationTriggers,
}

impl FieldAttributes {
    pub(crate) fn parse(attrs: &[Attribute]) -> Result<Self> {
        let mut parsed = Self::default();
        let mut helpers = attrs.iter().filter(|attr| attr.path().is_ident("form"));
        let Some(attr) = helpers.next() else {
            return Ok(parsed);
        };
        if let Some(duplicate) = helpers.next() {
            return Err(syn::Error::new_spanned(
                duplicate,
                "duplicate #[form(...)] attribute",
            ));
        }
        let args = attr.parse_args::<FieldArgs>()?;
        if !matches!(args.shape, FieldShape::Value) {
            if !matches!(parsed.shape, FieldShape::Value) {
                return Err(syn::Error::new_spanned(attr, "duplicate field shape"));
            }
            parsed.shape = args.shape;
        }
        parsed.required |= args.required;
        merge_triggers(&mut parsed.triggers, args.triggers, attr)?;
        Ok(parsed)
    }
}

fn merge_triggers(
    target: &mut ValidationTriggers,
    next: ValidationTriggers,
    attr: &Attribute,
) -> Result<()> {
    for (target, next, name) in [
        (&mut target.mount, next.mount, "on_mount"),
        (&mut target.change, next.change, "on_change"),
        (&mut target.blur, next.blur, "on_blur"),
        (&mut target.dynamic, next.dynamic, "on_dynamic"),
        (&mut target.submit, next.submit, "on_submit"),
    ] {
        if *target && next {
            return Err(syn::Error::new_spanned(
                attr,
                format!("duplicate validation trigger `{name}`"),
            ));
        }
        *target |= next;
    }
    Ok(())
}

struct FormArgs {
    store: Option<Ident>,
    validation: ValidationAdapterKind,
    transform: TransformAdapterKind,
}

impl Parse for FormArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        if input.is_empty() {
            return Err(input.error("#[form(...)] requires at least one option"));
        }
        let mut store = None;
        let mut validation = ValidationAdapterKind::None;
        let mut transform = TransformAdapterKind::Identity;
        while !input.is_empty() {
            let key: Ident = input.parse()?;
            match key.to_string().as_str() {
                "store" => {
                    if store.is_some() {
                        return Err(syn::Error::new(key.span(), "duplicate form option `store`"));
                    }
                    input.parse::<Token![=]>()?;
                    store = Some(input.parse()?);
                }
                "validation" => {
                    if !matches!(validation, ValidationAdapterKind::None) {
                        return Err(syn::Error::new(
                            key.span(),
                            "duplicate form option `validation`",
                        ));
                    }
                    let content;
                    syn::parenthesized!(content in input);
                    validation = parse_validation(&content)?;
                }
                "transform" => {
                    if !matches!(transform, TransformAdapterKind::Identity) {
                        return Err(syn::Error::new(
                            key.span(),
                            "duplicate form option `transform`",
                        ));
                    }
                    let content;
                    syn::parenthesized!(content in input);
                    transform = parse_transform(&content)?;
                }
                _ => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unsupported form option `{key}`"),
                    ));
                }
            }
            consume_comma(input)?;
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
    shape: FieldShape,
    required: bool,
    triggers: ValidationTriggers,
}

impl Parse for FieldArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        if input.is_empty() {
            return Err(input.error("#[form(...)] requires at least one field option"));
        }
        let mut args = Self::default();
        let mut required = false;
        let mut validate = false;
        let mut shape = false;
        while !input.is_empty() {
            let key: Ident = input.parse()?;
            match key.to_string().as_str() {
                "required" => {
                    if required {
                        return Err(syn::Error::new(
                            key.span(),
                            "duplicate field option `required`",
                        ));
                    }
                    required = true;
                    args.required = true;
                }
                "validate" => {
                    if validate {
                        return Err(syn::Error::new(
                            key.span(),
                            "duplicate field option `validate`",
                        ));
                    }
                    validate = true;
                    let content;
                    syn::parenthesized!(content in input);
                    if content.is_empty() {
                        return Err(content.error("validate requires at least one trigger"));
                    }
                    args.triggers = parse_triggers(&content)?;
                }
                "group" => {
                    if shape {
                        return Err(syn::Error::new(key.span(), "duplicate field shape"));
                    }
                    shape = true;
                    if input.peek(syn::token::Paren) {
                        let content;
                        syn::parenthesized!(content in input);
                        if !content.is_empty() {
                            let removed: Ident = content.parse()?;
                            return Err(syn::Error::new(
                                removed.span(),
                                "group configuration was removed; use #[form(group)]",
                            ));
                        }
                    }
                    args.shape = FieldShape::Group;
                }
                "array" => {
                    if shape {
                        return Err(syn::Error::new(key.span(), "duplicate field shape"));
                    }
                    shape = true;
                    let content;
                    syn::parenthesized!(content in input);
                    let id_key: Ident = content.parse()?;
                    if id_key != "id" {
                        return Err(syn::Error::new(
                            id_key.span(),
                            "identified arrays require #[form(array(id = \"field_name\"))]",
                        ));
                    }
                    content.parse::<Token![=]>()?;
                    let literal = content.parse::<LitStr>().map_err(|_| {
                        content.error("array id must be a string literal: id = \"field_name\"")
                    })?;
                    let id = Ident::new(&literal.value(), literal.span());
                    if !content.is_empty() {
                        return Err(content.error("unexpected array configuration"));
                    }
                    args.shape = FieldShape::Array { id };
                }
                "component" | "codec" | "binding" | "state" | "focus" | "store" => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!(
                            "`{key}` is no longer a form field option; controls own component state and typed values need no codec"
                        ),
                    ));
                }
                _ => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unsupported form field option `{key}`"),
                    ));
                }
            }
            consume_comma(input)?;
        }
        Ok(args)
    }
}

fn parse_triggers(input: ParseStream<'_>) -> Result<ValidationTriggers> {
    let mut triggers = ValidationTriggers::default();
    while !input.is_empty() {
        let trigger: Ident = input.parse()?;
        let slot = match trigger.to_string().as_str() {
            "on_mount" => &mut triggers.mount,
            "on_change" => &mut triggers.change,
            "on_blur" => &mut triggers.blur,
            "on_dynamic" => &mut triggers.dynamic,
            "on_submit" => &mut triggers.submit,
            _ => {
                return Err(syn::Error::new(
                    trigger.span(),
                    "unsupported validate trigger",
                ));
            }
        };
        if *slot {
            return Err(syn::Error::new(
                trigger.span(),
                "duplicate validation trigger",
            ));
        }
        *slot = true;
        consume_comma(input)?;
    }
    Ok(triggers)
}

fn parse_validation(input: ParseStream<'_>) -> Result<ValidationAdapterKind> {
    if input.is_empty() {
        return Err(input.error("validation requires an adapter"));
    }
    let mut adapter = None;
    let mut context = None;
    let mut i18n = None;
    while !input.is_empty() {
        let key: Ident = input.parse()?;
        input.parse::<Token![=]>()?;
        match key.to_string().as_str() {
            "adapter" => {
                if adapter.is_some() {
                    return Err(syn::Error::new(
                        key.span(),
                        "duplicate validation option `adapter`",
                    ));
                }
                if input.peek(LitStr) {
                    let value = input.parse::<LitStr>()?;
                    if value.value() != "garde" {
                        return Err(syn::Error::new(
                            value.span(),
                            "unsupported validation adapter",
                        ));
                    }
                    adapter = Some(ValidationAdapterKind::Garde { i18n: None });
                } else {
                    adapter = Some(ValidationAdapterKind::Custom {
                        adapter: Box::new(parse_type_path(input)?),
                        context: None,
                    });
                }
            }
            "context" => {
                if context.is_some() {
                    return Err(syn::Error::new(
                        key.span(),
                        "duplicate validation option `context`",
                    ));
                }
                context = Some(Box::new(parse_type_path(input)?));
            }
            "i18n" => {
                if i18n.is_some() {
                    return Err(syn::Error::new(
                        key.span(),
                        "duplicate validation option `i18n`",
                    ));
                }
                i18n = Some(Box::new(parse_type_path(input)?));
            }
            _ => return Err(syn::Error::new(key.span(), "unsupported validation option")),
        }
        consume_comma(input)?;
    }
    match adapter.unwrap_or(ValidationAdapterKind::None) {
        ValidationAdapterKind::Garde { .. } if context.is_some() => Err(syn::Error::new_spanned(
            context.unwrap(),
            "Garde validation context comes from garde::Validate::Context",
        )),
        ValidationAdapterKind::Garde { .. } => Ok(ValidationAdapterKind::Garde { i18n }),
        ValidationAdapterKind::Custom { .. } if i18n.is_some() => Err(syn::Error::new_spanned(
            i18n.unwrap(),
            "i18n is only supported by the Garde adapter",
        )),
        ValidationAdapterKind::Custom { adapter, .. } => {
            Ok(ValidationAdapterKind::Custom { adapter, context })
        }
        ValidationAdapterKind::None if context.is_some() || i18n.is_some() => Err(syn::Error::new(
            input.span(),
            "validation context/i18n requires an adapter",
        )),
        ValidationAdapterKind::None => Ok(ValidationAdapterKind::None),
    }
}

fn parse_transform(input: ParseStream<'_>) -> Result<TransformAdapterKind> {
    if input.is_empty() {
        return Err(input.error("transform requires an adapter"));
    }
    let key: Ident = input.parse()?;
    if key != "adapter" {
        return Err(syn::Error::new(key.span(), "unsupported transform option"));
    }
    input.parse::<Token![=]>()?;
    let adapter = if input.peek(LitStr) {
        let value = input.parse::<LitStr>()?;
        if value.value() != "validify" {
            return Err(syn::Error::new(
                value.span(),
                "unsupported transform adapter",
            ));
        }
        TransformAdapterKind::Validify
    } else {
        TransformAdapterKind::Custom {
            adapter: Box::new(parse_type_path(input)?),
        }
    };
    consume_comma(input)?;
    if !input.is_empty() {
        return Err(input.error("unexpected transform configuration"));
    }
    Ok(adapter)
}

fn parse_type_path(input: ParseStream<'_>) -> Result<TypePath> {
    if input.peek(LitStr) {
        let literal = input.parse::<LitStr>()?;
        return Err(syn::Error::new(
            literal.span(),
            "custom types must be written as an unquoted type path",
        ));
    }
    input.parse()
}

fn consume_comma(input: ParseStream<'_>) -> Result<()> {
    if input.peek(Token![,]) {
        input.parse::<Token![,]>()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{FieldAttributes, FormAttributes, ValidationAdapterKind};
    use syn::{Attribute, parse_quote};

    #[test]
    fn parses_garde_i18n_provider_without_custom_context() {
        let attrs: Vec<Attribute> = vec![parse_quote!(
            #[form(validation(adapter = "garde", i18n = AppI18nProvider))]
        )];
        let parsed = FormAttributes::parse(&attrs).expect("valid Garde configuration");
        assert!(matches!(
            parsed.validation,
            ValidationAdapterKind::Garde { i18n: Some(_) }
        ));
    }

    #[test]
    fn rejects_context_for_garde_and_i18n_for_custom_adapter() {
        let garde: Vec<Attribute> = vec![parse_quote!(
            #[form(validation(adapter = "garde", context = AppContext))]
        )];
        assert!(
            FormAttributes::parse(&garde)
                .err()
                .expect("Garde context must be rejected")
                .to_string()
                .contains("garde::Validate::Context")
        );

        let custom: Vec<Attribute> = vec![parse_quote!(
            #[form(validation(adapter = AppValidator, i18n = AppI18nProvider))]
        )];
        assert!(
            FormAttributes::parse(&custom)
                .err()
                .expect("custom i18n must be rejected")
                .to_string()
                .contains("only supported by the Garde adapter")
        );
    }

    #[test]
    fn rejects_duplicate_validation_trigger_and_removed_field_options() {
        let duplicate: Vec<Attribute> = vec![parse_quote!(
            #[form(validate(on_change, on_change))]
        )];
        assert!(
            FieldAttributes::parse(&duplicate)
                .err()
                .expect("duplicate trigger must be rejected")
                .to_string()
                .contains("duplicate validation trigger")
        );

        let removed: Vec<Attribute> = vec![parse_quote!(#[form(codec = TextCodec)])];
        assert!(
            FieldAttributes::parse(&removed)
                .err()
                .expect("removed option must be rejected")
                .to_string()
                .contains("no longer a form field option")
        );
    }

    #[test]
    fn rejects_duplicate_helper_attributes_and_options() {
        let helpers: Vec<Attribute> = vec![
            parse_quote!(#[form(store = ExampleForm)]),
            parse_quote!(#[form(transform(adapter = "validify"))]),
        ];
        assert!(
            FormAttributes::parse(&helpers)
                .err()
                .expect("duplicate helper attributes must be rejected")
                .to_string()
                .contains("duplicate #[form(...)] attribute")
        );

        let options: Vec<Attribute> = vec![parse_quote!(
            #[form(store = ExampleForm, store = OtherForm)]
        )];
        assert!(
            FormAttributes::parse(&options)
                .err()
                .expect("duplicate options must be rejected")
                .to_string()
                .contains("duplicate form option `store`")
        );
    }

    #[test]
    fn rejects_empty_configuration_clauses() {
        let helper: Vec<Attribute> = vec![parse_quote!(#[form()])];
        assert!(
            FormAttributes::parse(&helper)
                .err()
                .expect("an empty helper must be rejected")
                .to_string()
                .contains("requires at least one option")
        );

        let validation: Vec<Attribute> = vec![parse_quote!(#[form(validation())])];
        assert!(
            FormAttributes::parse(&validation)
                .err()
                .expect("empty validation configuration must be rejected")
                .to_string()
                .contains("validation requires an adapter")
        );

        let validate: Vec<Attribute> = vec![parse_quote!(#[form(validate())])];
        assert!(
            FieldAttributes::parse(&validate)
                .err()
                .expect("empty validation triggers must be rejected")
                .to_string()
                .contains("validate requires at least one trigger")
        );
    }

    #[test]
    fn rejects_quoted_custom_types_and_bare_array_ids() {
        let quoted_context: Vec<Attribute> = vec![parse_quote!(
            #[form(validation(adapter = crate::ValidationAdapter, context = "crate::Context"))]
        )];
        assert!(
            FormAttributes::parse(&quoted_context)
                .err()
                .expect("quoted context types must be rejected")
                .to_string()
                .contains("unquoted type path")
        );

        let bare_id: Vec<Attribute> = vec![parse_quote!(#[form(array(id = row_id))])];
        assert!(
            FieldAttributes::parse(&bare_id)
                .err()
                .expect("array ids must be string literals")
                .to_string()
                .contains("array id must be a string literal")
        );
    }
}

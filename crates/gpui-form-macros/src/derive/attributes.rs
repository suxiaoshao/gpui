use proc_macro2::Span;
use syn::{Attribute, spanned::Spanned};

pub(super) fn reject_container_attributes(attributes: &[Attribute]) -> syn::Result<()> {
    if let Some(attribute) = attributes
        .iter()
        .find(|attribute| attribute.path().is_ident("form"))
    {
        return Err(syn::Error::new_spanned(
            attribute,
            "FormSchema has no container-level #[form(...)] options",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FieldKind {
    Leaf,
    Child,
    Items,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct TriggerSelection {
    pub(super) mount: bool,
    pub(super) change: bool,
    pub(super) blur: bool,
    pub(super) external: bool,
    pub(super) submit: bool,
}

impl TriggerSelection {
    fn is_empty(self) -> bool {
        !self.mount && !self.change && !self.blur && !self.external && !self.submit
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct FieldOptions {
    pub(super) kind: FieldKind,
    pub(super) required: bool,
    pub(super) triggers: TriggerSelection,
    pub(super) has_validation: bool,
    pub(super) required_span: Option<Span>,
    pub(super) validation_span: Option<Span>,
}

impl Default for FieldOptions {
    fn default() -> Self {
        Self {
            kind: FieldKind::Leaf,
            required: false,
            triggers: TriggerSelection::default(),
            has_validation: false,
            required_span: None,
            validation_span: None,
        }
    }
}

pub(super) fn parse_field_options(attributes: &[Attribute]) -> syn::Result<FieldOptions> {
    let mut options = FieldOptions::default();
    let mut child_span = None;
    let mut items_span = None;

    for attribute in attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("form"))
    {
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("required") {
                if options.required {
                    return Err(meta.error("duplicate `required` option"));
                }
                options.required = true;
                options.required_span = Some(meta.path.span());
                return Ok(());
            }
            if meta.path.is_ident("child") {
                if child_span.is_some() {
                    return Err(meta.error("duplicate `child` option"));
                }
                if items_span.is_some() {
                    return Err(meta.error("only one of `child` or `items` may be specified"));
                }
                child_span = Some(meta.path.span());
                options.kind = FieldKind::Child;
                return Ok(());
            }
            if meta.path.is_ident("items") {
                if items_span.is_some() {
                    return Err(meta.error("duplicate `items` option"));
                }
                if child_span.is_some() {
                    return Err(meta.error("only one of `child` or `items` may be specified"));
                }
                items_span = Some(meta.path.span());
                options.kind = FieldKind::Items;
                return Ok(());
            }
            if meta.path.is_ident("validate") {
                if options.has_validation {
                    return Err(meta.error("duplicate `validate` option"));
                }
                options.has_validation = true;
                options.validation_span = Some(meta.path.span());
                return meta.parse_nested_meta(|trigger| {
                    let (value, name) = if trigger.path.is_ident("on_mount") {
                        (&mut options.triggers.mount, "on_mount")
                    } else if trigger.path.is_ident("on_change") {
                        (&mut options.triggers.change, "on_change")
                    } else if trigger.path.is_ident("on_blur") {
                        (&mut options.triggers.blur, "on_blur")
                    } else if trigger.path.is_ident("on_external") {
                        (&mut options.triggers.external, "on_external")
                    } else if trigger.path.is_ident("on_submit") {
                        (&mut options.triggers.submit, "on_submit")
                    } else {
                        return Err(trigger.error(
                            "expected on_mount, on_change, on_blur, on_external, or on_submit",
                        ));
                    };

                    if *value {
                        return Err(trigger.error(format!("duplicate `{name}` trigger")));
                    }
                    *value = true;
                    Ok(())
                });
            }
            Err(meta.error("unsupported FormSchema field option"))
        })?;
    }

    if (options.required || options.has_validation) && options.triggers.is_empty() {
        options.triggers.submit = true;
    }

    Ok(options)
}

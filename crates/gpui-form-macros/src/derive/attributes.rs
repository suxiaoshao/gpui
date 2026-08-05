use syn::Attribute;

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

#[derive(Default)]
pub(super) struct FieldOptions {
    pub(super) kind: Option<FieldKind>,
    pub(super) required: bool,
    pub(super) mount: bool,
    pub(super) change: bool,
    pub(super) blur: bool,
    pub(super) dynamic: bool,
    pub(super) submit: bool,
}

pub(super) fn parse_field_options(attributes: &[Attribute]) -> syn::Result<FieldOptions> {
    let mut options = FieldOptions::default();
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
                return Ok(());
            }
            if meta.path.is_ident("child") {
                return set_kind(&mut options, FieldKind::Child, &meta);
            }
            if meta.path.is_ident("items") {
                return set_kind(&mut options, FieldKind::Items, &meta);
            }
            if meta.path.is_ident("validate") {
                return meta.parse_nested_meta(|trigger| {
                    if trigger.path.is_ident("on_mount") {
                        options.mount = true;
                    } else if trigger.path.is_ident("on_change") {
                        options.change = true;
                    } else if trigger.path.is_ident("on_blur") {
                        options.blur = true;
                    } else if trigger.path.is_ident("on_dynamic") {
                        options.dynamic = true;
                    } else if trigger.path.is_ident("on_submit") {
                        options.submit = true;
                    } else {
                        return Err(trigger.error(
                            "expected on_mount, on_change, on_blur, on_dynamic, or on_submit",
                        ));
                    }
                    Ok(())
                });
            }
            Err(meta.error("unsupported FormSchema field option"))
        })?;
    }
    if options.required
        && !(options.mount || options.change || options.blur || options.dynamic || options.submit)
    {
        options.mount = true;
        options.change = true;
        options.blur = true;
        options.submit = true;
    }
    Ok(options)
}

fn set_kind(
    options: &mut FieldOptions,
    kind: FieldKind,
    meta: &syn::meta::ParseNestedMeta<'_>,
) -> syn::Result<()> {
    if options.kind.replace(kind).is_some() {
        return Err(meta.error("only one of `child` or `items` may be specified"));
    }
    Ok(())
}

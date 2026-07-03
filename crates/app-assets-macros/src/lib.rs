use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Attribute, Error, Fields, Ident, ItemEnum, LitStr, Result, Token, Visibility,
    parse::{Parse, ParseStream},
    parse_macro_input,
    spanned::Spanned,
};

#[proc_macro]
pub fn define_lucide_icons(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as LucideInput);
    expand_icons(
        input.vis,
        input.name,
        format_ident!("LucideAssets"),
        input.icons,
    )
    .unwrap_or_else(Error::into_compile_error)
    .into()
}

#[proc_macro]
pub fn define_svg_icons(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as SvgIconEnum);
    let asset_source = input
        .asset_source
        .unwrap_or_else(|| format_ident!("{}Assets", input.item.ident));

    expand_icons(input.item.vis, input.item.ident, asset_source, input.icons)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

struct LucideInput {
    vis: Visibility,
    name: Ident,
    icons: Vec<IconSpec>,
}

impl Parse for LucideInput {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let vis = input.parse()?;
        input.parse::<Token![enum]>()?;
        let name = input.parse()?;

        let content;
        syn::braced!(content in input);

        let mut icons = Vec::new();
        while !content.is_empty() {
            let variant = content.parse()?;
            content.parse::<Token![=>]>()?;
            let slug: LitStr = content.parse()?;
            icons.push(IconSpec::lucide(variant, slug));

            if content.peek(Token![,]) {
                content.parse::<Token![,]>()?;
            }
        }

        Ok(Self { vis, name, icons })
    }
}

struct SvgIconEnum {
    item: ItemEnum,
    asset_source: Option<Ident>,
    icons: Vec<IconSpec>,
}

impl Parse for SvgIconEnum {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut item: ItemEnum = input.parse()?;
        let asset_source = take_asset_source(&mut item.attrs)?;
        let mut icons = Vec::new();

        for variant in &mut item.variants {
            if !matches!(variant.fields, Fields::Unit) {
                return Err(Error::new(
                    variant.fields.span(),
                    "app asset icon variants must be unit variants",
                ));
            }

            icons.push(take_icon_spec(&variant.ident, &mut variant.attrs)?);
        }

        Ok(Self {
            item,
            asset_source,
            icons,
        })
    }
}

struct IconSpec {
    variant: Ident,
    path: LitStr,
    include_path: proc_macro2::TokenStream,
    source: LitStr,
    slug: Option<LitStr>,
}

impl IconSpec {
    fn lucide(variant: Ident, slug: LitStr) -> Self {
        let path = LitStr::new(&format!("icons/{}.svg", slug.value()), slug.span());
        let include_path = quote! {
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../third_party/lucide/icons/",
                #slug,
                ".svg"
            )
        };
        Self {
            variant,
            path,
            include_path,
            source: LitStr::new("lucide", slug.span()),
            slug: Some(slug),
        }
    }

    fn svg(variant: Ident, attr: SvgAttr) -> Result<Self> {
        let path_value = attr.path.value();
        if path_value.starts_with('/') || path_value.split('/').any(|part| part == "..") {
            return Err(Error::new(
                attr.path.span(),
                "custom SVG asset paths must be relative to the app assets directory",
            ));
        }

        let path = attr.path;
        let include_path = quote! {
            concat!(env!("CARGO_MANIFEST_DIR"), "/assets/", #path)
        };
        let source = attr
            .source
            .unwrap_or_else(|| LitStr::new("custom-svg", path.span()));
        Ok(Self {
            variant,
            path,
            include_path,
            source,
            slug: attr.slug,
        })
    }
}

struct SvgAttr {
    path: LitStr,
    source: Option<LitStr>,
    slug: Option<LitStr>,
}

impl Parse for SvgAttr {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let path = input.parse()?;
        let mut source = None;
        let mut slug = None;

        while input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                break;
            }

            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let value: LitStr = input.parse()?;

            if key == "source" {
                source = Some(value);
            } else if key == "slug" {
                slug = Some(value);
            } else {
                return Err(Error::new(key.span(), "unsupported svg icon attribute"));
            }
        }

        Ok(Self { path, source, slug })
    }
}

fn take_asset_source(attrs: &mut Vec<Attribute>) -> Result<Option<Ident>> {
    let mut asset_source = None;
    let mut retained = Vec::new();

    for attr in attrs.drain(..) {
        if attr.path().is_ident("asset_source") {
            if asset_source.is_some() {
                return Err(Error::new(attr.span(), "duplicate asset_source attribute"));
            }
            asset_source = Some(attr.parse_args()?);
        } else {
            retained.push(attr);
        }
    }

    *attrs = retained;
    Ok(asset_source)
}

fn take_icon_spec(variant: &Ident, attrs: &mut Vec<Attribute>) -> Result<IconSpec> {
    let mut icon = None;
    let mut retained = Vec::new();

    for attr in attrs.drain(..) {
        if attr.path().is_ident("lucide") {
            if icon.is_some() {
                return Err(Error::new(attr.span(), "duplicate icon source attribute"));
            }
            icon = Some(IconSpec::lucide(variant.clone(), attr.parse_args()?));
        } else if attr.path().is_ident("svg") {
            if icon.is_some() {
                return Err(Error::new(attr.span(), "duplicate icon source attribute"));
            }
            icon = Some(IconSpec::svg(variant.clone(), attr.parse_args()?)?);
        } else {
            retained.push(attr);
        }
    }

    *attrs = retained;
    icon.ok_or_else(|| {
        Error::new(
            variant.span(),
            "icon variants must declare either #[lucide(\"slug\")] or #[svg(\"path\")]",
        )
    })
}

fn expand_icons(
    vis: Visibility,
    name: Ident,
    asset_source: Ident,
    icons: Vec<IconSpec>,
) -> Result<proc_macro2::TokenStream> {
    if icons.is_empty() {
        return Err(Error::new(
            name.span(),
            "icon enum must declare at least one icon",
        ));
    }

    let variants = icons.iter().map(|icon| &icon.variant);
    let path_arms = icons.iter().map(|icon| {
        let variant = &icon.variant;
        let path = &icon.path;
        quote! { Self::#variant => #path.into(), }
    });
    let metadata_arms = icons.iter().map(|icon| {
        let variant = &icon.variant;
        let source = &icon.source;
        let slug = option_literal(icon.slug.as_ref());
        quote! {
            Self::#variant => ::app_assets::SvgIconMetadata {
                source: #source,
                slug: #slug,
            },
        }
    });
    let load_arms = icons.iter().map(|icon| {
        let path = &icon.path;
        let include_path = &icon.include_path;
        quote! { #path => Some(include_bytes!(#include_path)), }
    });
    let list_items = icons.iter().map(|icon| &icon.path);

    Ok(quote! {
        #[allow(dead_code)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #vis enum #name {
            #( #variants, )*
        }

        impl ::gpui_component::IconNamed for #name {
            fn path(self) -> ::gpui::SharedString {
                match self {
                    #( #path_arms )*
                }
            }
        }

        impl ::app_assets::SvgIconNamed for #name {
            fn metadata(self) -> ::app_assets::SvgIconMetadata {
                match self {
                    #( #metadata_arms )*
                }
            }
        }

        impl #name {
            fn __app_assets_load(path: &str) -> Option<&'static [u8]> {
                match path {
                    #( #load_arms )*
                    _ => None,
                }
            }

            fn __app_assets_list(path: &str) -> Vec<::gpui::SharedString> {
                let icons = [#( ::gpui::SharedString::from(#list_items), )*];
                icons
                    .into_iter()
                    .filter(|icon| path.is_empty() || icon.as_ref().starts_with(path))
                    .collect()
            }
        }

        #[derive(Default)]
        #vis struct #asset_source;

        impl ::gpui::AssetSource for #asset_source {
            fn load(
                &self,
                path: &str,
            ) -> ::gpui::Result<Option<std::borrow::Cow<'static, [u8]>>> {
                if path.is_empty() {
                    return Ok(None);
                }

                Ok(#name::__app_assets_load(path).map(std::borrow::Cow::Borrowed))
            }

            fn list(&self, path: &str) -> ::gpui::Result<Vec<::gpui::SharedString>> {
                Ok(#name::__app_assets_list(path))
            }
        }
    })
}

fn option_literal(value: Option<&LitStr>) -> proc_macro2::TokenStream {
    match value {
        Some(value) => quote! { Some(#value) },
        None => quote! { None },
    }
}

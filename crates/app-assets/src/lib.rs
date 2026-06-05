use gpui::{AssetSource, SharedString};
use std::{borrow::Cow, collections::BTreeSet};

extern crate self as app_assets;

pub use app_assets_macros::{define_lucide_icons, define_svg_icons};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SvgIconMetadata {
    pub source: &'static str,
    pub slug: Option<&'static str>,
}

pub trait SvgIconNamed: gpui_component::IconNamed + Copy {
    fn metadata(self) -> SvgIconMetadata;
}

pub struct AppAssets<A, B = gpui_component_assets::Assets> {
    app_assets: A,
    fallback_assets: B,
}

impl<A, B> AppAssets<A, B> {
    pub fn new(app_assets: A, fallback_assets: B) -> Self {
        Self {
            app_assets,
            fallback_assets,
        }
    }
}

impl<A> Default for AppAssets<A>
where
    A: Default,
{
    fn default() -> Self {
        Self {
            app_assets: A::default(),
            fallback_assets: gpui_component_assets::Assets,
        }
    }
}

impl<A> Default for AppAssets<A, ()>
where
    A: Default,
{
    fn default() -> Self {
        Self {
            app_assets: A::default(),
            fallback_assets: (),
        }
    }
}

impl<A, B> AssetSource for AppAssets<A, B>
where
    A: AssetSource,
    B: AssetSource,
{
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }

        match self.app_assets.load(path) {
            Ok(Some(data)) => Ok(Some(data)),
            Ok(None) | Err(_) => self.fallback_assets.load(path),
        }
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        let mut names = BTreeSet::new();

        if let Ok(assets) = self.app_assets.list(path) {
            names.extend(assets);
        }
        if let Ok(assets) = self.fallback_assets.list(path) {
            names.extend(assets);
        }

        Ok(names.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::{AppAssets, SvgIconMetadata, SvgIconNamed as _};
    use gpui::{AssetSource, SharedString};
    use gpui_component::IconNamed as _;
    use std::borrow::Cow;

    crate::define_lucide_icons!(
        pub enum TestIconName {
            CircleCheck => "circle-check",
            LoaderCircle => "loader-circle",
        }
    );

    crate::define_svg_icons!(
        #[asset_source(TestSvgAssets)]
        pub enum TestSvgName {
            #[lucide("circle-check")]
            CircleCheck,
            #[svg("test-icons/provider.svg", source = "simple-icons", slug = "provider")]
            Provider,
        }
    );

    #[test]
    fn declared_icons_have_lucide_paths() {
        assert_eq!(
            TestIconName::CircleCheck.path(),
            SharedString::from("icons/circle-check.svg")
        );
        assert_eq!(
            TestIconName::LoaderCircle.path(),
            SharedString::from("icons/loader-circle.svg")
        );
    }

    #[test]
    fn declared_svg_icons_have_paths_and_metadata() {
        assert_eq!(
            TestSvgName::CircleCheck.path(),
            SharedString::from("icons/circle-check.svg")
        );
        assert_eq!(
            TestSvgName::Provider.path(),
            SharedString::from("test-icons/provider.svg")
        );
        assert_eq!(
            TestSvgName::Provider.metadata(),
            SvgIconMetadata {
                source: "simple-icons",
                slug: Some("provider")
            }
        );
    }

    #[test]
    fn declared_icons_are_loadable() {
        let assets = LucideAssets;

        let icon = assets
            .load("icons/circle-check.svg")
            .expect("load declared icon")
            .expect("declared icon exists");

        assert!(!icon.is_empty());
    }

    #[test]
    fn app_assets_lists_declared_icons() {
        let assets = AppAssets::<LucideAssets, ()>::default();
        let icons = assets.list("icons/").expect("list icons");

        assert!(icons.contains(&SharedString::from("icons/circle-check.svg")));
        assert!(!icons.contains(&SharedString::from("icons/x.svg")));
    }

    #[test]
    fn declared_svg_icons_are_loadable() {
        let assets = TestSvgAssets;

        let icon = assets
            .load("test-icons/provider.svg")
            .expect("load declared provider icon")
            .expect("declared provider icon exists");

        assert!(!icon.is_empty());
    }

    #[test]
    fn declared_svg_icons_are_listed() {
        let assets = TestSvgAssets;
        let icons = assets.list("test-icons/").expect("list custom svg icons");

        assert!(icons.contains(&SharedString::from("test-icons/provider.svg")));
        assert!(!icons.contains(&SharedString::from("icons/circle-check.svg")));
    }

    #[test]
    fn app_assets_list_combines_and_deduplicates_sources() {
        #[derive(Default)]
        struct ExtraAssets;

        impl AssetSource for ExtraAssets {
            fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
                Ok((path == "icons/extra.svg").then(|| Cow::Borrowed(b"extra".as_slice())))
            }

            fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
                let icons = ["icons/circle-check.svg", "icons/extra.svg"];
                Ok(icons
                    .into_iter()
                    .filter(|icon| path.is_empty() || icon.starts_with(path))
                    .map(SharedString::from)
                    .collect())
            }
        }

        let assets = AppAssets::new(LucideAssets, ExtraAssets);
        let icons = assets.list("icons/").expect("list combined icons");

        assert_eq!(
            icons
                .iter()
                .filter(|icon| icon.as_ref() == "icons/circle-check.svg")
                .count(),
            1
        );
        assert!(icons.contains(&SharedString::from("icons/extra.svg")));
    }
}

use gpui::{AssetSource, SharedString};
use std::{borrow::Cow, collections::BTreeSet};

#[macro_export]
macro_rules! define_lucide_icons {
    ($vis:vis enum $name:ident { $( $variant:ident => $slug:literal ),+ $(,)? }) => {
        #[allow(dead_code)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        $vis enum $name {
            $( $variant, )+
        }

        impl ::gpui_component::IconNamed for $name {
            fn path(self) -> ::gpui::SharedString {
                match self {
                    $( Self::$variant => concat!("icons/", $slug, ".svg").into(), )+
                }
            }
        }

        fn load_lucide_icon(path: &str) -> Option<&'static [u8]> {
            match path {
                $( concat!("icons/", $slug, ".svg") => Some(include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../third_party/lucide/icons/",
                    $slug,
                    ".svg"
                ))), )+
                _ => None,
            }
        }

        fn list_lucide_icons(path: &str) -> Vec<::gpui::SharedString> {
            let icons = [$( ::gpui::SharedString::from(concat!("icons/", $slug, ".svg")), )+];
            icons
                .into_iter()
                .filter(|icon| path.is_empty() || icon.as_ref().starts_with(path))
                .collect()
        }

        #[derive(Default)]
        $vis struct LucideAssets;

        impl ::gpui::AssetSource for LucideAssets {
            fn load(
                &self,
                path: &str,
            ) -> ::gpui::Result<Option<std::borrow::Cow<'static, [u8]>>> {
                if path.is_empty() {
                    return Ok(None);
                }

                Ok(load_lucide_icon(path).map(std::borrow::Cow::Borrowed))
            }

            fn list(&self, path: &str) -> ::gpui::Result<Vec<::gpui::SharedString>> {
                Ok(list_lucide_icons(path))
            }
        }
    };
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
    use super::AppAssets;
    use gpui::{AssetSource, SharedString};
    use gpui_component::IconNamed as _;

    crate::define_lucide_icons!(
        pub enum TestIconName {
            CircleCheck => "circle-check",
            LoaderCircle => "loader-circle",
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
}

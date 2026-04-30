use anyhow::{Result, anyhow};
use gpui::{AssetSource, SharedString};
use gpui_component::IconNamed;
use rust_embed::RustEmbed;
use std::{borrow::Cow, collections::BTreeSet};

pub(crate) const APP_ICON_ASSET_PATH: &str = "build-assets/icon/app-icon.ico";

macro_rules! define_icon_assets {
    ($( $variant:ident => $slug:literal ),+ $(,)?) => {
        #[allow(dead_code)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub(crate) enum IconName {
            $( $variant, )+
        }

        impl IconNamed for IconName {
            fn path(self) -> SharedString {
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

        fn list_lucide_icons(path: &str) -> Vec<SharedString> {
            let icons = [$( SharedString::from(concat!("icons/", $slug, ".svg")), )+];
            icons
                .into_iter()
                .filter(|icon| path.is_empty() || icon.as_ref().starts_with(path))
                .collect()
        }
    };
}

define_icon_assets!(
    ArrowLeft => "arrow-left",
    Bot => "bot",
    BrushCleaning => "brush-cleaning",
    Bug => "bug",
    Check => "check",
    ChevronDown => "chevron-down",
    ChevronRight => "chevron-right",
    ChevronUp => "chevron-up",
    CircleCheck => "circle-check",
    Copy => "copy",
    Edit => "square-pen",
    EllipsisVertical => "ellipsis-vertical",
    Eye => "eye",
    EyeOff => "eye-off",
    FilePen => "file-pen",
    Folder => "folder",
    FolderClosed => "folder-closed",
    FolderCode => "folder-code",
    FolderInput => "folder-input",
    FolderOpen => "folder-open",
    GripVertical => "grip-vertical",
    Info => "info",
    LayoutTemplate => "layout-template",
    Loader2 => "loader-circle",
    OctagonX => "octagon-x",
    PanelLeft => "panel-left",
    Plug => "plug",
    Plus => "plus",
    RefreshCcw => "refresh-ccw",
    Save => "save",
    Search => "search",
    Send => "send",
    Settings => "settings",
    Share => "share",
    Shield => "shield",
    Trash => "trash",
    TriangleAlert => "triangle-alert",
    Upload => "upload",
    UserRound => "user-round",
    X => "x",
);

#[derive(RustEmbed)]
#[folder = "assets"]
#[include = "jpg/*.jpg"]
#[include = "png/*.png"]
#[include = "themes/**/*.json"]
struct AssetsInner;

#[derive(RustEmbed)]
#[folder = "."]
#[include = "build-assets/icon/app-icon.ico"]
struct BuildAssetsInner;

impl AssetSource for AssetsInner {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }

        Self::get(path)
            .map(|f| Some(f.data))
            .ok_or_else(|| anyhow!("could not find asset at path \"{path}\""))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(Self::iter()
            .filter_map(|p| p.starts_with(path).then(|| p.into()))
            .collect())
    }
}

impl AssetSource for BuildAssetsInner {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }

        Self::get(path)
            .map(|f| Some(f.data))
            .ok_or_else(|| anyhow!("could not find asset at path \"{path}\""))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(Self::iter()
            .filter_map(|p| p.starts_with(path).then(|| p.into()))
            .collect())
    }
}

#[derive(Default)]
struct LucideAssets;

impl AssetSource for LucideAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }

        load_lucide_icon(path)
            .map(|icon| Some(Cow::Borrowed(icon)))
            .ok_or_else(|| anyhow!("could not find asset at path \"{path}\""))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(list_lucide_icons(path))
    }
}

pub struct Assets {
    assets: AssetsInner,
    build_assets: BuildAssetsInner,
    lucide_assets: LucideAssets,
    components_assets: gpui_component_assets::Assets,
}

pub(crate) fn bundled_theme_sets() -> Vec<String> {
    AssetsInner::iter()
        .filter(|path| path.starts_with("themes/gpui-component/") && path.ends_with(".json"))
        .filter_map(|path| {
            AssetsInner::get(path.as_ref()).and_then(|file| {
                std::str::from_utf8(file.data.as_ref())
                    .ok()
                    .map(ToOwned::to_owned)
            })
        })
        .collect()
}

impl Default for Assets {
    fn default() -> Self {
        Self {
            assets: AssetsInner,
            build_assets: BuildAssetsInner,
            lucide_assets: LucideAssets,
            components_assets: gpui_component_assets::Assets,
        }
    }
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        self.assets
            .load(path)
            .or_else(|_| self.build_assets.load(path))
            .or_else(|_| self.lucide_assets.load(path))
            .or_else(|_| self.components_assets.load(path))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut names = BTreeSet::new();

        for asset in self.assets.list(path)? {
            names.insert(asset);
        }
        for asset in self.build_assets.list(path)? {
            names.insert(asset);
        }
        for asset in self.lucide_assets.list(path)? {
            names.insert(asset);
        }
        for asset in self.components_assets.list(path)? {
            names.insert(asset);
        }

        Ok(names.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::{APP_ICON_ASSET_PATH, Assets, IconName};
    use gpui::{AssetSource, SharedString};
    use gpui_component::IconNamed;

    #[test]
    fn lucide_icon_paths_are_declared_explicitly() {
        assert_eq!(IconName::Send.path(), SharedString::from("icons/send.svg"));
        assert_eq!(IconName::X.path(), SharedString::from("icons/x.svg"));
        assert_eq!(
            IconName::Loader2.path(),
            SharedString::from("icons/loader-circle.svg")
        );
    }

    #[test]
    fn declared_lucide_icons_load() {
        let assets = Assets::default();

        let send = assets
            .load("icons/send.svg")
            .expect("load send icon")
            .expect("send icon exists");
        let trash = assets
            .load("icons/trash.svg")
            .expect("load trash icon")
            .expect("trash icon exists");

        assert!(!send.is_empty());
        assert!(!trash.is_empty());
    }

    #[test]
    fn app_icon_asset_loads() {
        let assets = Assets::default();
        let icon = assets
            .load(APP_ICON_ASSET_PATH)
            .expect("load app icon")
            .expect("app icon exists");

        assert!(!icon.is_empty());
    }

    #[test]
    fn assets_list_declared_lucide_icons() {
        let assets = Assets::default();
        let icons = assets.list("icons/").expect("list icons");

        assert!(icons.contains(&SharedString::from("icons/send.svg")));
        assert!(icons.contains(&SharedString::from("icons/x.svg")));
    }

    #[test]
    fn undeclared_lucide_catalog_icons_are_not_listed() {
        let assets = Assets::default();
        let icons = assets.list("icons/").expect("list icons");

        assert!(!icons.contains(&SharedString::from("icons/airplay.svg")));
    }
}

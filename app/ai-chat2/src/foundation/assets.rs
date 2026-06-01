use app_assets::define_lucide_icons;
use gpui::{AssetSource, SharedString};
use rust_embed::RustEmbed;
use std::{borrow::Cow, collections::BTreeSet};

pub(crate) const APP_ICON_ASSET_PATH: &str = "build-assets/icon/app-icon.png";

define_lucide_icons!(
    pub(crate) enum IconName {
        Check => "check",
        ChevronDown => "chevron-down",
        ChevronUp => "chevron-up",
        Database => "database",
        FilePen => "file-pen",
        Folder => "folder",
        FolderOpen => "folder-open",
        FolderPlus => "folder-plus",
        FolderX => "folder-x",
        Keyboard => "keyboard",
        Languages => "languages",
        Lightbulb => "lightbulb",
        Palette => "palette",
        Plus => "plus",
        Search => "search",
        Send => "send",
        Settings => "settings",
        Sparkles => "sparkles",
        Trash => "trash",
        X => "x",
    }
);

#[derive(RustEmbed)]
#[folder = "assets"]
#[include = "themes/**/*.json"]
struct AssetsInner;

#[derive(RustEmbed)]
#[folder = "."]
#[include = "build-assets/icon/app-icon.png"]
struct BuildAssets;

impl AssetSource for AssetsInner {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }

        Ok(Self::get(path).map(|file| file.data))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(Self::iter()
            .filter_map(|item| {
                let item = item.into_owned();
                (path.is_empty() || item.starts_with(path)).then(|| item.into())
            })
            .collect())
    }
}

impl AssetSource for BuildAssets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }

        Ok(Self::get(path).map(|file| file.data))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(Self::iter()
            .filter_map(|item| {
                let item = item.into_owned();
                (path.is_empty() || item.starts_with(path)).then(|| item.into())
            })
            .collect())
    }
}

pub(crate) struct Assets {
    assets: AssetsInner,
    build_assets: BuildAssets,
    lucide_assets: LucideAssets,
    component_assets: gpui_component_assets::Assets,
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
            build_assets: BuildAssets,
            lucide_assets: LucideAssets,
            component_assets: gpui_component_assets::Assets,
        }
    }
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }

        for source in [
            &self.assets as &dyn AssetSource,
            &self.build_assets as &dyn AssetSource,
            &self.lucide_assets as &dyn AssetSource,
        ] {
            if let Some(data) = source.load(path)? {
                return Ok(Some(data));
            }
        }

        self.component_assets.load(path)
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        let mut names = BTreeSet::new();

        names.extend(self.assets.list(path)?);
        names.extend(self.build_assets.list(path)?);
        names.extend(self.lucide_assets.list(path)?);
        names.extend(self.component_assets.list(path)?);

        Ok(names.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::{APP_ICON_ASSET_PATH, Assets, IconName, bundled_theme_sets};
    use gpui::{AssetSource, SharedString};
    use gpui_component::IconNamed;

    #[test]
    fn declared_icons_have_lucide_paths() {
        assert_eq!(
            IconName::Database.path(),
            SharedString::from("icons/database.svg")
        );
        assert_eq!(
            IconName::Keyboard.path(),
            SharedString::from("icons/keyboard.svg")
        );
        assert_eq!(
            IconName::Folder.path(),
            SharedString::from("icons/folder.svg")
        );
        assert_eq!(
            IconName::FolderOpen.path(),
            SharedString::from("icons/folder-open.svg")
        );
        assert_eq!(
            IconName::FolderPlus.path(),
            SharedString::from("icons/folder-plus.svg")
        );
        assert_eq!(
            IconName::FolderX.path(),
            SharedString::from("icons/folder-x.svg")
        );
        assert_eq!(
            IconName::Lightbulb.path(),
            SharedString::from("icons/lightbulb.svg")
        );
        assert_eq!(
            IconName::Search.path(),
            SharedString::from("icons/search.svg")
        );
        assert_eq!(IconName::Send.path(), SharedString::from("icons/send.svg"));
        assert_eq!(
            IconName::Trash.path(),
            SharedString::from("icons/trash.svg")
        );
    }

    #[test]
    fn assets_load_app_local_lucide_icons() {
        let assets = Assets::default();
        let icon = assets
            .load("icons/database.svg")
            .expect("load database icon")
            .expect("database icon exists");

        assert!(!icon.is_empty());
    }

    #[test]
    fn assets_embed_bundled_theme_sets() {
        let theme_sets = bundled_theme_sets();

        assert!(theme_sets.len() >= 20);
        assert!(
            theme_sets
                .iter()
                .any(|theme_set| theme_set.contains("Ayu Light"))
        );
    }

    #[test]
    fn assets_list_app_local_lucide_icons() {
        let assets = Assets::default();
        let icons = assets.list("icons/").expect("list icons");

        assert!(icons.contains(&SharedString::from("icons/database.svg")));
        assert!(icons.contains(&SharedString::from("icons/folder.svg")));
        assert!(icons.contains(&SharedString::from("icons/folder-open.svg")));
        assert!(icons.contains(&SharedString::from("icons/folder-plus.svg")));
        assert!(icons.contains(&SharedString::from("icons/folder-x.svg")));
        assert!(icons.contains(&SharedString::from("icons/keyboard.svg")));
        assert!(icons.contains(&SharedString::from("icons/lightbulb.svg")));
        assert!(icons.contains(&SharedString::from("icons/send.svg")));
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
}

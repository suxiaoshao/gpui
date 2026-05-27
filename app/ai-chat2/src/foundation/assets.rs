use app_assets::define_lucide_icons;

define_lucide_icons!(
    pub(crate) enum IconName {
        Database => "database",
        Keyboard => "keyboard",
        Languages => "languages",
        Palette => "palette",
        Settings => "settings",
    }
);

pub(crate) type Assets = app_assets::AppAssets<LucideAssets, gpui_component_assets::Assets>;

#[cfg(test)]
mod tests {
    use super::{Assets, IconName};
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
    fn assets_list_app_local_lucide_icons() {
        let assets = Assets::default();
        let icons = assets.list("icons/").expect("list icons");

        assert!(icons.contains(&SharedString::from("icons/database.svg")));
        assert!(icons.contains(&SharedString::from("icons/keyboard.svg")));
    }
}

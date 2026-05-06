use app_assets::{AppAssets, define_lucide_icons};

define_lucide_icons!(
    pub(crate) enum IconName {
        CircleCheck => "circle-check",
        CirclePause => "circle-pause",
        CirclePlay => "circle-play",
        CircleStop => "circle-stop",
        Clock => "clock",
        Cookie => "cookie",
        Database => "database",
        EyeOff => "eye-off",
        FileText => "file-text",
        Info => "info",
        Link => "link",
        List => "list",
        LoaderCircle => "loader-circle",
        OctagonX => "octagon-x",
        RefreshCcw => "refresh-ccw",
        RotateCcw => "rotate-ccw",
        Settings => "settings",
        TriangleAlert => "triangle-alert",
    }
);

pub(crate) type Assets = AppAssets<LucideAssets>;

#[cfg(test)]
mod tests {
    use super::{Assets, IconName};
    use gpui::{AssetSource, SharedString};
    use gpui_component::IconNamed as _;

    #[test]
    fn declared_lucide_icons_have_expected_paths() {
        assert_eq!(
            IconName::Settings.path(),
            SharedString::from("icons/settings.svg")
        );
        assert_eq!(
            IconName::Cookie.path(),
            SharedString::from("icons/cookie.svg")
        );
        assert_eq!(
            IconName::CirclePlay.path(),
            SharedString::from("icons/circle-play.svg")
        );
    }

    #[test]
    fn declared_lucide_icons_load() {
        let assets = Assets::default();

        let icon = assets
            .load("icons/settings.svg")
            .expect("load settings icon")
            .expect("settings icon exists");

        assert!(!icon.is_empty());
    }
}

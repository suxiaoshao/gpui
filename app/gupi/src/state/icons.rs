use gpui_kit::{App, Global};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum IconTheme {
    #[default]
    Classic,
    ClassicGradient,
    Color,
    ColorGradient,
    Pride,
    Ukraine,
    UkraineGradient,
}
impl IconTheme {
    pub const ALL: [Self; 7] = [
        Self::Classic,
        Self::ClassicGradient,
        Self::Color,
        Self::ColorGradient,
        Self::Pride,
        Self::Ukraine,
        Self::UkraineGradient,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Classic => "icon-theme-classic",
            Self::ClassicGradient => "icon-theme-classic-gradient",
            Self::Color => "icon-theme-color",
            Self::ColorGradient => "icon-theme-color-gradient",
            Self::Pride => "icon-theme-pride",
            Self::Ukraine => "icon-theme-ukraine",
            Self::UkraineGradient => "icon-theme-ukraine-gradient",
        }
    }
    pub fn preview(self) -> &'static str {
        match self {
            Self::Classic => "brand/icon-classic.png",
            Self::ClassicGradient => "brand/icon-classic-gradient.png",
            Self::Color => "brand/icon-color.png",
            Self::ColorGradient => "brand/icon-color-gradient.png",
            Self::Pride => "brand/icon-pride.png",
            Self::Ukraine => "brand/icon-ukraine.png",
            Self::UkraineGradient => "brand/icon-ukraine-gradient.png",
        }
    }
    pub fn logo(self) -> &'static str {
        match self {
            Self::Classic => "brand/logo-black.svg",
            Self::ClassicGradient => "brand/icon-classic-gradient.svg",
            Self::Color => "brand/icon-color.svg",
            Self::ColorGradient => "brand/icon-color-gradient.svg",
            Self::Pride => "brand/icon-pride.svg",
            Self::Ukraine => "brand/icon-ukraine.svg",
            Self::UkraineGradient => "brand/icon-ukraine-gradient.svg",
        }
    }
    #[cfg(target_os = "macos")]
    pub fn native_name(self) -> &'static str {
        match self {
            Self::Classic => "Gupi",
            Self::ClassicGradient => "GupiClassicGradient",
            Self::Color => "GupiColor",
            Self::ColorGradient => "GupiColorGradient",
            Self::Pride => "GupiPride",
            Self::Ukraine => "GupiUkraine",
            Self::UkraineGradient => "GupiUkraineGradient",
        }
    }
    #[cfg(target_os = "macos")]
    pub fn png(self) -> &'static [u8] {
        match self {
            Self::Classic => include_bytes!("../../assets/brand/icon-classic.png"),
            Self::ClassicGradient => include_bytes!("../../assets/brand/icon-classic-gradient.png"),
            Self::Color => include_bytes!("../../assets/brand/icon-color.png"),
            Self::ColorGradient => include_bytes!("../../assets/brand/icon-color-gradient.png"),
            Self::Pride => include_bytes!("../../assets/brand/icon-pride.png"),
            Self::Ukraine => include_bytes!("../../assets/brand/icon-ukraine.png"),
            Self::UkraineGradient => include_bytes!("../../assets/brand/icon-ukraine-gradient.png"),
        }
    }
}
// Last applied preference avoids repeating native side effects. AppConfig owns persistence.
struct Applied(IconTheme);
impl Global for Applied {}
pub(crate) fn current(cx: &App) -> IconTheme {
    cx.try_global::<Applied>()
        .map_or_default(|applied| applied.0)
}
pub(crate) fn apply(theme: IconTheme, cx: &mut App) {
    if cx.try_global::<Applied>().is_some_and(|old| old.0 == theme) {
        return;
    }
    #[cfg(target_os = "macos")]
    {
        // Bundled runs use named Icon Composer assets. cargo run has no Assets.car.
        let name = (theme != IconTheme::Classic).then(|| theme.native_name());
        let result = platform_ext::app::set_application_icon_named(name).and_then(|loaded| {
            if loaded {
                Ok(())
            } else {
                platform_ext::app::set_application_icon_from_bytes(theme.png())
            }
        });
        if let Err(error) = result {
            tracing::warn!(%error, "apply Dock icon theme failed");
        }
    }
    cx.set_global(Applied(theme));
    cx.refresh_windows();
}

use super::config::{AppConfig, ThemeMode};
use gpui_kit::component::{Theme, ThemeMode as Mode, ThemeRegistry};
use gpui_kit::{App, Window};

pub(crate) fn init(cx: &mut App) {
    let registry = ThemeRegistry::global_mut(cx);
    for source in [
        include_str!("../../assets/themes/adventure.json"),
        include_str!("../../assets/themes/alduin.json"),
        include_str!("../../assets/themes/asciinema.json"),
        include_str!("../../assets/themes/aurora.json"),
        include_str!("../../assets/themes/ayu.json"),
        include_str!("../../assets/themes/catppuccin.json"),
        include_str!("../../assets/themes/everforest.json"),
        include_str!("../../assets/themes/fahrenheit.json"),
        include_str!("../../assets/themes/flexoki.json"),
        include_str!("../../assets/themes/gruvbox.json"),
        include_str!("../../assets/themes/harper.json"),
        include_str!("../../assets/themes/hybrid.json"),
        include_str!("../../assets/themes/jellybeans.json"),
        include_str!("../../assets/themes/kibble.json"),
        include_str!("../../assets/themes/macos-classic.json"),
        include_str!("../../assets/themes/matrix.json"),
        include_str!("../../assets/themes/mellifluous.json"),
        include_str!("../../assets/themes/molokai.json"),
        include_str!("../../assets/themes/solarized.json"),
        include_str!("../../assets/themes/spaceduck.json"),
        include_str!("../../assets/themes/tokyonight.json"),
        include_str!("../../assets/themes/twilight.json"),
    ] {
        if let Err(error) = registry.load_themes_from_str(source) {
            tracing::error!(%error, "load bundled theme");
        }
    }
}
pub(crate) fn resolved_mode(mode: ThemeMode, window: &Window) -> Mode {
    match mode {
        ThemeMode::Light => Mode::Light,
        ThemeMode::Dark => Mode::Dark,
        ThemeMode::System => app_theme::component_theme_mode_from_appearance(window.appearance()),
    }
}
pub(crate) fn selected_id(config: &AppConfig, mode: Mode) -> &str {
    match mode {
        Mode::Light => config
            .light_theme
            .as_deref()
            .unwrap_or(app_theme::DEFAULT_LIGHT_THEME_ID),
        Mode::Dark => config
            .dark_theme
            .as_deref()
            .unwrap_or(app_theme::DEFAULT_DARK_THEME_ID),
    }
}
pub(crate) fn apply(config: &AppConfig, window: &mut Window, cx: &mut App) {
    let mode = resolved_mode(config.theme, window);
    let theme = app_theme::resolve_theme_config(
        ThemeRegistry::global(cx),
        mode,
        selected_id(config, mode),
        &[],
    );
    Theme::global_mut(cx).apply_config(&theme);
    cx.refresh_windows();
}

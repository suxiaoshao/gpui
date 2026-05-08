use crate::foundation::assets;
use gpui::App;
use gpui_component::ThemeRegistry;
use tracing::{Level, event};

#[cfg(test)]
pub(crate) use app_theme::SYSTEM_ACCENT_MATERIAL_YOU_THEME_ID;
pub(crate) use app_theme::{
    DEFAULT_CUSTOM_THEME_COLOR, DEFAULT_DARK_THEME_ID, DEFAULT_LIGHT_THEME_ID,
    SystemAccentThemeState, ThemeChoice, is_system_accent_material_you_theme_id,
    material_you_color_from_id, material_you_theme_id, normalize_hex_color, normalize_theme_id,
    preview_theme, resolve_theme_config, system_accent_hsla, theme_choices,
};

pub(crate) fn init(cx: &mut App) {
    let registry = ThemeRegistry::global_mut(cx);
    for theme_set in assets::bundled_theme_sets() {
        if let Err(err) = registry.load_themes_from_str(&theme_set) {
            event!(Level::ERROR, "Failed to load bundled theme set: {}", err);
        }
    }
    app_theme::init_system_accent_theme(cx);
}

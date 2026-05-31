use ai_chat_core::{AppThemeMode, AppThemeSettings};
use gpui::{App, Window, WindowAppearance};
use gpui_component::{Theme, ThemeMode as ComponentThemeMode, ThemeRegistry};
use tracing::{Level, event};

use crate::foundation::assets;

pub(crate) use app_theme::SystemAccentThemeState;

pub(crate) fn init(cx: &mut App) {
    let registry = ThemeRegistry::global_mut(cx);
    for theme_set in assets::bundled_theme_sets() {
        if let Err(err) = registry.load_themes_from_str(&theme_set) {
            event!(Level::ERROR, error = ?err, "failed to load ai-chat2 bundled theme set");
        }
    }
    app_theme::init_system_accent_theme(cx);
}

pub(crate) fn apply_current_theme(window: &mut Window, cx: &mut App) {
    let settings = cx.global::<crate::state::AiChat2AppSettings>().theme();
    let mode = resolved_component_theme_mode(settings, window.appearance());
    let theme_id = theme_id_for_component_mode(settings, mode);
    let custom_theme_colors = normalized_custom_theme_colors(settings);
    let config = {
        let registry = ThemeRegistry::global(cx);
        app_theme::resolve_theme_config(registry, mode, &theme_id, &custom_theme_colors)
    };
    Theme::global_mut(cx).apply_config(&config);
}

pub(crate) fn resolved_component_theme_mode(
    settings: &AppThemeSettings,
    appearance: WindowAppearance,
) -> ComponentThemeMode {
    match (appearance, settings.mode) {
        (_, AppThemeMode::Light)
        | (WindowAppearance::Light | WindowAppearance::VibrantLight, AppThemeMode::System) => {
            ComponentThemeMode::Light
        }
        (_, AppThemeMode::Dark)
        | (WindowAppearance::Dark | WindowAppearance::VibrantDark, AppThemeMode::System) => {
            ComponentThemeMode::Dark
        }
    }
}

fn theme_id_for_component_mode(settings: &AppThemeSettings, mode: ComponentThemeMode) -> String {
    match mode {
        ComponentThemeMode::Light => settings
            .light_theme
            .as_deref()
            .map(app_theme::normalize_theme_id)
            .unwrap_or_else(|| app_theme::DEFAULT_LIGHT_THEME_ID.to_string()),
        ComponentThemeMode::Dark => settings
            .dark_theme
            .as_deref()
            .map(app_theme::normalize_theme_id)
            .unwrap_or_else(|| app_theme::DEFAULT_DARK_THEME_ID.to_string()),
    }
}

fn normalized_custom_theme_colors(settings: &AppThemeSettings) -> Vec<String> {
    let mut colors = settings
        .custom_theme_colors
        .iter()
        .filter_map(|color| app_theme::normalize_hex_color(color))
        .fold(Vec::new(), |mut colors, color| {
            append_custom_theme_color(&mut colors, color);
            colors
        });

    for theme_id in [&settings.light_theme, &settings.dark_theme]
        .into_iter()
        .flatten()
    {
        if let Some(color) = app_theme::material_you_color_from_id(theme_id) {
            append_custom_theme_color(&mut colors, color);
        }
    }

    if colors.is_empty() {
        colors.push(app_theme::DEFAULT_CUSTOM_THEME_COLOR.to_string());
    }

    colors
}

fn append_custom_theme_color(colors: &mut Vec<String>, color: String) {
    if !colors.contains(&color) {
        colors.push(color);
    }
}

#[cfg(test)]
mod tests {
    use super::{normalized_custom_theme_colors, resolved_component_theme_mode};
    use ai_chat_core::{AppThemeMode, AppThemeSettings};
    use gpui::WindowAppearance;
    use gpui_component::ThemeMode as ComponentThemeMode;

    #[test]
    fn theme_mode_respects_explicit_and_system_appearance() {
        let mut settings = AppThemeSettings {
            mode: AppThemeMode::System,
            ..Default::default()
        };
        assert_eq!(
            resolved_component_theme_mode(&settings, WindowAppearance::VibrantDark),
            ComponentThemeMode::Dark
        );
        assert_eq!(
            resolved_component_theme_mode(&settings, WindowAppearance::Light),
            ComponentThemeMode::Light
        );

        settings.mode = AppThemeMode::Light;
        assert_eq!(
            resolved_component_theme_mode(&settings, WindowAppearance::Dark),
            ComponentThemeMode::Light
        );

        settings.mode = AppThemeMode::Dark;
        assert_eq!(
            resolved_component_theme_mode(&settings, WindowAppearance::Light),
            ComponentThemeMode::Dark
        );
    }

    #[test]
    fn custom_theme_colors_are_normalized_and_non_empty() {
        let settings = AppThemeSettings {
            custom_theme_colors: vec!["3271ae".to_string(), "#3271AE".to_string()],
            ..Default::default()
        };

        assert_eq!(normalized_custom_theme_colors(&settings), vec!["#3271AE"]);
        assert_eq!(
            normalized_custom_theme_colors(&AppThemeSettings::default()),
            vec![app_theme::DEFAULT_CUSTOM_THEME_COLOR.to_string()]
        );
    }
}

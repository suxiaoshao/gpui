use gpui::{App, AppContext, Entity, Global, Subscription, Window, WindowAppearance};
use gpui_component::{Theme, ThemeMode as ComponentThemeMode, ThemeRegistry};
use gpui_store::Select;
use jaco_core::{AppThemeMode, AppThemeSettings};
use tracing::{Level, event};

use crate::foundation::assets;

#[derive(Clone, Copy, Default)]
struct SelectThemeSettings;

impl Select<crate::state::config::ConfigOperation> for SelectThemeSettings {
    type Output = AppThemeSettings;

    fn select(&self, operation: &crate::state::config::ConfigOperation) -> Self::Output {
        operation
            .data()
            .map(|config| config.app_settings_payload().theme)
            .unwrap_or_default()
    }
}

pub(crate) use app_theme::SystemAccentThemeState;

struct ThemeRuntime {
    settings: AppThemeSettings,
    appearance: WindowAppearance,
    resolved: Option<ResolvedThemeKey>,
    _subscriptions: Vec<Subscription>,
}

struct ThemeRuntimeGlobal(Entity<ThemeRuntime>);

impl Global for ThemeRuntimeGlobal {}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ResolvedThemeKey {
    settings: AppThemeSettings,
    mode: ComponentThemeMode,
    system_accent: Option<String>,
    system_text_highlight: Option<String>,
}

pub(crate) struct WindowThemeBinding {
    _appearance_subscription: Subscription,
}

pub(crate) fn init(cx: &mut App) {
    let registry = ThemeRegistry::global_mut(cx);
    for theme_set in assets::bundled_theme_sets() {
        if let Err(err) = registry.load_themes_from_str(&theme_set) {
            event!(Level::ERROR, error = ?err, "failed to load jaco bundled theme set");
        }
    }
    app_theme::init_system_accent_theme(cx);
    let settings = crate::state::config::store(cx).read(cx, |operation| {
        operation
            .data()
            .map(|config| config.app_settings_payload().theme)
            .unwrap_or_default()
    });
    let runtime = cx.new(|cx| {
        let config_subscription = crate::state::config::store(cx).observe_select(
            cx,
            SelectThemeSettings,
            |runtime: &mut ThemeRuntime, settings, cx| {
                runtime.settings = settings.clone();
                runtime.apply(cx);
            },
        );
        let accent_subscription =
            cx.observe_global::<SystemAccentThemeState>(|runtime, cx| runtime.apply(cx));
        ThemeRuntime {
            settings,
            appearance: cx.window_appearance(),
            resolved: None,
            _subscriptions: vec![config_subscription, accent_subscription],
        }
    });
    runtime.update(cx, |runtime, cx| runtime.apply(cx));
    cx.set_global(ThemeRuntimeGlobal(runtime));
}

impl ThemeRuntime {
    fn set_window_appearance(&mut self, appearance: WindowAppearance, cx: &mut App) {
        if self.appearance == appearance {
            return;
        }
        self.appearance = appearance;
        self.apply(cx);
    }

    fn apply(&mut self, cx: &mut App) {
        cx.set_window_appearance(native_window_appearance(self.settings.mode));
        if self.settings.mode == AppThemeMode::System {
            self.appearance = cx.window_appearance();
        }
        let mode = resolved_component_theme_mode(&self.settings, self.appearance);
        let theme_id = theme_id_for_component_mode(&self.settings, mode);
        let uses_system_accent = app_theme::is_system_accent_material_you_theme_id(&theme_id);
        let key = ResolvedThemeKey {
            settings: self.settings.clone(),
            mode,
            system_accent: uses_system_accent
                .then(app_theme::system_accent_color)
                .flatten(),
            system_text_highlight: uses_system_accent
                .then(app_theme::system_text_highlight_color)
                .flatten(),
        };
        if self.resolved.as_ref() == Some(&key) {
            return;
        }
        let custom_theme_colors = normalized_custom_theme_colors(&self.settings);
        let config = {
            let registry = ThemeRegistry::global(cx);
            app_theme::resolve_theme_config(registry, mode, &theme_id, &custom_theme_colors)
        };
        Theme::global_mut(cx).apply_config(&config);
        Theme::sync_base(cx);
        self.resolved = Some(key);
        cx.refresh_windows();
    }
}

impl WindowThemeBinding {
    pub(crate) fn new(window: &Window, cx: &mut App) -> Self {
        let runtime = theme_runtime(cx).downgrade();
        let appearance_subscription = window.observe_window_appearance(move |window, cx| {
            let appearance = window.appearance();
            let _ = runtime.update(cx, |runtime, cx| {
                runtime.set_window_appearance(appearance, cx);
            });
        });
        Self {
            _appearance_subscription: appearance_subscription,
        }
    }
}

fn theme_runtime(cx: &App) -> Entity<ThemeRuntime> {
    cx.global::<ThemeRuntimeGlobal>().0.clone()
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

fn native_window_appearance(mode: AppThemeMode) -> Option<WindowAppearance> {
    match mode {
        AppThemeMode::System => None,
        AppThemeMode::Light => Some(WindowAppearance::Light),
        AppThemeMode::Dark => Some(WindowAppearance::Dark),
    }
}

pub(crate) fn normalized_custom_theme_colors(settings: &AppThemeSettings) -> Vec<String> {
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
    use super::{
        native_window_appearance, normalized_custom_theme_colors, resolved_component_theme_mode,
    };
    use gpui::WindowAppearance;
    use gpui_component::ThemeMode as ComponentThemeMode;
    use jaco_core::{AppThemeMode, AppThemeSettings};

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
    fn explicit_theme_mode_controls_native_window_appearance() {
        assert_eq!(native_window_appearance(AppThemeMode::System), None);
        assert_eq!(
            native_window_appearance(AppThemeMode::Light),
            Some(WindowAppearance::Light)
        );
        assert_eq!(
            native_window_appearance(AppThemeMode::Dark),
            Some(WindowAppearance::Dark)
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

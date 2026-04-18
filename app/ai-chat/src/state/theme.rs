use crate::assets;
use gpui::{App, SharedString};
use gpui_component::{
    Theme, ThemeColor, ThemeConfig, ThemeConfigColors, ThemeMode as ComponentThemeMode,
    ThemeRegistry,
};
use material_color_utils::{
    MaterializedScheme, dynamic::variant::Variant, theme_from_color, utils::color_utils::Argb,
};
use std::rc::Rc;
use tracing::{Level, event};

const PRESET_PREFIX: &str = "preset:";
const MATERIAL_YOU_PREFIX: &str = "material-you:";
pub(crate) const DEFAULT_LIGHT_THEME_ID: &str = "preset:Default Light";
pub(crate) const DEFAULT_DARK_THEME_ID: &str = "preset:Default Dark";
pub(crate) const DEFAULT_CUSTOM_THEME_COLOR: &str = "#3271AE";

#[derive(Clone)]
pub(crate) struct ThemeChoice {
    pub(crate) id: String,
    pub(crate) name: SharedString,
    pub(crate) config: Rc<ThemeConfig>,
}

pub(crate) fn init(cx: &mut App) {
    let registry = ThemeRegistry::global_mut(cx);
    for theme_set in assets::bundled_theme_sets() {
        if let Err(err) = registry.load_themes_from_str(&theme_set) {
            event!(Level::ERROR, "Failed to load bundled theme set: {}", err);
        }
    }
}

pub(crate) fn preset_theme_id(name: &str) -> String {
    format!("{PRESET_PREFIX}{name}")
}

pub(crate) fn material_you_theme_id(color: &str) -> Option<String> {
    normalize_hex_color(color).map(|color| format!("{MATERIAL_YOU_PREFIX}{color}"))
}

pub(crate) fn normalize_theme_id(id: &str) -> String {
    if id.starts_with(PRESET_PREFIX) || id.starts_with(MATERIAL_YOU_PREFIX) {
        return id.to_string();
    }
    preset_theme_id(id)
}

pub(crate) fn normalize_hex_color(color: &str) -> Option<String> {
    Argb::from_hex(color).ok().map(|color| color.to_hex())
}

pub(crate) fn material_you_color_from_id(id: &str) -> Option<String> {
    id.strip_prefix(MATERIAL_YOU_PREFIX)
        .and_then(normalize_hex_color)
}

pub(crate) fn theme_choices(
    registry: &ThemeRegistry,
    mode: ComponentThemeMode,
    custom_theme_colors: &[String],
) -> Vec<ThemeChoice> {
    let mut choices = registry
        .sorted_themes()
        .into_iter()
        .filter(|theme| theme.mode == mode)
        .map(|theme| ThemeChoice {
            id: preset_theme_id(&theme.name),
            name: theme.name.clone(),
            config: Rc::clone(theme),
        })
        .collect::<Vec<_>>();

    choices.extend(
        custom_theme_colors
            .iter()
            .filter_map(|color| generated_theme_choice(color, mode)),
    );

    choices
}

pub(crate) fn resolve_theme_config(
    registry: &ThemeRegistry,
    mode: ComponentThemeMode,
    theme_id: &str,
    custom_theme_colors: &[String],
) -> Rc<ThemeConfig> {
    let theme_id = normalize_theme_id(theme_id);
    if let Some(name) = theme_id.strip_prefix(PRESET_PREFIX)
        && let Some(theme) = registry.themes().get(name)
        && theme.mode == mode
    {
        return Rc::clone(theme);
    }

    if let Some(color) = material_you_color_from_id(&theme_id)
        && custom_theme_colors.iter().any(|item| item == &color)
        && let Some(theme) = generated_theme_config(&color, mode)
    {
        return Rc::new(theme);
    }

    match mode {
        ComponentThemeMode::Light => Rc::clone(registry.default_light_theme()),
        ComponentThemeMode::Dark => Rc::clone(registry.default_dark_theme()),
    }
}

pub(crate) fn preview_theme(config: &Rc<ThemeConfig>) -> Theme {
    let default_colors = if config.mode.is_dark() {
        ThemeColor::dark()
    } else {
        ThemeColor::light()
    };
    let mut theme = Theme::from(default_colors.as_ref());
    theme.apply_config(config);
    theme
}

fn generated_theme_choice(color: &str, mode: ComponentThemeMode) -> Option<ThemeChoice> {
    let color = normalize_hex_color(color)?;
    let config = generated_theme_config(&color, mode)?;
    Some(ThemeChoice {
        id: material_you_theme_id(&color)?,
        name: config.name.clone(),
        config: Rc::new(config),
    })
}

fn generated_theme_config(color: &str, mode: ComponentThemeMode) -> Option<ThemeConfig> {
    let color = normalize_hex_color(color)?;
    let source_color = Argb::from_hex(&color).ok()?;
    let theme = theme_from_color(source_color)
        .variant(Variant::TonalSpot)
        .call();
    let scheme = if mode.is_dark() {
        &theme.schemes.dark
    } else {
        &theme.schemes.light
    };

    Some(adapt_material_scheme(&color, mode, scheme))
}

fn adapt_material_scheme(
    seed_color: &str,
    mode: ComponentThemeMode,
    scheme: &MaterializedScheme,
) -> ThemeConfig {
    let mut colors = ThemeConfigColors::default();
    colors.background = Some(hex(scheme.surface));
    colors.foreground = Some(hex(scheme.on_surface));
    colors.border = Some(hex(scheme.outline_variant));
    colors.accent = Some(hex(scheme.secondary_container));
    colors.accent_foreground = Some(hex(scheme.on_secondary_container));
    colors.accordion = Some(hex(scheme.surface_container_low));
    colors.accordion_hover = Some(hex(scheme.surface_container));
    colors.button_primary = Some(hex(scheme.primary));
    colors.button_primary_active = Some(hex(scheme.primary_container));
    colors.button_primary_foreground = Some(hex(scheme.on_primary));
    colors.button_primary_hover = Some(hex(scheme.primary_container));
    colors.group_box = Some(hex(scheme.surface_container_low));
    colors.group_box_foreground = Some(hex(scheme.on_surface));
    colors.caret = Some(hex(scheme.primary));
    colors.chart_1 = Some(hex(scheme.primary));
    colors.chart_2 = Some(hex(scheme.secondary));
    colors.chart_3 = Some(hex(scheme.tertiary));
    colors.chart_4 = Some(hex(scheme.primary_fixed_dim));
    colors.chart_5 = Some(hex(scheme.secondary_fixed_dim));
    colors.danger = Some(hex(scheme.error));
    colors.danger_active = Some(hex(scheme.error_container));
    colors.danger_foreground = Some(hex(scheme.on_error));
    colors.danger_hover = Some(hex(scheme.error_container));
    colors.description_list_label = Some(hex(scheme.surface_container));
    colors.description_list_label_foreground = Some(hex(scheme.on_surface_variant));
    colors.drag_border = Some(hex(scheme.primary));
    colors.drop_target = Some(hex(scheme.primary_container));
    colors.info = Some(hex(scheme.tertiary));
    colors.info_foreground = Some(hex(scheme.on_tertiary));
    colors.info_hover = Some(hex(scheme.tertiary_container));
    colors.info_active = Some(hex(scheme.tertiary_container));
    colors.input = Some(hex(scheme.outline_variant));
    colors.link = Some(hex(scheme.primary));
    colors.link_active = Some(hex(scheme.primary));
    colors.link_hover = Some(hex(scheme.primary));
    colors.list = Some(hex(scheme.surface));
    colors.list_active = Some(hex(scheme.primary_container));
    colors.list_active_border = Some(hex(scheme.primary));
    colors.list_even = Some(hex(scheme.surface_container_lowest));
    colors.list_head = Some(hex(scheme.surface_container_low));
    colors.list_hover = Some(hex(scheme.surface_container));
    colors.muted = Some(hex(scheme.surface_container));
    colors.muted_foreground = Some(hex(scheme.on_surface_variant));
    colors.popover = Some(hex(scheme.surface_container_low));
    colors.popover_foreground = Some(hex(scheme.on_surface));
    colors.primary = Some(hex(scheme.primary));
    colors.primary_active = Some(hex(scheme.primary_container));
    colors.primary_foreground = Some(hex(scheme.on_primary));
    colors.primary_hover = Some(hex(scheme.primary_container));
    colors.progress_bar = Some(hex(scheme.primary));
    colors.ring = Some(hex(scheme.primary));
    colors.scrollbar = Some(hex(scheme.surface));
    colors.scrollbar_thumb = Some(hex(scheme.outline));
    colors.scrollbar_thumb_hover = Some(hex(scheme.outline));
    colors.secondary = Some(hex(scheme.secondary_container));
    colors.secondary_active = Some(hex(scheme.secondary));
    colors.secondary_foreground = Some(hex(scheme.on_secondary_container));
    colors.secondary_hover = Some(hex(scheme.secondary_container));
    colors.selection = Some(hex(scheme.primary));
    colors.sidebar = Some(hex(scheme.surface_container_low));
    colors.sidebar_accent = Some(hex(scheme.secondary_container));
    colors.sidebar_accent_foreground = Some(hex(scheme.on_secondary_container));
    colors.sidebar_border = Some(hex(scheme.outline_variant));
    colors.sidebar_foreground = Some(hex(scheme.on_surface));
    colors.sidebar_primary = Some(hex(scheme.primary));
    colors.sidebar_primary_foreground = Some(hex(scheme.on_primary));
    colors.skeleton = Some(hex(scheme.surface_container_high));
    colors.slider_bar = Some(hex(scheme.primary));
    colors.slider_thumb = Some(hex(scheme.on_primary));
    colors.success = Some(hex(scheme.tertiary));
    colors.success_foreground = Some(hex(scheme.on_tertiary));
    colors.success_hover = Some(hex(scheme.tertiary_container));
    colors.success_active = Some(hex(scheme.tertiary_container));
    colors.switch = Some(hex(scheme.surface_container_highest));
    colors.switch_thumb = Some(hex(scheme.surface));
    colors.tab = Some(hex(scheme.surface_container_low));
    colors.tab_active = Some(hex(scheme.surface));
    colors.tab_active_foreground = Some(hex(scheme.on_surface));
    colors.tab_bar = Some(hex(scheme.surface_container_low));
    colors.tab_bar_segmented = Some(hex(scheme.surface_container));
    colors.tab_foreground = Some(hex(scheme.on_surface_variant));
    colors.table = Some(hex(scheme.surface));
    colors.table_active = Some(hex(scheme.primary_container));
    colors.table_active_border = Some(hex(scheme.primary));
    colors.table_even = Some(hex(scheme.surface_container_lowest));
    colors.table_head = Some(hex(scheme.surface_container_low));
    colors.table_head_foreground = Some(hex(scheme.on_surface_variant));
    colors.table_foot = Some(hex(scheme.surface_container_low));
    colors.table_foot_foreground = Some(hex(scheme.on_surface_variant));
    colors.table_hover = Some(hex(scheme.surface_container));
    colors.table_row_border = Some(hex(scheme.outline_variant));
    colors.title_bar = Some(hex(scheme.surface_container_low));
    colors.title_bar_border = Some(hex(scheme.outline_variant));
    colors.tiles = Some(hex(scheme.surface));
    colors.warning = Some(hex(scheme.tertiary));
    colors.warning_active = Some(hex(scheme.tertiary_container));
    colors.warning_hover = Some(hex(scheme.tertiary_container));
    colors.warning_foreground = Some(hex(scheme.on_tertiary));
    colors.overlay = Some("#0000001F".into());
    colors.window_border = Some(hex(scheme.outline_variant));

    ThemeConfig {
        name: SharedString::from(format!(
            "Material You {} {}",
            seed_color,
            if mode.is_dark() { "Dark" } else { "Light" }
        )),
        mode,
        radius: Some(8),
        radius_lg: Some(8),
        colors,
        ..Default::default()
    }
}

fn hex(color: Argb) -> SharedString {
    color.to_hex().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_you_id_normalizes_hex_color() {
        assert_eq!(
            material_you_theme_id("3271ae"),
            Some("material-you:#3271AE".to_string())
        );
    }

    #[test]
    fn generated_theme_config_has_expected_mode_and_colors() {
        let light = generated_theme_config("#3271AE", ComponentThemeMode::Light)
            .expect("light material theme");
        let dark = generated_theme_config("#3271AE", ComponentThemeMode::Dark)
            .expect("dark material theme");

        assert_eq!(light.mode, ComponentThemeMode::Light);
        assert_eq!(dark.mode, ComponentThemeMode::Dark);
        assert!(light.colors.primary.is_some());
        assert!(dark.colors.primary.is_some());
        assert!(light.colors.background.is_some());
        assert!(dark.colors.background.is_some());
    }
}

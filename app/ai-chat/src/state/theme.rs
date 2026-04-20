use crate::assets;
use gpui::{App, SharedString};
use gpui_component::{
    Theme, ThemeColor, ThemeConfig, ThemeConfigColors, ThemeMode as ComponentThemeMode,
    ThemeRegistry,
};
use material_color_utils::{
    MaterializedScheme,
    blend::blend_functions::Blend,
    dynamic::{
        dynamic_scheme::DynamicScheme, material_dynamic_colors::MaterialDynamicColors,
        variant::Variant,
    },
    hct::hct_color::Hct,
    palettes::tonal_palette::TonalPalette,
    theme_from_color,
    utils::color_utils::Argb,
};
use std::rc::Rc;
use tracing::{Level, event};

const PRESET_PREFIX: &str = "preset:";
const MATERIAL_YOU_PREFIX: &str = "material-you:";
pub(crate) const DEFAULT_LIGHT_THEME_ID: &str = "preset:Default Light";
pub(crate) const DEFAULT_DARK_THEME_ID: &str = "preset:Default Dark";
pub(crate) const DEFAULT_CUSTOM_THEME_COLOR: &str = "#3271AE";
const SEMANTIC_CHROMA: f64 = 60.0;
const INFO_SEED_COLOR: Argb = Argb::from_rgb(0x0E, 0xA5, 0xE9);
const SUCCESS_SEED_COLOR: Argb = Argb::from_rgb(0x22, 0xC5, 0x5E);
const WARNING_SEED_COLOR: Argb = Argb::from_rgb(0xF5, 0x9E, 0x0B);
const CHART_EXTRA_SEED_COLOR: Argb = Argb::from_rgb(0xA8, 0x55, 0xF7);
const MATERIAL_SOFT_DIVIDER_ALPHA: u8 = 0x1F;
const MATERIAL_HOVER_STATE_LAYER_ALPHA: u8 = 0x14;
const MATERIAL_PRESSED_STATE_LAYER_ALPHA: u8 = 0x1A;

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
    if id.starts_with(PRESET_PREFIX) {
        return id.to_string();
    }
    if id.starts_with(MATERIAL_YOU_PREFIX) {
        return material_you_color_from_id(id)
            .and_then(|color| material_you_theme_id(&color))
            .unwrap_or_else(|| id.to_string());
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
    let palette = MaterialPalette::new(mode, scheme);

    apply_material_surface_tokens(&mut colors, scheme, &palette);
    apply_material_control_tokens(&mut colors, scheme, &palette);
    apply_material_interaction_tokens(&mut colors, mode, scheme, &palette);
    apply_material_status_tokens(&mut colors, scheme, &palette);

    colors.overlay = Some(palette.overlay.clone());
    colors.window_border = Some(palette.divider.clone());

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

fn apply_material_surface_tokens(
    colors: &mut ThemeConfigColors,
    scheme: &MaterializedScheme,
    palette: &MaterialPalette,
) {
    colors.background = Some(hex(scheme.surface));
    colors.foreground = Some(hex(scheme.on_surface));
    colors.border = Some(palette.divider.clone());
    colors.accordion = Some(hex(scheme.surface_container_low));
    colors.accordion_hover = Some(hex(scheme.surface_container));
    colors.group_box = Some(hex(scheme.surface_container_low));
    colors.group_box_foreground = Some(hex(scheme.on_surface));
    colors.description_list_label = Some(hex(scheme.surface_container));
    colors.description_list_label_foreground = Some(hex(scheme.on_surface_variant));
    colors.input = Some(hex(scheme.outline_variant));
    colors.list = Some(hex(scheme.surface));
    colors.list_even = Some(hex(scheme.surface_container_lowest));
    colors.list_head = Some(hex(scheme.surface_container_low));
    colors.list_hover = Some(hex(scheme.surface_container));
    colors.muted = Some(hex(scheme.surface_container));
    colors.muted_foreground = Some(hex(scheme.on_surface_variant));
    colors.popover = Some(hex(scheme.surface_container_low));
    colors.popover_foreground = Some(hex(scheme.on_surface));
    colors.scrollbar = Some(hex_alpha(scheme.surface, 0x00));
    colors.scrollbar_thumb = Some(hex_alpha(scheme.outline, 0xE6));
    colors.scrollbar_thumb_hover = Some(hex(scheme.outline));
    colors.sidebar = Some(hex(scheme.surface_container_low));
    colors.sidebar_border = Some(palette.divider.clone());
    colors.sidebar_foreground = Some(hex(scheme.on_surface));
    colors.skeleton = Some(hex(scheme.surface_container_high));
    colors.switch = Some(hex(scheme.surface_container_highest));
    colors.switch_thumb = Some(hex(scheme.surface));
    colors.tab = Some(hex(scheme.surface_container));
    colors.tab_active = Some(hex(scheme.surface));
    colors.tab_active_foreground = Some(hex(scheme.on_surface));
    colors.tab_bar = Some(hex(scheme.surface_container_high));
    colors.tab_bar_segmented = Some(hex(scheme.surface_container_high));
    colors.tab_foreground = Some(hex(scheme.on_surface_variant));
    colors.table = Some(hex(scheme.surface));
    colors.table_even = Some(hex(scheme.surface_container_lowest));
    colors.table_head = Some(hex(scheme.surface_container_low));
    colors.table_head_foreground = Some(hex(scheme.on_surface_variant));
    colors.table_foot = Some(hex(scheme.surface_container_low));
    colors.table_foot_foreground = Some(hex(scheme.on_surface_variant));
    colors.table_hover = Some(hex(scheme.surface_container));
    colors.table_row_border = Some(palette.divider.clone());
    colors.title_bar = Some(hex(scheme.surface_container_highest));
    colors.title_bar_border = Some(palette.divider.clone());
    colors.tiles = Some(hex(scheme.surface_container_low));
}

fn apply_material_control_tokens(
    colors: &mut ThemeConfigColors,
    scheme: &MaterializedScheme,
    palette: &MaterialPalette,
) {
    colors.button_primary = Some(hex(scheme.primary));
    colors.button_primary_active = Some(palette.primary.active.clone());
    colors.button_primary_foreground = Some(hex(scheme.on_primary));
    colors.button_primary_hover = Some(palette.primary.hover.clone());
    colors.caret = Some(hex(scheme.primary));
    colors.link = Some(hex(scheme.primary));
    colors.link_active = Some(hex(scheme.primary));
    colors.link_hover = Some(hex(scheme.primary));
    colors.primary = Some(hex(scheme.primary));
    colors.primary_active = Some(palette.primary.active.clone());
    colors.primary_foreground = Some(hex(scheme.on_primary));
    colors.primary_hover = Some(palette.primary.hover.clone());
    colors.progress_bar = Some(hex(scheme.primary));
    colors.ring = Some(hex(scheme.primary));
    colors.secondary = Some(hex(scheme.secondary_container));
    colors.secondary_active = Some(palette.secondary.active.clone());
    colors.secondary_foreground = Some(hex(scheme.on_secondary_container));
    colors.secondary_hover = Some(palette.secondary.hover.clone());
    colors.sidebar_primary = Some(hex(scheme.primary));
    colors.sidebar_primary_foreground = Some(hex(scheme.on_primary));
    colors.slider_bar = Some(hex(scheme.primary));
    colors.slider_thumb = Some(hex(scheme.on_primary));
}

fn apply_material_interaction_tokens(
    colors: &mut ThemeConfigColors,
    mode: ComponentThemeMode,
    scheme: &MaterializedScheme,
    palette: &MaterialPalette,
) {
    colors.accent = Some(hex(scheme.secondary_container));
    colors.accent_foreground = Some(hex(scheme.on_secondary_container));
    colors.drag_border = Some(hex(scheme.primary));
    colors.drop_target = Some(hex_alpha(
        scheme.primary,
        if mode.is_dark() { 0x26 } else { 0x40 },
    ));
    colors.list_active = Some(hex_alpha(scheme.primary, 0x33));
    colors.list_active_border = Some(hex(scheme.primary));
    colors.list_hover = Some(palette.action_hover(scheme.surface));
    colors.selection = Some(hex_alpha(scheme.primary, 0x66));
    colors.sidebar_accent = Some(hex(scheme.secondary_container));
    colors.sidebar_accent_foreground = Some(hex(scheme.on_secondary_container));
    colors.table_active = Some(hex_alpha(scheme.primary, 0x33));
    colors.table_active_border = Some(hex(scheme.primary));
    colors.table_hover = Some(palette.action_hover(scheme.surface));
}

fn apply_material_status_tokens(
    colors: &mut ThemeConfigColors,
    scheme: &MaterializedScheme,
    palette: &MaterialPalette,
) {
    let primary_roles = material_error_roles_for_palette(scheme, scheme.primary_palette.clone());
    let danger_roles = material_error_roles_for_palette(scheme, scheme.error_palette.clone());
    let info_roles = material_error_roles_for_palette(
        scheme,
        semantic_palette(scheme.source_color, INFO_SEED_COLOR, SEMANTIC_CHROMA),
    );
    let success_roles = material_error_roles_for_palette(
        scheme,
        semantic_palette(scheme.source_color, SUCCESS_SEED_COLOR, SEMANTIC_CHROMA),
    );
    let warning_roles = material_error_roles_for_palette(
        scheme,
        semantic_palette(scheme.source_color, WARNING_SEED_COLOR, SEMANTIC_CHROMA),
    );
    let chart_extra_roles = material_error_roles_for_palette(
        scheme,
        semantic_palette(scheme.source_color, CHART_EXTRA_SEED_COLOR, SEMANTIC_CHROMA),
    );

    colors.chart_1 = Some(hex(primary_roles.color));
    colors.chart_2 = Some(hex(info_roles.color));
    colors.chart_3 = Some(hex(success_roles.color));
    colors.chart_4 = Some(hex(warning_roles.color));
    colors.chart_5 = Some(hex(chart_extra_roles.color));
    colors.chart_bullish = Some(hex(success_roles.color));
    colors.chart_bearish = Some(hex(danger_roles.color));
    colors.danger = Some(hex(danger_roles.color));
    colors.danger_active = Some(palette.role_active(danger_roles.color, danger_roles.on_color));
    colors.danger_foreground = Some(hex(danger_roles.on_color));
    colors.danger_hover = Some(palette.role_hover(danger_roles.color, danger_roles.on_color));
    colors.info = Some(hex(info_roles.color));
    colors.info_foreground = Some(hex(info_roles.on_color));
    colors.info_hover = Some(palette.role_hover(info_roles.color, info_roles.on_color));
    colors.info_active = Some(palette.role_active(info_roles.color, info_roles.on_color));
    colors.success = Some(hex(success_roles.color));
    colors.success_foreground = Some(hex(success_roles.on_color));
    colors.success_hover = Some(palette.role_hover(success_roles.color, success_roles.on_color));
    colors.success_active = Some(palette.role_active(success_roles.color, success_roles.on_color));
    colors.warning = Some(hex(warning_roles.color));
    colors.warning_active = Some(palette.role_active(warning_roles.color, warning_roles.on_color));
    colors.warning_hover = Some(palette.role_hover(warning_roles.color, warning_roles.on_color));
    colors.warning_foreground = Some(hex(warning_roles.on_color));
}

#[derive(Clone)]
struct MaterialPalette {
    divider: SharedString,
    overlay: SharedString,
    on_surface: Argb,
    primary: MaterialInteractiveRole,
    secondary: MaterialInteractiveRole,
    action: MaterialActionPalette,
}

impl MaterialPalette {
    fn new(mode: ComponentThemeMode, scheme: &MaterializedScheme) -> Self {
        let action = MaterialActionPalette::new();

        Self {
            divider: hex_alpha(scheme.on_surface, MATERIAL_SOFT_DIVIDER_ALPHA),
            overlay: if mode.is_dark() {
                "#FFFFFF08".into()
            } else {
                "#0000001F".into()
            },
            on_surface: scheme.on_surface,
            primary: MaterialInteractiveRole::new(scheme.primary, scheme.on_primary, &action),
            secondary: MaterialInteractiveRole::new(
                scheme.secondary_container,
                scheme.on_secondary_container,
                &action,
            ),
            action,
        }
    }

    fn action_hover(&self, container: Argb) -> SharedString {
        state_layer(container, self.on_surface, self.action.hover_alpha)
    }

    fn role_hover(&self, container: Argb, content: Argb) -> SharedString {
        state_layer(container, content, self.action.hover_alpha)
    }

    fn role_active(&self, container: Argb, content: Argb) -> SharedString {
        state_layer(container, content, self.action.active_alpha)
    }
}

#[derive(Clone)]
struct MaterialInteractiveRole {
    hover: SharedString,
    active: SharedString,
}

impl MaterialInteractiveRole {
    fn new(container: Argb, content: Argb, action: &MaterialActionPalette) -> Self {
        Self {
            hover: state_layer(container, content, action.hover_alpha),
            active: state_layer(container, content, action.active_alpha),
        }
    }
}

#[derive(Clone)]
struct MaterialActionPalette {
    hover_alpha: u8,
    active_alpha: u8,
}

impl MaterialActionPalette {
    fn new() -> Self {
        Self {
            hover_alpha: MATERIAL_HOVER_STATE_LAYER_ALPHA,
            active_alpha: MATERIAL_PRESSED_STATE_LAYER_ALPHA,
        }
    }
}

#[derive(Clone, Copy)]
struct MaterialSemanticRoles {
    color: Argb,
    on_color: Argb,
    #[cfg(test)]
    container: Argb,
    #[cfg(test)]
    on_container: Argb,
}

fn material_error_roles_for_palette(
    scheme: &MaterializedScheme,
    error_palette: TonalPalette,
) -> MaterialSemanticRoles {
    let dynamic_scheme = DynamicScheme::new_with_platform_and_spec(
        Hct::from_argb(scheme.source_color),
        scheme.variant,
        scheme.is_dark,
        scheme.contrast_level,
        scheme.platform,
        scheme.spec_version,
        scheme.primary_palette.clone(),
        scheme.secondary_palette.clone(),
        scheme.tertiary_palette.clone(),
        scheme.neutral_palette.clone(),
        scheme.neutral_variant_palette.clone(),
        error_palette,
    );
    let dynamic_colors = MaterialDynamicColors::new_with_spec(scheme.spec_version);
    let color = dynamic_colors.error();
    let on_color = dynamic_colors.on_error();
    #[cfg(test)]
    let container = dynamic_colors.error_container();
    #[cfg(test)]
    let on_container = dynamic_colors.on_error_container();

    MaterialSemanticRoles {
        color: dynamic_scheme.get_argb(&color),
        on_color: dynamic_scheme.get_argb(&on_color),
        #[cfg(test)]
        container: dynamic_scheme.get_argb(&container),
        #[cfg(test)]
        on_container: dynamic_scheme.get_argb(&on_container),
    }
}

fn semantic_palette(source_color: Argb, design_color: Argb, chroma: f64) -> TonalPalette {
    let harmonized = Blend::harmonize(design_color, source_color);
    let hct = Hct::from_argb(harmonized);
    TonalPalette::from_hue_and_chroma(hct.hue(), chroma)
}

fn hex(color: Argb) -> SharedString {
    color.to_hex().into()
}

fn hex_alpha(color: Argb, alpha: u8) -> SharedString {
    format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        color.red(),
        color.green(),
        color.blue(),
        alpha
    )
    .into()
}

fn state_layer(container: Argb, content: Argb, alpha: u8) -> SharedString {
    hex(blend_argb(container, content, alpha))
}

fn blend_argb(container: Argb, overlay: Argb, alpha: u8) -> Argb {
    fn blend_channel(container: u8, overlay: u8, alpha: u8) -> u8 {
        let container = u16::from(container);
        let overlay = u16::from(overlay);
        let alpha = u16::from(alpha);
        let inverse_alpha = 255 - alpha;

        ((overlay * alpha + container * inverse_alpha + 127) / 255) as u8
    }

    Argb::from_rgb(
        blend_channel(container.red(), overlay.red(), alpha),
        blend_channel(container.green(), overlay.green(), alpha),
        blend_channel(container.blue(), overlay.blue(), alpha),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    const TEST_THEME_COLOR: &str = "#3271AE";

    fn material_scheme(mode: ComponentThemeMode) -> MaterializedScheme {
        let source_color = Argb::from_hex(TEST_THEME_COLOR).expect("test theme color");
        let theme = theme_from_color(source_color)
            .variant(Variant::TonalSpot)
            .call();

        if mode.is_dark() {
            theme.schemes.dark
        } else {
            theme.schemes.light
        }
    }

    fn color_from_option(color: &Option<SharedString>) -> Argb {
        Argb::from_hex(color.as_ref().expect("theme color").as_ref()).expect("valid theme color")
    }

    fn relative_luminance(color: Argb) -> f64 {
        fn channel(value: u8) -> f64 {
            let value = f64::from(value) / 255.0;
            if value <= 0.03928 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        }

        0.2126 * channel(color.red())
            + 0.7152 * channel(color.green())
            + 0.0722 * channel(color.blue())
    }

    fn contrast_ratio(first: Argb, second: Argb) -> f64 {
        let first = relative_luminance(first);
        let second = relative_luminance(second);
        let (lighter, darker) = if first > second {
            (first, second)
        } else {
            (second, first)
        };

        (lighter + 0.05) / (darker + 0.05)
    }

    #[test]
    fn material_you_id_normalizes_hex_color() {
        assert_eq!(
            material_you_theme_id("3271ae"),
            Some("material-you:#3271AE".to_string())
        );
    }

    #[test]
    fn normalize_theme_id_canonicalizes_material_you_color() {
        assert_eq!(
            normalize_theme_id("material-you:#aabbcc"),
            "material-you:#AABBCC"
        );
        assert_eq!(
            normalize_theme_id("material-you:aabbcc"),
            "material-you:#AABBCC"
        );
    }

    #[test]
    fn generated_theme_config_has_expected_mode_and_colors() {
        let light = generated_theme_config(TEST_THEME_COLOR, ComponentThemeMode::Light)
            .expect("light material theme");
        let dark = generated_theme_config(TEST_THEME_COLOR, ComponentThemeMode::Dark)
            .expect("dark material theme");

        assert_eq!(light.mode, ComponentThemeMode::Light);
        assert_eq!(dark.mode, ComponentThemeMode::Dark);
        assert!(light.colors.primary.is_some());
        assert!(dark.colors.primary.is_some());
        assert!(light.colors.background.is_some());
        assert!(dark.colors.background.is_some());
    }

    #[test]
    fn generated_dark_material_theme_keeps_secondary_selection_readable() {
        let scheme = material_scheme(ComponentThemeMode::Dark);
        let palette = MaterialPalette::new(ComponentThemeMode::Dark, &scheme);
        let config = generated_theme_config(TEST_THEME_COLOR, ComponentThemeMode::Dark)
            .expect("dark material theme");

        assert_eq!(
            config.colors.secondary,
            Some(hex(scheme.secondary_container))
        );
        assert_eq!(
            config.colors.secondary_hover,
            Some(palette.secondary.hover.clone())
        );
        assert_eq!(
            config.colors.secondary_active,
            Some(palette.secondary.active.clone())
        );
        assert_eq!(
            config.colors.secondary_foreground,
            Some(hex(scheme.on_secondary_container))
        );
        assert_ne!(config.colors.secondary, config.colors.secondary_hover);
        assert_ne!(config.colors.secondary, config.colors.secondary_active);
        assert_ne!(
            config.colors.secondary_hover,
            config.colors.secondary_active
        );

        let foreground = color_from_option(&config.colors.secondary_foreground);
        for background in [
            &config.colors.secondary,
            &config.colors.secondary_hover,
            &config.colors.secondary_active,
        ] {
            assert!(
                contrast_ratio(color_from_option(background), foreground) >= 4.5,
                "secondary states should remain readable"
            );
        }
    }

    #[test]
    fn generated_material_theme_uses_soft_general_dividers() {
        for mode in [ComponentThemeMode::Light, ComponentThemeMode::Dark] {
            let scheme = material_scheme(mode);
            let config = generated_theme_config(TEST_THEME_COLOR, mode).expect("material theme");
            let divider = hex_alpha(scheme.on_surface, MATERIAL_SOFT_DIVIDER_ALPHA);

            for border in [
                &config.colors.border,
                &config.colors.sidebar_border,
                &config.colors.title_bar_border,
                &config.colors.table_row_border,
                &config.colors.window_border,
            ] {
                assert_eq!(border, &Some(divider.clone()));
            }

            assert_ne!(config.colors.border, Some(hex(scheme.outline_variant)));
            assert_eq!(config.colors.input, Some(hex(scheme.outline_variant)));
        }
    }

    #[test]
    fn generated_material_theme_uses_distinct_tab_surface_layers() {
        for mode in [ComponentThemeMode::Light, ComponentThemeMode::Dark] {
            let scheme = material_scheme(mode);
            let config = generated_theme_config(TEST_THEME_COLOR, mode).expect("material theme");
            let colors = &config.colors;

            assert_eq!(
                colors.title_bar,
                Some(hex(scheme.surface_container_highest))
            );
            assert_eq!(colors.tab_bar, Some(hex(scheme.surface_container_high)));
            assert_eq!(colors.tab, Some(hex(scheme.surface_container)));
            assert_eq!(
                colors.tab_bar_segmented,
                Some(hex(scheme.surface_container_high))
            );
            assert_eq!(colors.tab_active, Some(hex(scheme.surface)));
            assert_eq!(colors.tab_active, colors.background);

            assert_ne!(colors.title_bar, colors.tab_bar);
            assert_ne!(colors.tab_bar, colors.tab);
            assert_ne!(colors.tab_bar, colors.tab_active);
            assert_ne!(colors.tab, colors.tab_active);
            assert_ne!(colors.title_bar, colors.tab_active);

            assert!(
                contrast_ratio(
                    color_from_option(&colors.tab),
                    color_from_option(&colors.tab_foreground)
                ) >= 4.5
            );
            assert!(
                contrast_ratio(
                    color_from_option(&colors.tab_active),
                    color_from_option(&colors.tab_active_foreground)
                ) >= 4.5
            );
        }
    }

    #[test]
    fn generated_material_theme_uses_material_state_layers() {
        for mode in [ComponentThemeMode::Light, ComponentThemeMode::Dark] {
            let scheme = material_scheme(mode);
            let config = generated_theme_config(TEST_THEME_COLOR, mode).expect("material theme");
            let danger_roles =
                material_error_roles_for_palette(&scheme, scheme.error_palette.clone());

            assert_eq!(
                config.colors.primary_hover,
                Some(state_layer(
                    scheme.primary,
                    scheme.on_primary,
                    MATERIAL_HOVER_STATE_LAYER_ALPHA
                ))
            );
            assert_eq!(
                config.colors.primary_active,
                Some(state_layer(
                    scheme.primary,
                    scheme.on_primary,
                    MATERIAL_PRESSED_STATE_LAYER_ALPHA
                ))
            );
            assert_eq!(
                config.colors.button_primary_hover,
                config.colors.primary_hover
            );
            assert_eq!(
                config.colors.button_primary_active,
                config.colors.primary_active
            );
            assert_eq!(
                config.colors.secondary_hover,
                Some(state_layer(
                    scheme.secondary_container,
                    scheme.on_secondary_container,
                    MATERIAL_HOVER_STATE_LAYER_ALPHA
                ))
            );
            assert_eq!(
                config.colors.secondary_active,
                Some(state_layer(
                    scheme.secondary_container,
                    scheme.on_secondary_container,
                    MATERIAL_PRESSED_STATE_LAYER_ALPHA
                ))
            );
            assert_eq!(
                config.colors.danger_hover,
                Some(state_layer(
                    danger_roles.color,
                    danger_roles.on_color,
                    MATERIAL_HOVER_STATE_LAYER_ALPHA
                ))
            );
            assert_eq!(
                config.colors.danger_active,
                Some(state_layer(
                    danger_roles.color,
                    danger_roles.on_color,
                    MATERIAL_PRESSED_STATE_LAYER_ALPHA
                ))
            );
        }
    }

    #[test]
    fn material_error_roles_for_palette_matches_scheme_error_roles() {
        for mode in [ComponentThemeMode::Light, ComponentThemeMode::Dark] {
            let scheme = material_scheme(mode);
            let roles = material_error_roles_for_palette(&scheme, scheme.error_palette.clone());

            assert_eq!(roles.color, scheme.error);
            assert_eq!(roles.on_color, scheme.on_error);
            assert_eq!(roles.container, scheme.error_container);
            assert_eq!(roles.on_container, scheme.on_error_container);
        }
    }

    #[test]
    fn generated_material_status_colors_are_semantic_and_readable() {
        for mode in [ComponentThemeMode::Light, ComponentThemeMode::Dark] {
            let config =
                generated_theme_config(TEST_THEME_COLOR, mode).expect("material theme config");
            let colors = &config.colors;

            let status_colors = [
                &colors.danger,
                &colors.info,
                &colors.success,
                &colors.warning,
            ]
            .into_iter()
            .map(|color| color.as_ref().expect("status color").to_string())
            .collect::<HashSet<_>>();
            assert_eq!(status_colors.len(), 4);

            for (background, foreground) in [
                (&colors.danger, &colors.danger_foreground),
                (&colors.info, &colors.info_foreground),
                (&colors.success, &colors.success_foreground),
                (&colors.warning, &colors.warning_foreground),
            ] {
                assert!(
                    contrast_ratio(color_from_option(background), color_from_option(foreground))
                        >= 4.5
                );
            }
        }
    }

    #[test]
    fn generated_material_control_states_are_distinct_and_readable() {
        for mode in [ComponentThemeMode::Light, ComponentThemeMode::Dark] {
            let config =
                generated_theme_config(TEST_THEME_COLOR, mode).expect("material theme config");
            let colors = &config.colors;

            for (name, background, hover, active, foreground) in [
                (
                    "primary",
                    &colors.primary,
                    &colors.primary_hover,
                    &colors.primary_active,
                    &colors.primary_foreground,
                ),
                (
                    "button_primary",
                    &colors.button_primary,
                    &colors.button_primary_hover,
                    &colors.button_primary_active,
                    &colors.button_primary_foreground,
                ),
                (
                    "danger",
                    &colors.danger,
                    &colors.danger_hover,
                    &colors.danger_active,
                    &colors.danger_foreground,
                ),
                (
                    "info",
                    &colors.info,
                    &colors.info_hover,
                    &colors.info_active,
                    &colors.info_foreground,
                ),
                (
                    "success",
                    &colors.success,
                    &colors.success_hover,
                    &colors.success_active,
                    &colors.success_foreground,
                ),
                (
                    "warning",
                    &colors.warning,
                    &colors.warning_hover,
                    &colors.warning_active,
                    &colors.warning_foreground,
                ),
            ] {
                assert_ne!(background, hover, "{name} hover should be derived");
                assert_ne!(background, active, "{name} active should be derived");

                let foreground = color_from_option(foreground);
                for state in [background, hover, active] {
                    assert!(
                        contrast_ratio(color_from_option(state), foreground) >= 4.5,
                        "{name} foreground should remain readable"
                    );
                }
            }
        }
    }

    #[test]
    fn generated_material_theme_fills_component_tokens() {
        let config = generated_theme_config(TEST_THEME_COLOR, ComponentThemeMode::Dark)
            .expect("dark material theme");
        let colors = &config.colors;

        for (name, color) in [
            ("background", &colors.background),
            ("foreground", &colors.foreground),
            ("border", &colors.border),
            ("input", &colors.input),
            ("group_box", &colors.group_box),
            ("muted", &colors.muted),
            ("popover", &colors.popover),
            ("sidebar", &colors.sidebar),
            ("sidebar_border", &colors.sidebar_border),
            ("tab", &colors.tab),
            ("tab_active", &colors.tab_active),
            ("tab_active_foreground", &colors.tab_active_foreground),
            ("tab_bar", &colors.tab_bar),
            ("tab_bar_segmented", &colors.tab_bar_segmented),
            ("tab_foreground", &colors.tab_foreground),
            ("table", &colors.table),
            ("table_hover", &colors.table_hover),
            ("title_bar", &colors.title_bar),
            ("window_border", &colors.window_border),
            ("primary", &colors.primary),
            ("primary_hover", &colors.primary_hover),
            ("primary_active", &colors.primary_active),
            ("secondary", &colors.secondary),
            ("secondary_hover", &colors.secondary_hover),
            ("secondary_active", &colors.secondary_active),
            ("list_active", &colors.list_active),
            ("table_active", &colors.table_active),
            ("danger", &colors.danger),
            ("info", &colors.info),
            ("success", &colors.success),
            ("warning", &colors.warning),
        ] {
            assert!(color.is_some(), "{name} should be generated");
        }
    }

    #[test]
    fn generated_material_chart_colors_are_distinct() {
        let config = generated_theme_config(TEST_THEME_COLOR, ComponentThemeMode::Light)
            .expect("light material theme");
        let colors = &config.colors;
        let chart_colors = [
            &colors.chart_1,
            &colors.chart_2,
            &colors.chart_3,
            &colors.chart_4,
            &colors.chart_5,
        ]
        .into_iter()
        .map(|color| color.as_ref().expect("chart color").to_string())
        .collect::<HashSet<_>>();

        assert_eq!(chart_colors.len(), 5);
        assert_eq!(colors.chart_bullish, colors.success);
        assert_eq!(colors.chart_bearish, colors.danger);
    }

    #[test]
    fn generated_material_theme_uses_translucent_selection_tokens() {
        let scheme = material_scheme(ComponentThemeMode::Dark);
        let config = generated_theme_config(TEST_THEME_COLOR, ComponentThemeMode::Dark)
            .expect("dark material theme");

        assert_eq!(
            config.colors.list_active,
            Some(hex_alpha(scheme.primary, 0x33))
        );
        assert_eq!(
            config.colors.table_active,
            Some(hex_alpha(scheme.primary, 0x33))
        );
        assert_eq!(
            config.colors.selection,
            Some(hex_alpha(scheme.primary, 0x66))
        );

        let theme = preview_theme(&std::rc::Rc::new(config));
        assert!(theme.list_active.a <= 0.2);
        assert!(theme.table_active.a <= 0.2);
        assert!(theme.selection.a <= 0.3);
    }
}

mod material;

#[cfg(feature = "system-accent")]
use gpui::BorrowAppContext;
use gpui::{App, Global, Hsla, SharedString, Task, Window, WindowAppearance};
use gpui_component::{
    Colorize, Theme, ThemeColor, ThemeConfig, ThemeConfigColors, ThemeMode as ComponentThemeMode,
    ThemeRegistry, highlighter::HighlightThemeStyle,
};
use material::adapt_material_scheme;
#[cfg(test)]
use material::{hex, hex_alpha, material_semantic_roles_for_palette, state_layer};
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
#[cfg(feature = "system-accent")]
use platform_ext::appearance::{SystemAccentColorObserver, observe_system_accent_color_changes};
use serde_json::{Map, Value, json};
use std::rc::Rc;

const PRESET_PREFIX: &str = "preset:";
const MATERIAL_YOU_PREFIX: &str = "material-you:";
pub const SYSTEM_ACCENT_MATERIAL_YOU_THEME_ID: &str = "material-you:system-accent";
pub const DEFAULT_LIGHT_THEME_ID: &str = "preset:Default Light";
pub const DEFAULT_DARK_THEME_ID: &str = "preset:Default Dark";
pub const DEFAULT_CUSTOM_THEME_COLOR: &str = "#3271AE";
const SEMANTIC_CHROMA: f64 = 60.0;
const INFO_SEED_COLOR: Argb = Argb::from_rgb(0x0E, 0xA5, 0xE9);
const SUCCESS_SEED_COLOR: Argb = Argb::from_rgb(0x22, 0xC5, 0x5E);
const WARNING_SEED_COLOR: Argb = Argb::from_rgb(0xF5, 0x9E, 0x0B);
const CHART_EXTRA_SEED_COLOR: Argb = Argb::from_rgb(0xA8, 0x55, 0xF7);
const SYNTAX_KEYWORD_SEED_COLOR: Argb = Argb::from_rgb(0xD9, 0x46, 0xEF);
const SYNTAX_FUNCTION_SEED_COLOR: Argb = Argb::from_rgb(0x63, 0x66, 0xF1);
const SYNTAX_TYPE_SEED_COLOR: Argb = Argb::from_rgb(0xEA, 0xB3, 0x08);
const SYNTAX_PROPERTY_SEED_COLOR: Argb = Argb::from_rgb(0x06, 0xB6, 0xD4);
const SYNTAX_PROPERTY_CHROMA: f64 = 54.0;
const SYNTAX_ATTRIBUTE_SEED_COLOR: Argb = Argb::from_rgb(0xF4, 0x3F, 0x5E);
const SYNTAX_ATTRIBUTE_CHROMA: f64 = 36.0;
const SYNTAX_TAG_SEED_COLOR: Argb = Argb::from_rgb(0xEC, 0x48, 0x99);
const SYNTAX_TAG_CHROMA: f64 = 84.0;
const SYNTAX_STRING_SEED_COLOR: Argb = SUCCESS_SEED_COLOR;
const SYNTAX_CONSTANT_SEED_COLOR: Argb = Argb::from_rgb(0xF9, 0x73, 0x16);
const SYNTAX_CONSTANT_CHROMA: f64 = 78.0;
const MATERIAL_SOFT_DIVIDER_ALPHA: u8 = 0x1F;
const MATERIAL_HOVER_STATE_LAYER_ALPHA: u8 = 0x14;
const MATERIAL_PRESSED_STATE_LAYER_ALPHA: u8 = 0x1A;
const MATERIAL_EDITOR_INVISIBLE_ALPHA: u8 = 0x66;

#[derive(Clone)]
pub struct ThemeChoice {
    pub id: String,
    pub name: SharedString,
    pub config: Rc<ThemeConfig>,
}

pub struct SystemAccentThemeState {
    #[cfg(feature = "system-accent")]
    _observer: Option<SystemAccentColorObserver>,
    _task: Option<Task<()>>,
    #[cfg_attr(not(feature = "system-accent"), allow(dead_code))]
    color: Option<String>,
    #[cfg_attr(not(feature = "system-accent"), allow(dead_code))]
    text_highlight_color: Option<String>,
}

impl Global for SystemAccentThemeState {}

pub fn init(cx: &mut App) {
    init_system_accent_theme(cx);
}

pub fn preset_theme_id(name: &str) -> String {
    format!("{PRESET_PREFIX}{name}")
}

pub fn material_you_theme_id(color: &str) -> Option<String> {
    normalize_hex_color(color).map(|color| format!("{MATERIAL_YOU_PREFIX}{color}"))
}

pub fn normalize_theme_id(id: &str) -> String {
    if id.starts_with(PRESET_PREFIX) {
        return id.to_string();
    }
    if is_system_accent_material_you_theme_id(id) {
        return SYSTEM_ACCENT_MATERIAL_YOU_THEME_ID.to_string();
    }
    if id.starts_with(MATERIAL_YOU_PREFIX) {
        return material_you_color_from_id(id)
            .and_then(|color| material_you_theme_id(&color))
            .unwrap_or_else(|| id.to_string());
    }
    preset_theme_id(id)
}

pub fn normalize_hex_color(color: &str) -> Option<String> {
    Argb::from_hex(color).ok().map(|color| color.to_hex())
}

pub fn material_you_color_from_id(id: &str) -> Option<String> {
    id.strip_prefix(MATERIAL_YOU_PREFIX)
        .and_then(normalize_hex_color)
}

pub fn is_system_accent_material_you_theme_id(id: &str) -> bool {
    id == SYSTEM_ACCENT_MATERIAL_YOU_THEME_ID
}

pub fn system_accent_color() -> Option<String> {
    #[cfg(feature = "system-accent")]
    {
        platform_ext::appearance::system_accent_color().map(|color| color.to_hex())
    }

    #[cfg(not(feature = "system-accent"))]
    {
        None
    }
}

pub fn system_text_highlight_color() -> Option<String> {
    #[cfg(feature = "system-accent")]
    {
        platform_ext::appearance::system_text_highlight_color().map(|color| color.to_hex())
    }

    #[cfg(not(feature = "system-accent"))]
    {
        None
    }
}

pub fn system_accent_hsla() -> Option<Hsla> {
    system_accent_color().and_then(|color| Hsla::parse_hex(&color).ok())
}

pub fn theme_choices(
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

    if let Some(choice) = system_accent_theme_choice(mode) {
        choices.push(choice);
    }

    choices
}

pub fn resolve_theme_config(
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

    if is_system_accent_material_you_theme_id(&theme_id)
        && let Some(theme) = system_accent_theme_config(mode)
    {
        return Rc::new(theme);
    }

    match mode {
        ComponentThemeMode::Light => Rc::clone(registry.default_light_theme()),
        ComponentThemeMode::Dark => Rc::clone(registry.default_dark_theme()),
    }
}

pub fn preview_theme(config: &Rc<ThemeConfig>) -> Theme {
    let default_colors = if config.mode.is_dark() {
        ThemeColor::dark()
    } else {
        ThemeColor::light()
    };
    let mut theme = Theme::from(default_colors.as_ref());
    theme.apply_config(config);
    theme
}

pub fn init_system_accent_theme(cx: &mut App) {
    #[cfg(feature = "system-accent")]
    {
        let (tx, rx) = smol::channel::bounded(1);
        let observer = observe_system_accent_color_changes(move || {
            let _ = tx.try_send(());
        });
        let color = system_accent_color();
        let text_highlight_color = system_text_highlight_color();
        let task = observer.as_ref().map(|_| {
            cx.spawn(async move |cx| {
                while rx.recv().await.is_ok() {
                    let next_color = system_accent_color();
                    let next_text_highlight_color = system_text_highlight_color();
                    cx.update(|cx| {
                        let should_update = {
                            let state = cx.global::<SystemAccentThemeState>();
                            system_accent_theme_colors_changed(
                                &state.color,
                                &state.text_highlight_color,
                                &next_color,
                                &next_text_highlight_color,
                            )
                        };

                        if should_update {
                            cx.update_global::<SystemAccentThemeState, _>(|state, _cx| {
                                state.color = next_color;
                                state.text_highlight_color = next_text_highlight_color;
                            });
                        }
                    });
                }
            })
        });

        cx.set_global(SystemAccentThemeState {
            _observer: observer,
            _task: task,
            color,
            text_highlight_color,
        });
    }

    #[cfg(not(feature = "system-accent"))]
    cx.set_global(SystemAccentThemeState {
        _task: None,
        color: None,
        text_highlight_color: None,
    });
}

pub fn system_accent_color_changed(current: &Option<String>, next: &Option<String>) -> bool {
    current != next
}

fn system_accent_theme_colors_changed(
    current_accent: &Option<String>,
    current_text_highlight: &Option<String>,
    next_accent: &Option<String>,
    next_text_highlight: &Option<String>,
) -> bool {
    system_accent_color_changed(current_accent, next_accent)
        || current_text_highlight != next_text_highlight
}

pub fn generated_theme_choice(color: &str, mode: ComponentThemeMode) -> Option<ThemeChoice> {
    let color = normalize_hex_color(color)?;
    let config = generated_theme_config(&color, mode)?;
    Some(ThemeChoice {
        id: material_you_theme_id(&color)?,
        name: config.name.clone(),
        config: Rc::new(config),
    })
}

pub fn system_accent_theme_choice(mode: ComponentThemeMode) -> Option<ThemeChoice> {
    let config = system_accent_theme_config(mode)?;
    Some(ThemeChoice {
        id: SYSTEM_ACCENT_MATERIAL_YOU_THEME_ID.to_string(),
        name: config.name.clone(),
        config: Rc::new(config),
    })
}

pub fn system_accent_theme_config(mode: ComponentThemeMode) -> Option<ThemeConfig> {
    let color = system_accent_color()?;
    let mut config = generated_theme_config(&color, mode)?;
    apply_system_text_highlight_selection(&mut config, system_text_highlight_color());
    config.name = SharedString::from(format!(
        "System Accent Material You {}",
        if mode.is_dark() { "Dark" } else { "Light" }
    ));
    Some(config)
}

fn apply_system_text_highlight_selection(
    config: &mut ThemeConfig,
    text_highlight_color: Option<String>,
) {
    if let Some(color) = text_highlight_color {
        config.colors.selection = Some(color.into());
    }
}

pub fn component_theme_mode_from_appearance(appearance: WindowAppearance) -> ComponentThemeMode {
    match appearance {
        WindowAppearance::Light | WindowAppearance::VibrantLight => ComponentThemeMode::Light,
        WindowAppearance::Dark | WindowAppearance::VibrantDark => ComponentThemeMode::Dark,
    }
}

pub fn fixed_system_accent_theme_config(mode: ComponentThemeMode) -> Rc<ThemeConfig> {
    Rc::new(
        system_accent_theme_config(mode)
            .or_else(|| generated_theme_config(DEFAULT_CUSTOM_THEME_COLOR, mode))
            .expect("default Material You seed color should be valid"),
    )
}

pub fn apply_fixed_system_accent_theme(window: &mut Window, cx: &mut App) {
    let mode = component_theme_mode_from_appearance(window.appearance());
    let config = fixed_system_accent_theme_config(mode);
    Theme::global_mut(cx).apply_config(&config);
    Theme::sync_base(cx);
}

pub fn generated_theme_config(color: &str, mode: ComponentThemeMode) -> Option<ThemeConfig> {
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

#[cfg(test)]
mod tests;

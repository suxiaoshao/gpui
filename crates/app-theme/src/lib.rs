#[cfg(feature = "system-accent")]
use gpui::BorrowAppContext;
use gpui::{App, Global, Hsla, SharedString, Task, Window, WindowAppearance};
use gpui_component::{
    Colorize, Theme, ThemeColor, ThemeConfig, ThemeConfigColors, ThemeMode as ComponentThemeMode,
    ThemeRegistry, highlighter::HighlightThemeStyle,
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

fn adapt_material_scheme(
    seed_color: &str,
    mode: ComponentThemeMode,
    scheme: &MaterializedScheme,
) -> ThemeConfig {
    let palette = MaterialPalette::new(mode, scheme);
    let code = MaterialCodePalette::new(scheme);
    let editor = MaterialEditorChrome::new(scheme, &code);
    let colors = build_material_theme_colors(mode, scheme, &palette, &code);
    let highlight = material_highlight_theme_style(scheme, &code, &editor);

    ThemeConfig {
        name: SharedString::from(format!(
            "Material You {} {}",
            seed_color,
            if mode.is_dark() { "Dark" } else { "Light" }
        )),
        mode,
        colors: colors.into(),
        highlight: Some(highlight),
        ..Default::default()
    }
}

#[derive(Clone)]
struct MaterialCodePalette {
    plain_text: SharedString,
    muted_text: SharedString,
    keyword: SharedString,
    keyword_background: SharedString,
    function: SharedString,
    type_: SharedString,
    property: SharedString,
    attribute_link: SharedString,
    tag: SharedString,
    string: SharedString,
    constant: SharedString,
}

impl MaterialCodePalette {
    fn new(scheme: &MaterializedScheme) -> Self {
        let keyword = material_semantic_roles_for_seed(scheme, SYNTAX_KEYWORD_SEED_COLOR).color;

        Self {
            plain_text: hex(scheme.on_surface),
            muted_text: hex(scheme.on_surface_variant),
            keyword: hex(keyword),
            keyword_background: hex_alpha(keyword, MATERIAL_PRESSED_STATE_LAYER_ALPHA),
            function: hex(
                material_semantic_roles_for_seed(scheme, SYNTAX_FUNCTION_SEED_COLOR).color,
            ),
            type_: hex(material_semantic_roles_for_seed(scheme, SYNTAX_TYPE_SEED_COLOR).color),
            property: hex(material_semantic_roles_for_seed_and_chroma(
                scheme,
                SYNTAX_PROPERTY_SEED_COLOR,
                SYNTAX_PROPERTY_CHROMA,
            )
            .color),
            attribute_link: hex(material_semantic_roles_for_seed_and_chroma(
                scheme,
                SYNTAX_ATTRIBUTE_SEED_COLOR,
                SYNTAX_ATTRIBUTE_CHROMA,
            )
            .color),
            tag: hex(material_semantic_roles_for_seed_and_chroma(
                scheme,
                SYNTAX_TAG_SEED_COLOR,
                SYNTAX_TAG_CHROMA,
            )
            .color),
            string: hex(material_semantic_roles_for_seed(scheme, SYNTAX_STRING_SEED_COLOR).color),
            constant: hex(material_semantic_roles_for_seed_and_chroma(
                scheme,
                SYNTAX_CONSTANT_SEED_COLOR,
                SYNTAX_CONSTANT_CHROMA,
            )
            .color),
        }
    }
}

struct MaterialEditorChrome {
    background: SharedString,
    active_line: SharedString,
    line_number: SharedString,
    active_line_number: SharedString,
    invisible: SharedString,
}

impl MaterialEditorChrome {
    fn new(scheme: &MaterializedScheme, code: &MaterialCodePalette) -> Self {
        Self {
            background: hex(scheme.surface_container_lowest),
            active_line: hex(scheme.surface_container_low),
            line_number: code.muted_text.clone(),
            active_line_number: code.plain_text.clone(),
            invisible: hex_alpha(scheme.on_surface_variant, MATERIAL_EDITOR_INVISIBLE_ALPHA),
        }
    }
}

struct MaterialThemeColors {
    surface: MaterialSurfaceTokens,
    control: MaterialControlTokens,
    interaction: MaterialInteractionTokens,
    status: MaterialStatusTokens,
    code: MaterialCodePalette,
    overlay: SharedString,
    window_border: SharedString,
}

struct MaterialSurfaceTokens {
    background: SharedString,
    border: SharedString,
    accordion: SharedString,
    accordion_hover: SharedString,
    group_box: SharedString,
    group_box_foreground: SharedString,
    group_box_title_foreground: SharedString,
    description_list_label: SharedString,
    description_list_label_foreground: SharedString,
    input: SharedString,
    list: SharedString,
    list_even: SharedString,
    list_head: SharedString,
    muted: SharedString,
    popover: SharedString,
    popover_foreground: SharedString,
    scrollbar: SharedString,
    scrollbar_thumb: SharedString,
    scrollbar_thumb_hover: SharedString,
    sidebar: SharedString,
    sidebar_border: SharedString,
    sidebar_foreground: SharedString,
    skeleton: SharedString,
    status_bar: SharedString,
    status_bar_border: SharedString,
    switch: SharedString,
    switch_thumb: SharedString,
    tab: SharedString,
    tab_active: SharedString,
    tab_active_foreground: SharedString,
    tab_bar: SharedString,
    tab_bar_segmented: SharedString,
    tab_foreground: SharedString,
    table: SharedString,
    table_even: SharedString,
    table_head: SharedString,
    table_head_foreground: SharedString,
    table_foot: SharedString,
    table_foot_foreground: SharedString,
    table_row_border: SharedString,
    title_bar: SharedString,
    title_bar_border: SharedString,
    tiles: SharedString,
}

struct MaterialControlTokens {
    primary: MaterialButtonTokens,
    secondary: MaterialButtonTokens,
    caret: SharedString,
    link: SharedString,
    link_active: SharedString,
    link_hover: SharedString,
    progress_bar: SharedString,
    ring: SharedString,
    sidebar_primary: SharedString,
    sidebar_primary_foreground: SharedString,
    slider_bar: SharedString,
    slider_thumb: SharedString,
}

#[derive(Clone)]
struct MaterialButtonTokens {
    background: SharedString,
    hover: SharedString,
    active: SharedString,
    foreground: SharedString,
}

#[derive(Clone, Copy)]
struct MaterialColorPair {
    color: Argb,
    on_color: Argb,
}

#[derive(Clone, Copy)]
struct MaterialButtonStateLayers {
    hover_alpha: u8,
    pressed_alpha: u8,
}

impl MaterialButtonStateLayers {
    const fn material_3() -> Self {
        Self {
            hover_alpha: MATERIAL_HOVER_STATE_LAYER_ALPHA,
            pressed_alpha: MATERIAL_PRESSED_STATE_LAYER_ALPHA,
        }
    }
}

fn material_button_tokens(
    colors: MaterialColorPair,
    states: MaterialButtonStateLayers,
) -> MaterialButtonTokens {
    MaterialButtonTokens {
        background: hex(colors.color),
        hover: state_layer(colors.color, colors.on_color, states.hover_alpha),
        active: state_layer(colors.color, colors.on_color, states.pressed_alpha),
        foreground: hex(colors.on_color),
    }
}

struct MaterialInteractionTokens {
    accent: SharedString,
    accent_foreground: SharedString,
    drag_border: SharedString,
    drop_target: SharedString,
    list_active: SharedString,
    list_active_border: SharedString,
    list_hover: SharedString,
    selection: SharedString,
    sidebar_accent: SharedString,
    sidebar_accent_foreground: SharedString,
    table_active: SharedString,
    table_active_border: SharedString,
    table_hover: SharedString,
}

struct MaterialStatusTokens {
    chart_1: SharedString,
    chart_2: SharedString,
    chart_3: SharedString,
    chart_4: SharedString,
    chart_5: SharedString,
    chart_bullish: SharedString,
    chart_bearish: SharedString,
    danger: MaterialButtonTokens,
    info: MaterialButtonTokens,
    success: MaterialButtonTokens,
    warning: MaterialButtonTokens,
}

fn build_material_theme_colors(
    mode: ComponentThemeMode,
    scheme: &MaterializedScheme,
    palette: &MaterialPalette,
    code: &MaterialCodePalette,
) -> MaterialThemeColors {
    MaterialThemeColors {
        surface: material_surface_tokens(scheme, palette),
        control: material_control_tokens(scheme),
        interaction: material_interaction_tokens(mode, scheme, palette),
        status: material_status_tokens(scheme),
        code: code.clone(),
        overlay: palette.overlay.clone(),
        window_border: palette.divider.clone(),
    }
}

fn material_surface_tokens(
    scheme: &MaterializedScheme,
    palette: &MaterialPalette,
) -> MaterialSurfaceTokens {
    MaterialSurfaceTokens {
        background: hex(scheme.surface),
        border: palette.divider.clone(),
        accordion: hex(scheme.surface_container_low),
        accordion_hover: hex(scheme.surface_container),
        group_box: hex(scheme.surface_container_low),
        group_box_foreground: hex(scheme.on_surface),
        group_box_title_foreground: hex(scheme.on_surface_variant),
        description_list_label: hex(scheme.surface_container),
        description_list_label_foreground: hex(scheme.on_surface_variant),
        input: hex(scheme.outline_variant),
        list: hex(scheme.surface),
        list_even: hex(scheme.surface_container_lowest),
        list_head: hex(scheme.surface_container_low),
        muted: hex(scheme.surface_container),
        popover: hex(scheme.surface_container_low),
        popover_foreground: hex(scheme.on_surface),
        scrollbar: hex_alpha(scheme.surface, 0x00),
        scrollbar_thumb: hex_alpha(scheme.outline, 0xE6),
        scrollbar_thumb_hover: hex(scheme.outline),
        sidebar: hex(scheme.surface_container_low),
        sidebar_border: palette.divider.clone(),
        sidebar_foreground: hex(scheme.on_surface),
        skeleton: hex(scheme.surface_container_high),
        status_bar: hex(scheme.surface_container_highest),
        status_bar_border: palette.divider.clone(),
        switch: hex(scheme.surface_container_highest),
        switch_thumb: hex(scheme.surface),
        tab: hex(scheme.surface_container),
        tab_active: hex(scheme.surface),
        tab_active_foreground: hex(scheme.on_surface),
        tab_bar: hex(scheme.surface_container_high),
        tab_bar_segmented: hex(scheme.surface_container_high),
        tab_foreground: hex(scheme.on_surface_variant),
        table: hex(scheme.surface),
        table_even: hex(scheme.surface_container_lowest),
        table_head: hex(scheme.surface_container_low),
        table_head_foreground: hex(scheme.on_surface_variant),
        table_foot: hex(scheme.surface_container_low),
        table_foot_foreground: hex(scheme.on_surface_variant),
        table_row_border: palette.divider.clone(),
        title_bar: hex(scheme.surface_container_highest),
        title_bar_border: palette.divider.clone(),
        tiles: hex(scheme.surface_container_low),
    }
}

fn material_control_tokens(scheme: &MaterializedScheme) -> MaterialControlTokens {
    let states = MaterialButtonStateLayers::material_3();
    let primary = material_button_tokens(
        MaterialColorPair {
            color: scheme.primary,
            on_color: scheme.on_primary,
        },
        states,
    );
    let secondary = material_button_tokens(
        MaterialColorPair {
            color: scheme.secondary_container,
            on_color: scheme.on_secondary_container,
        },
        states,
    );

    MaterialControlTokens {
        primary,
        secondary,
        caret: hex(scheme.primary),
        link: hex(scheme.primary),
        link_active: hex(scheme.primary),
        link_hover: hex(scheme.primary),
        progress_bar: hex(scheme.primary),
        ring: hex(scheme.primary),
        sidebar_primary: hex(scheme.primary),
        sidebar_primary_foreground: hex(scheme.on_primary),
        slider_bar: hex(scheme.primary),
        slider_thumb: hex(scheme.on_primary),
    }
}

fn material_interaction_tokens(
    mode: ComponentThemeMode,
    scheme: &MaterializedScheme,
    palette: &MaterialPalette,
) -> MaterialInteractionTokens {
    MaterialInteractionTokens {
        accent: hex(scheme.secondary_container),
        accent_foreground: hex(scheme.on_secondary_container),
        drag_border: hex(scheme.primary),
        drop_target: hex_alpha(scheme.primary, if mode.is_dark() { 0x26 } else { 0x40 }),
        list_active: hex_alpha(scheme.primary, 0x33),
        list_active_border: hex(scheme.primary),
        list_hover: palette.action_hover(scheme.surface),
        selection: hex_alpha(scheme.primary, 0x66),
        sidebar_accent: hex(scheme.secondary_container),
        sidebar_accent_foreground: hex(scheme.on_secondary_container),
        table_active: hex_alpha(scheme.primary, 0x33),
        table_active_border: hex(scheme.primary),
        table_hover: palette.action_hover(scheme.surface),
    }
}

fn material_status_tokens(scheme: &MaterializedScheme) -> MaterialStatusTokens {
    let primary_roles = material_semantic_roles_for_palette(scheme, scheme.primary_palette.clone());
    let danger_roles = material_semantic_roles_for_palette(scheme, scheme.error_palette.clone());
    let info_roles = material_semantic_roles_for_seed(scheme, INFO_SEED_COLOR);
    let success_roles = material_semantic_roles_for_seed(scheme, SUCCESS_SEED_COLOR);
    let warning_roles = material_semantic_roles_for_seed(scheme, WARNING_SEED_COLOR);
    let chart_extra_roles = material_semantic_roles_for_seed(scheme, CHART_EXTRA_SEED_COLOR);
    let states = MaterialButtonStateLayers::material_3();

    MaterialStatusTokens {
        chart_1: hex(primary_roles.color),
        chart_2: hex(info_roles.color),
        chart_3: hex(success_roles.color),
        chart_4: hex(warning_roles.color),
        chart_5: hex(chart_extra_roles.color),
        chart_bullish: hex(success_roles.color),
        chart_bearish: hex(danger_roles.color),
        danger: material_button_tokens(danger_roles, states),
        info: material_button_tokens(info_roles, states),
        success: material_button_tokens(success_roles, states),
        warning: material_button_tokens(warning_roles, states),
    }
}

impl From<MaterialThemeColors> for ThemeConfigColors {
    fn from(tokens: MaterialThemeColors) -> Self {
        let MaterialThemeColors {
            surface,
            control,
            interaction,
            status,
            code,
            overlay,
            window_border,
        } = tokens;
        let mut colors = ThemeConfigColors::default();

        colors.accent = Some(interaction.accent);
        colors.accent_foreground = Some(interaction.accent_foreground);
        colors.accordion = Some(surface.accordion);
        colors.accordion_hover = Some(surface.accordion_hover);
        colors.background = Some(surface.background);
        colors.border = Some(surface.border);
        colors.button = Some(control.secondary.background.clone());
        colors.button_active = Some(control.secondary.active.clone());
        colors.button_foreground = Some(control.secondary.foreground.clone());
        colors.button_hover = Some(control.secondary.hover.clone());
        colors.button_danger = Some(status.danger.background.clone());
        colors.button_danger_active = Some(status.danger.active.clone());
        colors.button_danger_foreground = Some(status.danger.foreground.clone());
        colors.button_danger_hover = Some(status.danger.hover.clone());
        colors.button_info = Some(status.info.background.clone());
        colors.button_info_active = Some(status.info.active.clone());
        colors.button_info_foreground = Some(status.info.foreground.clone());
        colors.button_info_hover = Some(status.info.hover.clone());
        colors.button_primary = Some(control.primary.background.clone());
        colors.button_primary_active = Some(control.primary.active.clone());
        colors.button_primary_foreground = Some(control.primary.foreground.clone());
        colors.button_primary_hover = Some(control.primary.hover.clone());
        colors.button_secondary = Some(control.secondary.background.clone());
        colors.button_secondary_active = Some(control.secondary.active.clone());
        colors.button_secondary_foreground = Some(control.secondary.foreground.clone());
        colors.button_secondary_hover = Some(control.secondary.hover.clone());
        colors.button_success = Some(status.success.background.clone());
        colors.button_success_active = Some(status.success.active.clone());
        colors.button_success_foreground = Some(status.success.foreground.clone());
        colors.button_success_hover = Some(status.success.hover.clone());
        colors.button_warning = Some(status.warning.background.clone());
        colors.button_warning_active = Some(status.warning.active.clone());
        colors.button_warning_foreground = Some(status.warning.foreground.clone());
        colors.button_warning_hover = Some(status.warning.hover.clone());
        colors.caret = Some(control.caret);
        colors.chart_1 = Some(status.chart_1);
        colors.chart_2 = Some(status.chart_2);
        colors.chart_3 = Some(status.chart_3);
        colors.chart_4 = Some(status.chart_4);
        colors.chart_5 = Some(status.chart_5);
        colors.chart_bullish = Some(status.chart_bullish);
        colors.chart_bearish = Some(status.chart_bearish);
        colors.danger = Some(status.danger.background);
        colors.danger_active = Some(status.danger.active);
        colors.danger_foreground = Some(status.danger.foreground);
        colors.danger_hover = Some(status.danger.hover);
        colors.description_list_label = Some(surface.description_list_label);
        colors.description_list_label_foreground = Some(surface.description_list_label_foreground);
        colors.drag_border = Some(interaction.drag_border);
        colors.drop_target = Some(interaction.drop_target);
        colors.foreground = Some(code.plain_text);
        colors.group_box = Some(surface.group_box);
        colors.group_box_foreground = Some(surface.group_box_foreground);
        colors.group_box_title_foreground = Some(surface.group_box_title_foreground);
        colors.info = Some(status.info.background);
        colors.info_active = Some(status.info.active);
        colors.info_foreground = Some(status.info.foreground);
        colors.info_hover = Some(status.info.hover);
        colors.input = Some(surface.input);
        colors.link = Some(control.link);
        colors.link_active = Some(control.link_active);
        colors.link_hover = Some(control.link_hover);
        colors.list = Some(surface.list);
        colors.list_active = Some(interaction.list_active);
        colors.list_active_border = Some(interaction.list_active_border);
        colors.list_even = Some(surface.list_even);
        colors.list_head = Some(surface.list_head);
        colors.list_hover = Some(interaction.list_hover);
        colors.muted = Some(surface.muted);
        colors.muted_foreground = Some(code.muted_text);
        colors.overlay = Some(overlay);
        colors.popover = Some(surface.popover);
        colors.popover_foreground = Some(surface.popover_foreground);
        colors.primary = Some(control.primary.background);
        colors.primary_active = Some(control.primary.active);
        colors.primary_foreground = Some(control.primary.foreground);
        colors.primary_hover = Some(control.primary.hover);
        colors.progress_bar = Some(control.progress_bar);
        colors.ring = Some(control.ring);
        colors.scrollbar = Some(surface.scrollbar);
        colors.scrollbar_thumb = Some(surface.scrollbar_thumb);
        colors.scrollbar_thumb_hover = Some(surface.scrollbar_thumb_hover);
        colors.secondary = Some(control.secondary.background);
        colors.secondary_active = Some(control.secondary.active);
        colors.secondary_foreground = Some(control.secondary.foreground);
        colors.secondary_hover = Some(control.secondary.hover);
        colors.selection = Some(interaction.selection);
        colors.sidebar = Some(surface.sidebar);
        colors.sidebar_accent = Some(interaction.sidebar_accent);
        colors.sidebar_accent_foreground = Some(interaction.sidebar_accent_foreground);
        colors.sidebar_border = Some(surface.sidebar_border);
        colors.sidebar_foreground = Some(surface.sidebar_foreground);
        colors.sidebar_primary = Some(control.sidebar_primary);
        colors.sidebar_primary_foreground = Some(control.sidebar_primary_foreground);
        colors.skeleton = Some(surface.skeleton);
        colors.status_bar = Some(surface.status_bar);
        colors.status_bar_border = Some(surface.status_bar_border);
        colors.slider_bar = Some(control.slider_bar);
        colors.slider_thumb = Some(control.slider_thumb);
        colors.success = Some(status.success.background);
        colors.success_active = Some(status.success.active);
        colors.success_foreground = Some(status.success.foreground);
        colors.success_hover = Some(status.success.hover);
        colors.switch = Some(surface.switch);
        colors.switch_thumb = Some(surface.switch_thumb);
        colors.tab = Some(surface.tab);
        colors.tab_active = Some(surface.tab_active);
        colors.tab_active_foreground = Some(surface.tab_active_foreground);
        colors.tab_bar = Some(surface.tab_bar);
        colors.tab_bar_segmented = Some(surface.tab_bar_segmented);
        colors.tab_foreground = Some(surface.tab_foreground);
        colors.table = Some(surface.table);
        colors.table_active = Some(interaction.table_active);
        colors.table_active_border = Some(interaction.table_active_border);
        colors.table_even = Some(surface.table_even);
        colors.table_head = Some(surface.table_head);
        colors.table_head_foreground = Some(surface.table_head_foreground);
        colors.table_foot = Some(surface.table_foot);
        colors.table_foot_foreground = Some(surface.table_foot_foreground);
        colors.table_hover = Some(interaction.table_hover);
        colors.table_row_border = Some(surface.table_row_border);
        colors.tiles = Some(surface.tiles);
        colors.title_bar = Some(surface.title_bar);
        colors.title_bar_border = Some(surface.title_bar_border);
        colors.warning = Some(status.warning.background);
        colors.warning_active = Some(status.warning.active);
        colors.warning_foreground = Some(status.warning.foreground);
        colors.warning_hover = Some(status.warning.hover);
        colors.window_border = Some(window_border);

        colors
    }
}

fn material_highlight_theme_style(
    scheme: &MaterializedScheme,
    code: &MaterialCodePalette,
    editor: &MaterialEditorChrome,
) -> HighlightThemeStyle {
    let danger_roles = material_semantic_roles_for_palette(scheme, scheme.error_palette.clone());
    let info_roles = material_semantic_roles_for_seed(scheme, INFO_SEED_COLOR);
    let success_roles = material_semantic_roles_for_seed(scheme, SUCCESS_SEED_COLOR);
    let warning_roles = material_semantic_roles_for_seed(scheme, WARNING_SEED_COLOR);

    let mut root = Map::new();
    root.insert("editor.background".into(), json!(editor.background));
    root.insert("editor.foreground".into(), json!(code.plain_text));
    root.insert(
        "editor.active_line.background".into(),
        json!(editor.active_line),
    );
    root.insert("editor.line_number".into(), json!(editor.line_number));
    root.insert(
        "editor.active_line_number".into(),
        json!(editor.active_line_number),
    );
    root.insert("editor.invisible".into(), json!(editor.invisible));
    root.insert("error".into(), json!(hex(danger_roles.color)));
    root.insert(
        "error.background".into(),
        json!(hex_alpha(
            danger_roles.color,
            MATERIAL_PRESSED_STATE_LAYER_ALPHA
        )),
    );
    root.insert("error.border".into(), json!(hex(danger_roles.color)));
    root.insert("warning".into(), json!(hex(warning_roles.color)));
    root.insert(
        "warning.background".into(),
        json!(hex_alpha(
            warning_roles.color,
            MATERIAL_PRESSED_STATE_LAYER_ALPHA
        )),
    );
    root.insert("warning.border".into(), json!(hex(warning_roles.color)));
    root.insert("info".into(), json!(hex(info_roles.color)));
    root.insert(
        "info.background".into(),
        json!(hex_alpha(
            info_roles.color,
            MATERIAL_PRESSED_STATE_LAYER_ALPHA
        )),
    );
    root.insert("info.border".into(), json!(hex(info_roles.color)));
    root.insert("success".into(), json!(hex(success_roles.color)));
    root.insert(
        "success.background".into(),
        json!(hex_alpha(
            success_roles.color,
            MATERIAL_PRESSED_STATE_LAYER_ALPHA
        )),
    );
    root.insert("success.border".into(), json!(hex(success_roles.color)));
    root.insert("hint".into(), json!(code.keyword));
    root.insert("hint.background".into(), json!(code.keyword_background));
    root.insert("hint.border".into(), json!(code.keyword));

    let mut syntax = Map::new();
    insert_syntax_color(&mut syntax, "attribute", code.attribute_link.clone());
    insert_syntax_color(&mut syntax, "boolean", code.constant.clone());
    insert_syntax_color(&mut syntax, "comment", code.muted_text.clone());
    insert_syntax_color(&mut syntax, "comment.doc", code.muted_text.clone());
    insert_syntax_color(&mut syntax, "constant", code.constant.clone());
    insert_syntax_color(&mut syntax, "constructor", code.type_.clone());
    insert_syntax_color(&mut syntax, "embedded", code.plain_text.clone());
    insert_syntax_color(&mut syntax, "function", code.function.clone());
    insert_syntax_color(&mut syntax, "keyword", code.keyword.clone());
    insert_syntax_text_style(
        &mut syntax,
        "link_text",
        code.attribute_link.clone(),
        Some("normal"),
        None,
    );
    insert_syntax_text_style(
        &mut syntax,
        "link_uri",
        code.attribute_link.clone(),
        Some("italic"),
        None,
    );
    insert_syntax_color(&mut syntax, "number", code.constant.clone());
    insert_syntax_color(&mut syntax, "operator", code.muted_text.clone());
    insert_syntax_color(&mut syntax, "property", code.property.clone());
    insert_syntax_color(&mut syntax, "punctuation", code.muted_text.clone());
    insert_syntax_color(&mut syntax, "punctuation.bracket", code.muted_text.clone());
    insert_syntax_color(
        &mut syntax,
        "punctuation.delimiter",
        code.muted_text.clone(),
    );
    insert_syntax_color(
        &mut syntax,
        "punctuation.list_marker",
        code.muted_text.clone(),
    );
    insert_syntax_color(&mut syntax, "punctuation.special", code.constant.clone());
    insert_syntax_color(&mut syntax, "string", code.string.clone());
    insert_syntax_color(&mut syntax, "string.escape", code.string.clone());
    insert_syntax_color(&mut syntax, "string.regex", code.string.clone());
    insert_syntax_color(&mut syntax, "string.special", code.constant.clone());
    insert_syntax_color(&mut syntax, "string.special.symbol", code.constant.clone());
    insert_syntax_color(&mut syntax, "tag", code.tag.clone());
    insert_syntax_color(&mut syntax, "text.literal", code.constant.clone());
    insert_syntax_text_style(&mut syntax, "title", code.function.clone(), None, Some(600));
    insert_syntax_color(&mut syntax, "type", code.type_.clone());
    insert_syntax_color(&mut syntax, "variable", code.plain_text.clone());
    insert_syntax_color(&mut syntax, "variable.special", code.function.clone());
    insert_syntax_color(&mut syntax, "variant", code.type_.clone());
    root.insert("syntax".into(), Value::Object(syntax));

    let mut style: HighlightThemeStyle = serde_json::from_value(Value::Object(root))
        .expect("generated Material You highlight theme should be valid");
    style.editor_background = Some(material_color(&editor.background));
    style.editor_foreground = Some(material_color(&code.plain_text));
    style.editor_active_line = Some(material_color(&editor.active_line));
    style.editor_line_number = Some(material_color(&editor.line_number));
    style.editor_active_line_number = Some(material_color(&editor.active_line_number));
    style.editor_invisible = Some(material_color(&editor.invisible));
    style
}

fn material_color(color: &str) -> Hsla {
    Hsla::parse_hex(color).expect("generated Material You color should be valid")
}

fn insert_syntax_color(syntax: &mut Map<String, Value>, name: &str, color: SharedString) {
    insert_syntax_text_style(syntax, name, color, None, None);
}

fn insert_syntax_text_style(
    syntax: &mut Map<String, Value>,
    name: &str,
    color: SharedString,
    font_style: Option<&str>,
    font_weight: Option<u16>,
) {
    let mut style = Map::new();
    style.insert("color".into(), json!(color));
    if let Some(font_style) = font_style {
        style.insert("font_style".into(), json!(font_style));
    }
    if let Some(font_weight) = font_weight {
        style.insert("font_weight".into(), json!(font_weight));
    }
    syntax.insert(name.into(), Value::Object(style));
}

#[derive(Clone)]
struct MaterialPalette {
    divider: SharedString,
    overlay: SharedString,
    on_surface: Argb,
    button_states: MaterialButtonStateLayers,
}

impl MaterialPalette {
    fn new(mode: ComponentThemeMode, scheme: &MaterializedScheme) -> Self {
        Self {
            divider: hex_alpha(scheme.on_surface, MATERIAL_SOFT_DIVIDER_ALPHA),
            overlay: if mode.is_dark() {
                "#FFFFFF08".into()
            } else {
                "#0000001F".into()
            },
            on_surface: scheme.on_surface,
            button_states: MaterialButtonStateLayers::material_3(),
        }
    }

    fn action_hover(&self, container: Argb) -> SharedString {
        state_layer(container, self.on_surface, self.button_states.hover_alpha)
    }
}

fn material_semantic_roles_for_seed(
    scheme: &MaterializedScheme,
    design_color: Argb,
) -> MaterialColorPair {
    material_semantic_roles_for_seed_and_chroma(scheme, design_color, SEMANTIC_CHROMA)
}

fn material_semantic_roles_for_seed_and_chroma(
    scheme: &MaterializedScheme,
    design_color: Argb,
    chroma: f64,
) -> MaterialColorPair {
    material_semantic_roles_for_palette(
        scheme,
        semantic_palette(scheme.source_color, design_color, chroma),
    )
}

fn material_semantic_roles_for_palette(
    scheme: &MaterializedScheme,
    semantic_palette: TonalPalette,
) -> MaterialColorPair {
    // material-color-utils exposes dynamic role contrast through MaterialDynamicColors.
    // Replacing the DynamicScheme error palette lets app-specific semantic palettes
    // reuse Material's error/on-error tone rules for status, chart, and syntax roles.
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
        semantic_palette,
    );
    let dynamic_colors = MaterialDynamicColors::new_with_spec(scheme.spec_version);
    let color = dynamic_colors.error();
    let on_color = dynamic_colors.on_error();

    MaterialColorPair {
        color: dynamic_scheme.get_argb(&color),
        on_color: dynamic_scheme.get_argb(&on_color),
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
mod tests;

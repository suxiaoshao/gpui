use crate::foundation::assets;
use gpui::{App, BorrowAppContext, Global, Hsla, SharedString, Task};
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
use platform_ext::appearance::{SystemAccentColorObserver, observe_system_accent_color_changes};
use serde_json::{Map, Value, json};
use std::rc::Rc;
use tracing::{Level, event};

const PRESET_PREFIX: &str = "preset:";
const MATERIAL_YOU_PREFIX: &str = "material-you:";
pub(crate) const SYSTEM_ACCENT_MATERIAL_YOU_THEME_ID: &str = "material-you:system-accent";
pub(crate) const DEFAULT_LIGHT_THEME_ID: &str = "preset:Default Light";
pub(crate) const DEFAULT_DARK_THEME_ID: &str = "preset:Default Dark";
pub(crate) const DEFAULT_CUSTOM_THEME_COLOR: &str = "#3271AE";
const SEMANTIC_CHROMA: f64 = 60.0;
const INFO_SEED_COLOR: Argb = Argb::from_rgb(0x0E, 0xA5, 0xE9);
const SUCCESS_SEED_COLOR: Argb = Argb::from_rgb(0x22, 0xC5, 0x5E);
const WARNING_SEED_COLOR: Argb = Argb::from_rgb(0xF5, 0x9E, 0x0B);
const CHART_EXTRA_SEED_COLOR: Argb = Argb::from_rgb(0xA8, 0x55, 0xF7);
const SYNTAX_KEYWORD_SEED_COLOR: Argb = Argb::from_rgb(0xA8, 0x55, 0xF7);
const SYNTAX_FUNCTION_SEED_COLOR: Argb = Argb::from_rgb(0xF9, 0x73, 0x16);
const MATERIAL_SOFT_DIVIDER_ALPHA: u8 = 0x1F;
const MATERIAL_HOVER_STATE_LAYER_ALPHA: u8 = 0x14;
const MATERIAL_PRESSED_STATE_LAYER_ALPHA: u8 = 0x1A;
const MATERIAL_EDITOR_INVISIBLE_ALPHA: u8 = 0x66;

#[derive(Clone)]
pub(crate) struct ThemeChoice {
    pub(crate) id: String,
    pub(crate) name: SharedString,
    pub(crate) config: Rc<ThemeConfig>,
}

pub(crate) struct SystemAccentThemeState {
    _observer: Option<SystemAccentColorObserver>,
    _task: Option<Task<()>>,
    color: Option<String>,
}

impl Global for SystemAccentThemeState {}

pub(crate) fn init(cx: &mut App) {
    let registry = ThemeRegistry::global_mut(cx);
    for theme_set in assets::bundled_theme_sets() {
        if let Err(err) = registry.load_themes_from_str(&theme_set) {
            event!(Level::ERROR, "Failed to load bundled theme set: {}", err);
        }
    }
    init_system_accent_theme(cx);
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

pub(crate) fn normalize_hex_color(color: &str) -> Option<String> {
    Argb::from_hex(color).ok().map(|color| color.to_hex())
}

pub(crate) fn material_you_color_from_id(id: &str) -> Option<String> {
    id.strip_prefix(MATERIAL_YOU_PREFIX)
        .and_then(normalize_hex_color)
}

pub(crate) fn is_system_accent_material_you_theme_id(id: &str) -> bool {
    id == SYSTEM_ACCENT_MATERIAL_YOU_THEME_ID
}

pub(crate) fn system_accent_color() -> Option<String> {
    platform_ext::appearance::system_accent_color().map(|color| color.to_hex())
}

pub(crate) fn system_accent_hsla() -> Option<Hsla> {
    system_accent_color().and_then(|color| Hsla::parse_hex(&color).ok())
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

    if let Some(choice) = system_accent_theme_choice(mode) {
        choices.push(choice);
    }

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

fn init_system_accent_theme(cx: &mut App) {
    let (tx, rx) = smol::channel::bounded(1);
    let observer = observe_system_accent_color_changes(move || {
        let _ = tx.try_send(());
    });
    let color = system_accent_color();
    let task = observer.as_ref().map(|_| {
        cx.spawn(async move |cx| {
            while rx.recv().await.is_ok() {
                let next_color = system_accent_color();
                cx.update(|cx| {
                    if system_accent_color_changed(
                        &cx.global::<SystemAccentThemeState>().color,
                        &next_color,
                    ) {
                        cx.update_global::<SystemAccentThemeState, _>(|state, _cx| {
                            state.color = next_color;
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
    });
}

fn system_accent_color_changed(current: &Option<String>, next: &Option<String>) -> bool {
    current != next
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

fn system_accent_theme_choice(mode: ComponentThemeMode) -> Option<ThemeChoice> {
    let config = system_accent_theme_config(mode)?;
    Some(ThemeChoice {
        id: SYSTEM_ACCENT_MATERIAL_YOU_THEME_ID.to_string(),
        name: config.name.clone(),
        config: Rc::new(config),
    })
}

fn system_accent_theme_config(mode: ComponentThemeMode) -> Option<ThemeConfig> {
    let color = system_accent_color()?;
    let mut config = generated_theme_config(&color, mode)?;
    config.name = SharedString::from(format!(
        "System Accent Material You {}",
        if mode.is_dark() { "Dark" } else { "Light" }
    ));
    Some(config)
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
    let palette = MaterialPalette::new(mode, scheme);
    let colors = build_material_theme_colors(mode, scheme, &palette);
    let highlight = apply_material_highlight_tokens(scheme);

    ThemeConfig {
        name: SharedString::from(format!(
            "Material You {} {}",
            seed_color,
            if mode.is_dark() { "Dark" } else { "Light" }
        )),
        mode,
        radius: Some(8),
        radius_lg: Some(8),
        colors: colors.into(),
        highlight: Some(highlight),
        ..Default::default()
    }
}

struct MaterialThemeColors {
    surface: MaterialSurfaceTokens,
    control: MaterialControlTokens,
    interaction: MaterialInteractionTokens,
    status: MaterialStatusTokens,
    overlay: SharedString,
    window_border: SharedString,
}

struct MaterialSurfaceTokens {
    background: SharedString,
    foreground: SharedString,
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
    muted_foreground: SharedString,
    popover: SharedString,
    popover_foreground: SharedString,
    scrollbar: SharedString,
    scrollbar_thumb: SharedString,
    scrollbar_thumb_hover: SharedString,
    sidebar: SharedString,
    sidebar_border: SharedString,
    sidebar_foreground: SharedString,
    skeleton: SharedString,
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
    button_primary: SharedString,
    button_primary_active: SharedString,
    button_primary_foreground: SharedString,
    button_primary_hover: SharedString,
    caret: SharedString,
    link: SharedString,
    link_active: SharedString,
    link_hover: SharedString,
    primary: SharedString,
    primary_active: SharedString,
    primary_foreground: SharedString,
    primary_hover: SharedString,
    progress_bar: SharedString,
    ring: SharedString,
    secondary: SharedString,
    secondary_active: SharedString,
    secondary_foreground: SharedString,
    secondary_hover: SharedString,
    sidebar_primary: SharedString,
    sidebar_primary_foreground: SharedString,
    slider_bar: SharedString,
    slider_thumb: SharedString,
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
    danger: SharedString,
    danger_active: SharedString,
    danger_foreground: SharedString,
    danger_hover: SharedString,
    info: SharedString,
    info_active: SharedString,
    info_foreground: SharedString,
    info_hover: SharedString,
    success: SharedString,
    success_active: SharedString,
    success_foreground: SharedString,
    success_hover: SharedString,
    warning: SharedString,
    warning_active: SharedString,
    warning_foreground: SharedString,
    warning_hover: SharedString,
}

fn build_material_theme_colors(
    mode: ComponentThemeMode,
    scheme: &MaterializedScheme,
    palette: &MaterialPalette,
) -> MaterialThemeColors {
    MaterialThemeColors {
        surface: material_surface_tokens(scheme, palette),
        control: material_control_tokens(scheme, palette),
        interaction: material_interaction_tokens(mode, scheme, palette),
        status: material_status_tokens(scheme, palette),
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
        foreground: hex(scheme.on_surface),
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
        muted_foreground: hex(scheme.on_surface_variant),
        popover: hex(scheme.surface_container_low),
        popover_foreground: hex(scheme.on_surface),
        scrollbar: hex_alpha(scheme.surface, 0x00),
        scrollbar_thumb: hex_alpha(scheme.outline, 0xE6),
        scrollbar_thumb_hover: hex(scheme.outline),
        sidebar: hex(scheme.surface_container_low),
        sidebar_border: palette.divider.clone(),
        sidebar_foreground: hex(scheme.on_surface),
        skeleton: hex(scheme.surface_container_high),
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

fn material_control_tokens(
    scheme: &MaterializedScheme,
    palette: &MaterialPalette,
) -> MaterialControlTokens {
    MaterialControlTokens {
        button_primary: hex(scheme.primary),
        button_primary_active: palette.primary.active.clone(),
        button_primary_foreground: hex(scheme.on_primary),
        button_primary_hover: palette.primary.hover.clone(),
        caret: hex(scheme.primary),
        link: hex(scheme.primary),
        link_active: hex(scheme.primary),
        link_hover: hex(scheme.primary),
        primary: hex(scheme.primary),
        primary_active: palette.primary.active.clone(),
        primary_foreground: hex(scheme.on_primary),
        primary_hover: palette.primary.hover.clone(),
        progress_bar: hex(scheme.primary),
        ring: hex(scheme.primary),
        secondary: hex(scheme.secondary_container),
        secondary_active: palette.secondary.active.clone(),
        secondary_foreground: hex(scheme.on_secondary_container),
        secondary_hover: palette.secondary.hover.clone(),
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

fn material_status_tokens(
    scheme: &MaterializedScheme,
    palette: &MaterialPalette,
) -> MaterialStatusTokens {
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

    MaterialStatusTokens {
        chart_1: hex(primary_roles.color),
        chart_2: hex(info_roles.color),
        chart_3: hex(success_roles.color),
        chart_4: hex(warning_roles.color),
        chart_5: hex(chart_extra_roles.color),
        chart_bullish: hex(success_roles.color),
        chart_bearish: hex(danger_roles.color),
        danger: hex(danger_roles.color),
        danger_active: palette.role_active(danger_roles.color, danger_roles.on_color),
        danger_foreground: hex(danger_roles.on_color),
        danger_hover: palette.role_hover(danger_roles.color, danger_roles.on_color),
        info: hex(info_roles.color),
        info_foreground: hex(info_roles.on_color),
        info_hover: palette.role_hover(info_roles.color, info_roles.on_color),
        info_active: palette.role_active(info_roles.color, info_roles.on_color),
        success: hex(success_roles.color),
        success_foreground: hex(success_roles.on_color),
        success_hover: palette.role_hover(success_roles.color, success_roles.on_color),
        success_active: palette.role_active(success_roles.color, success_roles.on_color),
        warning: hex(warning_roles.color),
        warning_active: palette.role_active(warning_roles.color, warning_roles.on_color),
        warning_hover: palette.role_hover(warning_roles.color, warning_roles.on_color),
        warning_foreground: hex(warning_roles.on_color),
    }
}

impl From<MaterialThemeColors> for ThemeConfigColors {
    fn from(tokens: MaterialThemeColors) -> Self {
        let MaterialThemeColors {
            surface,
            control,
            interaction,
            status,
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
        colors.button_primary = Some(control.button_primary);
        colors.button_primary_active = Some(control.button_primary_active);
        colors.button_primary_foreground = Some(control.button_primary_foreground);
        colors.button_primary_hover = Some(control.button_primary_hover);
        colors.caret = Some(control.caret);
        colors.chart_1 = Some(status.chart_1);
        colors.chart_2 = Some(status.chart_2);
        colors.chart_3 = Some(status.chart_3);
        colors.chart_4 = Some(status.chart_4);
        colors.chart_5 = Some(status.chart_5);
        colors.chart_bullish = Some(status.chart_bullish);
        colors.chart_bearish = Some(status.chart_bearish);
        colors.danger = Some(status.danger);
        colors.danger_active = Some(status.danger_active);
        colors.danger_foreground = Some(status.danger_foreground);
        colors.danger_hover = Some(status.danger_hover);
        colors.description_list_label = Some(surface.description_list_label);
        colors.description_list_label_foreground = Some(surface.description_list_label_foreground);
        colors.drag_border = Some(interaction.drag_border);
        colors.drop_target = Some(interaction.drop_target);
        colors.foreground = Some(surface.foreground);
        colors.group_box = Some(surface.group_box);
        colors.group_box_foreground = Some(surface.group_box_foreground);
        colors.group_box_title_foreground = Some(surface.group_box_title_foreground);
        colors.info = Some(status.info);
        colors.info_active = Some(status.info_active);
        colors.info_foreground = Some(status.info_foreground);
        colors.info_hover = Some(status.info_hover);
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
        colors.muted_foreground = Some(surface.muted_foreground);
        colors.overlay = Some(overlay);
        colors.popover = Some(surface.popover);
        colors.popover_foreground = Some(surface.popover_foreground);
        colors.primary = Some(control.primary);
        colors.primary_active = Some(control.primary_active);
        colors.primary_foreground = Some(control.primary_foreground);
        colors.primary_hover = Some(control.primary_hover);
        colors.progress_bar = Some(control.progress_bar);
        colors.ring = Some(control.ring);
        colors.scrollbar = Some(surface.scrollbar);
        colors.scrollbar_thumb = Some(surface.scrollbar_thumb);
        colors.scrollbar_thumb_hover = Some(surface.scrollbar_thumb_hover);
        colors.secondary = Some(control.secondary);
        colors.secondary_active = Some(control.secondary_active);
        colors.secondary_foreground = Some(control.secondary_foreground);
        colors.secondary_hover = Some(control.secondary_hover);
        colors.selection = Some(interaction.selection);
        colors.sidebar = Some(surface.sidebar);
        colors.sidebar_accent = Some(interaction.sidebar_accent);
        colors.sidebar_accent_foreground = Some(interaction.sidebar_accent_foreground);
        colors.sidebar_border = Some(surface.sidebar_border);
        colors.sidebar_foreground = Some(surface.sidebar_foreground);
        colors.sidebar_primary = Some(control.sidebar_primary);
        colors.sidebar_primary_foreground = Some(control.sidebar_primary_foreground);
        colors.skeleton = Some(surface.skeleton);
        colors.slider_bar = Some(control.slider_bar);
        colors.slider_thumb = Some(control.slider_thumb);
        colors.success = Some(status.success);
        colors.success_active = Some(status.success_active);
        colors.success_foreground = Some(status.success_foreground);
        colors.success_hover = Some(status.success_hover);
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
        colors.warning = Some(status.warning);
        colors.warning_active = Some(status.warning_active);
        colors.warning_foreground = Some(status.warning_foreground);
        colors.warning_hover = Some(status.warning_hover);
        colors.window_border = Some(window_border);

        colors
    }
}

fn apply_material_highlight_tokens(scheme: &MaterializedScheme) -> HighlightThemeStyle {
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
    let keyword_roles = material_error_roles_for_palette(
        scheme,
        semantic_palette(
            scheme.source_color,
            SYNTAX_KEYWORD_SEED_COLOR,
            SEMANTIC_CHROMA,
        ),
    );
    let function_roles = material_error_roles_for_palette(
        scheme,
        semantic_palette(
            scheme.source_color,
            SYNTAX_FUNCTION_SEED_COLOR,
            SEMANTIC_CHROMA,
        ),
    );

    let editor_background = hex(scheme.surface_container_lowest);
    let editor_foreground = hex(scheme.on_surface);
    let editor_active_line = hex(scheme.surface_container_low);
    let editor_line_number = hex(scheme.on_surface_variant);
    let editor_active_line_number = hex(scheme.on_surface);
    let editor_invisible = hex_alpha(scheme.on_surface_variant, MATERIAL_EDITOR_INVISIBLE_ALPHA);

    let mut root = Map::new();
    root.insert("editor.background".into(), json!(editor_background));
    root.insert("editor.foreground".into(), json!(editor_foreground));
    root.insert(
        "editor.active_line.background".into(),
        json!(editor_active_line),
    );
    root.insert("editor.line_number".into(), json!(editor_line_number));
    root.insert(
        "editor.active_line_number".into(),
        json!(editor_active_line_number),
    );
    root.insert("editor.invisible".into(), json!(editor_invisible));
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
    root.insert("hint".into(), json!(hex(keyword_roles.color)));
    root.insert(
        "hint.background".into(),
        json!(hex_alpha(
            keyword_roles.color,
            MATERIAL_PRESSED_STATE_LAYER_ALPHA
        )),
    );
    root.insert("hint.border".into(), json!(hex(keyword_roles.color)));

    let mut syntax = Map::new();
    insert_syntax_color(&mut syntax, "attribute", hex(info_roles.color));
    insert_syntax_color(&mut syntax, "boolean", hex(warning_roles.color));
    insert_syntax_color(&mut syntax, "comment", hex(scheme.on_surface_variant));
    insert_syntax_color(&mut syntax, "comment.doc", hex(scheme.on_surface_variant));
    insert_syntax_color(&mut syntax, "constant", hex(warning_roles.color));
    insert_syntax_color(&mut syntax, "constructor", hex(info_roles.color));
    insert_syntax_color(&mut syntax, "embedded", editor_foreground.clone());
    insert_syntax_color(&mut syntax, "function", hex(function_roles.color));
    insert_syntax_color(&mut syntax, "keyword", hex(keyword_roles.color));
    insert_syntax_text_style(
        &mut syntax,
        "link_text",
        hex(info_roles.color),
        Some("normal"),
        None,
    );
    insert_syntax_text_style(
        &mut syntax,
        "link_uri",
        hex(info_roles.color),
        Some("italic"),
        None,
    );
    insert_syntax_color(&mut syntax, "number", hex(warning_roles.color));
    insert_syntax_color(&mut syntax, "operator", hex(scheme.on_surface_variant));
    insert_syntax_color(&mut syntax, "property", editor_foreground.clone());
    insert_syntax_color(&mut syntax, "punctuation", hex(scheme.on_surface_variant));
    insert_syntax_color(
        &mut syntax,
        "punctuation.bracket",
        hex(scheme.on_surface_variant),
    );
    insert_syntax_color(
        &mut syntax,
        "punctuation.delimiter",
        hex(scheme.on_surface_variant),
    );
    insert_syntax_color(
        &mut syntax,
        "punctuation.list_marker",
        hex(scheme.on_surface_variant),
    );
    insert_syntax_color(&mut syntax, "punctuation.special", hex(warning_roles.color));
    insert_syntax_color(&mut syntax, "string", hex(success_roles.color));
    insert_syntax_color(&mut syntax, "string.escape", hex(success_roles.color));
    insert_syntax_color(&mut syntax, "string.regex", hex(success_roles.color));
    insert_syntax_color(&mut syntax, "string.special", hex(warning_roles.color));
    insert_syntax_color(
        &mut syntax,
        "string.special.symbol",
        hex(warning_roles.color),
    );
    insert_syntax_color(&mut syntax, "tag", hex(info_roles.color));
    insert_syntax_color(&mut syntax, "text.literal", hex(warning_roles.color));
    insert_syntax_text_style(
        &mut syntax,
        "title",
        hex(function_roles.color),
        None,
        Some(600),
    );
    insert_syntax_color(&mut syntax, "type", hex(info_roles.color));
    insert_syntax_color(&mut syntax, "variable", editor_foreground);
    insert_syntax_color(&mut syntax, "variable.special", hex(function_roles.color));
    insert_syntax_color(&mut syntax, "variant", hex(info_roles.color));
    root.insert("syntax".into(), Value::Object(syntax));

    serde_json::from_value(Value::Object(root))
        .expect("generated Material You highlight theme should be valid")
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
mod tests;

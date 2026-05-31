use crate::{
    components::delete_confirm::{DestructiveAction, open_destructive_confirm_dialog},
    foundation::{I18n, assets::IconName},
    state::{self, AiChat2AppSettings},
};
use ai_chat_core::{AppThemeMode, AppThemeSettings};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Colorize, Sizable, Size, StyledExt,
    button::{Button, ButtonVariants},
    color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState},
    h_flex,
    label::Label,
    v_flex,
};

use super::push_settings_error;

pub(super) struct AppearanceSettingsPage {
    color_picker: Entity<ColorPickerState>,
    selected_color: Hsla,
    _subscriptions: Vec<Subscription>,
}

const SETTINGS_THEME_GRID_WIDTH_CHROME: f32 = 338.;
const THEME_TILE_MIN_WIDTH: f32 = 178.;
const THEME_TILE_MAX_WIDTH: f32 = 220.;
const THEME_TILE_HEIGHT: f32 = 128.;
const THEME_TILE_GAP: f32 = 12.;

#[derive(Clone)]
struct ThemeGridText {
    selected_prefix: SharedString,
    selected_label: SharedString,
    delete_material_theme_label: SharedString,
}

impl AppearanceSettingsPage {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let selected_color = default_custom_color(cx);
        let color_picker =
            cx.new(|cx| ColorPickerState::new(window, cx).default_value(selected_color));
        let _subscriptions = vec![cx.subscribe(&color_picker, Self::subscribe_color_picker)];

        Self {
            color_picker,
            selected_color,
            _subscriptions,
        }
    }

    fn subscribe_color_picker(
        &mut self,
        _: Entity<ColorPickerState>,
        event: &ColorPickerEvent,
        cx: &mut Context<Self>,
    ) {
        if let ColorPickerEvent::Change(Some(color)) = event {
            self.selected_color = *color;
            cx.notify();
        }
    }

    fn update_theme(update: impl FnOnce(&mut AppThemeSettings), window: &mut Window, cx: &mut App) {
        match state::config::update_app_settings(cx, |payload| update(&mut payload.theme)) {
            Ok(_) => {
                cx.refresh_windows();
            }
            Err(err) => {
                let title = cx.global::<I18n>().t("notify-save-settings-failed");
                push_settings_error(window, cx, title, err);
            }
        }
    }

    fn set_theme_mode(mode: AppThemeMode, window: &mut Window, cx: &mut App) {
        Self::update_theme(
            |settings| {
                settings.mode = mode;
            },
            window,
            cx,
        );
    }

    fn set_light_theme(theme_id: String, window: &mut Window, cx: &mut App) {
        Self::update_theme(
            |settings| {
                set_light_theme_id(settings, theme_id);
            },
            window,
            cx,
        );
    }

    fn set_dark_theme(theme_id: String, window: &mut Window, cx: &mut App) {
        Self::update_theme(
            |settings| {
                set_dark_theme_id(settings, theme_id);
            },
            window,
            cx,
        );
    }

    fn add_material_theme(&self, window: &mut Window, cx: &mut App) {
        let color = self.selected_color.to_hex();
        Self::update_theme(
            |settings| {
                append_custom_theme_color(&mut settings.custom_theme_colors, color);
            },
            window,
            cx,
        );
    }

    fn delete_material_theme(theme_id: String, window: &mut Window, cx: &mut App) {
        Self::update_theme(
            |settings| {
                delete_custom_theme_color(settings, &theme_id);
            },
            window,
            cx,
        );
    }

    fn render_mode_button(
        &self,
        id: &'static str,
        label: SharedString,
        mode: AppThemeMode,
        current: AppThemeMode,
    ) -> Button {
        let button = Button::new(format!("appearance-mode-{id}")).label(label);
        let button = if mode == current {
            button.primary()
        } else {
            button.ghost()
        };
        button.on_click(move |_, window, cx| Self::set_theme_mode(mode, window, cx))
    }

    fn render_theme_grid(
        &self,
        title: SharedString,
        text: ThemeGridText,
        mode: gpui_component::ThemeMode,
        selected_id: &str,
        columns: u16,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let settings = cx.global::<AiChat2AppSettings>().theme().clone();
        let custom_theme_colors = custom_theme_colors_for_choices(&settings);
        let deletable_material_theme_ids = settings
            .custom_theme_colors
            .iter()
            .filter_map(|color| app_theme::normalize_hex_color(color))
            .filter_map(|color| app_theme::material_you_theme_id(&color))
            .collect::<Vec<_>>();
        let registry = gpui_component::ThemeRegistry::global(cx);
        let choices = app_theme::theme_choices(registry, mode, &custom_theme_colors);
        let selected_id = app_theme::normalize_theme_id(selected_id);
        let selected_name = choices
            .iter()
            .find(|choice| choice.id == selected_id)
            .map(|choice| choice.name.clone())
            .unwrap_or_else(|| SharedString::from(selected_id.clone()));
        let selected_summary = SharedString::from(format!(
            "{} {}",
            text.selected_prefix.as_ref(),
            selected_name.as_ref()
        ));
        let selected_border = cx.theme().primary;
        let mut tiles = Vec::with_capacity(choices.len());
        for choice in choices {
            let selected = choice.id == selected_id;
            let can_delete_material_theme = deletable_material_theme_ids
                .iter()
                .any(|theme_id| theme_id == &choice.id);
            tiles.push(self.render_theme_tile(
                choice,
                mode,
                selected,
                selected_border,
                can_delete_material_theme,
                text.selected_label.clone(),
                text.delete_material_theme_label.clone(),
            ));
        }

        v_flex()
            .gap_2()
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .items_center()
                    .justify_between()
                    .child(Label::new(title).text_sm().font_medium())
                    .child(
                        div()
                            .truncate()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(selected_summary),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .grid()
                    .grid_cols(columns)
                    .gap(px(THEME_TILE_GAP))
                    .children(tiles),
            )
    }

    fn render_theme_tile(
        &self,
        choice: app_theme::ThemeChoice,
        mode: gpui_component::ThemeMode,
        selected: bool,
        selected_border: Hsla,
        can_delete_material_theme: bool,
        selected_label: SharedString,
        delete_material_theme_label: SharedString,
    ) -> AnyElement {
        let preview = app_theme::preview_theme(&choice.config);
        let colors = preview.colors;
        let id = choice.id.clone();
        let select_id = id.clone();
        let label = choice.name.clone();
        let border_color = if selected {
            selected_border
        } else {
            colors.border
        };
        let selected_indicator =
            selected.then(|| selected_badge(selected_label, colors.primary).into_any_element());
        let primary_swatch = if app_theme::is_system_accent_material_you_theme_id(&id) {
            app_theme::system_accent_hsla().unwrap_or(colors.primary)
        } else {
            colors.primary
        };
        let delete_button = can_delete_material_theme.then(|| {
            let delete_id = id.clone();
            Button::new(SharedString::from(format!(
                "delete-material-theme-{}-{id}",
                mode.name()
            )))
            .icon(IconName::Trash)
            .danger()
            .xsmall()
            .tooltip(delete_material_theme_label)
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                let delete_id = delete_id.clone();
                let (title, message) = {
                    let i18n = cx.global::<I18n>();
                    (
                        i18n.t("dialog-delete-material-theme-title"),
                        i18n.t("dialog-delete-material-theme-message"),
                    )
                };
                open_destructive_confirm_dialog(
                    title,
                    message,
                    DestructiveAction::Delete,
                    move |window, cx| {
                        Self::delete_material_theme(delete_id.clone(), window, cx);
                    },
                    window,
                    cx,
                );
            })
            .into_any_element()
        });

        div()
            .id(format!("theme-preview-{}-{}", mode.name(), id))
            .w_full()
            .h(px(THEME_TILE_HEIGHT))
            .rounded(px(8.))
            .border_1()
            .border_color(border_color)
            .bg(colors.background)
            .overflow_hidden()
            .when(selected, |this| this.shadow_md())
            .hover(move |this| this.border_color(selected_border).shadow_xs())
            .on_click(move |_, window, cx| match mode {
                gpui_component::ThemeMode::Light => {
                    Self::set_light_theme(select_id.clone(), window, cx)
                }
                gpui_component::ThemeMode::Dark => {
                    Self::set_dark_theme(select_id.clone(), window, cx)
                }
            })
            .child(
                h_flex()
                    .h(px(24.))
                    .px_2()
                    .gap_1()
                    .items_center()
                    .bg(colors.title_bar)
                    .child(
                        div()
                            .size(px(7.))
                            .rounded_full()
                            .bg(colors.danger.opacity(0.85)),
                    )
                    .child(
                        div()
                            .size(px(7.))
                            .rounded_full()
                            .bg(colors.warning.opacity(0.85)),
                    )
                    .child(
                        div()
                            .size(px(7.))
                            .rounded_full()
                            .bg(colors.success.opacity(0.85)),
                    ),
            )
            .child(
                v_flex()
                    .gap_1()
                    .p_3()
                    .child(
                        div()
                            .h(px(20.))
                            .overflow_hidden()
                            .truncate()
                            .text_sm()
                            .font_medium()
                            .text_color(colors.foreground)
                            .child(label),
                    )
                    .child(
                        h_flex()
                            .h(px(24.))
                            .gap_2()
                            .items_center()
                            .justify_between()
                            .child(
                                h_flex()
                                    .flex_1()
                                    .gap_1()
                                    .items_center()
                                    .when_some(selected_indicator, |this, indicator| {
                                        this.child(indicator)
                                    }),
                            )
                            .when_some(delete_button, |this, button| this.child(button)),
                    )
                    .child(h_flex().gap_1().children([
                        swatch(primary_swatch),
                        swatch(colors.secondary),
                        swatch(colors.accent),
                        swatch(colors.muted),
                    ]))
                    .child(
                        h_flex()
                            .h(px(26.))
                            .rounded(px(6.))
                            .px_2()
                            .items_center()
                            .justify_between()
                            .bg(colors.secondary)
                            .child(
                                div()
                                    .w(relative(0.58))
                                    .max_w(px(132.))
                                    .h(px(5.))
                                    .rounded_full()
                                    .bg(colors.secondary_foreground.opacity(0.55)),
                            )
                            .child(div().size(px(14.)).rounded(px(4.)).bg(colors.primary)),
                    ),
            )
            .into_any_element()
    }
}

impl Render for AppearanceSettingsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (
            mode_title,
            mode_system,
            mode_light,
            mode_dark,
            custom_title,
            custom_description,
            add_material_theme,
            delete_material_theme,
            selected_prefix,
            selected_label,
            light_title,
            dark_title,
        ) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("settings-appearance-mode"),
                i18n.t("appearance-mode-system"),
                i18n.t("appearance-mode-light"),
                i18n.t("appearance-mode-dark"),
                i18n.t("settings-custom-theme-color"),
                i18n.t("settings-custom-theme-color-description"),
                i18n.t("button-add-material-theme"),
                i18n.t("button-delete-material-theme"),
                i18n.t("theme-selected-prefix"),
                i18n.t("theme-selected"),
                i18n.t("settings-light-themes"),
                i18n.t("settings-dark-themes"),
            )
        };
        let settings = cx.global::<AiChat2AppSettings>().theme().clone();
        let theme_mode = settings.mode;
        let light_theme_id = light_theme_id(&settings);
        let dark_theme_id = dark_theme_id(&settings);
        let custom_theme_colors = custom_theme_colors_for_choices(&settings);
        let theme_grid_text = ThemeGridText {
            selected_prefix: selected_prefix.into(),
            selected_label: selected_label.into(),
            delete_material_theme_label: delete_material_theme.into(),
        };
        let theme_grid_columns = theme_grid_columns(theme_grid_available_width(window));

        v_flex()
            .gap_5()
            .child(
                v_flex()
                    .gap_2()
                    .child(Label::new(mode_title).text_sm().font_medium())
                    .child(h_flex().gap_2().children([
                        self.render_mode_button(
                            "system",
                            mode_system.into(),
                            AppThemeMode::System,
                            theme_mode,
                        ),
                        self.render_mode_button(
                            "light",
                            mode_light.into(),
                            AppThemeMode::Light,
                            theme_mode,
                        ),
                        self.render_mode_button(
                            "dark",
                            mode_dark.into(),
                            AppThemeMode::Dark,
                            theme_mode,
                        ),
                    ])),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(Label::new(custom_title).text_sm().font_medium())
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(custom_description),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                ColorPicker::new(&self.color_picker)
                                    .with_size(Size::Small)
                                    .featured_colors(
                                        custom_theme_colors
                                            .iter()
                                            .filter_map(|color| Hsla::parse_hex(color).ok())
                                            .collect(),
                                    ),
                            )
                            .child(
                                Button::new("add-material-theme")
                                    .label(add_material_theme)
                                    .primary()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.add_material_theme(window, cx);
                                    })),
                            ),
                    ),
            )
            .child(self.render_theme_grid(
                light_title.into(),
                theme_grid_text.clone(),
                gpui_component::ThemeMode::Light,
                &light_theme_id,
                theme_grid_columns,
                cx,
            ))
            .child(self.render_theme_grid(
                dark_title.into(),
                theme_grid_text,
                gpui_component::ThemeMode::Dark,
                &dark_theme_id,
                theme_grid_columns,
                cx,
            ))
    }
}

fn light_theme_id(settings: &AppThemeSettings) -> String {
    settings
        .light_theme
        .as_deref()
        .map(app_theme::normalize_theme_id)
        .unwrap_or_else(|| app_theme::DEFAULT_LIGHT_THEME_ID.to_string())
}

fn dark_theme_id(settings: &AppThemeSettings) -> String {
    settings
        .dark_theme
        .as_deref()
        .map(app_theme::normalize_theme_id)
        .unwrap_or_else(|| app_theme::DEFAULT_DARK_THEME_ID.to_string())
}

fn set_light_theme_id(settings: &mut AppThemeSettings, theme_id: String) {
    let theme_id = app_theme::normalize_theme_id(&theme_id);
    add_custom_color_from_theme_id(settings, &theme_id);
    settings.light_theme = Some(theme_id);
}

fn set_dark_theme_id(settings: &mut AppThemeSettings, theme_id: String) {
    let theme_id = app_theme::normalize_theme_id(&theme_id);
    add_custom_color_from_theme_id(settings, &theme_id);
    settings.dark_theme = Some(theme_id);
}

fn custom_theme_colors_for_choices(settings: &AppThemeSettings) -> Vec<String> {
    state::theme::normalized_custom_theme_colors(settings)
}

fn append_custom_theme_color(colors: &mut Vec<String>, color: String) {
    if let Some(color) = app_theme::normalize_hex_color(&color)
        && !colors.contains(&color)
    {
        colors.push(color);
    }
}

fn add_custom_color_from_theme_id(settings: &mut AppThemeSettings, theme_id: &str) {
    add_custom_theme_color_from_theme_id(&mut settings.custom_theme_colors, theme_id);
}

fn add_custom_theme_color_from_theme_id(colors: &mut Vec<String>, theme_id: &str) {
    if let Some(color) = app_theme::material_you_color_from_id(theme_id) {
        append_custom_theme_color(colors, color);
    }
}

fn delete_custom_theme_color(settings: &mut AppThemeSettings, theme_id_or_color: &str) -> bool {
    let Some(color) = app_theme::material_you_color_from_id(theme_id_or_color)
        .or_else(|| app_theme::normalize_hex_color(theme_id_or_color))
    else {
        return false;
    };

    let before_len = settings.custom_theme_colors.len();
    settings
        .custom_theme_colors
        .retain(|existing| app_theme::normalize_hex_color(existing).as_ref() != Some(&color));
    if settings.custom_theme_colors.len() == before_len {
        return false;
    }

    let Some(theme_id) = app_theme::material_you_theme_id(&color) else {
        return false;
    };
    if settings.light_theme.as_deref() == Some(theme_id.as_str()) {
        settings.light_theme = Some(app_theme::DEFAULT_LIGHT_THEME_ID.to_string());
    }
    if settings.dark_theme.as_deref() == Some(theme_id.as_str()) {
        settings.dark_theme = Some(app_theme::DEFAULT_DARK_THEME_ID.to_string());
    }
    true
}

fn default_custom_color(cx: &App) -> Hsla {
    let colors =
        state::theme::normalized_custom_theme_colors(cx.global::<AiChat2AppSettings>().theme());
    colors
        .last()
        .and_then(|color| Hsla::parse_hex(color).ok())
        .or_else(|| Hsla::parse_hex(app_theme::DEFAULT_CUSTOM_THEME_COLOR).ok())
        .unwrap_or_else(|| hsla(0.58, 0.55, 0.44, 1.))
}

fn swatch(color: Hsla) -> impl IntoElement {
    div().size(px(16.)).rounded(px(5.)).bg(color)
}

fn selected_badge(label: SharedString, color: Hsla) -> impl IntoElement {
    div()
        .px(px(6.))
        .py(px(2.))
        .rounded_full()
        .border_1()
        .border_color(color.opacity(0.45))
        .bg(color.opacity(0.14))
        .text_xs()
        .font_medium()
        .text_color(color)
        .child(label)
}

fn theme_grid_available_width(window: &Window) -> f32 {
    (window.viewport_size().width.as_f32() - SETTINGS_THEME_GRID_WIDTH_CHROME)
        .max(THEME_TILE_MIN_WIDTH)
}

fn theme_grid_columns(available_width: f32) -> u16 {
    if !available_width.is_finite() || available_width <= THEME_TILE_MIN_WIDTH {
        return 1;
    }

    let columns = ((available_width + THEME_TILE_GAP) / (THEME_TILE_MIN_WIDTH + THEME_TILE_GAP))
        .floor()
        .max(1.) as u16;
    let tile_width = theme_tile_width_for_columns(available_width, columns);
    if tile_width <= THEME_TILE_MAX_WIDTH {
        return columns;
    }

    let next_columns = columns + 1;
    let next_tile_width = theme_tile_width_for_columns(available_width, next_columns);
    if next_tile_width >= THEME_TILE_MIN_WIDTH * 0.95 {
        next_columns
    } else {
        columns
    }
}

fn theme_tile_width_for_columns(available_width: f32, columns: u16) -> f32 {
    let columns = columns.max(1) as f32;
    let gaps = THEME_TILE_GAP * (columns - 1.);
    ((available_width - gaps) / columns).max(0.)
}

#[cfg(test)]
mod tests {
    use super::{
        THEME_TILE_MAX_WIDTH, THEME_TILE_MIN_WIDTH, custom_theme_colors_for_choices,
        delete_custom_theme_color, set_dark_theme_id, set_light_theme_id, theme_grid_columns,
        theme_tile_width_for_columns,
    };
    use ai_chat_core::AppThemeSettings;

    #[test]
    fn theme_grid_columns_uses_single_column_for_narrow_widths() {
        assert_eq!(theme_grid_columns(0.), 1);
        assert_eq!(theme_grid_columns(180.), 1);
        assert_eq!(theme_grid_columns(THEME_TILE_MIN_WIDTH), 1);
    }

    #[test]
    fn theme_grid_columns_increases_at_min_tile_thresholds() {
        assert_eq!(theme_grid_columns(320.), 1);
        assert_eq!(theme_grid_columns(368.), 2);
        assert_eq!(theme_grid_columns(558.), 3);
        assert_eq!(theme_grid_columns(748.), 4);
    }

    #[test]
    fn theme_grid_columns_keep_tiles_at_or_above_min_width() {
        for width in [368., 558., 748., 1600.] {
            let columns = theme_grid_columns(width);
            let tile_width = theme_tile_width_for_columns(width, columns);

            assert!(
                tile_width >= THEME_TILE_MIN_WIDTH,
                "tile width {tile_width} should be at least {THEME_TILE_MIN_WIDTH}"
            );
        }
    }

    #[test]
    fn theme_grid_columns_keep_wide_layouts_below_max_width() {
        let columns = theme_grid_columns(1600.);
        let tile_width = theme_tile_width_for_columns(1600., columns);

        assert!(
            tile_width <= THEME_TILE_MAX_WIDTH,
            "tile width {tile_width} should not exceed {THEME_TILE_MAX_WIDTH}"
        );
    }

    #[test]
    fn material_theme_selection_adds_custom_color() {
        let mut settings = AppThemeSettings::default();

        set_light_theme_id(&mut settings, "material-you:#123456".to_string());
        set_dark_theme_id(&mut settings, "material-you:#abcdef".to_string());

        assert_eq!(
            settings.light_theme.as_deref(),
            Some("material-you:#123456")
        );
        assert_eq!(settings.dark_theme.as_deref(), Some("material-you:#ABCDEF"));
        assert_eq!(
            custom_theme_colors_for_choices(&settings),
            vec!["#123456".to_string(), "#ABCDEF".to_string()]
        );
    }

    #[test]
    fn material_theme_choices_include_default_color_when_settings_are_empty() {
        assert_eq!(
            custom_theme_colors_for_choices(&AppThemeSettings::default()),
            vec![app_theme::DEFAULT_CUSTOM_THEME_COLOR.to_string()]
        );
    }

    #[test]
    fn delete_selected_material_theme_falls_back_to_defaults() {
        let mut settings = AppThemeSettings {
            light_theme: Some("material-you:#123456".to_string()),
            dark_theme: Some("material-you:#123456".to_string()),
            custom_theme_colors: vec!["#123456".to_string()],
            ..Default::default()
        };

        assert!(delete_custom_theme_color(
            &mut settings,
            "material-you:#123456"
        ));

        assert_eq!(settings.custom_theme_colors, Vec::<String>::new());
        assert_eq!(
            settings.light_theme.as_deref(),
            Some(app_theme::DEFAULT_LIGHT_THEME_ID)
        );
        assert_eq!(
            settings.dark_theme.as_deref(),
            Some(app_theme::DEFAULT_DARK_THEME_ID)
        );
    }
}

use super::*;
use gpui::Hsla;
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

fn color_from_hsla(color: Hsla) -> Argb {
    let color = color.to_rgb();
    Argb::from_rgb(
        (color.r * 255.0).round() as u8,
        (color.g * 255.0).round() as u8,
        (color.b * 255.0).round() as u8,
    )
}

fn syntax_color(highlight: &HighlightThemeStyle, name: &str) -> Argb {
    color_from_hsla(
        highlight
            .syntax
            .style(name)
            .unwrap_or_else(|| panic!("{name} syntax style should be generated"))
            .color
            .unwrap_or_else(|| panic!("{name} syntax color should be generated")),
    )
}

fn collect_null_paths(prefix: &str, value: &serde_json::Value, paths: &mut Vec<String>) {
    match value {
        serde_json::Value::Null => paths.push(prefix.to_string()),
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                let path = if prefix.is_empty() {
                    key.to_string()
                } else {
                    format!("{prefix}.{key}")
                };
                collect_null_paths(&path, value, paths);
            }
        }
        _ => {}
    }
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

    0.2126 * channel(color.red()) + 0.7152 * channel(color.green()) + 0.0722 * channel(color.blue())
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
fn system_accent_material_you_id_is_stable() {
    assert_eq!(
        normalize_theme_id(SYSTEM_ACCENT_MATERIAL_YOU_THEME_ID),
        SYSTEM_ACCENT_MATERIAL_YOU_THEME_ID
    );
    assert!(material_you_color_from_id(SYSTEM_ACCENT_MATERIAL_YOU_THEME_ID).is_none());
}

#[test]
fn system_accent_color_changed_only_updates_on_real_changes() {
    assert!(!system_accent_color_changed(&None, &None));
    assert!(!system_accent_color_changed(
        &Some("#123456".to_string()),
        &Some("#123456".to_string())
    ));
    assert!(system_accent_color_changed(
        &Some("#123456".to_string()),
        &Some("#654321".to_string())
    ));
    assert!(system_accent_color_changed(
        &Some("#123456".to_string()),
        &None
    ));
    assert!(system_accent_color_changed(
        &None,
        &Some("#123456".to_string())
    ));
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
        let danger_roles = material_error_roles_for_palette(&scheme, scheme.error_palette.clone());

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
        let config = generated_theme_config(TEST_THEME_COLOR, mode).expect("material theme config");
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
                contrast_ratio(color_from_option(background), color_from_option(foreground)) >= 4.5
            );
        }
    }
}

#[test]
fn generated_material_control_states_are_distinct_and_readable() {
    for mode in [ComponentThemeMode::Light, ComponentThemeMode::Dark] {
        let config = generated_theme_config(TEST_THEME_COLOR, mode).expect("material theme config");
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
fn generated_material_theme_config_has_no_unexpected_missing_public_color_tokens() {
    let allowed_missing = HashSet::from([
        "base.blue",
        "base.blue.light",
        "base.cyan",
        "base.cyan.light",
        "base.green",
        "base.green.light",
        "base.magenta",
        "base.magenta.light",
        "base.red",
        "base.red.light",
        "base.yellow",
        "base.yellow.light",
    ]);

    for mode in [ComponentThemeMode::Light, ComponentThemeMode::Dark] {
        let config = generated_theme_config(TEST_THEME_COLOR, mode).expect("material theme");
        let value = serde_json::to_value(&config.colors).expect("serialize theme colors");
        let mut missing = Vec::new();
        collect_null_paths("", &value, &mut missing);
        missing.retain(|path| !allowed_missing.contains(path.as_str()));
        missing.sort();

        assert!(
            missing.is_empty(),
            "Material You {mode:?} should not leave public color tokens unset: {missing:?}"
        );
    }
}

#[test]
fn generated_material_theme_fills_highlight_tokens() {
    for mode in [ComponentThemeMode::Light, ComponentThemeMode::Dark] {
        let config = generated_theme_config(TEST_THEME_COLOR, mode).expect("material theme");
        let highlight = config.highlight.expect("highlight should be generated");

        assert!(highlight.editor_background.is_some());
        assert!(highlight.editor_foreground.is_some());
        assert!(highlight.editor_active_line.is_some());
        assert!(highlight.editor_line_number.is_some());
        assert!(highlight.editor_active_line_number.is_some());
        assert!(highlight.editor_invisible.is_some());

        for name in [
            "attribute",
            "boolean",
            "comment",
            "constant",
            "constructor",
            "embedded",
            "function",
            "keyword",
            "link_text",
            "link_uri",
            "number",
            "operator",
            "property",
            "punctuation",
            "punctuation.bracket",
            "punctuation.delimiter",
            "string",
            "string.escape",
            "tag",
            "text.literal",
            "title",
            "type",
            "variable",
            "variable.special",
            "variant",
        ] {
            _ = syntax_color(&highlight, name);
        }
    }
}

#[test]
fn generated_material_editor_highlight_is_readable() {
    for mode in [ComponentThemeMode::Light, ComponentThemeMode::Dark] {
        let scheme = material_scheme(mode);
        let config = generated_theme_config(TEST_THEME_COLOR, mode).expect("material theme");
        let highlight = config.highlight.expect("highlight should be generated");
        let background = color_from_hsla(highlight.editor_background.expect("editor background"));

        assert_eq!(background, scheme.surface_container_lowest);
        assert_eq!(
            color_from_hsla(highlight.editor_active_line.expect("active line")),
            scheme.surface_container_low
        );
        assert_ne!(
            highlight.editor_background, highlight.editor_active_line,
            "editor surface and active line should remain distinct"
        );
        assert_ne!(
            Some(hex(scheme.surface)),
            config.colors.tab,
            "editor highlight must not reuse tab token semantics"
        );

        for (name, color) in [
            (
                "editor foreground",
                color_from_hsla(highlight.editor_foreground.expect("editor foreground")),
            ),
            (
                "line number",
                color_from_hsla(highlight.editor_line_number.expect("line number")),
            ),
            ("property", syntax_color(&highlight, "property")),
            ("string", syntax_color(&highlight, "string")),
            ("punctuation", syntax_color(&highlight, "punctuation")),
            ("number", syntax_color(&highlight, "number")),
            ("boolean", syntax_color(&highlight, "boolean")),
            ("keyword", syntax_color(&highlight, "keyword")),
            ("function", syntax_color(&highlight, "function")),
            ("type", syntax_color(&highlight, "type")),
            ("comment", syntax_color(&highlight, "comment")),
        ] {
            assert!(
                contrast_ratio(background, color) >= 4.5,
                "{name} should be readable on editor background for {mode:?}"
            );
        }
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

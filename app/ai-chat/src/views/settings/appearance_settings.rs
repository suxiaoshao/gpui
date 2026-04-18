use crate::{
    i18n::I18n,
    state::{AiChatConfig, ThemeMode, theme as app_theme},
};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Colorize, Sizable, Size, StyledExt,
    button::{Button, ButtonVariants},
    color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState},
    h_flex,
    label::Label,
    v_flex,
};

pub(super) struct AppearanceSettingsPage {
    color_picker: Entity<ColorPickerState>,
    selected_color: Hsla,
    _subscriptions: Vec<Subscription>,
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

    fn set_theme_mode(mode: ThemeMode, cx: &mut App) {
        cx.update_global::<AiChatConfig, _>(|config, _cx| {
            config.set_theme_mode(mode);
        });
        cx.refresh_windows();
    }

    fn set_light_theme(theme_id: String, cx: &mut App) {
        cx.update_global::<AiChatConfig, _>(|config, _cx| {
            config.set_light_theme_id(theme_id);
        });
        cx.refresh_windows();
    }

    fn set_dark_theme(theme_id: String, cx: &mut App) {
        cx.update_global::<AiChatConfig, _>(|config, _cx| {
            config.set_dark_theme_id(theme_id);
        });
        cx.refresh_windows();
    }

    fn add_material_theme(&self, cx: &mut App) {
        let color = self.selected_color.to_hex();
        cx.update_global::<AiChatConfig, _>(|config, _cx| {
            config.add_custom_theme_color(&color);
        });
        cx.refresh_windows();
    }

    fn render_mode_button(
        &self,
        id: &'static str,
        label: SharedString,
        mode: ThemeMode,
        current: ThemeMode,
    ) -> Button {
        let button = Button::new(format!("appearance-mode-{id}")).label(label);
        let button = if mode == current {
            button.primary()
        } else {
            button.ghost()
        };
        button.on_click(move |_, _, cx| Self::set_theme_mode(mode, cx))
    }

    fn render_theme_grid(
        &self,
        title: SharedString,
        mode: gpui_component::ThemeMode,
        selected_id: &str,
        cx: &mut App,
    ) -> impl IntoElement {
        let config = cx.global::<AiChatConfig>();
        let registry = gpui_component::ThemeRegistry::global(cx);
        let choices = app_theme::theme_choices(registry, mode, config.custom_theme_colors());
        let selected_border = cx.theme().primary;

        v_flex()
            .gap_2()
            .child(Label::new(title).text_sm().font_medium())
            .child(
                h_flex()
                    .gap_3()
                    .flex_wrap()
                    .children(choices.into_iter().map(|choice| {
                        let selected = choice.id == selected_id;
                        self.render_theme_tile(choice, mode, selected, selected_border)
                    })),
            )
    }

    fn render_theme_tile(
        &self,
        choice: app_theme::ThemeChoice,
        mode: gpui_component::ThemeMode,
        selected: bool,
        selected_border: Hsla,
    ) -> impl IntoElement {
        let preview = app_theme::preview_theme(&choice.config);
        let colors = preview.colors;
        let id = choice.id.clone();
        let label = choice.name.clone();
        let border_color = if selected {
            selected_border
        } else {
            colors.border
        };

        div()
            .id(format!("theme-preview-{}-{}", mode.name(), id))
            .w(px(178.))
            .h(px(118.))
            .rounded(px(8.))
            .border_1()
            .border_color(border_color)
            .bg(colors.background)
            .overflow_hidden()
            .when(selected, |this| this.shadow_md())
            .hover(move |this| this.border_color(selected_border).shadow_xs())
            .on_click(move |_, _, cx| match mode {
                gpui_component::ThemeMode::Light => Self::set_light_theme(id.clone(), cx),
                gpui_component::ThemeMode::Dark => Self::set_dark_theme(id.clone(), cx),
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
                    .gap_2()
                    .p_3()
                    .child(
                        div()
                            .truncate()
                            .text_sm()
                            .font_medium()
                            .text_color(colors.foreground)
                            .child(label),
                    )
                    .child(h_flex().gap_1().children([
                        swatch(colors.primary),
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
                                    .w(px(66.))
                                    .h(px(5.))
                                    .rounded_full()
                                    .bg(colors.secondary_foreground.opacity(0.55)),
                            )
                            .child(div().size(px(14.)).rounded(px(4.)).bg(colors.primary)),
                    ),
            )
    }
}

impl Render for AppearanceSettingsPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (
            mode_title,
            mode_system,
            mode_light,
            mode_dark,
            custom_title,
            custom_description,
            add_material_theme,
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
                i18n.t("settings-light-themes"),
                i18n.t("settings-dark-themes"),
            )
        };
        let config = cx.global::<AiChatConfig>();
        let theme_mode = config.theme_mode();
        let light_theme_id = config.light_theme_id().to_string();
        let dark_theme_id = config.dark_theme_id().to_string();

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
                            ThemeMode::System,
                            theme_mode,
                        ),
                        self.render_mode_button(
                            "light",
                            mode_light.into(),
                            ThemeMode::Light,
                            theme_mode,
                        ),
                        self.render_mode_button(
                            "dark",
                            mode_dark.into(),
                            ThemeMode::Dark,
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
                                        config
                                            .custom_theme_colors()
                                            .iter()
                                            .filter_map(|color| Hsla::parse_hex(color).ok())
                                            .collect(),
                                    ),
                            )
                            .child(
                                Button::new("add-material-theme")
                                    .label(add_material_theme)
                                    .primary()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.add_material_theme(cx);
                                    })),
                            ),
                    ),
            )
            .child(self.render_theme_grid(
                light_title.into(),
                gpui_component::ThemeMode::Light,
                &light_theme_id,
                cx,
            ))
            .child(self.render_theme_grid(
                dark_title.into(),
                gpui_component::ThemeMode::Dark,
                &dark_theme_id,
                cx,
            ))
    }
}

fn default_custom_color(cx: &App) -> Hsla {
    cx.global::<AiChatConfig>()
        .custom_theme_colors()
        .last()
        .and_then(|color| Hsla::parse_hex(color).ok())
        .or_else(|| Hsla::parse_hex(app_theme::DEFAULT_CUSTOM_THEME_COLOR).ok())
        .unwrap_or_else(|| hsla(0.58, 0.55, 0.44, 1.))
}

fn swatch(color: Hsla) -> impl IntoElement {
    div().size(px(16.)).rounded(px(5.)).bg(color)
}

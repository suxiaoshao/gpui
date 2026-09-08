use super::*;
use crate::state::theme;
use gpui_kit::component::scroll::Scrollbar;
use gpui_kit::component::{
    Sizable, ThemeMode as Mode, ThemeRegistry,
    combobox::Combobox,
    form::{field, v_form},
    searchable_list::SearchableListItem,
    tooltip::Tooltip,
};
use gpui_kit::prelude::FluentBuilder;

#[derive(Clone)]
pub(super) struct LanguageItem {
    value: AppLanguage,
    title: SharedString,
    keywords: &'static str,
}
impl SearchableListItem for LanguageItem {
    type Value = AppLanguage;
    fn title(&self) -> SharedString {
        self.title.clone()
    }
    fn value(&self) -> &AppLanguage {
        &self.value
    }
    fn matches(&self, query: &str) -> bool {
        let query = query.to_lowercase();
        self.title.to_lowercase().contains(&query) || self.keywords.contains(&query)
    }
}
pub(super) fn language_items(cx: &App) -> SearchableVec<LanguageItem> {
    SearchableVec::new(vec![
        LanguageItem {
            value: AppLanguage::System,
            title: t(cx, "language-system").into(),
            keywords: "system auto",
        },
        LanguageItem {
            value: AppLanguage::English,
            title: "English".into(),
            keywords: "en english 英语 英文",
        },
        LanguageItem {
            value: AppLanguage::Chinese,
            title: "简体中文".into(),
            keywords: "zh chinese 中文 汉语",
        },
    ])
}
impl SettingsView {
    pub(super) fn render_language(&self, cx: &Context<Self>) -> AnyElement {
        v_flex()
            .gap_3()
            .child(
                v_form().child(
                    field().label(t(cx, "settings-language")).child(
                        Combobox::new(&self.language)
                            .w_full()
                            .large()
                            .cleanable(false)
                            .disabled(self.controller.read(cx).busy(cx))
                            .search_placeholder(t(cx, "setup-search-language")),
                    ),
                ),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(t(
                        cx,
                        if crate::foundation::i18n::system_is_chinese() {
                            "setup-system-chinese"
                        } else {
                            "setup-system-english"
                        },
                    )),
            )
            .into_any_element()
    }
    pub(super) fn render_appearance(&self, _window: &Window, cx: &Context<Self>) -> AnyElement {
        let draft = AppConfig::ROOT.get(&self.form, cx);
        let busy = self.controller.read(cx).busy(cx);
        let modes = [ThemeMode::System, ThemeMode::Light, ThemeMode::Dark];
        let mode_control = gpui_kit::component::radio::RadioGroup::horizontal("color-mode")
            .children([
                t(cx, "theme-system"),
                t(cx, "theme-light"),
                t(cx, "theme-dark"),
            ])
            .selected_index(modes.iter().position(|mode| *mode == draft.theme))
            .disabled(busy)
            .on_click(cx.listener(move |this, index, _, cx| {
                if this.controller.read(cx).busy(cx) {
                    return;
                }
                AppConfig::THEME.set(&this.form, modes[*index], cx);
            }));
        let form = self.form.clone();
        let controller = self.controller.clone();
        let light = app_theme::theme_choices(ThemeRegistry::global(cx), Mode::Light, &[]);
        let dark = app_theme::theme_choices(ThemeRegistry::global(cx), Mode::Dark, &[]);
        let controls = v_form().child(field().label(t(cx, "setup-color-mode")).child(mode_control));
        let scroll = self.theme_scroll.clone();
        container_query(move |size, _, cx| {
            let columns = theme_columns(size.width.as_f32());
            div()
                .relative()
                .size_full()
                .overflow_hidden()
                .child(
                    v_flex()
                        .id("theme-scroll")
                        .size_full()
                        .min_h_0()
                        .overflow_y_scroll()
                        .track_scroll(&scroll)
                        .gap_6()
                        .child(div().flex_shrink_0().child(controls))
                        .child(theme_grid(
                            ("light-themes", Mode::Light, light),
                            &draft,
                            &form,
                            &controller,
                            columns,
                            busy,
                            cx,
                        ))
                        .child(theme_grid(
                            ("dark-themes", Mode::Dark, dark),
                            &draft,
                            &form,
                            &controller,
                            columns,
                            busy,
                            cx,
                        )),
                )
                .child(super::sticky::overlay(scroll.clone()))
                .child(Scrollbar::vertical(&scroll))
        })
        .into_any_element()
    }
    pub(super) fn probe_matches(&self, cx: &App) -> bool {
        let command = AppConfig::PI_COMMAND.get(&self.form, cx);
        self.draft_pi.read(cx).matches_command(command.as_deref())
    }
    pub(super) fn probe_ready(&self, cx: &App) -> bool {
        let command = AppConfig::PI_COMMAND.get(&self.form, cx);
        self.draft_pi
            .read(cx)
            .ready_for(command.as_deref())
            .is_some()
    }
    pub(super) fn render_pi(&self, cx: &Context<Self>) -> AnyElement {
        let pi = self.draft_pi.read(cx);
        let matching = self.probe_matches(cx);
        let busy = self.controller.read(cx).busy(cx);
        let mut view = v_flex().gap_4().child(
            v_form().child(
                field()
                    .label(t(cx, "settings-pi-command"))
                    .description(t(cx, "settings-path-help"))
                    .child(Input::new(&self.input).large().disabled(busy)),
            ),
        );
        view = view.child(
            Button::new("check-draft-pi")
                .icon(IconName::RotateCw)
                .loading(pi.operation.is_running())
                .label(t(
                    cx,
                    if pi.operation.is_running() {
                        "startup-checking"
                    } else {
                        "setup-check-pi"
                    },
                ))
                .disabled(busy || pi.operation.is_running())
                .on_click(cx.listener(|this, _, _, cx| {
                    match AppConfig::ROOT.get(&this.form, cx).normalized() {
                        Ok(config) => {
                            this.error = None;
                            this.draft_pi
                                .update(cx, |pi, cx| pi.request(config.pi_command, true, cx));
                        }
                        Err(error) => this.error = Some(error),
                    }
                    cx.notify();
                })),
        );
        if matching && !pi.operation.is_running() {
            if let Some(problem) = pi.operation.problem() {
                view = view.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(t(cx, problem.key())),
                );
            } else if let Some(data) = pi.operation.data() {
                view = view.child(
                    v_flex()
                        .gap_1()
                        .child(
                            h_flex()
                                .gap_2()
                                .child(t(cx, "home-ready"))
                                .child(data.version.clone()),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(data.command.display().to_string()),
                        ),
                );
            }
        }
        view.into_any_element()
    }
}

// Match Jaco's equal-width grid: choose columns from the available width,
// then let grid tracks share all remaining space after the gaps.
fn theme_columns(width: f32) -> u16 {
    if !width.is_finite() || width <= 178. {
        return 1;
    }
    let columns = ((width + 12.) / (178. + 12.)).floor().max(1.) as u16;
    let tile_width = (width - 12. * (columns - 1) as f32) / columns as f32;
    let next_width = (width - 12. * columns as f32) / (columns + 1) as f32;
    if tile_width > 220. && next_width >= 178. * 0.95 {
        columns + 1
    } else {
        columns
    }
}

fn theme_grid(
    group: (&'static str, Mode, Vec<app_theme::ThemeChoice>),
    draft: &AppConfig,
    form: &Entity<Form<AppConfig>>,
    controller: &Entity<ConfigController>,
    columns: u16,
    busy: bool,
    cx: &App,
) -> AnyElement {
    let (id, mode, choices) = group;
    let selected_id = theme::selected_id(draft, mode);
    let selected_border = cx.theme().primary;
    let mut grid = gpui_kit::base::RadioGroup::new(id)
        .aria_label(t(cx, id))
        .axis(Axis::Horizontal)
        .w_full()
        .grid()
        .grid_cols(columns)
        .gap_3();
    let count = choices.len();
    for (index, choice) in choices.into_iter().enumerate() {
        let selected = choice.id == selected_id;
        let colors = app_theme::preview_theme(&choice.config).colors;
        let name = if app_theme::is_system_accent_material_you_theme_id(&choice.id) {
            SharedString::from(t(cx, "setup-system-accent"))
        } else {
            choice.name
        };
        let form = form.clone();
        let theme_id = choice.id.clone();
        let key_form = form.clone();
        let key_controller = controller.clone();
        let change_controller = controller.clone();
        let key_id = theme_id.clone();
        let tooltip_name = name.clone();
        grid = grid.child(
            gpui_kit::base::Radio::new(SharedString::from(format!("{id}-{}", choice.id)))
                .checked(selected)
                .disabled(busy)
                .accessibility_label(name.clone())
                .tooltip(move |window, cx| Tooltip::new(tooltip_name.clone()).build(window, cx))
                .set_position(index + 1, count)
                .w_full()
                .min_w_0()
                .rounded_lg()
                .border_2()
                .border_color(if selected {
                    selected_border
                } else {
                    colors.border
                })
                .bg(colors.background)
                .text_color(colors.foreground)
                .overflow_hidden()
                .focus(|style| style.border_color(selected_border))
                .on_key_down(move |event, _, cx| {
                    if !key_controller.read(cx).busy(cx)
                        && matches!(event.keystroke.key.as_str(), "space" | "enter")
                    {
                        match mode {
                            Mode::Light => {
                                AppConfig::LIGHT_THEME.set(&key_form, Some(key_id.clone()), cx)
                            }
                            Mode::Dark => {
                                AppConfig::DARK_THEME.set(&key_form, Some(key_id.clone()), cx)
                            }
                        };
                        cx.stop_propagation();
                    }
                })
                .on_change(move |_, _, _, cx| {
                    if change_controller.read(cx).busy(cx) {
                        return;
                    }
                    match mode {
                        Mode::Light => {
                            AppConfig::LIGHT_THEME.set(&form, Some(theme_id.clone()), cx)
                        }
                        Mode::Dark => AppConfig::DARK_THEME.set(&form, Some(theme_id.clone()), cx),
                    };
                })
                .child(
                    v_flex()
                        .w_full()
                        .child(
                            h_flex()
                                .h(px(88.))
                                .w_full()
                                .child(
                                    v_flex()
                                        .w(px(48.))
                                        .h_full()
                                        .p_2()
                                        .gap_2()
                                        .bg(colors.sidebar)
                                        .child(
                                            div()
                                                .h(px(5.))
                                                .w_full()
                                                .rounded_sm()
                                                .bg(colors.primary),
                                        )
                                        .child(
                                            div()
                                                .h(px(4.))
                                                .w_full()
                                                .rounded_sm()
                                                .bg(colors.muted_foreground),
                                        )
                                        .child(
                                            div()
                                                .h(px(4.))
                                                .w(px(20.))
                                                .rounded_sm()
                                                .bg(colors.border),
                                        ),
                                )
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .min_w_0()
                                        .p_3()
                                        .gap_2()
                                        .child(
                                            div()
                                                .h(px(6.))
                                                .w(px(65.))
                                                .rounded_sm()
                                                .bg(colors.foreground),
                                        )
                                        .child(
                                            div()
                                                .h(px(4.))
                                                .w_full()
                                                .rounded_sm()
                                                .bg(colors.muted_foreground),
                                        )
                                        .child(
                                            div()
                                                .h(px(4.))
                                                .w(px(65.))
                                                .rounded_sm()
                                                .bg(colors.border),
                                        )
                                        .child(
                                            div()
                                                .h(px(13.))
                                                .w(px(30.))
                                                .rounded_sm()
                                                .bg(colors.primary),
                                        ),
                                ),
                        )
                        .child(
                            h_flex()
                                .w_full()
                                .min_w_0()
                                .px_3()
                                .py_2()
                                .gap_2()
                                .justify_between()
                                .child(div().text_sm().truncate().child(name))
                                .child(div().size_4().flex_shrink_0().when(selected, |view| {
                                    view.child(
                                        gpui_kit::component::Icon::new(IconName::Check)
                                            .small()
                                            .text_color(colors.foreground),
                                    )
                                })),
                        ),
                ),
        );
    }
    v_flex()
        .w_full()
        .flex_shrink_0()
        .gap_3()
        .child(super::sticky::heading(id, cx))
        .child(grid)
        .into_any_element()
}

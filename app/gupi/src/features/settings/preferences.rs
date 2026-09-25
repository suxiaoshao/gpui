use super::*;
use crate::pi::ProbeFailureKey;
use crate::state::theme;
use gpui_kit::component::{
    Sizable, ThemeMode as Mode,
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
            title: t(cx, "language-english").into(),
            keywords: "en english 英语 英文",
        },
        LanguageItem {
            value: AppLanguage::Chinese,
            title: t(cx, "language-chinese").into(),
            keywords: "zh zh-cn chinese simplified 简体中文 简体 中文 汉语",
        },
        LanguageItem {
            value: AppLanguage::TraditionalChinese,
            title: t(cx, "language-traditional-chinese").into(),
            keywords: "zh-tw zh-hant traditional chinese 繁体中文 繁體中文",
        },
        LanguageItem {
            value: AppLanguage::Japanese,
            title: t(cx, "language-japanese").into(),
            keywords: "ja japanese 日本語 日语 日文",
        },
        LanguageItem {
            value: AppLanguage::Korean,
            title: t(cx, "language-korean").into(),
            keywords: "ko korean 한국어 조선어 韩语 韓語",
        },
        LanguageItem {
            value: AppLanguage::German,
            title: t(cx, "language-german").into(),
            keywords: "de german deutsch 德语 德文",
        },
        LanguageItem {
            value: AppLanguage::French,
            title: t(cx, "language-french").into(),
            keywords: "fr french français francais 法语 法文",
        },
        LanguageItem {
            value: AppLanguage::Spanish,
            title: t(cx, "language-spanish").into(),
            keywords: "es spanish español espanol 西班牙语 西语",
        },
        LanguageItem {
            value: AppLanguage::BrazilianPortuguese,
            title: t(cx, "language-portuguese-brazil").into(),
            keywords: "pt pt-br portuguese português portugues brasil brazilian 巴西葡萄牙语",
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
                            .when(self.controller.read(cx).is_onboarding(cx), |control| {
                                control.large()
                            })
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
                    .child({
                        let mut args = fluent_bundle::FluentArgs::new();
                        args.set(
                            "language",
                            crate::foundation::i18n::system_language_autonym(cx).to_owned(),
                        );
                        crate::foundation::i18n::t_with_args(cx, "setup-system-language", &args)
                    }),
            )
            .into_any_element()
    }
    pub(super) fn probe_matches(&self, cx: &App) -> bool {
        let command = self.pi_command(cx);
        self.draft_pi.read(cx).matches_command(command.as_deref())
    }
    pub(super) fn probe_ready(&self, cx: &App) -> bool {
        let command = self.pi_command(cx);
        self.draft_pi
            .read(cx)
            .ready_for(command.as_deref())
            .is_some()
    }
    pub(super) fn render_pi(&self, cx: &Context<Self>) -> AnyElement {
        let pi = self.draft_pi.read(cx);
        let matching = self.probe_matches(cx);
        let busy = self.controller.read(cx).busy(cx)
            || self
                .resources
                .read(cx)
                .controller
                .read(cx)
                .mutation
                .is_running();
        let onboarding = self.controller.read(cx).is_onboarding(cx);
        let dirty = self.controller.read(cx).pi_form.read(cx).is_dirty();
        let mut view = v_flex().gap_4().child(
            v_form().child(
                field()
                    .label(t(cx, "settings-pi-command"))
                    .description(t(cx, "settings-path-help"))
                    .child(
                        Input::new(self.pi_input(cx))
                            .when(self.controller.read(cx).is_onboarding(cx), |input| {
                                input.large()
                            })
                            .disabled(busy),
                    ),
            ),
        );
        let actions = h_flex().gap_2().child(
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
                    match (PiSettings {
                        command: this.pi_command(cx),
                    })
                    .normalized()
                    {
                        Ok(config) => {
                            this.error = None;
                            this.draft_pi
                                .update(cx, |pi, cx| pi.request(config.command, true, cx));
                        }
                        Err(error) => this.error = Some(error),
                    }
                    cx.notify();
                })),
        );
        view = view.child(actions.when(!onboarding, |actions| {
            actions.child(
                Button::new("save-pi")
                    .primary()
                    .label(t(cx, "settings-save-pi"))
                    .disabled(busy || !dirty)
                    .loading(self.controller.read(cx).busy(cx))
                    .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx))),
            )
        }));
        if !onboarding {
            view = view.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(t(
                        cx,
                        if dirty {
                            "settings-pi-unsaved"
                        } else {
                            "settings-pi-saved"
                        },
                    )),
            );
        }
        if matching && !pi.operation.is_running() {
            if let Some(problem) = pi.operation.problem() {
                view = view.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(t(cx, problem.key())),
                );
            } else if self.probe_ready(cx)
                && let Some(data) = pi.operation.data()
            {
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
pub(super) fn theme_columns(width: f32) -> u16 {
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

pub(super) fn theme_grid(
    group: (&'static str, Mode, Vec<app_theme::ThemeChoice>),
    draft: &AppConfig,
    controller: &Entity<ConfigController>,
    busy: bool,
) -> AnyElement {
    super::theme_grid::ThemeGrid {
        group,
        draft: draft.clone(),
        controller: controller.clone(),
        busy,
    }
    .into_any_element()
}

pub(super) fn theme_grid_content(
    group: (&'static str, Mode, Vec<app_theme::ThemeChoice>),
    draft: &AppConfig,
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
        let theme_id = choice.id.clone();
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
                .debug_selector(move || format!("{id}-tile-{index}"))
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
                        key_controller.update(cx, |owner, cx| {
                            owner.set_preference(
                                match mode {
                                    Mode::Light => {
                                        PreferenceChange::LightTheme(Some(key_id.clone()))
                                    }
                                    Mode::Dark => PreferenceChange::DarkTheme(Some(key_id.clone())),
                                },
                                cx,
                            )
                        });
                        cx.stop_propagation();
                    }
                })
                .on_change(move |_, _, _, cx| {
                    if change_controller.read(cx).busy(cx) {
                        return;
                    }
                    change_controller.update(cx, |owner, cx| {
                        owner.set_preference(
                            match mode {
                                Mode::Light => PreferenceChange::LightTheme(Some(theme_id.clone())),
                                Mode::Dark => PreferenceChange::DarkTheme(Some(theme_id.clone())),
                            },
                            cx,
                        )
                    });
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
    grid.into_any_element()
}

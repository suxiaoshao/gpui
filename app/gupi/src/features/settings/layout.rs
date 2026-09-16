use super::*;
use crate::foundation::pi_resources::Kind;
use gpui_kit::component::{
    ThemeMode as Mode, ThemeRegistry,
    setting::{SettingField, SettingGroup, SettingItem, SettingPage, Settings},
};

impl SettingsView {
    pub(super) fn render_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let this = cx.entity().downgrade();
        let item = |key: &'static str,
                    aliases: &'static str,
                    render: fn(
            &mut SettingsView,
            &mut Window,
            &mut Context<SettingsView>,
        ) -> AnyElement| {
            let this = this.clone();
            SettingItem::render(move |_, window, cx| {
                this.update(cx, |this, cx| render(this, window, cx))
                    .unwrap_or_else(|_| div().into_any_element())
            })
            .keywords(vec![t(cx, key), aliases.to_owned()])
        };
        let page = |key: &'static str, items: Vec<SettingItem>| {
            SettingPage::new(t(cx, key))
                .icon(match key {
                    "settings-page-general" => IconName::Settings,
                    "settings-theme" => IconName::Sparkles,
                    "settings-page-pi" => IconName::Terminal,
                    "settings-page-keys" => IconName::Keyboard,
                    "settings-page-plugins" => IconName::Puzzle,
                    "settings-page-skills" => IconName::BookOpen,
                    "settings-page-prompts" => IconName::FileText,
                    _ => IconName::Info,
                })
                .resettable(false)
                .group(SettingGroup::new().items(items))
        };
        let general = page(
            "settings-page-general",
            vec![
                item(
                    "settings-language",
                    "通用 general language locale 语言",
                    |this, _, cx| this.render_language(cx),
                ),
                item(
                    "settings-reload",
                    "通用 general config file 配置文件 重新读取",
                    |this, _, cx| this.render_config_actions(cx),
                ),
            ],
        );
        let appearance = page(
            "settings-theme",
            vec![
                item(
                    "setup-color-mode",
                    "外观 appearance color mode dark light system 颜色模式 深色 浅色 暗色 跟随系统",
                    |this, _, cx| this.render_mode(cx),
                ),
                item(
                    "light-themes",
                    "外观 appearance light theme 浅色主题",
                    |this, _, cx| this.render_theme_choices(Mode::Light, cx),
                ),
                item(
                    "dark-themes",
                    "外观 appearance dark theme 深色主题",
                    |this, _, cx| this.render_theme_choices(Mode::Dark, cx),
                ),
            ],
        );
        let pi = page(
            "settings-page-pi",
            vec![item(
                "settings-pi-command",
                "pi executable path environment version 路径 环境 检查",
                |this, _, cx| {
                    let busy = this.controller.read(cx).busy(cx)
                        || this
                            .resources
                            .read(cx)
                            .controller
                            .read(cx)
                            .mutation
                            .is_running();
                    v_flex()
                        .gap_4()
                        .child(this.render_pi(cx))
                        .child(
                            Button::new("save-pi")
                                .label(t(cx, "settings-save-pi"))
                                .loading(this.controller.read(cx).busy(cx))
                                .disabled(busy)
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.submit(window, cx)),
                                ),
                        )
                        .into_any_element()
                },
            )],
        );
        let keys = self.keys.downgrade();
        let keyboard = page(
            "settings-page-keys",
            crate::state::keybindings::COMMANDS
                .iter()
                .enumerate()
                .map(|(index, command)| {
                    let keys = keys.clone();
                    SettingItem::render(move |_, _, cx| {
                        keys.update(cx, |this, cx| this.render_row(index, cx))
                            .unwrap_or_else(|_| div().into_any_element())
                    })
                    .keywords(vec![
                        t(cx, command.label),
                        command.id.to_owned(),
                        "快捷键 keys keyboard shortcut".into(),
                    ])
                })
                .collect(),
        );
        let resource_page = |key, kind, keywords: &str| {
            let resources = self.resources.downgrade();
            page(
                key,
                vec![
                    SettingItem::render(move |_, _, cx| {
                        resources
                            .update(cx, |view, cx| view.render_page(kind, cx))
                            .unwrap_or_else(|_| div().into_any_element())
                    })
                    .keywords(vec![t(cx, key), keywords.to_owned()]),
                ],
            )
        };
        let probe = self.applied_pi.read(cx);
        let unavailable = t(cx, "settings-about-unavailable");
        let about = page(
            "settings-page-about",
            [
                (
                    "settings-about-gupi",
                    env!("CARGO_PKG_VERSION").to_owned(),
                    "about gupi version 关于 版本",
                ),
                (
                    "settings-about-pi",
                    probe
                        .operation
                        .data()
                        .map(|p| p.version.clone())
                        .unwrap_or_else(|| unavailable.clone()),
                    "about pi version 关于 版本",
                ),
                (
                    "settings-about-path",
                    probe
                        .operation
                        .data()
                        .map(|p| p.command.to_string_lossy().into_owned())
                        .unwrap_or_else(|| unavailable.clone()),
                    "about pi command executable path 关于 路径",
                ),
                (
                    "settings-about-status",
                    if probe.operation.is_running() {
                        t(cx, "startup-checking")
                    } else if let Some(error) = probe.operation.problem() {
                        t(cx, crate::pi::ProbeFailureKey::key(error))
                    } else if probe.operation.data().is_some() {
                        t(cx, "home-ready")
                    } else {
                        unavailable
                    },
                    "about pi status probe 关于 检查 状态",
                ),
            ]
            .into_iter()
            .map(|(key, value, aliases)| {
                SettingItem::new(
                    t(cx, key),
                    SettingField::render(move |_, _, _| div().child(value.clone())),
                )
                .keywords(vec![aliases])
            })
            .collect(),
        );
        let panel = Settings::new("gupi-settings")
            .page(general).page(appearance).page(pi).page(keyboard)
            .page(resource_page("settings-page-plugins",Kind::Extension,"package plugin extension install update remove source version path enable disable 插件 扩展 包 安装 更新 移除 启停"))
            .page(resource_page("settings-page-skills",Kind::Skill,"skill name description source path create edit delete enable disable 技能 创建 编辑 删除 启停"))
            .page(resource_page("settings-page-prompts",Kind::Prompt,"prompt template system append 创建 编辑 删除 模板 系统提示词"))
            .page(about)
            .into_any_element();
        let problem = self
            .controller
            .read(cx)
            .store
            .read(cx, |op| op.problem().map(|p| p.key()));
        v_flex()
            .size_full()
            .min_h_0()
            .children(
                self.error
                    .as_deref()
                    .or(problem)
                    .map(|key| div().text_color(cx.theme().danger).child(t(cx, key))),
            )
            .child(div().flex_1().min_h_0().child(panel))
            .into_any_element()
    }
    fn render_mode(&self, cx: &Context<Self>) -> AnyElement {
        let modes = [ThemeMode::System, ThemeMode::Light, ThemeMode::Dark];
        v_flex()
            .gap_2()
            .child(t(cx, "setup-color-mode"))
            .child(
                gpui_kit::component::radio::RadioGroup::horizontal("settings-mode")
                    .children([
                        t(cx, "theme-system"),
                        t(cx, "theme-light"),
                        t(cx, "theme-dark"),
                    ])
                    .selected_index(
                        modes
                            .iter()
                            .position(|m| *m == self.controller.read(cx).preferences(cx).theme),
                    )
                    .disabled(self.controller.read(cx).busy(cx))
                    .on_click(cx.listener(move |this, index, _, cx| {
                        this.controller.update(cx, |c, cx| {
                            c.set_preference(PreferenceChange::Theme(modes[*index]), cx)
                        })
                    })),
            )
            .into_any_element()
    }
    fn render_theme_choices(&self, mode: Mode, cx: &Context<Self>) -> AnyElement {
        let draft = self.controller.read(cx).preferences(cx);
        let busy = self.controller.read(cx).busy(cx);
        let controller = self.controller.clone();
        let choices = app_theme::theme_choices(ThemeRegistry::global(cx), mode, &[]);
        container_query(move |size, _, cx| {
            preferences::theme_grid(
                (
                    if mode == Mode::Light {
                        "light-themes"
                    } else {
                        "dark-themes"
                    },
                    mode,
                    choices.clone(),
                ),
                &draft,
                &controller,
                preferences::theme_columns(size.width.as_f32()),
                busy,
                cx,
            )
        })
        .into_any_element()
    }
}

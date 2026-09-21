use super::*;
use crate::foundation::pi_resources::Kind;
use gpui_kit::component::{
    Sizable, ThemeMode as Mode, ThemeRegistry, WindowExt,
    group_box::GroupBoxVariant,
    setting::{SettingField, SettingGroup, SettingItem, SettingPage, Settings},
};

impl SettingsView {
    pub(super) fn settings_panel(&self, cx: &mut Context<Self>) -> Settings {
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
        let general = SettingPage::new(t(cx, "settings-page-general"))
            .icon(IconName::Settings)
            .resettable(false)
            .group(SettingGroup::new().item({
                let this = this.clone();
                SettingItem::new(
                    t(cx, "settings-language"),
                    SettingField::render(move |_, _, cx| {
                        this.update(cx, |this, cx| {
                            gpui_kit::component::combobox::Combobox::new(&this.language)
                                .w(px(220.))
                                .cleanable(false)
                                .disabled(this.controller.read(cx).busy(cx))
                                .into_any_element()
                        })
                        .unwrap_or_else(|_| div().into_any_element())
                    }),
                )
                .keywords(["通用 general language locale 语言"])
            }))
            .group(SettingGroup::new().item({
                let this = this.clone();
                SettingItem::new(
                    t(cx, "settings-config-heading"),
                    SettingField::render(move |options, _, cx| {
                        this.update(cx, |this, cx| this.render_config_actions(options, cx))
                            .unwrap_or_else(|_| div().into_any_element())
                    }),
                )
                .keywords(["通用 general config file 配置文件 reload 重新读取"])
            }));
        let general = general.group(
            SettingGroup::new().title(t(cx, "temporary-title")).item(
                SettingItem::new(
                    t(cx, "temporary-clean-released"),
                    SettingField::render(|_, _, cx| {
                        let owner = cx.try_global::<crate::app::temporary::Temporary>();
                        let busy = owner.is_some_and(|s| s.cleanup.is_some());
                        let result = owner.and_then(|s| s.cleanup_result.clone());
                        v_flex()
                            .gap_1()
                            .child(
                                Button::new("temporary-clean-released")
                                    .small()
                                    .icon(IconName::Trash)
                                    .label(t(cx, "temporary-clean"))
                                    .loading(busy)
                                    .disabled(busy || owner.is_none())
                                    .on_click(|_, window, cx| {
                                        window.open_dialog(cx, |dialog, _, cx| {
                                            dialog
                                                .title(t(cx, "temporary-clean-released"))
                                                .child(t(cx, "temporary-clean-help"))
                                                .footer(dialog_buttons(
                                                    "temporary-clean",
                                                    false,
                                                    false,
                                                    cx,
                                                ))
                                                .on_ok(|_, _, cx| {
                                                    crate::app::temporary::clean_released(cx);
                                                    true
                                                })
                                        });
                                    }),
                            )
                            .children(result.map(|result| {
                                div().text_sm().child(match result {
                                    Ok(count) => format!("{}: {count}", t(cx, "temporary-cleaned")),
                                    Err(error) => error,
                                })
                            }))
                            .into_any_element()
                    }),
                )
                .description(t(cx, "temporary-clean-help"))
                .keywords(["temporary cleanup 临时对话 清理"]),
            ),
        );
        let appearance = SettingPage::new(t(cx, "settings-theme"))
            .icon(IconName::Sparkles)
            .resettable(false)
            .group(SettingGroup::new().item({
                let this = this.clone();
                SettingItem::new(
                    t(cx, "setup-color-mode"),
                    SettingField::render(move |_, _, cx| {
                        this.update(cx, |this, cx| this.render_mode(cx))
                            .unwrap_or_else(|_| div().into_any_element())
                    }),
                )
                .description(t(cx, "settings-mode-help"))
                .keywords([
                    "外观 appearance color mode dark light system 颜色模式 深色 浅色 跟随系统",
                ])
            }))
            .group(
                SettingGroup::new()
                    .title(t(cx, "light-themes"))
                    .description(t(cx, "settings-light-help"))
                    .item(item(
                        "light-themes",
                        "外观 appearance light theme 浅色主题",
                        |this, _, cx| this.render_theme_choices(Mode::Light, cx),
                    )),
            )
            .group(
                SettingGroup::new()
                    .title(t(cx, "dark-themes"))
                    .description(t(cx, "settings-dark-help"))
                    .item(item(
                        "dark-themes",
                        "外观 appearance dark theme 深色主题",
                        |this, _, cx| this.render_theme_choices(Mode::Dark, cx),
                    )),
            );
        let pi = page(
            "settings-page-pi",
            vec![item(
                "settings-pi-command",
                "pi executable path environment version 路径 环境 检查",
                |this, _, cx| {
                    let probe = this.applied_pi.clone();
                    let command = this.controller.read(cx).preferences(cx).pi_command;
                    if !probe.read(cx).matches_command(command.as_deref()) {
                        cx.defer(move |cx| {
                            probe.update(cx, |probe, cx| probe.request(command, false, cx))
                        });
                    }
                    this.render_pi(cx)
                },
            )],
        );
        let keys = self.keys.clone();
        let keyboard = SettingPage::new(t(cx, "settings-page-keys"))
            .icon(IconName::Keyboard)
            .resettable(false)
            .group(keys::KeysView::actions(&keys, &self.global_keys))
            .groups(global_keys::GlobalKeys::groups(&self.global_keys, cx))
            .groups(
                [
                    keys::Group::Application,
                    keys::Group::Conversation,
                    keys::Group::Files,
                    keys::Group::Temporary,
                ]
                .into_iter()
                .map(|group| {
                    SettingGroup::new().title(t(cx, group.label())).items(
                        crate::state::keybindings::COMMANDS
                            .iter()
                            .enumerate()
                            .filter(|(_, command)| keys::Group::of(command.kind) == group)
                            .map(|(index, _)| keys::KeysView::item(&self.keys, index, cx)),
                    )
                }),
            );
        let resource_page = |key, kind, keywords: &str| {
            let resource_view = self.resources.downgrade();
            let section = |section, aliases: &str| {
                let resources = resource_view.clone();
                SettingItem::render(move |_, _, cx| {
                    resources
                        .update(cx, |view, cx| view.render_page(kind, section, cx))
                        .unwrap_or_else(|_| div().into_any_element())
                })
                .keywords(vec![
                    t(cx, key),
                    aliases.to_owned(),
                    "refresh reload 刷新 重新读取".to_owned(),
                ])
            };
            let header = resource_view.clone();
            let mut page = SettingPage::new(t(cx, key))
                .resettable(false)
                .icon(match kind {
                    Kind::Extension => IconName::Puzzle,
                    Kind::Skill => IconName::BookOpen,
                    _ => IconName::FileText,
                })
                .title_suffix(move |_, cx| {
                    header
                        .update(cx, |view, cx| view.render_header(cx))
                        .unwrap_or_else(|_| div().into_any_element())
                });
            if self.resources.read(cx).has_status(kind, cx) {
                page = page.group(SettingGroup::new().item(section(
                    resources::Section::Overview,
                    "refresh reload 刷新 重新读取",
                )));
            }
            if kind == Kind::Extension {
                return page.group(SettingGroup::new()
                    .item(section(resources::Section::Packages,"package extension skill theme template install update remove source version 包 扩展 技能 主题 模板 安装 更新 移除 来源 版本")));
            }
            if kind == Kind::Prompt {
                return page
                    .group(
                        SettingGroup::new()
                            .title(t(cx, "settings-system-heading"))
                            .items(resources::ResourcesView::system_prompt_items(
                                &self.resources,
                                cx,
                            )),
                    )
                    .group(
                        SettingGroup::new()
                            .item(section(resources::Section::Catalog, keywords))
                            .items(resources::ResourcesView::prompt_items(&self.resources, cx)),
                    );
            }
            page = page.group(
                SettingGroup::new()
                    .item(section(resources::Section::Catalog, keywords))
                    .items(resources::ResourcesView::skill_items(&self.resources, cx)),
            );
            page
        };
        let probe = self.applied_pi.read(cx);
        let unavailable = t(cx, "settings-about-unavailable");
        let mut about_items: Vec<_> = [
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
                SettingField::render(move |_, _, _| {
                    let tooltip = value.clone();
                    div()
                        .id(key)
                        .max_w(px(320.))
                        .text_sm()
                        .truncate()
                        .child(value.clone())
                        .tooltip(move |window, cx| {
                            gpui_kit::component::tooltip::Tooltip::new(tooltip.clone())
                                .build(window, cx)
                        })
                }),
            )
            .keywords(vec![aliases])
        })
        .collect();
        let gupi_item = about_items.remove(0);
        let about = SettingPage::new(t(cx, "settings-page-about"))
            .icon(IconName::Info)
            .resettable(false)
            .group(SettingGroup::new().title("Gupi").item(gupi_item))
            .group(
                SettingGroup::new()
                    .title("Pi")
                    .description(t(cx, "settings-about-help"))
                    .items(about_items),
            );
        Settings::new("gupi-settings")
            .with_group_variant(GroupBoxVariant::Normal)
            .page(general).page(appearance).page(pi).page(keyboard)
            .page(resource_page("settings-page-plugins",Kind::Extension,"package plugin extension install update remove source version path enable disable 插件 扩展 包 安装 更新 移除 启停"))
            .page(resource_page("settings-page-skills",Kind::Skill,"skill name description source path create edit delete enable disable 技能 创建 编辑 删除 启停"))
            .page(resource_page("settings-page-prompts",Kind::Prompt,"prompt template 创建 编辑 删除 模板"))
            .page(about)
    }
    pub(super) fn render_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let panel = self.settings_panel(cx);
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
            }))
            .into_any_element()
    }
    fn render_theme_choices(&self, mode: Mode, cx: &Context<Self>) -> AnyElement {
        let draft = self.controller.read(cx).preferences(cx);
        preferences::theme_grid(
            (
                if mode == Mode::Light {
                    "light-themes"
                } else {
                    "dark-themes"
                },
                mode,
                app_theme::theme_choices(ThemeRegistry::global(cx), mode, &[]),
            ),
            &draft,
            &self.controller,
            None,
            self.controller.read(cx).busy(cx),
            cx,
        )
    }
}

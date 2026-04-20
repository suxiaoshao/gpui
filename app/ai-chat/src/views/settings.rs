use crate::{
    app_menus,
    assets::IconName,
    components::hotkey_input::{HotkeyEvent, HotkeyInput, string_to_keystroke},
    i18n::{self, I18n},
    llm::provider_setting_groups,
    state::{AiChatConfig, Language, WindowPlacementKind, WorkspaceStore},
    tray,
};
use gpui::*;
use gpui_component::{
    Root, StyledExt, TitleBar, WindowExt,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    notification::{Notification, NotificationType},
    setting::{SettingField, SettingGroup, SettingItem, SettingPage, Settings},
    v_flex,
};
use std::{any::TypeId, ops::Deref};
use tracing::{Level, event};

pub(super) mod appearance_settings;
pub(super) mod shortcut_settings;

use self::appearance_settings::AppearanceSettingsPage;
use self::shortcut_settings::ShortcutSettingsPage;

actions!([OpenSetting]);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SettingsOpenTarget {
    General,
    Provider,
}

const SETTINGS_WINDOW_FALLBACK_SIZE: Size<Pixels> = size(px(960.), px(720.));

pub fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new(
        if cfg!(target_os = "macos") {
            "cmd-,"
        } else {
            "ctrl-,"
        },
        OpenSetting,
        None,
    )]);
    cx.on_action(open_settings_window);
}

pub struct SettingsView {
    focus_handle: FocusHandle,
    hotkey_input: Entity<HotkeyInput>,
    appearance_settings: Entity<AppearanceSettingsPage>,
    shortcut_settings: Entity<ShortcutSettingsPage>,
    open_target: SettingsOpenTarget,
    _subscriptions: Vec<Subscription>,
}

impl SettingsView {
    fn new(open_target: SettingsOpenTarget, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        let hotkey_input = cx.new(|cx| {
            let temporary_hotkey = cx.global::<AiChatConfig>().temporary_hotkey.clone();
            HotkeyInput::new("temporary-hotkey-input", window, cx)
                .default_value(temporary_hotkey.and_then(|x| string_to_keystroke(&x)))
        });
        let appearance_settings = cx.new(|cx| AppearanceSettingsPage::new(window, cx));
        let shortcut_settings = cx.new(|cx| ShortcutSettingsPage::new(window, cx));
        let _subscriptions = vec![
            cx.subscribe(&hotkey_input, Self::subscribe_hotkey_changes),
            cx.observe_window_bounds(window, |_settings, window, cx| {
                if !cx.has_global::<WorkspaceStore>() {
                    return;
                }

                let window_bounds = window.window_bounds();
                let display_id = window.display(cx).map(|display| display.id());
                cx.global::<WorkspaceStore>()
                    .deref()
                    .clone()
                    .update(cx, |workspace, cx| {
                        workspace.set_window_bounds(
                            WindowPlacementKind::Settings,
                            window_bounds,
                            display_id,
                            cx,
                        );
                    });
            }),
        ];
        Self {
            focus_handle,
            hotkey_input,
            appearance_settings,
            shortcut_settings,
            open_target,
            _subscriptions,
        }
    }
    fn subscribe_hotkey_changes(
        &mut self,
        state: Entity<HotkeyInput>,
        event: &HotkeyEvent,
        cx: &mut Context<Self>,
    ) {
        cx.update_global::<AiChatConfig, _>(|config, cx| match event {
            HotkeyEvent::Confirm(shared_string) => {
                config.set_temporary_hotkey(Some(shared_string.to_string()), cx);
                state.update(cx, move |this, _cx| {
                    this.set_default_value(string_to_keystroke(shared_string));
                });
            }
            HotkeyEvent::Cancel => {
                config.set_temporary_hotkey(None, cx);
                state.update(cx, move |this, _cx| {
                    this.set_default_value(None);
                });
            }
        });
    }

    fn minimize(&mut self, _: &app_menus::Minimize, window: &mut Window, _: &mut Context<Self>) {
        window.minimize_window();
    }

    fn zoom(&mut self, _: &app_menus::Zoom, window: &mut Window, _: &mut Context<Self>) {
        window.zoom_window();
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (
            page_general,
            page_appearance,
            page_provider,
            page_shortcuts,
            group_basic_options,
            field_language,
            field_http_proxy,
            field_temporary_hotkey,
            field_config_file,
            button_open,
            open_config_failed,
        ) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("settings-page-general"),
                i18n.t("settings-page-appearance"),
                i18n.t("settings-page-provider"),
                i18n.t("settings-page-shortcuts"),
                i18n.t("settings-group-basic-options"),
                i18n.t("field-language"),
                i18n.t("field-http-proxy"),
                i18n.t("field-temporary-conversation-hotkey"),
                i18n.t("field-config-file"),
                i18n.t("button-open"),
                i18n.t("notify-open-config-file-failed"),
            )
        };
        let settings_title = cx.global::<I18n>().t("settings-title");
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);
        let hotkey_input = self.hotkey_input.clone();
        let provider_page = provider_setting_groups().into_iter().fold(
            SettingPage::new(page_provider),
            |page: SettingPage, group| page.group(group),
        );
        let shortcuts_page = SettingPage::new(page_shortcuts).group(SettingGroup::new().item(
            SettingItem::render({
                let page = self.shortcut_settings.clone();
                move |_options, _window, _cx| page.clone()
            }),
        ));
        let appearance_page = SettingPage::new(page_appearance).group(SettingGroup::new().item(
            SettingItem::render({
                let page = self.appearance_settings.clone();
                move |_options, _window, _cx| page.clone()
            }),
        ));
        let general_page = SettingPage::new(page_general).group(
            SettingGroup::new()
                .title(group_basic_options)
                .item(SettingItem::new(
                    field_language,
                    SettingField::dropdown(
                        Language::options()
                            .into_iter()
                            .map(|language| {
                                (
                                    language.to_string().into(),
                                    cx.global::<I18n>().t(language.label_key()).into(),
                                )
                            })
                            .collect(),
                        |cx: &App| {
                            let config = cx.global::<AiChatConfig>();
                            config.language().to_string().into()
                        },
                        |val: SharedString, cx: &mut App| {
                            {
                                let config = cx.global_mut::<AiChatConfig>();
                                config.set_language(Language::from_str(&val));
                            }
                            i18n::refresh_i18n(cx);
                            cx.set_menus(app_menus::app_menus(cx.global::<I18n>()));
                            tray::refresh(cx);
                            cx.refresh_windows();
                        },
                    ),
                ))
                .item(SettingItem::new(
                    field_http_proxy,
                    SettingField::input(
                        |cx: &App| {
                            let config = cx.global::<AiChatConfig>();
                            config
                                .http_proxy
                                .as_ref()
                                .map(|proxy| proxy.into())
                                .unwrap_or_default()
                        },
                        |val: SharedString, cx: &mut App| {
                            if val.is_empty() {
                                cx.global_mut::<AiChatConfig>().set_http_proxy(None);
                            } else {
                                cx.global_mut::<AiChatConfig>()
                                    .set_http_proxy(Some(val.into()));
                            }
                        },
                    ),
                ))
                .item(SettingItem::new(
                    field_temporary_hotkey,
                    SettingField::render(move |_options, _window, _cx| hotkey_input.clone()),
                ))
                .item(SettingItem::new(
                    field_config_file,
                    SettingField::render(move |_options, _window, _cx| {
                        Button::new("open-config-file")
                            .icon(IconName::FilePen)
                            .label(button_open.clone())
                            .ghost()
                            .on_click({
                                let open_config_failed = open_config_failed.clone();
                                move |_, window, cx| match AiChatConfig::path() {
                                    Ok(path) => cx.open_with_system(&path),
                                    Err(err) => window.push_notification(
                                        Notification::new()
                                            .title(open_config_failed.clone())
                                            .message(err.to_string())
                                            .with_type(NotificationType::Error),
                                        cx,
                                    ),
                                }
                            })
                    }),
                )),
        );
        let (settings_id, pages) = match self.open_target {
            SettingsOpenTarget::General => (
                "my-settings-general",
                vec![general_page, appearance_page, provider_page, shortcuts_page],
            ),
            SettingsOpenTarget::Provider => (
                "my-settings-provider",
                vec![provider_page, general_page, appearance_page, shortcuts_page],
            ),
        };
        v_flex()
            .id("settings")
            .track_focus(&self.focus_handle)
            .size_full()
            .overflow_hidden()
            .on_action(cx.listener(|_this, _: &OpenSetting, window, _cx| {
                window.remove_window();
            }))
            .on_action(cx.listener(Self::minimize))
            .on_action(cx.listener(Self::zoom))
            .child(
                div()
                    .child(TitleBar::new().child(settings_title_bar_title(settings_title)))
                    .flex_initial(),
            )
            .child(
                div().flex_1().overflow_hidden().child(
                    Settings::new(settings_id)
                        .with_group_variant(gpui_component::group_box::GroupBoxVariant::Outline)
                        .sidebar_width(px(280.))
                        .pages(pages),
                ),
            )
            .children(dialog_layer)
            .children(notification_layer)
    }
}

pub fn open_settings_window(_: &OpenSetting, cx: &mut App) {
    open_settings_window_to(SettingsOpenTarget::General, true, cx);
}

pub(crate) fn open_settings_window_from_menu(cx: &mut App) {
    open_settings_window_to(SettingsOpenTarget::General, false, cx);
}

pub(crate) fn open_provider_settings_window(cx: &mut App) {
    open_settings_window_to(SettingsOpenTarget::Provider, false, cx);
}

fn open_settings_window_to(target: SettingsOpenTarget, toggle_if_active: bool, cx: &mut App) {
    let span = tracing::info_span!("open_settings_window");
    let _guard = span.enter();
    let exists_settings = cx.windows().iter().find_map(|window| {
        window
            .downcast::<Root>()
            .and_then(|window_root| match window_root.read(cx) {
                Ok(root) if root.view().entity_type() == TypeId::of::<SettingsView>() => {
                    Some(window_root)
                }
                _ => None,
            })
    });
    match exists_settings {
        Some(window) => {
            match window.update(cx, |root, window, cx| {
                if let Ok(settings) = root.view().clone().downcast::<SettingsView>() {
                    settings.update(cx, |settings, cx| {
                        settings.open_target = target;
                        cx.notify();
                    });
                }
                if toggle_if_active && window.is_window_active() {
                    window.remove_window();
                } else {
                    window.activate_window();
                }
            }) {
                Ok(_) => {}
                Err(err) => {
                    event!(Level::ERROR, "activate settings window error: {}", err);
                }
            };
        }
        None => {
            inner_open_settings_window(target, cx);
        }
    }
}

fn inner_open_settings_window(target: SettingsOpenTarget, cx: &mut App) {
    let title = cx.global::<I18n>().t("settings-title");
    let placement = crate::state::workspace::restored_window_placement(
        WindowPlacementKind::Settings,
        SETTINGS_WINDOW_FALLBACK_SIZE,
        cx,
    );
    match cx.open_window(
        WindowOptions {
            window_bounds: Some(placement.window_bounds),
            display_id: placement.display_id,
            titlebar: Some(settings_titlebar_options(title)),
            window_background: WindowBackgroundAppearance::Blurred,
            ..Default::default()
        },
        |window, cx| {
            let setting = cx.new(|cx| SettingsView::new(target, window, cx));
            cx.new(|cx| Root::new(setting, window, cx))
        },
    ) {
        Ok(_) => {}
        Err(err) => {
            event!(Level::ERROR, "open settings window: {}", err);
        }
    };
}

fn settings_title_bar_title(title: impl Into<SharedString>) -> impl IntoElement {
    h_flex()
        .w_full()
        .h_full()
        .justify_center()
        .overflow_hidden()
        .pr_2()
        .child(Label::new(title).text_sm().font_medium().truncate())
}

fn settings_titlebar_options(title: impl Into<SharedString>) -> TitlebarOptions {
    TitlebarOptions {
        title: Some(title.into()),
        ..TitleBar::title_bar_options()
    }
}

#[cfg(test)]
mod tests {
    use super::settings_titlebar_options;
    use gpui_component::TitleBar;

    #[test]
    fn settings_window_uses_component_titlebar_options() {
        let titlebar = settings_titlebar_options("Settings");
        let expected = TitleBar::title_bar_options();

        assert_eq!(
            titlebar.title.as_ref().map(|title| title.as_ref()),
            Some("Settings")
        );
        assert_eq!(titlebar.appears_transparent, expected.appears_transparent);
        assert_eq!(
            titlebar.traffic_light_position,
            expected.traffic_light_position
        );
    }
}

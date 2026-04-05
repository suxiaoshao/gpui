use crate::{
    components::hotkey_input::{HotkeyEvent, HotkeyInput, string_to_keystroke},
    i18n::I18n,
    llm::provider_setting_groups,
    state::{AiChatConfig, ThemeMode},
};
use gpui::*;
use gpui_component::{
    Root,
    setting::{SettingField, SettingGroup, SettingItem, SettingPage, Settings},
    v_flex,
};
use std::any::TypeId;
use tracing::{Level, event};

pub(super) mod shortcut_settings;

use self::shortcut_settings::ShortcutSettingsPage;

actions!([OpenSetting]);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SettingsOpenTarget {
    General,
    Provider,
}

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
    shortcut_settings: Entity<ShortcutSettingsPage>,
    open_target: SettingsOpenTarget,
    _subscriptions: Vec<Subscription>,
}

impl SettingsView {
    fn new(open_target: SettingsOpenTarget, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window);
        let hotkey_input = cx.new(|cx| {
            let temporary_hotkey = cx.global::<AiChatConfig>().temporary_hotkey.clone();
            HotkeyInput::new("temporary-hotkey-input", window, cx)
                .default_value(temporary_hotkey.and_then(|x| string_to_keystroke(&x)))
        });
        let shortcut_settings = cx.new(|cx| ShortcutSettingsPage::new(window, cx));
        let _subscriptions = vec![cx.subscribe(&hotkey_input, Self::subscribe_hotkey_changes)];
        Self {
            focus_handle,
            hotkey_input,
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
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (
            page_general,
            page_provider,
            page_shortcuts,
            group_basic_options,
            field_theme,
            field_http_proxy,
            field_temporary_hotkey,
        ) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("settings-page-general"),
                i18n.t("settings-page-provider"),
                i18n.t("settings-page-shortcuts"),
                i18n.t("settings-group-basic-options"),
                i18n.t("field-theme"),
                i18n.t("field-http-proxy"),
                i18n.t("field-temporary-conversation-hotkey"),
            )
        };
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
        let general_page = SettingPage::new(page_general).group(
            SettingGroup::new()
                .title(group_basic_options)
                .item(SettingItem::new(
                    field_theme,
                    SettingField::dropdown(
                        vec![
                            (
                                ThemeMode::Light.to_string().into(),
                                ThemeMode::Light.to_string().into(),
                            ),
                            (
                                ThemeMode::Dark.to_string().into(),
                                ThemeMode::Dark.to_string().into(),
                            ),
                            (
                                ThemeMode::System.to_string().into(),
                                ThemeMode::System.to_string().into(),
                            ),
                        ],
                        |cx: &App| {
                            let config = cx.global::<AiChatConfig>();
                            config.theme_mode().to_string().into()
                        },
                        |val: SharedString, cx: &mut App| {
                            let config = cx.global_mut::<AiChatConfig>();
                            config.set_theme_mode(ThemeMode::from_str(&val));
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
                )),
        );
        let (settings_id, pages) = match self.open_target {
            SettingsOpenTarget::General => (
                "my-settings-general",
                vec![general_page, provider_page, shortcuts_page],
            ),
            SettingsOpenTarget::Provider => (
                "my-settings-provider",
                vec![provider_page, general_page, shortcuts_page],
            ),
        };
        v_flex()
            .id("settings")
            .track_focus(&self.focus_handle)
            .size_full()
            .children(dialog_layer)
            .children(notification_layer)
            .on_action(cx.listener(|_this, _: &OpenSetting, window, _cx| {
                window.remove_window();
            }))
            .child(
                Settings::new(settings_id)
                    .with_group_variant(gpui_component::group_box::GroupBoxVariant::Outline)
                    .pages(pages),
            )
    }
}

pub fn open_settings_window(_: &OpenSetting, cx: &mut App) {
    open_settings_window_to(SettingsOpenTarget::General, true, cx);
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
    match cx.open_window(
        WindowOptions {
            titlebar: Some(TitlebarOptions {
                title: Some(title.into()),
                ..Default::default()
            }),
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

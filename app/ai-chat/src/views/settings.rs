use crate::{
    adapter::{Adapter, OpenAIAdapter, OpenAIStreamAdapter},
    components::hotkey_input::{HotkeyEvent, HotkeyInput, string_to_keystroke},
    config::{AiChatConfig, ThemeMode},
    i18n::I18n,
};
use gpui::*;
use gpui_component::{
    Root,
    setting::{SettingField, SettingGroup, SettingItem, SettingPage, Settings},
    v_flex,
};
use std::any::TypeId;
use tracing::{Level, event};

actions!([OpenSetting]);

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
    _subscriptions: Vec<Subscription>,
}

impl SettingsView {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window);
        let hotkey_input = cx.new(|cx| {
            let temporary_hotkey = cx.global::<AiChatConfig>().temporary_hotkey.clone();
            HotkeyInput::new(window, cx)
                .default_value(temporary_hotkey.and_then(|x| string_to_keystroke(&x)))
        });
        let _subscriptions = vec![cx.subscribe(&hotkey_input, Self::subscribe_hotkey_changes)];
        Self {
            focus_handle,
            hotkey_input,
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
            page_adapter,
            group_basic_options,
            field_theme,
            field_http_proxy,
            field_temporary_hotkey,
        ) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("settings-page-general"),
                i18n.t("settings-page-adapter"),
                i18n.t("settings-group-basic-options"),
                i18n.t("field-theme"),
                i18n.t("field-http-proxy"),
                i18n.t("field-temporary-conversation-hotkey"),
            )
        };
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);
        let hotkey_input = self.hotkey_input.clone();
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
                Settings::new("my-settings")
                    .with_group_variant(gpui_component::group_box::GroupBoxVariant::Outline)
                    .pages(vec![
                        SettingPage::new(page_general).group(
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
                                                cx.global_mut::<AiChatConfig>()
                                                    .set_http_proxy(None);
                                            } else {
                                                cx.global_mut::<AiChatConfig>()
                                                    .set_http_proxy(Some(val.into()));
                                            }
                                        },
                                    ),
                                ))
                                .item(SettingItem::new(
                                    field_temporary_hotkey,
                                    SettingField::render(move |_options, _window, _cx| {
                                        hotkey_input.clone()
                                    }),
                                )),
                        ),
                        SettingPage::new(page_adapter)
                            .group(OpenAIAdapter.setting_group())
                            .group(OpenAIStreamAdapter.setting_group()),
                    ]),
            )
    }
}

pub fn open_settings_window(_: &OpenSetting, cx: &mut App) {
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
            match window.update(cx, |_this, window, _cx| {
                if window.is_window_active() {
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
            inner_open_settings_window(cx);
        }
    }
}

fn inner_open_settings_window(cx: &mut App) {
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
            let setting = cx.new(|cx| SettingsView::new(window, cx));
            cx.new(|cx| Root::new(setting, window, cx))
        },
    ) {
        Ok(_) => {}
        Err(err) => {
            event!(Level::ERROR, "open settings window: {}", err);
        }
    };
}

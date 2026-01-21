use crate::config::{AiChatConfig, ThemeMode};
use gpui::*;
use gpui_component::{
    Root, Sizable, TitleBar,
    input::{Input, InputState},
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
    hotkey_input: Entity<InputState>,
}

impl SettingsView {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window);
        let hotkey_input = cx.new(|cx| InputState::new(window, cx));
        Self {
            focus_handle,
            hotkey_input,
        }
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
            .child(TitleBar::new().child(div().flex().items_center().gap_3().child("Settings")))
            .child(Settings::new("my-settings").pages(vec![
                    SettingPage::new("General").group(
                        SettingGroup::new()
                            .title("Basic Options")
                            .item(SettingItem::new(
                                "Theme",
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
                                "Http Proxy",
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
                                "Temporary Conversation Hotkey",
                                SettingField::render(move |options, window, cx| {
                                    Input::new(&hotkey_input).with_size(options.size).w_64()
                                }),
                            )),
                    ),
                    SettingPage::new("Adapter"),
                ]))
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
    match cx.open_window(
        WindowOptions {
            titlebar: Some(TitleBar::title_bar_options()),
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

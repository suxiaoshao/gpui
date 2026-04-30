use crate::{
    app::{self, menus, tray},
    components::hotkey_input::HotkeyInput,
    foundation::assets::IconName,
    foundation::i18n::{self, I18n},
    state::{AiChatConfig, Language},
};
use gpui::*;
use gpui_component::{
    Sizable, WindowExt,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    menu::{DropdownMenu, PopupMenuItem},
    notification::{Notification, NotificationType},
};

use super::layout::{SettingsRow, SettingsSection};

struct SettingsTextInputState {
    input: Entity<InputState>,
    last_value: String,
    _subscription: Subscription,
}

pub(super) fn render(
    hotkey_input: Entity<HotkeyInput>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let (
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
            i18n.t("settings-group-basic-options"),
            i18n.t("field-language"),
            i18n.t("field-http-proxy"),
            i18n.t("field-temporary-conversation-hotkey"),
            i18n.t("field-config-file"),
            i18n.t("button-open"),
            i18n.t("notify-open-config-file-failed"),
        )
    };

    SettingsSection::new(group_basic_options)
        .child(SettingsRow::new(field_language, language_dropdown(cx)))
        .child(SettingsRow::new(
            field_http_proxy,
            app_http_proxy_input(window, cx),
        ))
        .child(SettingsRow::new(field_temporary_hotkey, hotkey_input))
        .child(SettingsRow::new(
            field_config_file,
            Button::new("open-config-file")
                .icon(IconName::FilePen)
                .label(button_open)
                .ghost()
                .small()
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
                }),
        ))
        .into_any_element()
}

fn language_dropdown(cx: &mut App) -> AnyElement {
    let current_language = cx.global::<AiChatConfig>().language();
    let language_options = Language::options()
        .into_iter()
        .map(|language| (language, cx.global::<I18n>().t(language.label_key())))
        .collect::<Vec<_>>();
    let current_label = language_options
        .iter()
        .find(|(language, _)| *language == current_language)
        .map(|(_, label)| label.clone())
        .unwrap_or_else(|| current_language.to_string());

    Button::new("settings-language-dropdown")
        .label(current_label)
        .dropdown_caret(true)
        .outline()
        .small()
        .w(px(220.))
        .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _, _| {
            language_options
                .iter()
                .fold(menu, |menu, (language, label)| {
                    let language = *language;
                    menu.item(
                        PopupMenuItem::new(label.clone())
                            .checked(language == current_language)
                            .on_click(move |_, _, cx| {
                                {
                                    let config = cx.global_mut::<AiChatConfig>();
                                    config.set_language(language);
                                }
                                i18n::refresh_i18n(cx);
                                menus::sync_app_menus(cx);
                                app::reload_app_menu_bars(cx);
                                tray::refresh(cx);
                                cx.refresh_windows();
                            }),
                    )
                })
        })
        .into_any_element()
}

fn app_http_proxy_input(window: &mut Window, cx: &mut App) -> AnyElement {
    let initial_value = cx
        .global::<AiChatConfig>()
        .http_proxy
        .as_ref()
        .cloned()
        .unwrap_or_default();
    let state = window
        .use_keyed_state("settings-http-proxy-input", cx, |window, cx| {
            let input =
                cx.new(|cx| InputState::new(window, cx).default_value(initial_value.clone()));
            let _subscription = cx.subscribe_in(&input, window, {
                move |state: &mut SettingsTextInputState, input, event: &InputEvent, _window, cx| {
                    if !matches!(event, InputEvent::Change) {
                        return;
                    }

                    let next_value = input.read(cx).value().to_string();
                    if next_value == state.last_value {
                        return;
                    }

                    let next_proxy = if next_value.is_empty() {
                        None
                    } else {
                        Some(next_value.clone())
                    };
                    cx.global_mut::<AiChatConfig>().set_http_proxy(next_proxy);
                    state.last_value = next_value;
                }
            });

            SettingsTextInputState {
                input,
                last_value: initial_value,
                _subscription,
            }
        })
        .read(cx);

    Input::new(&state.input)
        .small()
        .w(px(320.))
        .into_any_element()
}

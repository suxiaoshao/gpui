use crate::{
    app::{self, menus},
    components::hotkey_input::HotkeyInput,
    foundation::{self, I18n, assets::IconName},
    state::{self, AiChat2AppSettings, AiChat2Config},
};
use ai_chat_core::AppLanguage;
use gpui::*;
use gpui_component::{
    Sizable,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    menu::{DropdownMenu, PopupMenuItem},
};

use super::{
    layout::{settings_group, settings_row_item},
    push_settings_error,
};

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

    settings_group(
        group_basic_options,
        [
            settings_row_item(field_language, |_, cx| language_dropdown(cx)),
            settings_row_item(field_http_proxy, |window, cx| {
                app_http_proxy_input(window, cx)
            }),
            settings_row_item(field_temporary_hotkey, move |_, _| {
                hotkey_input.clone().into_any_element()
            }),
            settings_row_item(field_config_file, move |_window, _cx| {
                Button::new("open-config-file")
                    .icon(IconName::FilePen)
                    .label(button_open.clone())
                    .ghost()
                    .small()
                    .on_click({
                        let open_config_failed = open_config_failed.clone();
                        move |_, window, cx| match AiChat2Config::path() {
                            Ok(path) => cx.open_with_system(&path),
                            Err(err) => {
                                push_settings_error(window, cx, open_config_failed.clone(), err)
                            }
                        }
                    })
                    .into_any_element()
            }),
        ],
        window,
        cx,
    )
}

fn language_dropdown(cx: &mut App) -> AnyElement {
    let current_language = cx.global::<AiChat2AppSettings>().language();
    let language_options = language_options()
        .into_iter()
        .map(|language| {
            (
                language,
                cx.global::<I18n>().t(language_label_key(language)),
            )
        })
        .collect::<Vec<_>>();
    let current_label = language_options
        .iter()
        .find(|(language, _)| *language == current_language)
        .map(|(_, label)| label.clone())
        .unwrap_or_else(|| language_label_key(current_language).to_string());
    let save_failed = cx.global::<I18n>().t("notify-save-settings-failed");

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
                    let save_failed = save_failed.clone();
                    menu.item(
                        PopupMenuItem::new(label.clone())
                            .checked(language == current_language)
                            .on_click(
                                move |_, window, cx| match state::config::update_app_settings(
                                    cx,
                                    |payload| {
                                        payload.language = language;
                                    },
                                ) {
                                    Ok(_) => {
                                        foundation::init_i18n(cx);
                                        menus::sync_app_menus(cx);
                                        app::reload_app_menu_bars(cx);
                                        cx.refresh_windows();
                                    }
                                    Err(err) => {
                                        push_settings_error(window, cx, save_failed.clone(), err);
                                    }
                                },
                            ),
                    )
                })
        })
        .into_any_element()
}

fn app_http_proxy_input(window: &mut Window, cx: &mut App) -> AnyElement {
    let initial_value = cx
        .global::<AiChat2AppSettings>()
        .http_proxy()
        .map(str::to_string)
        .unwrap_or_default();
    let state = window
        .use_keyed_state("settings-http-proxy-input", cx, |window, cx| {
            let input =
                cx.new(|cx| InputState::new(window, cx).default_value(initial_value.clone()));
            let _subscription = cx.subscribe_in(&input, window, {
                move |input_state: &mut SettingsTextInputState,
                      input,
                      event: &InputEvent,
                      window,
                      cx| {
                    if !matches!(event, InputEvent::Change) {
                        return;
                    }

                    let next_value = input.read(cx).value().to_string();
                    if next_value == input_state.last_value {
                        return;
                    }

                    let next_proxy = if next_value.is_empty() {
                        None
                    } else {
                        Some(next_value.clone())
                    };
                    match state::config::update_app_settings(cx, |payload| {
                        payload.http_proxy = next_proxy.clone();
                    }) {
                        Ok(_) => {
                            input_state.last_value = next_value;
                        }
                        Err(err) => {
                            let title = cx.global::<I18n>().t("notify-save-settings-failed");
                            push_settings_error(window, cx, title, err);
                        }
                    }
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

const fn language_options() -> [AppLanguage; 3] {
    [
        AppLanguage::System,
        AppLanguage::English,
        AppLanguage::Chinese,
    ]
}

const fn language_label_key(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::System => "language-system",
        AppLanguage::English => "language-english",
        AppLanguage::Chinese => "language-chinese",
    }
}

#[cfg(test)]
mod tests {
    use super::{language_label_key, language_options};
    use ai_chat_core::AppLanguage;

    #[test]
    fn language_options_match_settings_order() {
        assert_eq!(
            language_options(),
            [
                AppLanguage::System,
                AppLanguage::English,
                AppLanguage::Chinese
            ]
        );
        assert_eq!(language_label_key(AppLanguage::System), "language-system");
    }
}

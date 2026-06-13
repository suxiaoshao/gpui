use crate::{
    app::{self, menus},
    components::hotkey_input::{HotkeyInput, format_hotkey_label, string_to_keystroke},
    foundation::{self, I18n, assets::IconName},
    state::{self, AiChat2AppSettings, AiChat2Config},
};
use ai_chat_core::AppLanguage;
use gpui::*;
use gpui_component::{
    ActiveTheme, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    dialog::{DialogAction, DialogClose, DialogFooter},
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    menu::{DropdownMenu, PopupMenuItem},
    v_flex,
};
use tracing::{Level, event};

use super::{
    layout::{settings_group, settings_row_item},
    push_settings_error,
};

struct SettingsTextInputState {
    input: Entity<InputState>,
    last_value: String,
    _subscription: Subscription,
}

pub(super) fn render(window: &mut Window, cx: &mut App) -> AnyElement {
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
            settings_row_item(field_temporary_hotkey, temporary_hotkey_control),
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

fn temporary_hotkey_control(_window: &mut Window, cx: &mut App) -> AnyElement {
    let current_hotkey = cx
        .global::<AiChat2AppSettings>()
        .temporary_hotkey()
        .map(str::to_string);
    let has_hotkey = current_hotkey.is_some();
    let current_label = current_hotkey
        .as_deref()
        .map(format_hotkey_label)
        .unwrap_or_else(|| cx.global::<I18n>().t("hotkey-not-set").to_string());
    let edit_label = cx.global::<I18n>().t("button-edit");

    h_flex()
        .items_center()
        .justify_end()
        .gap_2()
        .child(
            Label::new(current_label)
                .text_sm()
                .text_color(if has_hotkey {
                    cx.theme().foreground
                } else {
                    cx.theme().muted_foreground
                }),
        )
        .child(
            Button::new("temporary-hotkey-edit")
                .icon(IconName::Pencil)
                .label(edit_label)
                .outline()
                .small()
                .on_click(|_, window, cx| {
                    open_temporary_hotkey_dialog(window, cx);
                }),
        )
        .into_any_element()
}

fn open_temporary_hotkey_dialog(window: &mut Window, cx: &mut App) -> Entity<HotkeyInput> {
    let (title, cancel_label, save_label) = {
        let i18n = cx.global::<I18n>();
        (
            i18n.t("dialog-temporary-hotkey-title"),
            i18n.t("button-cancel"),
            i18n.t("provider-action-save"),
        )
    };
    let current_hotkey = cx
        .global::<AiChat2AppSettings>()
        .temporary_hotkey()
        .and_then(string_to_keystroke);
    let hotkey_input = cx.new(|cx| {
        HotkeyInput::new("temporary-hotkey-dialog-input", window, cx).default_value(current_hotkey)
    });
    let hotkey_input_to_focus = hotkey_input.clone();
    let hotkey_input_to_return = hotkey_input.clone();

    window.open_dialog(cx, move |dialog, _window, _cx| {
        let hotkey_input = hotkey_input.clone();
        dialog
            .title(title.clone())
            .w(px(420.))
            .on_ok({
                let hotkey_input = hotkey_input.clone();
                move |_, window, cx| confirm_temporary_hotkey_dialog(&hotkey_input, window, cx)
            })
            .child(v_flex().w_full().child(hotkey_input.clone()))
            .footer(
                DialogFooter::new()
                    .child(
                        DialogClose::new().child(
                            Button::new("temporary-hotkey-cancel").label(cancel_label.clone()),
                        ),
                    )
                    .child(
                        DialogAction::new().child(
                            Button::new("temporary-hotkey-save")
                                .primary()
                                .label(save_label.clone()),
                        ),
                    ),
            )
    });

    window.defer(cx, move |window, cx| {
        hotkey_input_to_focus.update(cx, |hotkey_input, cx| {
            hotkey_input.focus(window, cx);
        });
    });

    hotkey_input_to_return
}

fn confirm_temporary_hotkey_dialog(
    hotkey_input: &Entity<HotkeyInput>,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let next_hotkey = hotkey_input.read(cx).current_hotkey_string();
    save_temporary_hotkey(next_hotkey, window, cx)
}

fn save_temporary_hotkey(next_hotkey: Option<String>, window: &mut Window, cx: &mut App) -> bool {
    let previous_hotkey = cx
        .global::<AiChat2AppSettings>()
        .temporary_hotkey()
        .map(str::to_string);

    if let Err(err) = state::GlobalHotkeyState::update_temporary_hotkey(
        previous_hotkey.as_deref(),
        next_hotkey.as_deref(),
        cx,
    ) {
        let title = cx.global::<I18n>().t("notify-hotkey-register-failed");
        push_settings_error(window, cx, title, err);
        return false;
    }

    if let Err(err) = state::config::update_app_settings(cx, |payload| {
        payload.temporary_hotkey = next_hotkey.clone();
    }) {
        if let Err(rollback_err) = state::GlobalHotkeyState::update_temporary_hotkey(
            next_hotkey.as_deref(),
            previous_hotkey.as_deref(),
            cx,
        ) {
            event!(
                Level::ERROR,
                error = ?rollback_err,
                "rollback ai-chat2 temporary hotkey runtime failed after settings save failure"
            );
        }
        let title = cx.global::<I18n>().t("notify-save-settings-failed");
        push_settings_error(window, cx, title, err);
        return false;
    }

    true
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
    use super::{
        confirm_temporary_hotkey_dialog, language_label_key, language_options,
        open_temporary_hotkey_dialog, save_temporary_hotkey,
    };
    use crate::{
        database::FreshStoreGlobal,
        foundation,
        state::{self, AiChat2AppSettings},
    };
    use ai_chat_core::{AppLanguage, AppSettingsPayload};
    use gpui::{AppContext as _, Render, TestAppContext, VisualTestContext, WindowHandle};
    use gpui_component::WindowExt;
    use tempfile::{TempDir, tempdir};

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

    #[gpui::test]
    fn save_temporary_hotkey_updates_config_and_runtime(cx: &mut TestAppContext) {
        let _dir = init_hotkey_settings_test(cx, Some("cmd+shift+j"));
        let window = open_test_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);

        cx.update(|window, cx| {
            assert!(save_temporary_hotkey(
                Some("cmd+shift+k".to_string()),
                window,
                cx
            ));
        });

        cx.update(|_, cx| {
            assert_eq!(
                cx.global::<AiChat2AppSettings>().temporary_hotkey(),
                Some("cmd+shift+k")
            );
            assert_eq!(
                state::GlobalHotkeyState::diagnostics_snapshot(cx)
                    .temporary_hotkey
                    .as_deref(),
                Some("cmd+shift+k")
            );
        });
    }

    #[gpui::test]
    fn invalid_temporary_hotkey_does_not_change_settings_or_runtime(cx: &mut TestAppContext) {
        let _dir = init_hotkey_settings_test(cx, Some("cmd+shift+j"));
        let window = open_test_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);

        cx.update(|window, cx| {
            assert!(!save_temporary_hotkey(
                Some("cmd+shift+".to_string()),
                window,
                cx
            ));
        });

        cx.update(|_, cx| {
            assert_eq!(
                cx.global::<AiChat2AppSettings>().temporary_hotkey(),
                Some("cmd+shift+j")
            );
            assert_eq!(
                state::GlobalHotkeyState::diagnostics_snapshot(cx)
                    .temporary_hotkey
                    .as_deref(),
                Some("cmd+shift+j")
            );
            let persisted = crate::database::repository(cx)
                .get_app_settings()
                .expect("load app settings")
                .expect("settings record");
            assert_eq!(
                persisted.settings.temporary_hotkey.as_deref(),
                Some("cmd+shift+j")
            );
        });
    }

    #[gpui::test]
    fn invalid_temporary_hotkey_confirm_keeps_dialog_open(cx: &mut TestAppContext) {
        let _dir = init_hotkey_settings_test(cx, Some("cmd+shift+j"));
        let window = open_test_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);

        cx.update(|_, cx| {
            state::config::update_app_settings(cx, |payload| {
                payload.temporary_hotkey = Some("cmd+shift+".to_string());
            })
            .expect("seed invalid temporary hotkey setting");
        });
        let hotkey_input = cx.update(open_temporary_hotkey_dialog);
        let saved = cx.update(|window, cx| {
            assert!(window.has_active_dialog(cx));
            confirm_temporary_hotkey_dialog(&hotkey_input, window, cx)
        });
        assert!(!saved);

        cx.update(|window, cx| {
            assert!(window.has_active_dialog(cx));
        });

        cx.update(|_, cx| {
            assert_eq!(
                cx.global::<AiChat2AppSettings>().temporary_hotkey(),
                Some("cmd+shift+")
            );
            assert_eq!(
                state::GlobalHotkeyState::diagnostics_snapshot(cx)
                    .temporary_hotkey
                    .as_deref(),
                Some("cmd+shift+j")
            );
            let persisted = crate::database::repository(cx)
                .get_app_settings()
                .expect("load app settings")
                .expect("settings record");
            assert_eq!(
                persisted.settings.temporary_hotkey.as_deref(),
                Some("cmd+shift+")
            );
        });
    }

    #[gpui::test]
    fn cleared_hotkey_draft_does_not_change_settings_until_saved(cx: &mut TestAppContext) {
        let _dir = init_hotkey_settings_test(cx, Some("cmd+shift+j"));

        cx.update(|cx| {
            assert_eq!(
                cx.global::<AiChat2AppSettings>().temporary_hotkey(),
                Some("cmd+shift+j")
            );
            assert_eq!(
                state::GlobalHotkeyState::diagnostics_snapshot(cx)
                    .temporary_hotkey
                    .as_deref(),
                Some("cmd+shift+j")
            );
        });
    }

    #[gpui::test]
    fn save_temporary_hotkey_can_clear_config_and_runtime(cx: &mut TestAppContext) {
        let _dir = init_hotkey_settings_test(cx, Some("cmd+shift+j"));
        let window = open_test_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);

        cx.update(|window, cx| {
            assert!(save_temporary_hotkey(None, window, cx));
        });

        cx.update(|_, cx| {
            assert_eq!(cx.global::<AiChat2AppSettings>().temporary_hotkey(), None);
            assert_eq!(
                state::GlobalHotkeyState::diagnostics_snapshot(cx).temporary_hotkey,
                None
            );
        });
    }

    fn init_hotkey_settings_test(cx: &mut TestAppContext, hotkey: Option<&str>) -> TempDir {
        let dir = tempdir().unwrap();
        cx.update(|cx| {
            gpui_component::init(cx);
            cx.set_global(FreshStoreGlobal::open_in_dir(dir.path()).unwrap());
            let payload = AppSettingsPayload {
                temporary_hotkey: hotkey.map(str::to_string),
                ..Default::default()
            };
            crate::database::repository(cx)
                .set_app_settings(payload.clone())
                .unwrap();
            cx.set_global(AiChat2AppSettings::new(payload));
            foundation::init_i18n(cx);
            state::hotkey::set_test_hotkey_state(cx);
            if let Some(hotkey) = hotkey {
                state::GlobalHotkeyState::update_temporary_hotkey(None, Some(hotkey), cx)
                    .expect("register initial hotkey");
            }
        });
        dir
    }

    fn open_test_window(cx: &mut TestAppContext) -> WindowHandle<gpui_component::Root> {
        cx.update(|cx| {
            cx.open_window(Default::default(), |window, cx| {
                let view = cx.new(|_| TestView);
                cx.new(|cx| gpui_component::Root::new(view, window, cx))
            })
            .expect("open settings test window")
        })
    }

    struct TestView;

    impl Render for TestView {
        fn render(
            &mut self,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<Self>,
        ) -> impl gpui::IntoElement {
            gpui::div()
        }
    }
}

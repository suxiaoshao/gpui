use crate::{
    app::{self, menus},
    components::hotkey_input::{HotkeyInput, format_hotkey_label, string_to_keystroke},
    foundation::{self, I18n, assets::IconName},
    state::{self, AiChat2Config},
};
use ai_chat_core::AppLanguage;
use gpui::*;
use gpui_component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    menu::{DropdownMenu, PopupMenuItem},
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

struct TemporaryHotkeyControlState {
    editing: bool,
    hotkey_input: Entity<HotkeyInput>,
}

impl TemporaryHotkeyControlState {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let hotkey_input = Self::new_hotkey_input(current_temporary_hotkey(cx), window, cx);
        Self {
            editing: false,
            hotkey_input,
        }
    }

    fn new_hotkey_input(
        current_hotkey: Option<Keystroke>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<HotkeyInput> {
        cx.new(|cx| {
            HotkeyInput::new("temporary-hotkey-inline-input", window, cx)
                .small()
                .default_value(current_hotkey)
        })
    }

    fn reset_hotkey_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.hotkey_input = Self::new_hotkey_input(current_temporary_hotkey(cx), window, cx);
    }

    fn focus_hotkey_input(&self, window: &mut Window, cx: &mut Context<Self>) {
        let input = self.hotkey_input.clone();
        cx.defer_in(window, move |_, window, cx| {
            input.update(cx, |input, cx| input.focus(window, cx));
        });
    }

    fn start_editing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.reset_hotkey_input(window, cx);
        self.editing = true;
        self.focus_hotkey_input(window, cx);
        cx.notify();
    }

    fn cancel_editing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.reset_hotkey_input(window, cx);
        self.editing = false;
        cx.notify();
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let next_hotkey = self.hotkey_input.read(cx).current_hotkey_string();
        if save_temporary_hotkey(next_hotkey, window, cx) {
            self.reset_hotkey_input(window, cx);
            self.editing = false;
        }
        cx.notify();
    }

    #[cfg(test)]
    fn set_draft_hotkey_for_test(
        &mut self,
        hotkey: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.hotkey_input =
            Self::new_hotkey_input(hotkey.and_then(string_to_keystroke), window, cx);
    }
}

impl Render for TemporaryHotkeyControlState {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let current_hotkey = state::config::app_settings(cx)
            .temporary_hotkey()
            .map(str::to_string);
        let has_hotkey = current_hotkey.is_some();
        let current_label = current_hotkey
            .as_deref()
            .map(format_hotkey_label)
            .unwrap_or_else(|| cx.global::<I18n>().t("hotkey-not-set").to_string());

        if self.editing {
            let save_label = cx.global::<I18n>().t("provider-action-save");
            let cancel_label = cx.global::<I18n>().t("button-cancel");

            return h_flex()
                .items_center()
                .justify_end()
                .gap_2()
                .child(self.hotkey_input.clone())
                .child(
                    Button::new("temporary-hotkey-inline-save")
                        .icon(IconName::Check)
                        .tooltip(save_label)
                        .primary()
                        .small()
                        .on_click(cx.listener(|control, _, window, cx| {
                            control.save(window, cx);
                        })),
                )
                .child(
                    Button::new("temporary-hotkey-inline-cancel")
                        .icon(IconName::X)
                        .tooltip(cancel_label)
                        .ghost()
                        .small()
                        .on_click(cx.listener(|control, _, window, cx| {
                            control.cancel_editing(window, cx);
                        })),
                );
        }

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
                    .tooltip(cx.global::<I18n>().t("button-edit"))
                    .outline()
                    .small()
                    .on_click(cx.listener(|control, _, window, cx| {
                        control.start_editing(window, cx);
                    })),
            )
    }
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

fn temporary_hotkey_control(window: &mut Window, cx: &mut App) -> AnyElement {
    window
        .use_keyed_state("settings-temporary-hotkey-control", cx, |window, cx| {
            TemporaryHotkeyControlState::new(window, cx)
        })
        .into_any_element()
}

fn current_temporary_hotkey(cx: &App) -> Option<Keystroke> {
    state::config::app_settings(cx)
        .temporary_hotkey()
        .and_then(string_to_keystroke)
}

fn save_temporary_hotkey(next_hotkey: Option<String>, window: &mut Window, cx: &mut App) -> bool {
    let previous_hotkey = state::config::app_settings(cx)
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
    let current_language = state::config::app_settings(cx).language();
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
    let initial_value = state::config::app_settings(cx)
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
        TemporaryHotkeyControlState, language_label_key, language_options, save_temporary_hotkey,
    };
    use crate::{
        database::FreshStoreGlobal,
        foundation,
        state::{self, AiChat2Config},
    };
    use ai_chat_core::{AppLanguage, AppSettingsPayload};
    use gpui::{AppContext as _, Render, TestAppContext, VisualTestContext, WindowHandle};
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
        let dir = init_hotkey_settings_test(cx, Some("cmd+shift+j"));
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
                state::config::app_settings(cx).temporary_hotkey(),
                Some("cmd+shift+k")
            );
            assert_eq!(
                state::GlobalHotkeyState::diagnostics_snapshot(cx)
                    .temporary_hotkey
                    .as_deref(),
                Some("cmd+shift+k")
            );
        });
        assert_eq!(
            persisted_settings(&dir).temporary_hotkey.as_deref(),
            Some("cmd+shift+k")
        );
    }

    #[gpui::test]
    fn invalid_temporary_hotkey_does_not_change_settings_or_runtime(cx: &mut TestAppContext) {
        let dir = init_hotkey_settings_test(cx, Some("cmd+shift+j"));
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
                state::config::app_settings(cx).temporary_hotkey(),
                Some("cmd+shift+j")
            );
            assert_eq!(
                state::GlobalHotkeyState::diagnostics_snapshot(cx)
                    .temporary_hotkey
                    .as_deref(),
                Some("cmd+shift+j")
            );
        });
        assert_eq!(
            persisted_settings(&dir).temporary_hotkey.as_deref(),
            Some("cmd+shift+j")
        );
    }

    #[gpui::test]
    fn invalid_temporary_hotkey_inline_save_keeps_editor_open(cx: &mut TestAppContext) {
        let dir = init_hotkey_settings_test(cx, Some("cmd+shift+j"));
        let window = open_test_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);

        cx.update(|_, cx| {
            state::config::update_app_settings(cx, |payload| {
                payload.temporary_hotkey = Some("cmd+shift+".to_string());
            })
            .expect("seed invalid temporary hotkey setting");
        });
        let control =
            cx.update(|window, cx| cx.new(|cx| TemporaryHotkeyControlState::new(window, cx)));

        cx.update(|window, cx| {
            control.update(cx, |control, cx| {
                control.start_editing(window, cx);
                control.save(window, cx);
                assert!(control.editing);
            });
        });

        cx.update(|_, cx| {
            assert_eq!(
                state::config::app_settings(cx).temporary_hotkey(),
                Some("cmd+shift+")
            );
            assert_eq!(
                state::GlobalHotkeyState::diagnostics_snapshot(cx)
                    .temporary_hotkey
                    .as_deref(),
                Some("cmd+shift+j")
            );
        });
        assert_eq!(
            persisted_settings(&dir).temporary_hotkey.as_deref(),
            Some("cmd+shift+")
        );
    }

    #[gpui::test]
    fn inline_hotkey_cancel_discards_draft(cx: &mut TestAppContext) {
        let _dir = init_hotkey_settings_test(cx, Some("cmd+shift+j"));
        let window = open_test_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let control =
            cx.update(|window, cx| cx.new(|cx| TemporaryHotkeyControlState::new(window, cx)));

        cx.update(|window, cx| {
            control.update(cx, |control, cx| {
                control.start_editing(window, cx);
                control.set_draft_hotkey_for_test(Some("cmd+shift+k"), window, cx);
                control.cancel_editing(window, cx);
                assert!(!control.editing);
            });
        });

        cx.update(|_, cx| {
            assert_eq!(
                state::config::app_settings(cx).temporary_hotkey(),
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
    fn inline_hotkey_save_commits_draft_and_exits_editing(cx: &mut TestAppContext) {
        let dir = init_hotkey_settings_test(cx, Some("cmd+shift+j"));
        let window = open_test_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let control =
            cx.update(|window, cx| cx.new(|cx| TemporaryHotkeyControlState::new(window, cx)));

        cx.update(|window, cx| {
            control.update(cx, |control, cx| {
                control.start_editing(window, cx);
                control.set_draft_hotkey_for_test(Some("cmd+shift+k"), window, cx);
                control.save(window, cx);
                assert!(!control.editing);
            });
        });

        cx.update(|_, cx| {
            assert_eq!(
                state::config::app_settings(cx).temporary_hotkey(),
                Some("shift+super+k")
            );
            assert_eq!(
                state::GlobalHotkeyState::diagnostics_snapshot(cx)
                    .temporary_hotkey
                    .as_deref(),
                Some("shift+super+k")
            );
        });
        assert_eq!(
            persisted_settings(&dir).temporary_hotkey.as_deref(),
            Some("shift+super+k")
        );
    }

    #[gpui::test]
    fn save_temporary_hotkey_can_clear_config_and_runtime(cx: &mut TestAppContext) {
        let dir = init_hotkey_settings_test(cx, Some("cmd+shift+j"));
        let window = open_test_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);

        cx.update(|window, cx| {
            assert!(save_temporary_hotkey(None, window, cx));
        });

        cx.update(|_, cx| {
            assert_eq!(state::config::app_settings(cx).temporary_hotkey(), None);
            assert_eq!(
                state::GlobalHotkeyState::diagnostics_snapshot(cx).temporary_hotkey,
                None
            );
        });
        assert_eq!(persisted_settings(&dir).temporary_hotkey, None);
    }

    fn init_hotkey_settings_test(cx: &mut TestAppContext, hotkey: Option<&str>) -> TempDir {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        cx.update(|cx| {
            gpui_component::init(cx);
            cx.set_global(FreshStoreGlobal::open_in_dir(dir.path()).unwrap());
            let payload = AppSettingsPayload {
                temporary_hotkey: hotkey.map(str::to_string),
                ..Default::default()
            };
            let config = AiChat2Config::with_app_settings_for_test(config_path, payload.clone());
            config.save_for_test().expect("save test config");
            state::config::install_for_test(cx, config).expect("install config store");
            foundation::init_i18n(cx);
            state::hotkey::set_test_hotkey_state(cx);
            if let Some(hotkey) = hotkey {
                state::GlobalHotkeyState::update_temporary_hotkey(None, Some(hotkey), cx)
                    .expect("register initial hotkey");
            }
        });
        dir
    }

    fn persisted_settings(dir: &TempDir) -> AppSettingsPayload {
        AiChat2Config::load_from_path_for_test(&dir.path().join("config.toml"))
            .expect("load persisted config")
            .app_settings_payload()
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

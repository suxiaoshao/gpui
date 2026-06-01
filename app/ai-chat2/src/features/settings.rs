use crate::{
    app::{
        APP_NAME, menus,
        title_bar_menu::{TitleBarAppMenuBar, title_bar_leading},
    },
    components::hotkey_input::{HotkeyEvent, HotkeyInput, string_to_keystroke},
    foundation::{self, I18n},
    state,
};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    Root, StyledExt, TitleBar, WindowExt as NotificationWindowExt, h_flex,
    input::{InputEvent, InputState},
    label::Label,
    notification::{Notification, NotificationType},
    v_flex,
};
use std::any::TypeId;
use tracing::{Level, event};
use window_ext::WindowExt as SystemWindowExt;

mod appearance;
mod general;
mod layout;
mod projects;
mod provider;

use self::{
    appearance::AppearanceSettingsPage,
    layout::{
        SETTINGS_SIDEBAR_DEFAULT_WIDTH, SettingsPageFrame, SettingsPageKey, SettingsPageSpec,
        SettingsShell, settings_empty_message, settings_page_matches, settings_search_text,
    },
    projects::ProjectsSettingsPage,
    provider::ProviderSettingsPage,
};

actions!(ai_chat2_settings, [ToggleSettings]);

const SETTINGS_WINDOW_FALLBACK_SIZE: Size<Pixels> = size(px(960.), px(720.));

pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new(
        settings_key_binding(),
        ToggleSettings,
        None,
    )]);
    cx.on_action(|_: &ToggleSettings, cx: &mut App| open_settings_window_to(true, cx));
}

pub(crate) struct SettingsView {
    focus_handle: FocusHandle,
    hotkey_input: Entity<HotkeyInput>,
    settings_search_input: Entity<InputState>,
    appearance_settings: Entity<AppearanceSettingsPage>,
    provider_settings: Entity<ProviderSettingsPage>,
    projects_settings: Entity<ProjectsSettingsPage>,
    app_menu_bar: Entity<TitleBarAppMenuBar>,
    selected_page: SettingsPageKey,
    sidebar_width: Pixels,
    _subscriptions: Vec<Subscription>,
}

impl SettingsView {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        state::theme::apply_current_theme(window, cx);
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        let hotkey_input = cx.new(|cx| {
            let temporary_hotkey = cx
                .global::<state::AiChat2AppSettings>()
                .temporary_hotkey()
                .map(str::to_string);
            HotkeyInput::new("temporary-hotkey-input", window, cx)
                .default_value(temporary_hotkey.as_deref().and_then(string_to_keystroke))
        });
        let settings_search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(cx.global::<I18n>().t("field-search-settings"))
        });
        let appearance_settings = cx.new(|cx| AppearanceSettingsPage::new(window, cx));
        let provider_settings = cx.new(|cx| ProviderSettingsPage::new(window, cx));
        let projects_settings = cx.new(ProjectsSettingsPage::new);
        let app_menu_bar = TitleBarAppMenuBar::new(cx);
        let layout_state = cx.global::<state::LayoutStateStore>().entity();
        let _subscriptions = vec![
            cx.subscribe_in(&hotkey_input, window, Self::subscribe_hotkey_changes),
            cx.subscribe_in(
                &settings_search_input,
                window,
                Self::subscribe_settings_search_changes,
            ),
            cx.observe_window_bounds(window, move |_settings, window, cx| {
                let window_bounds = window.window_bounds();
                let display_id = window.display(cx).map(|display| display.id());
                layout_state.update(cx, |layout, cx| {
                    layout.set_window_bounds(
                        state::layout::WindowPlacementKind::Settings,
                        window_bounds,
                        display_id,
                        cx,
                    );
                });
            }),
            cx.observe_window_appearance(window, |_settings, window, cx| {
                state::theme::apply_current_theme(window, cx);
                cx.refresh_windows();
            }),
            cx.observe_global_in::<state::theme::SystemAccentThemeState>(
                window,
                |_state, window, cx| {
                    state::theme::apply_current_theme(window, cx);
                    cx.refresh_windows();
                },
            ),
            cx.observe_global_in::<state::AiChat2AppSettings>(window, |this, window, cx| {
                foundation::init_i18n(cx);
                menus::sync_app_menus(cx);
                state::theme::apply_current_theme(window, cx);
                this.reload_app_menu_bar(cx);
                cx.refresh_windows();
            }),
        ];
        Self {
            focus_handle,
            hotkey_input,
            settings_search_input,
            appearance_settings,
            provider_settings,
            projects_settings,
            app_menu_bar,
            selected_page: SettingsPageKey::General,
            sidebar_width: SETTINGS_SIDEBAR_DEFAULT_WIDTH,
            _subscriptions,
        }
    }

    pub(crate) fn reload_app_menu_bar(&mut self, cx: &mut Context<Self>) {
        self.app_menu_bar
            .update(cx, |app_menu_bar, cx| app_menu_bar.reload(cx));
    }

    fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_handle.focus(window, cx);
    }

    fn subscribe_settings_search_changes(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(event, InputEvent::Change) {
            cx.notify();
        }
    }

    fn subscribe_hotkey_changes(
        &mut self,
        state: &Entity<HotkeyInput>,
        event: &HotkeyEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let previous_hotkey = cx
            .global::<state::AiChat2AppSettings>()
            .temporary_hotkey()
            .map(str::to_string);
        let next_hotkey = match event {
            HotkeyEvent::Confirm(shared_string) => Some(shared_string.to_string()),
            HotkeyEvent::Cancel => None,
        };
        let save_result = state::config::update_app_settings(cx, |payload| {
            payload.temporary_hotkey = next_hotkey.clone();
        });

        match save_result {
            Ok(_) => {
                if let Err(err) = state::GlobalHotkeyState::update_temporary_hotkey(
                    previous_hotkey.as_deref(),
                    next_hotkey.as_deref(),
                    cx,
                ) {
                    let title = cx.global::<I18n>().t("notify-hotkey-register-failed");
                    push_settings_error(window, cx, title, err);
                }
                let default_value = next_hotkey.as_deref().and_then(string_to_keystroke);
                state.update(cx, move |this, _cx| {
                    this.set_default_value(default_value);
                });
            }
            Err(err) => {
                let title = cx.global::<I18n>().t("notify-save-settings-failed");
                push_settings_error(window, cx, title, err);
            }
        }
    }

    fn minimize(&mut self, _: &menus::Minimize, window: &mut Window, _: &mut Context<Self>) {
        window.minimize_window();
    }

    fn zoom(&mut self, _: &menus::Zoom, window: &mut Window, _: &mut Context<Self>) {
        window.zoom_window();
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let settings_title = cx.global::<I18n>().t("settings-title");
        let search_no_results = cx.global::<I18n>().t("settings-search-no-results");
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);
        let query = self
            .settings_search_input
            .read(cx)
            .value()
            .trim()
            .to_lowercase();
        let page_specs = settings_page_specs(cx);
        let visible_pages = page_specs
            .iter()
            .filter(|spec| settings_page_matches(spec, &query))
            .cloned()
            .collect::<Vec<_>>();
        let active_page_key = visible_pages
            .iter()
            .find(|spec| spec.key == self.selected_page)
            .or_else(|| visible_pages.first())
            .map(|spec| spec.key)
            .unwrap_or(self.selected_page);
        let active_page_title = page_specs
            .iter()
            .find(|spec| spec.key == active_page_key)
            .map(|spec| spec.title.clone())
            .unwrap_or_else(|| settings_title.clone().into());
        let page_body = if visible_pages.is_empty() {
            SettingsPageFrame::new(
                settings_title.clone(),
                settings_empty_message(search_no_results),
            )
            .into_any_element()
        } else {
            SettingsPageFrame::new(
                active_page_title,
                match active_page_key {
                    SettingsPageKey::General => {
                        general::render(self.hotkey_input.clone(), window, cx)
                    }
                    SettingsPageKey::Appearance => {
                        self.appearance_settings.clone().into_any_element()
                    }
                    SettingsPageKey::Provider => self.provider_settings.clone().into_any_element(),
                    SettingsPageKey::Projects => self.projects_settings.clone().into_any_element(),
                },
            )
            .when(active_page_key == SettingsPageKey::Provider, |frame| {
                frame.no_outer_body_scroll()
            })
            .into_any_element()
        };
        let resize_view = cx.entity().downgrade();
        let select_view = cx.entity().downgrade();
        window.set_window_title(&settings_title);

        v_flex()
            .id("settings")
            .track_focus(&self.focus_handle)
            .size_full()
            .overflow_hidden()
            .on_action(cx.listener(|_this, _: &ToggleSettings, window, _cx| {
                window.remove_window();
            }))
            .on_action(cx.listener(Self::minimize))
            .on_action(cx.listener(Self::zoom))
            .child(
                div()
                    .child(TitleBar::new().child(settings_title_bar_content(
                        self.app_menu_bar.clone(),
                        settings_title.clone(),
                    )))
                    .flex_initial(),
            )
            .child(
                div().flex_1().min_h_0().overflow_hidden().child(
                    SettingsShell::new(
                        self.sidebar_width,
                        self.settings_search_input.clone(),
                        visible_pages,
                        active_page_key,
                        page_body,
                    )
                    .on_resize(move |width, _window, cx| {
                        let _ = resize_view.update(cx, |view, cx| {
                            view.sidebar_width = width;
                            cx.notify();
                        });
                    })
                    .on_select(move |key, _window, cx| {
                        let _ = select_view.update(cx, |view, cx| {
                            view.selected_page = key;
                            cx.notify();
                        });
                    }),
                ),
            )
            .children(dialog_layer)
            .children(notification_layer)
    }
}

pub(crate) fn open_settings_window_from_menu(cx: &mut App) {
    open_settings_window_to(false, cx);
}

fn open_settings_window_to(toggle_if_active: bool, cx: &mut App) {
    let span = tracing::info_span!("open_ai_chat2_settings_window");
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
                        let search_input = settings.settings_search_input.clone();
                        search_input.update(cx, |search_input, cx| {
                            search_input.set_value("", window, cx);
                        });
                        settings.focus(window, cx);
                        cx.notify();
                    });
                }
                if toggle_if_active && window.is_window_active() {
                    window.remove_window();
                } else {
                    if let Err(err) = window.show() {
                        event!(Level::ERROR, error = ?err, "show ai-chat2 settings window failed");
                    }
                    window.activate_window();
                }
            }) {
                Ok(_) => {}
                Err(err) => {
                    event!(Level::ERROR, error = ?err, "activate ai-chat2 settings window failed");
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
    let placement = state::layout::restored_window_placement(
        state::layout::WindowPlacementKind::Settings,
        SETTINGS_WINDOW_FALLBACK_SIZE,
        cx,
    );
    match cx.open_window(
        WindowOptions {
            window_bounds: Some(placement.window_bounds),
            display_id: placement.display_id,
            titlebar: Some(settings_titlebar_options(title)),
            window_background: WindowBackgroundAppearance::Blurred,
            is_resizable: true,
            kind: WindowKind::Normal,
            app_id: Some(APP_NAME.to_owned()),
            ..Default::default()
        },
        |window, cx| {
            let setting = cx.new(|cx| SettingsView::new(window, cx));
            cx.new(|cx| Root::new(setting, window, cx))
        },
    ) {
        Ok(window) => {
            let _ = window.update(cx, |root, window, cx| {
                if let Ok(settings) = root.view().clone().downcast::<SettingsView>() {
                    settings.update(cx, |settings, cx| settings.focus(window, cx));
                }
                if let Err(err) = window.show() {
                    event!(Level::ERROR, error = ?err, "show ai-chat2 settings window failed");
                }
                window.activate_window();
            });
        }
        Err(err) => {
            event!(Level::ERROR, error = ?err, "open ai-chat2 settings window failed");
        }
    };
}

fn settings_page_specs(cx: &App) -> [SettingsPageSpec; 4] {
    let i18n = cx.global::<I18n>();
    settings_page_specs_for_i18n(i18n)
}

fn settings_page_specs_for_i18n(i18n: &I18n) -> [SettingsPageSpec; 4] {
    let page_general = i18n.t("settings-page-general");
    let page_appearance = i18n.t("settings-page-appearance");
    let page_provider = i18n.t("settings-page-provider");
    let page_projects = i18n.t("settings-page-projects");
    let group_basic_options = i18n.t("settings-group-basic-options");
    let field_language = i18n.t("field-language");
    let field_http_proxy = i18n.t("field-http-proxy");
    let field_temporary_hotkey = i18n.t("field-temporary-conversation-hotkey");
    let field_config_file = i18n.t("field-config-file");

    [
        SettingsPageSpec::new(
            SettingsPageKey::General,
            page_general.clone(),
            settings_search_text(
                [
                    page_general.as_str(),
                    group_basic_options.as_str(),
                    field_language.as_str(),
                    field_http_proxy.as_str(),
                    field_temporary_hotkey.as_str(),
                    field_config_file.as_str(),
                ],
                "general basic language proxy http hotkey shortcut temporary conversation config file",
            ),
        ),
        SettingsPageSpec::new(
            SettingsPageKey::Appearance,
            page_appearance.clone(),
            settings_search_text(
                [page_appearance.as_str()],
                "appearance theme color mode light dark system material you bright custom 主题 外观 亮色 暗色 系统 自定义",
            ),
        ),
        SettingsPageSpec::new(
            SettingsPageKey::Provider,
            page_provider.clone(),
            settings_search_text(
                [page_provider.as_str()],
                "provider model api key base url openai anthropic gemini ollama openrouter deepseek kimi azure mistral groq perplexity together 模型 提供商",
            ),
        ),
        SettingsPageSpec::new(
            SettingsPageKey::Projects,
            page_projects.clone(),
            settings_search_text(
                [page_projects.as_str()],
                "projects project workspace folder path directory 项目 工作区 文件夹 路径",
            ),
        ),
    ]
}

fn settings_title_bar_content(
    app_menu_bar: Entity<TitleBarAppMenuBar>,
    title: impl Into<SharedString>,
) -> impl IntoElement {
    h_flex()
        .w_full()
        .h_full()
        .min_w_0()
        .overflow_hidden()
        .when(menus::should_render_component_menu_bar(), |this| {
            this.child(title_bar_leading(app_menu_bar))
        })
        .child(settings_title_bar_title(title))
}

fn settings_title_bar_title(title: impl Into<SharedString>) -> impl IntoElement {
    h_flex()
        .flex_1()
        .min_w_0()
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

pub(super) fn push_settings_error(
    window: &mut Window,
    cx: &mut App,
    title: impl Into<SharedString>,
    error: impl ToString,
) {
    window.push_notification(
        Notification::new()
            .title(title.into())
            .message(error.to_string())
            .with_type(NotificationType::Error),
        cx,
    );
}

const fn settings_key_binding() -> &'static str {
    if cfg!(target_os = "macos") {
        "cmd-,"
    } else {
        "ctrl-,"
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SettingsPageKey, SettingsPageSpec, settings_key_binding, settings_page_matches,
        settings_page_specs_for_i18n, settings_search_text, settings_titlebar_options,
    };
    use crate::foundation::I18n;
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

    #[test]
    fn settings_search_matches_localized_labels_and_keywords() {
        let appearance = SettingsPageSpec::new(
            SettingsPageKey::Appearance,
            "外观",
            settings_search_text(["外观"], "appearance theme material"),
        );

        assert!(settings_page_matches(&appearance, "外观"));
        assert!(settings_page_matches(&appearance, "material"));
        assert!(!settings_page_matches(&appearance, "provider"));
    }

    #[test]
    fn settings_search_matches_localized_labels_by_pinyin() {
        let general = SettingsPageSpec::new(
            SettingsPageKey::General,
            "通用",
            settings_search_text(["通用"], "general"),
        );
        let appearance = SettingsPageSpec::new(
            SettingsPageKey::Appearance,
            "外观",
            settings_search_text(["外观"], "appearance"),
        );

        assert!(settings_page_matches(&general, "tongyong"));
        assert!(settings_page_matches(&general, "ty"));
        assert!(settings_page_matches(&appearance, "waiguan"));
        assert!(settings_page_matches(&appearance, "wg"));
    }

    #[test]
    fn settings_search_text_normalizes_case() {
        let text = settings_search_text(["HTTP Proxy"], "OpenAI Provider");

        assert!(text.contains("http proxy"));
        assert!(text.contains("openai provider"));
    }

    #[test]
    fn settings_provider_page_uses_i18n_title_and_search_terms() {
        let zh = I18n::for_locale_tag("zh-CN");
        let specs = settings_page_specs_for_i18n(&zh);
        let provider = specs
            .iter()
            .find(|spec| spec.key == SettingsPageKey::Provider)
            .expect("provider settings page exists");

        assert_eq!(provider.title.as_ref(), "提供商");
        assert!(settings_page_matches(provider, "provider"));
        assert!(settings_page_matches(provider, "model"));
        assert!(settings_page_matches(provider, "OpenAI"));
        assert!(settings_page_matches(provider, "Ollama"));
        assert!(settings_page_matches(provider, "提供商"));
        assert!(settings_page_matches(provider, "模型"));
    }

    #[test]
    fn settings_key_binding_matches_platform() {
        assert_eq!(
            settings_key_binding(),
            if cfg!(target_os = "macos") {
                "cmd-,"
            } else {
                "ctrl-,"
            }
        );
    }
}

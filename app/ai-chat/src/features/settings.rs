use crate::{
    app::menus,
    components::hotkey_input::{HotkeyEvent, HotkeyInput, string_to_keystroke},
    foundation::i18n::I18n,
    llm::provider_settings_specs,
    state::{AiChatConfig, WindowPlacementKind, WorkspaceStore},
};
use gpui::*;
use gpui_component::{
    Root, StyledExt, TitleBar, h_flex,
    input::{InputEvent, InputState},
    label::Label,
    v_flex,
};
use std::{any::TypeId, ops::Deref};
use tracing::{Level, event};

pub(super) mod appearance_settings;
mod general_settings;
mod layout;
mod provider_settings;
pub(super) mod shortcut_settings;

use self::{
    appearance_settings::AppearanceSettingsPage,
    layout::{
        SETTINGS_SIDEBAR_DEFAULT_WIDTH, SettingsPageFrame, SettingsPageKey, SettingsPageSpec,
        SettingsShell, settings_empty_message, settings_page_matches, settings_search_text,
    },
    shortcut_settings::ShortcutSettingsPage,
};

actions!([OpenSetting]);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SettingsOpenTarget {
    General,
    Provider,
}

fn page_key_from_open_target(target: SettingsOpenTarget) -> SettingsPageKey {
    match target {
        SettingsOpenTarget::General => SettingsPageKey::General,
        SettingsOpenTarget::Provider => SettingsPageKey::Provider,
    }
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
    settings_search_input: Entity<InputState>,
    appearance_settings: Entity<AppearanceSettingsPage>,
    shortcut_settings: Entity<ShortcutSettingsPage>,
    selected_page: SettingsPageKey,
    sidebar_width: Pixels,
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
        let settings_search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(cx.global::<I18n>().t("field-search-settings"))
        });
        let appearance_settings = cx.new(|cx| AppearanceSettingsPage::new(window, cx));
        let shortcut_settings = cx.new(|cx| ShortcutSettingsPage::new(window, cx));
        let _subscriptions = vec![
            cx.subscribe(&hotkey_input, Self::subscribe_hotkey_changes),
            cx.subscribe_in(
                &settings_search_input,
                window,
                Self::subscribe_settings_search_changes,
            ),
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
            settings_search_input,
            appearance_settings,
            shortcut_settings,
            selected_page: page_key_from_open_target(open_target),
            sidebar_width: SETTINGS_SIDEBAR_DEFAULT_WIDTH,
            _subscriptions,
        }
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
                        general_settings::render(self.hotkey_input.clone(), window, cx)
                    }
                    SettingsPageKey::Appearance => {
                        self.appearance_settings.clone().into_any_element()
                    }
                    SettingsPageKey::Provider => provider_settings::render(window, cx),
                    SettingsPageKey::Shortcuts => self.shortcut_settings.clone().into_any_element(),
                },
            )
            .into_any_element()
        };
        let resize_view = cx.entity().downgrade();
        let select_view = cx.entity().downgrade();

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
                    .child(TitleBar::new().child(settings_title_bar_title(settings_title.clone())))
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
                        settings.selected_page = page_key_from_open_target(target);
                        let search_input = settings.settings_search_input.clone();
                        search_input.update(cx, |search_input, cx| {
                            search_input.set_value("", window, cx);
                        });
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

fn settings_page_specs(cx: &App) -> [SettingsPageSpec; 4] {
    let i18n = cx.global::<I18n>();
    let page_general = i18n.t("settings-page-general");
    let page_appearance = i18n.t("settings-page-appearance");
    let page_provider = i18n.t("settings-page-provider");
    let page_shortcuts = i18n.t("settings-page-shortcuts");
    let group_basic_options = i18n.t("settings-group-basic-options");
    let field_language = i18n.t("field-language");
    let field_http_proxy = i18n.t("field-http-proxy");
    let field_temporary_hotkey = i18n.t("field-temporary-conversation-hotkey");
    let field_config_file = i18n.t("field-config-file");
    let mut provider_labels = vec![page_provider.clone()];
    let mut provider_keywords =
        String::from("provider model api key base url proxy openai ollama 提供方 模型 密钥");
    for spec in provider_settings_specs() {
        provider_labels.push(i18n.t(spec.title_key));
        for field in spec.fields {
            provider_labels.push(i18n.t(field.label_key));
            provider_keywords.push(' ');
            provider_keywords.push_str(field.search_keywords);
        }
    }

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
                provider_labels.iter().map(String::as_str),
                &provider_keywords,
            ),
        ),
        SettingsPageSpec::new(
            SettingsPageKey::Shortcuts,
            page_shortcuts.clone(),
            settings_search_text(
                [page_shortcuts.as_str()],
                "shortcuts shortcut hotkey template model mode send content preset enabled 快捷键 模板 模型 模式 发送内容 预设 启用",
            ),
        ),
    ]
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
    use super::{
        SettingsPageKey, SettingsPageSpec, settings_page_matches, settings_search_text,
        settings_titlebar_options,
    };
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
        let shortcuts = SettingsPageSpec::new(
            SettingsPageKey::Shortcuts,
            "快捷键",
            settings_search_text(["快捷键"], "shortcut hotkey template"),
        );

        assert!(settings_page_matches(&shortcuts, "快捷键"));
        assert!(settings_page_matches(&shortcuts, "hotkey"));
        assert!(!settings_page_matches(&shortcuts, "provider"));
    }

    #[test]
    fn settings_search_matches_localized_labels_by_pinyin() {
        let provider = SettingsPageSpec::new(
            SettingsPageKey::Provider,
            "提供方",
            settings_search_text(["提供方"], "provider api key"),
        );
        let shortcuts = SettingsPageSpec::new(
            SettingsPageKey::Shortcuts,
            "快捷键",
            settings_search_text(["快捷键"], "shortcut hotkey template"),
        );

        assert!(settings_page_matches(&provider, "tigongfang"));
        assert!(settings_page_matches(&provider, "tgf"));
        assert!(settings_page_matches(&shortcuts, "kuaijiejian"));
        assert!(settings_page_matches(&shortcuts, "kjj"));
    }

    #[test]
    fn settings_search_text_normalizes_case() {
        let text = settings_search_text(["API Key"], "OpenAI Provider");

        assert!(text.contains("api key"));
        assert!(text.contains("openai provider"));
    }
}

use crate::{
    app::{open_temporary_window, quit_app, show_or_create_main_window},
    features::{about::open_about_window, settings::open_settings_window_from_menu},
    foundation::i18n::I18n,
};
use fluent_bundle::FluentArgs;
#[cfg(target_os = "macos")]
use gpui::SystemMenuType;
use gpui::{App, KeyBinding, Menu, MenuItem, actions};
use tracing::{Level, event};

actions!(
    ai_chat,
    [
        About,
        OpenMainWindow,
        OpenTemporaryConversation,
        OpenSettings,
        Quit,
        Minimize,
        Zoom,
        Hide,
        HideOthers,
        ShowAll
    ]
);

const WINDOW_MENU_INDEX: usize = 1;

pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);

    #[cfg(target_os = "macos")]
    cx.bind_keys([
        KeyBinding::new("cmd-h", Hide, None),
        KeyBinding::new("alt-cmd-h", HideOthers, None),
    ]);

    cx.on_action(|_: &About, cx: &mut App| open_about_window(cx));
    cx.on_action(|_: &OpenMainWindow, cx: &mut App| show_or_create_main_window(cx));
    cx.on_action(open_temporary_conversation);
    cx.on_action(|_: &OpenSettings, cx: &mut App| open_settings_window_from_menu(cx));
    cx.on_action(quit);

    #[cfg(target_os = "macos")]
    cx.on_action(|_: &Hide, cx: &mut App| cx.hide());
    #[cfg(target_os = "macos")]
    cx.on_action(|_: &HideOthers, cx: &mut App| cx.hide_other_apps());
    #[cfg(target_os = "macos")]
    cx.on_action(|_: &ShowAll, cx: &mut App| cx.unhide_other_apps());
}

pub(crate) fn app_menus(i18n: &I18n) -> Vec<Menu> {
    let mut app_items = vec![
        MenuItem::action(app_name_message(i18n, "app-menu-about"), About),
        MenuItem::action(version_message(i18n), gpui::NoAction).disabled(true),
        MenuItem::separator(),
        MenuItem::action(i18n.t("app-menu-open-main"), OpenMainWindow),
        MenuItem::action(i18n.t("app-menu-open-temporary"), OpenTemporaryConversation),
        MenuItem::action(i18n.t("app-menu-settings"), OpenSettings),
    ];

    #[cfg(target_os = "macos")]
    {
        app_items.extend([
            MenuItem::separator(),
            MenuItem::os_submenu(i18n.t("app-menu-services"), SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action(app_name_message(i18n, "app-menu-hide"), Hide),
            MenuItem::action(i18n.t("app-menu-hide-others"), HideOthers),
            MenuItem::action(i18n.t("app-menu-show-all"), ShowAll),
        ]);
    }

    app_items.extend([
        MenuItem::separator(),
        MenuItem::action(app_name_message(i18n, "app-menu-quit"), Quit),
    ]);

    vec![
        Menu::new(i18n.t("app-title")).items(app_items),
        Menu::new(i18n.t("app-menu-window")).items([
            MenuItem::action(i18n.t("app-menu-minimize"), Minimize),
            MenuItem::action(i18n.t("app-menu-zoom"), Zoom),
        ]),
    ]
}

pub(crate) fn ensure_localized_window_menu_registered() {
    match platform_ext::app::set_windows_menu_from_main_menu_index(WINDOW_MENU_INDEX) {
        Ok(()) => {
            event!(
                Level::INFO,
                index = WINDOW_MENU_INDEX,
                "registered localized window menu with NSApp"
            );
        }
        Err(err) => {
            event!(
                Level::WARN,
                error = ?err,
                index = WINDOW_MENU_INDEX,
                "failed to register localized window menu with NSApp"
            );
        }
    }
}

fn app_name_message(i18n: &I18n, key: &str) -> String {
    let mut args = FluentArgs::new();
    args.set("app_name", i18n.t("app-title"));
    i18n.t_with_args(key, &args)
}

fn version_message(i18n: &I18n) -> String {
    let mut args = FluentArgs::new();
    args.set("version", env!("CARGO_PKG_VERSION"));
    i18n.t_with_args("app-menu-version", &args)
}

fn open_temporary_conversation(_: &OpenTemporaryConversation, cx: &mut App) {
    open_temporary_window(cx);
}

fn quit(_: &Quit, cx: &mut App) {
    quit_app(cx);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn menu_names(menus: &[Menu]) -> Vec<String> {
        menus.iter().map(|menu| menu.name.to_string()).collect()
    }

    fn item_names(items: Vec<MenuItem>) -> Vec<String> {
        items
            .into_iter()
            .map(|item| match item {
                MenuItem::Separator => "---".to_string(),
                MenuItem::Submenu(menu) => menu.name.to_string(),
                MenuItem::SystemMenu(menu) => menu.name.to_string(),
                MenuItem::Action { name, .. } => name.to_string(),
            })
            .collect()
    }

    #[test]
    fn builds_expected_top_level_menus() {
        let i18n = I18n::english_for_test();

        assert_eq!(menu_names(&app_menus(&i18n)), vec!["AI Chat", "Window"]);
    }

    #[test]
    fn builds_expected_top_level_menus_for_chinese() {
        let i18n = I18n::for_locale_tag("zh-CN");

        assert_eq!(menu_names(&app_menus(&i18n)), vec!["AI 对话", "窗口"]);
    }

    #[test]
    fn localized_window_menu_stays_in_expected_slot_for_chinese() {
        let i18n = I18n::for_locale_tag("zh-CN");
        let menus = app_menus(&i18n);

        assert_eq!(menus[WINDOW_MENU_INDEX].name.to_string(), "窗口");
    }

    #[test]
    fn builds_expected_app_menu_items() {
        let i18n = I18n::english_for_test();
        let mut menus = app_menus(&i18n);
        let app_menu = menus.remove(0);
        let item_names = item_names(app_menu.items);

        #[cfg(target_os = "macos")]
        assert_eq!(
            item_names,
            vec![
                "About AI Chat".to_string(),
                format!("Version (v{})", env!("CARGO_PKG_VERSION")),
                "---".to_string(),
                "Open AI Chat".to_string(),
                "Open Temporary Conversation".to_string(),
                "Settings".to_string(),
                "---".to_string(),
                "Services".to_string(),
                "---".to_string(),
                "Hide AI Chat".to_string(),
                "Hide Others".to_string(),
                "Show All".to_string(),
                "---".to_string(),
                "Quit AI Chat".to_string(),
            ]
        );

        #[cfg(not(target_os = "macos"))]
        assert_eq!(
            item_names,
            vec![
                "About AI Chat".to_string(),
                format!("Version (v{})", env!("CARGO_PKG_VERSION")),
                "---".to_string(),
                "Open AI Chat".to_string(),
                "Open Temporary Conversation".to_string(),
                "Settings".to_string(),
                "---".to_string(),
                "Quit AI Chat".to_string(),
            ]
        );
    }

    #[test]
    fn builds_expected_window_menu_items() {
        let i18n = I18n::english_for_test();
        let mut menus = app_menus(&i18n);
        let window_menu = menus.remove(WINDOW_MENU_INDEX);

        assert_eq!(item_names(window_menu.items), vec!["Minimize", "Zoom"]);
    }
}

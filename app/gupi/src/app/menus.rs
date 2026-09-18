use crate::foundation::i18n::t;
use gpui_kit::*;
actions!(
    gupi,
    [
        Quit,
        ShowSettings,
        ShowMainWindow,
        ShowCommandPalette,
        ShowTemporaryWindow
    ]
);
pub(crate) fn refresh(cx: &mut App) {
    super::tray::refresh(cx);
    cx.set_menus(vec![Menu {
        disabled: false,
        name: "Gupi".into(),
        items: vec![
            MenuItem::action(t(cx, "temporary-title"), ShowTemporaryWindow),
            MenuItem::action(t(cx, "menu-settings"), ShowSettings),
            MenuItem::action(t(cx, "menu-show-main"), ShowMainWindow),
            MenuItem::separator(),
            MenuItem::action(t(cx, "menu-quit"), Quit),
        ],
    }]);
}

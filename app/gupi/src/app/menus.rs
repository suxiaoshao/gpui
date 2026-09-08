use crate::foundation::i18n::t;
use gpui_kit::*;
actions!(gupi, [Quit, ShowSettings, ShowMainWindow]);
pub(crate) fn refresh(cx: &mut App) {
    cx.set_menus(vec![Menu {
        disabled: false,
        name: "Gupi".into(),
        items: vec![
            MenuItem::action(t(cx, "menu-settings"), ShowSettings),
            MenuItem::action(t(cx, "menu-show-main"), ShowMainWindow),
            MenuItem::separator(),
            MenuItem::action(t(cx, "menu-quit"), Quit),
        ],
    }]);
}

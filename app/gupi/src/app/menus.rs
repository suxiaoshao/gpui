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
struct AppliedLocale(&'static str);
impl Global for AppliedLocale {}
pub(crate) fn refresh(cx: &mut App) {
    let locale = crate::foundation::i18n::locale(cx);
    if cx
        .try_global::<AppliedLocale>()
        .is_some_and(|old| old.0 == locale)
    {
        return;
    }
    cx.set_global(AppliedLocale(locale));
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

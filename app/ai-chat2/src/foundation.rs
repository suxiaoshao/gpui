pub(crate) mod assets;
pub(crate) mod i18n;
pub(crate) mod search;

pub(crate) use assets::Assets;
pub(crate) use i18n::I18n;

use gpui::App;

pub(crate) fn init_i18n(cx: &mut App) {
    i18n::init(cx);
}

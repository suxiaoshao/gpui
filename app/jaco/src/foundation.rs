pub(crate) mod assets;
pub(crate) mod conversation_format;
pub(crate) mod i18n;
pub(crate) mod persistence;
pub(crate) mod search;

pub(crate) use assets::Assets;
pub(crate) use i18n::I18n;

use gpui::App;

pub(crate) fn init_i18n(cx: &mut App) {
    i18n::init(cx);
}

pub(crate) fn init_i18n_runtime(cx: &mut App) {
    i18n::init_runtime(cx);
}

pub(crate) fn init_bootstrap(cx: &mut App) {
    i18n::init_bootstrap(cx);
}

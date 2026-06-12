use gpui::App;

pub(crate) mod home;
pub(crate) mod settings;
pub(crate) mod temporary;

pub(crate) fn init(cx: &mut App) {
    home::init(cx);
    settings::init(cx);
    temporary::init(cx);
}

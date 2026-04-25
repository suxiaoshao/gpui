use gpui::App;

pub(crate) mod about;
pub(crate) mod conversation;
pub(crate) mod home;
pub(crate) mod hotkey;
pub(crate) mod screenshot;
pub(crate) mod settings;
pub(crate) mod temporary;

pub(crate) fn init(cx: &mut App) {
    home::init(cx);
    screenshot::init(cx);
    temporary::init(cx);
    settings::init(cx);
}

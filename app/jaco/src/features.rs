use gpui::App;

pub(crate) mod about;
pub(crate) mod conversation;
pub(crate) mod home;
pub(crate) mod screenshot;
pub(crate) mod settings;
pub(crate) mod skills;
pub(crate) mod temporary;

pub(crate) fn init(cx: &mut App) {
    conversation::resources::init(cx);
    home::init(cx);
    screenshot::init(cx);
    settings::init(cx);
    temporary::init(cx);
}

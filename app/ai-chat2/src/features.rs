use gpui::App;

pub(crate) mod home;
pub(crate) mod settings;

pub(crate) fn init(cx: &mut App) {
    home::init(cx);
    settings::init(cx);
}

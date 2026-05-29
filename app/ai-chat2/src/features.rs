use gpui::App;

pub(crate) mod home;

pub(crate) fn init(cx: &mut App) {
    home::init(cx);
}

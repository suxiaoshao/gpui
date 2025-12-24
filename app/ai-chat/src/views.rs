use gpui::App;

pub(crate) mod home;

pub fn init(cx: &mut App) {
    home::init(cx);
}

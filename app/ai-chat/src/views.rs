use gpui::App;

pub(crate) mod home;
pub(crate) mod message_preview;

pub fn init(cx: &mut App) {
    home::init(cx);
}

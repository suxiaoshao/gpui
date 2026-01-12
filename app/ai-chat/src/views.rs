use gpui::App;

pub(crate) mod home;
pub(crate) mod message_preview;
pub(crate) mod temporary;

pub fn init(cx: &mut App) {
    home::init(cx);
}

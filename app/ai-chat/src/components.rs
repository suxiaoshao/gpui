use gpui::App;

pub mod add_conversation;
pub mod add_folder;
pub mod chat_input;
pub mod hotkey_input;
pub mod message;
pub mod template_edit_dialog;

pub(crate) fn init(cx: &mut App) {
    chat_input::init(cx);
}

use gpui::App;

pub mod add_conversation;
pub mod add_folder;
pub mod chat_input;
pub mod delete_confirm;
pub mod hotkey_input;
pub mod message;
pub mod provider_chat_form;
pub mod provider_template_form;
pub mod template_edit_dialog;

pub(crate) fn init(cx: &mut App) {
    chat_input::init(cx);
}

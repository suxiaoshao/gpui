use gpui::App;

pub mod add_conversation;
pub mod add_folder;
pub mod chat_form;
pub mod delete_confirm;
pub mod hotkey_input;
pub mod message;
pub(crate) mod search_list;
pub mod template_edit_dialog;

pub(crate) fn init(cx: &mut App) {
    chat_form::init(cx);
}

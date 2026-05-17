use gpui::App;

pub mod add_conversation;
pub mod add_folder;
pub mod chat_form;
pub mod delete_confirm;
pub(crate) mod ext_setting_help;
pub mod hotkey_input;
pub mod message;
pub(crate) mod title_bar_menu;

pub(crate) fn init(cx: &mut App) {
    chat_form::init(cx);
    title_bar_menu::init(cx);
}

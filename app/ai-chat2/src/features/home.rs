pub(crate) mod actions;
pub(crate) mod chat_form;
pub(crate) mod conversation;
pub(crate) mod new_conversation;
pub(crate) mod shell;
pub(crate) mod sidebar;

pub(crate) use shell::HomeView;

use gpui::App;

pub(crate) fn init(cx: &mut App) {
    actions::init(cx);
    chat_form::init(cx);
}

pub(crate) mod actions;
pub(crate) mod new_conversation;
pub(crate) mod shell;
pub(crate) mod sidebar;

pub(crate) use shell::HomeView;

use gpui::App;

pub(crate) fn init(cx: &mut App) {
    actions::init(cx);
    crate::components::chat_form::init(cx);
}

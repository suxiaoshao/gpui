pub(crate) mod actions;
pub(crate) mod new_conversation;
mod root;
pub(crate) mod shell;
pub(crate) mod sidebar;
pub(crate) mod workspace;

pub(crate) use root::JacoRoot;
pub(crate) use shell::HomeView;

use gpui::App;

pub(crate) fn init(cx: &mut App) {
    actions::init(cx);
    crate::components::chat::input::init(cx);
}

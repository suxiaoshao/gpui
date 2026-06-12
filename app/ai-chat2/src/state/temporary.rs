use gpui::App;

use crate::{
    database,
    state::workspace::{self, SidebarConversationNode},
};

pub(crate) type TemporaryConversationNode = SidebarConversationNode;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct TemporaryConversationSnapshot {
    pub(crate) conversations: Vec<TemporaryConversationNode>,
}

pub(crate) fn load_no_project_conversations(
    query: &str,
    cx: &App,
) -> ai_chat_db::Result<TemporaryConversationSnapshot> {
    let conversations = database::repository(cx)
        .list_no_project_conversations(query)?
        .into_iter()
        .map(workspace::conversation_node)
        .collect();

    Ok(TemporaryConversationSnapshot { conversations })
}

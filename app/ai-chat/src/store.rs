use std::ops::Deref;

use gpui::*;

use crate::{
    database::{Conversation, Db, Folder},
    errors::AiChatResult,
};

pub struct ChatDataInner {
    pub(crate) conversations: Vec<Conversation>,
    pub(crate) folders: Vec<Folder>,
}

impl ChatDataInner {
    fn new(cx: &mut Context<AiChatResult<Self>>) -> AiChatResult<Self> {
        let conn = &mut cx.global::<Db>().get()?;
        let conversations = Conversation::query_without_folder(conn)?;
        let folders = Folder::query(conn)?;
        Ok(Self {
            conversations,
            folders,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ChatData(Entity<AiChatResult<ChatDataInner>>);

impl Deref for ChatData {
    type Target = Entity<AiChatResult<ChatDataInner>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Global for ChatData {}

pub(crate) fn init(cx: &mut App) -> AiChatResult<()> {
    let chat_data = cx.new(|cx| ChatDataInner::new(cx));
    cx.set_global(ChatData(chat_data));
    Ok(())
}

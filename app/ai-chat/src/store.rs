use crate::{
    database::{Conversation, Db, Folder, NewFolder},
    errors::AiChatResult,
    views::home::HomeView,
};
use gpui::*;
use gpui_component::{
    WindowExt,
    notification::{Notification, NotificationType},
    sidebar::SidebarMenuItem,
};
use std::ops::Deref;

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
    fn get_folder(folders: &mut Vec<Folder>, id: i32) -> Option<&mut Folder> {
        for folder in folders {
            if folder.id == id {
                return Some(folder);
            }
            if let Some(folder) = ChatDataInner::get_folder(&mut folder.folders, id) {
                return Some(folder);
            }
        }
        None
    }
    fn add_folder(&mut self, new_folder: Folder) {
        if let Some(parent_id) = new_folder.parent_id {
            if let Some(parent) = ChatDataInner::get_folder(&mut self.folders, parent_id) {
                parent.folders.push(new_folder);
            }
        } else {
            self.folders.push(new_folder);
        }
    }
    pub(crate) fn sidebar_items(&self) -> Vec<SidebarMenuItem> {
        let mut items = Vec::new();
        items.extend(self.folders.iter().map(From::from));
        items.extend(self.conversations.iter().map(From::from));
        items
    }
}

pub enum ChatDataEvent {
    AddConversation {
        name: String,
    },
    AddFolder {
        name: String,
        parent_id: Option<i32>,
    },
}

impl EventEmitter<ChatDataEvent> for AiChatResult<ChatDataInner> {}

#[derive(Debug)]
pub struct ChatData {
    data: Entity<AiChatResult<ChatDataInner>>,
    _subscriptions: Vec<Subscription>,
}

impl Deref for ChatData {
    type Target = Entity<AiChatResult<ChatDataInner>>;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl Global for ChatData {}

impl ChatData {
    pub fn subscribe_in(
        this: &mut HomeView,
        state: &Entity<AiChatResult<ChatDataInner>>,
        event: &ChatDataEvent,
        window: &mut Window,
        cx: &mut Context<HomeView>,
    ) {
        match event {
            ChatDataEvent::AddConversation { name } => todo!(),
            ChatDataEvent::AddFolder { name, parent_id } => {
                match Self::add_folder(state, name, *parent_id, cx) {
                    Ok(_) => {}
                    Err(err) => {
                        window.push_notification(
                            Notification::new()
                                .title("Add Folder Failed")
                                .message(SharedString::from(err.to_string()))
                                .with_type(NotificationType::Error),
                            cx,
                        );
                    }
                }
            }
        }
    }
    fn add_folder(
        state: &Entity<AiChatResult<ChatDataInner>>,
        name: &str,
        parent_id: Option<i32>,
        cx: &mut Context<HomeView>,
    ) -> AiChatResult<()> {
        let new_folder = NewFolder::new(name, parent_id);
        let conn = &mut cx.global::<Db>().get()?;
        let folder = Folder::insert(new_folder, conn)?;
        state.update(cx, |data, cx| {
            if let Ok(data) = data {
                data.add_folder(folder);
            }
        });
        Ok(())
    }
}

pub(crate) fn init(window: &mut Window, cx: &mut Context<HomeView>) {
    let chat_data = cx.new(ChatDataInner::new);
    let _subscriptions = vec![cx.subscribe_in(&chat_data, window, ChatData::subscribe_in)];
    cx.set_global(ChatData {
        data: chat_data,
        _subscriptions,
    });
}

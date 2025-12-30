use crate::{
    database::{Conversation, Db, Folder, NewConversation, NewFolder},
    errors::AiChatResult,
    views::home::{ConversationTabView, HomeView},
};
use gpui::*;
use gpui_component::{
    WindowExt,
    notification::{Notification, NotificationType},
    sidebar::SidebarMenuItem,
};
use std::ops::Deref;
use tracing::{Level, event};

pub struct ChatDataInner {
    pub(crate) conversations: Vec<Conversation>,
    pub(crate) folders: Vec<Folder>,
    tabs: Vec<ConversationTabView>,
    active_tab: Option<i32>,
}

impl ChatDataInner {
    fn new(cx: &mut Context<AiChatResult<Self>>) -> AiChatResult<Self> {
        let conn = &mut cx.global::<Db>().get()?;
        let conversations = Conversation::query_without_folder(conn)?;
        let folders = Folder::query(conn)?;
        let active_tab = conversations.first();
        let mut tabs = Vec::new();
        if let Some(tab) = active_tab {
            tabs.push(tab.into());
        }
        let active_tab_id = active_tab.map(|tab| tab.id);
        Ok(Self {
            conversations,
            folders,
            tabs,
            active_tab: active_tab_id,
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
    fn add_conversation(&mut self, new_conversation: Conversation) {
        if let Some(parent_id) = new_conversation.folder_id {
            if let Some(parent) = ChatDataInner::get_folder(&mut self.folders, parent_id) {
                parent.conversations.push(new_conversation);
            }
        } else {
            self.conversations.push(new_conversation);
        }
    }
    pub(crate) fn sidebar_items(&self) -> Vec<SidebarMenuItem> {
        let mut items = Vec::new();
        items.extend(self.folders.iter().map(From::from));
        items.extend(self.conversations.iter().map(From::from));
        items
    }
    fn get_conversation<'a>(
        folders: &'a [Folder],
        conversations: &'a [Conversation],
        conversation_id: i32,
    ) -> Option<&'a Conversation> {
        if let Some(find) = conversations
            .iter()
            .find(|Conversation { id, .. }| *id == conversation_id)
        {
            return Some(find);
        }
        for folder in folders {
            if let Some(conversation) =
                Self::get_conversation(&folder.folders, &folder.conversations, conversation_id)
            {
                return Some(conversation);
            }
        }
        None
    }
    fn add_tab(&mut self, conversation_id: i32) {
        match (
            self.tabs.iter().any(|id| id.id == conversation_id),
            Self::get_conversation(&self.folders, &self.conversations, conversation_id),
        ) {
            (true, Some(_)) => {
                self.active_tab = Some(conversation_id);
            }
            (false, Some(conversation)) => {
                self.tabs.push(conversation.into());
                self.active_tab = Some(conversation.id);
            }
            (false, None) => {}
            (true, None) => {
                self.tabs = self
                    .tabs
                    .iter()
                    .filter(|id| id.id != conversation_id)
                    .cloned()
                    .collect();
                self.active_tab = self.tabs.first().map(|conversation| conversation.id);
            }
        }
    }
    fn remove_tab(&mut self, conversation_id: i32) {
        if self.tabs.iter().any(|id| id.id == conversation_id) {
            self.tabs = self
                .tabs
                .iter()
                .filter(|id| id.id != conversation_id)
                .cloned()
                .collect();
            self.active_tab = self.tabs.first().map(|conversation| conversation.id);
        }
    }
    pub(crate) fn tabs(&self) -> Vec<ConversationTabView> {
        self.tabs.clone()
    }
    pub(crate) fn active_tab(&self) -> Option<&Conversation> {
        self.active_tab
            .and_then(|id| Self::get_conversation(&self.folders, &self.conversations, id))
    }
}

#[derive(Debug)]
pub enum ChatDataEvent {
    AddConversation {
        name: SharedString,
        icon: SharedString,
        info: Option<SharedString>,
        template: i32,
        parent_id: Option<i32>,
    },
    AddFolder {
        name: SharedString,
        parent_id: Option<i32>,
    },
    AddTab(i32),
    RemoveTab(i32),
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
        _this: &mut HomeView,
        state: &Entity<AiChatResult<ChatDataInner>>,
        chat_data_event: &ChatDataEvent,
        window: &mut Window,
        cx: &mut Context<HomeView>,
    ) {
        let span = tracing::info_span!("chat data event");
        let _enter = span.enter();
        event!(Level::INFO, "start:{chat_data_event:?}");
        match chat_data_event {
            ChatDataEvent::AddConversation {
                name,
                icon,
                info,
                template,
                parent_id,
            } => match Self::add_conversation(
                state,
                name,
                icon,
                info.as_ref().map(|x| x.as_str()),
                *parent_id,
                *template,
                cx,
            ) {
                Ok(_) => {}
                Err(err) => {
                    window.push_notification(
                        Notification::new()
                            .title("Add Conversation Failed")
                            .message(SharedString::from(err.to_string()))
                            .with_type(NotificationType::Error),
                        cx,
                    );
                    event!(Level::ERROR, "add conversation error:{err:?}")
                }
            },
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
                        event!(Level::ERROR, "add folder error:{err:?}")
                    }
                }
            }
            ChatDataEvent::AddTab(conversation_id) => {
                state.update(cx, |this, cx| {
                    if let Ok(this) = this {
                        this.add_tab(*conversation_id);
                    }
                });
            }
            ChatDataEvent::RemoveTab(conversation_id) => {
                state.update(cx, |this, cx| {
                    if let Ok(this) = this {
                        this.remove_tab(*conversation_id);
                    }
                });
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
        state.update(cx, |data, _cx| {
            if let Ok(data) = data {
                data.add_folder(folder);
            }
        });
        Ok(())
    }
    fn add_conversation(
        state: &Entity<AiChatResult<ChatDataInner>>,
        title: &str,
        icon: &str,
        info: Option<&str>,
        folder_id: Option<i32>,
        template_id: i32,
        cx: &mut Context<HomeView>,
    ) -> AiChatResult<()> {
        let new_conversation = NewConversation {
            title,
            folder_id,
            icon,
            info,
            template_id,
        };
        let conn = &mut cx.global::<Db>().get()?;
        let conversation = Conversation::insert(new_conversation, conn)?;
        state.update(cx, |data, _cx| {
            if let Ok(data) = data {
                data.add_conversation(conversation);
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

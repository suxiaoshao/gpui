use crate::{
    components::message::MessageView,
    database::{
        Content, Conversation, ConversationTemplate, Db, Folder, Message, NewConversation,
        NewFolder, NewMessage, Role, Status,
    },
    errors::AiChatResult,
    i18n::I18n,
    views::home::{
        ConversationPanelView, ConversationTabView, HomeView, TemplateDetailView, TemplateListView,
    },
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
    tabs: Vec<AppTab>,
    active_tab: Option<TabKind>,
}

#[derive(Debug, Clone)]
pub struct AddConversationMessage {
    pub role: Role,
    pub content: Content,
    pub status: Status,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TabKind {
    Conversation(i32),
    TemplateList,
    TemplateDetail(i32),
}

impl TabKind {
    fn key(self) -> i32 {
        match self {
            TabKind::Conversation(id) => id,
            TabKind::TemplateList => i32::MIN,
            TabKind::TemplateDetail(id) => id.saturating_add(1).saturating_neg(),
        }
    }
}

#[derive(Clone)]
enum TabPanel {
    Conversation(Entity<ConversationPanelView>),
    TemplateList(Entity<TemplateListView>),
    TemplateDetail(Entity<TemplateDetailView>),
}

impl TabPanel {
    fn into_any_element(self) -> AnyElement {
        match self {
            TabPanel::Conversation(panel) => panel.into_any_element(),
            TabPanel::TemplateList(panel) => panel.into_any_element(),
            TabPanel::TemplateDetail(panel) => panel.into_any_element(),
        }
    }
}

#[derive(Clone)]
struct AppTab {
    kind: TabKind,
    icon: SharedString,
    name: SharedString,
    panel: TabPanel,
}

impl ChatDataInner {
    fn new(window: &mut Window, cx: &mut Context<AiChatResult<Self>>) -> AiChatResult<Self> {
        let conn = &mut cx.global::<Db>().get()?;
        let conversations = Conversation::query_without_folder(conn)?;
        let folders = Folder::query(conn)?;
        let active_tab = conversations.first();
        let mut tabs = Vec::new();
        if let Some(tab) = active_tab {
            tabs.push(Self::conversation_tab(tab, window, cx));
        }
        let active_tab_kind = active_tab.map(|tab| TabKind::Conversation(tab.id));
        Ok(Self {
            conversations,
            folders,
            tabs,
            active_tab: active_tab_kind,
        })
    }
    fn conversation_tab(conversation: &Conversation, window: &mut Window, cx: &mut App) -> AppTab {
        AppTab {
            kind: TabKind::Conversation(conversation.id),
            icon: SharedString::from(&conversation.icon),
            name: SharedString::from(&conversation.title),
            panel: TabPanel::Conversation(
                cx.new(|cx| ConversationPanelView::new(conversation, window, cx)),
            ),
        }
    }
    fn template_tab(window: &mut Window, cx: &mut App) -> AppTab {
        AppTab {
            kind: TabKind::TemplateList,
            icon: "📋".into(),
            name: cx.global::<I18n>().t("tab-templates").into(),
            panel: TabPanel::TemplateList(cx.new(|cx| TemplateListView::new(window, cx))),
        }
    }
    fn template_detail_tab(template_id: i32, window: &mut Window, cx: &mut App) -> AppTab {
        let (icon, name) = cx
            .global::<Db>()
            .get()
            .ok()
            .and_then(|mut conn| ConversationTemplate::find(template_id, &mut conn).ok())
            .map(|template| {
                (
                    SharedString::from(template.icon),
                    SharedString::from(format!(
                        "{}: {}",
                        cx.global::<I18n>().t("tab-template"),
                        template.name
                    )),
                )
            })
            .unwrap_or_else(|| {
                (
                    SharedString::from("🧩"),
                    SharedString::from(format!(
                        "{} #{template_id}",
                        cx.global::<I18n>().t("tab-template")
                    )),
                )
            });
        AppTab {
            kind: TabKind::TemplateDetail(template_id),
            icon,
            name,
            panel: TabPanel::TemplateDetail(
                cx.new(|cx| TemplateDetailView::new(template_id, window, cx)),
            ),
        }
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
    fn get_conversation_mut<'a>(
        folders: &'a mut [Folder],
        conversations: &'a mut [Conversation],
        conversation_id: i32,
    ) -> Option<&'a mut Conversation> {
        if let Some(find) = conversations
            .iter_mut()
            .find(|Conversation { id, .. }| *id == conversation_id)
        {
            return Some(find);
        }
        for folder in folders {
            if let Some(conversation) = Self::get_conversation_mut(
                &mut folder.folders,
                &mut folder.conversations,
                conversation_id,
            ) {
                return Some(conversation);
            }
        }
        None
    }
    fn add_tab(&mut self, conversation_id: i32, window: &mut Window, cx: &mut App) {
        match (
            self.tabs
                .iter()
                .any(|tab| tab.kind == TabKind::Conversation(conversation_id)),
            Self::get_conversation(&self.folders, &self.conversations, conversation_id),
        ) {
            (true, Some(_)) => {
                self.active_tab = Some(TabKind::Conversation(conversation_id));
            }
            (false, Some(conversation)) => {
                self.tabs
                    .push(Self::conversation_tab(conversation, window, cx));
                self.active_tab = Some(TabKind::Conversation(conversation.id));
            }
            (false, None) => {}
            (true, None) => {
                self.tabs = self
                    .tabs
                    .iter()
                    .filter(|tab| tab.kind != TabKind::Conversation(conversation_id))
                    .cloned()
                    .collect();
                self.active_tab = self.tabs.first().map(|tab| tab.kind);
            }
        }
    }
    fn open_template_list_tab(&mut self, window: &mut Window, cx: &mut App) {
        if self
            .tabs
            .iter()
            .all(|tab| tab.kind != TabKind::TemplateList)
        {
            self.tabs.push(Self::template_tab(window, cx));
        }
        self.active_tab = Some(TabKind::TemplateList);
    }
    fn open_template_detail_tab(&mut self, template_id: i32, window: &mut Window, cx: &mut App) {
        let kind = TabKind::TemplateDetail(template_id);
        if self.tabs.iter().all(|tab| tab.kind != kind) {
            self.tabs
                .push(Self::template_detail_tab(template_id, window, cx));
        }
        self.active_tab = Some(kind);
    }
    fn activate_tab(&mut self, tab_key: i32) {
        self.active_tab = self
            .tabs
            .iter()
            .find(|tab| tab.kind.key() == tab_key)
            .map(|tab| tab.kind);
    }
    fn remove_tab(&mut self, tab_key: i32) {
        if self.tabs.iter().any(|tab| tab.kind.key() == tab_key) {
            self.tabs = self
                .tabs
                .iter()
                .filter(|tab| tab.kind.key() != tab_key)
                .cloned()
                .collect();
            if self.active_tab.is_some_and(|kind| kind.key() == tab_key) {
                self.active_tab = self.tabs.first().map(|tab| tab.kind);
            }
        }
    }
    fn move_tab(&mut self, from_id: i32, to_id: Option<i32>) {
        if to_id == Some(from_id) {
            return;
        }
        let Some(from_ix) = self.tabs.iter().position(|tab| tab.kind.key() == from_id) else {
            return;
        };
        let moved_kind = self.tabs[from_ix].kind;
        let to_ix = to_id
            .and_then(|target_id| self.tabs.iter().position(|tab| tab.kind.key() == target_id));
        Self::move_item(&mut self.tabs, from_ix, to_ix);
        self.active_tab = Some(moved_kind);
    }
    fn move_item<T>(items: &mut Vec<T>, from_ix: usize, to_ix: Option<usize>) {
        let item = items.remove(from_ix);
        match to_ix {
            Some(target_ix) => items.insert(target_ix.min(items.len()), item),
            None => items.push(item),
        }
    }
    pub(crate) fn tabs(&self) -> Vec<ConversationTabView> {
        self.tabs
            .iter()
            .map(|tab| ConversationTabView::new(tab.kind.key(), tab.icon.clone(), tab.name.clone()))
            .collect()
    }
    pub(crate) fn active_tab_key(&self) -> Option<i32> {
        self.active_tab.map(TabKind::key)
    }
    fn __delete_conversation(
        folders: &mut [Folder],
        conversations: &mut Vec<Conversation>,
        conversation_id: i32,
    ) {
        if let Some(index) = conversations.iter().position(|c| c.id == conversation_id) {
            conversations.remove(index);
        }
        for folder in folders.iter_mut() {
            Self::__delete_conversation(
                &mut folder.folders,
                &mut folder.conversations,
                conversation_id,
            );
        }
    }
    fn delete_conversation(&mut self, conversation_id: i32) {
        Self::__delete_conversation(&mut self.folders, &mut self.conversations, conversation_id);
        self.check_tabs();
    }

    fn delete_folder(&mut self, folder_id: i32) {
        Self::__delete_folder(&mut self.folders, folder_id);
        self.check_tabs();
    }
    fn __delete_folder(folders: &mut Vec<Folder>, folder_id: i32) {
        if let Some(index) = folders.iter().position(|f| f.id == folder_id) {
            folders.remove(index);
        }
        for folder in folders.iter_mut() {
            Self::__delete_folder(&mut folder.folders, folder_id);
        }
    }
    fn check_tabs(&mut self) {
        self.tabs = self
            .tabs
            .iter()
            .filter(|tab| match tab.kind {
                TabKind::Conversation(id) => {
                    Self::get_conversation(&self.folders, &self.conversations, id).is_some()
                }
                TabKind::TemplateList => true,
                TabKind::TemplateDetail(_) => true,
            })
            .cloned()
            .collect();
        if !self
            .tabs
            .iter()
            .any(|tab| Some(tab.kind) == self.active_tab)
        {
            self.active_tab = self.tabs.first().map(|tab| tab.kind);
        }
    }
    pub(crate) fn panel(&self) -> Option<AnyElement> {
        self.tabs.iter().find_map(|tab| {
            if Some(tab.kind) == self.active_tab {
                Some(tab.panel.clone().into_any_element())
            } else {
                None
            }
        })
    }
    pub(crate) fn panel_messages(&self) -> Vec<MessageView<Message>> {
        if let Some(TabKind::Conversation(conversation_id)) = self.active_tab
            && let Some(conversation) =
                Self::get_conversation(&self.folders, &self.conversations, conversation_id)
        {
            conversation
                .messages
                .iter()
                .cloned()
                .map(MessageView::new)
                .collect()
        } else {
            vec![]
        }
    }
    pub(crate) fn add_message(&mut self, conversation_id: i32, message: Message) {
        if let Some(conversation) =
            Self::get_conversation_mut(&mut self.folders, &mut self.conversations, conversation_id)
        {
            conversation.messages.push(message);
        }
    }
    fn __delete_message(
        folders: &mut [Folder],
        conversations: &mut [Conversation],
        message_id: i32,
    ) {
        for conversation in conversations {
            conversation
                .messages
                .retain(|message| message.id != message_id);
        }
        for folder in folders {
            Self::__delete_message(&mut folder.folders, &mut folder.conversations, message_id);
        }
    }
    pub(crate) fn delete_message(&mut self, message_id: i32) {
        Self::__delete_message(&mut self.folders, &mut self.conversations, message_id);
    }
    pub(crate) fn replace_message(&mut self, conversation_id: i32, message: Message) {
        if let Some(conversation) =
            Self::get_conversation_mut(&mut self.folders, &mut self.conversations, conversation_id)
        {
            if let Some(existing) = conversation
                .messages
                .iter_mut()
                .find(|message_item| message_item.id == message.id)
            {
                *existing = message;
            } else {
                conversation.messages.push(message);
            }
        }
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
        initial_messages: Option<Vec<AddConversationMessage>>,
    },
    AddFolder {
        name: SharedString,
        parent_id: Option<i32>,
    },
    AddTab(i32),
    ActivateTab(i32),
    OpenTemplateList,
    OpenTemplateDetail(i32),
    RemoveTab(i32),
    MoveTab {
        from_id: i32,
        to_id: Option<i32>,
    },
    DeleteMessage(i32),
    DeleteConversation(i32),
    DeleteFolder(i32),
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
        let (
            add_conversation_failed,
            add_folder_failed,
            delete_message_failed,
            delete_conversation_failed,
            delete_folder_failed,
        ) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("notify-add-conversation-failed"),
                i18n.t("notify-add-folder-failed"),
                i18n.t("notify-delete-message-failed"),
                i18n.t("notify-delete-conversation-failed"),
                i18n.t("notify-delete-folder-failed"),
            )
        };
        match chat_data_event {
            ChatDataEvent::AddConversation {
                name,
                icon,
                info,
                template,
                parent_id,
                initial_messages,
            } => match Self::add_conversation(
                state,
                name,
                icon,
                info.as_ref().map(|x| x.as_str()),
                *parent_id,
                *template,
                initial_messages.as_deref(),
                cx,
            ) {
                Ok(_) => {}
                Err(err) => {
                    window.push_notification(
                        Notification::new()
                            .title(add_conversation_failed)
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
                                .title(add_folder_failed)
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
                        this.add_tab(*conversation_id, window, cx);
                    }
                });
            }
            ChatDataEvent::ActivateTab(tab_key) => {
                state.update(cx, |this, _cx| {
                    if let Ok(this) = this {
                        this.activate_tab(*tab_key);
                    }
                });
            }
            ChatDataEvent::OpenTemplateList => {
                state.update(cx, |this, cx| {
                    if let Ok(this) = this {
                        this.open_template_list_tab(window, cx);
                    }
                });
            }
            ChatDataEvent::OpenTemplateDetail(template_id) => {
                state.update(cx, |this, cx| {
                    if let Ok(this) = this {
                        this.open_template_detail_tab(*template_id, window, cx);
                    }
                });
            }
            ChatDataEvent::RemoveTab(conversation_id) => {
                state.update(cx, |this, _cx| {
                    if let Ok(this) = this {
                        this.remove_tab(*conversation_id);
                    }
                });
            }
            ChatDataEvent::MoveTab { from_id, to_id } => {
                state.update(cx, |this, _cx| {
                    if let Ok(this) = this {
                        this.move_tab(*from_id, *to_id);
                    }
                });
            }
            ChatDataEvent::DeleteMessage(message_id) => {
                match Self::delete_message(state, *message_id, cx) {
                    Ok(_) => {}
                    Err(err) => {
                        window.push_notification(
                            Notification::new()
                                .title(delete_message_failed)
                                .message(SharedString::from(err.to_string()))
                                .with_type(NotificationType::Error),
                            cx,
                        );
                        event!(Level::ERROR, "delete message error:{err:?}")
                    }
                }
            }
            ChatDataEvent::DeleteConversation(conversation_id) => {
                match Self::delete_conversation(state, *conversation_id, cx) {
                    Ok(_) => {}
                    Err(err) => {
                        window.push_notification(
                            Notification::new()
                                .title(delete_conversation_failed)
                                .message(SharedString::from(err.to_string()))
                                .with_type(NotificationType::Error),
                            cx,
                        );
                        event!(Level::ERROR, "delete conversation error:{err:?}")
                    }
                }
            }
            ChatDataEvent::DeleteFolder(folder_id) => {
                match Self::delete_folder(state, *folder_id, cx) {
                    Ok(_) => {}
                    Err(err) => {
                        window.push_notification(
                            Notification::new()
                                .title(delete_folder_failed)
                                .message(SharedString::from(err.to_string()))
                                .with_type(NotificationType::Error),
                            cx,
                        );
                        event!(Level::ERROR, "delete folder error:{err:?}")
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
        initial_messages: Option<&[AddConversationMessage]>,
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
        let mut conversation = Conversation::insert(new_conversation, conn)?;
        if let Some(initial_messages) = initial_messages {
            for initial_message in initial_messages {
                let message = Message::insert(
                    NewMessage::new(
                        conversation.id,
                        initial_message.role,
                        initial_message.content.clone(),
                        serde_json::Value::Null,
                        initial_message.status,
                    ),
                    conn,
                )?;
                conversation.messages.push(message);
            }
        }
        state.update(cx, |data, _cx| {
            if let Ok(data) = data {
                data.add_conversation(conversation);
            }
        });
        Ok(())
    }
    fn delete_conversation(
        state: &Entity<AiChatResult<ChatDataInner>>,
        id: i32,
        cx: &mut Context<HomeView>,
    ) -> AiChatResult<()> {
        let conn = &mut cx.global::<Db>().get()?;
        Conversation::delete_by_id(id, conn)?;
        state.update(cx, |data, _cx| {
            if let Ok(data) = data {
                data.delete_conversation(id);
            }
        });
        Ok(())
    }
    fn delete_folder(
        state: &Entity<AiChatResult<ChatDataInner>>,
        id: i32,
        cx: &mut Context<HomeView>,
    ) -> AiChatResult<()> {
        let conn = &mut cx.global::<Db>().get()?;
        Folder::delete_by_id(id, conn)?;
        state.update(cx, |data, _cx| {
            if let Ok(data) = data {
                data.delete_folder(id);
            }
        });
        Ok(())
    }
    fn delete_message(
        state: &Entity<AiChatResult<ChatDataInner>>,
        message_id: i32,
        cx: &mut Context<HomeView>,
    ) -> AiChatResult<()> {
        let conn = &mut cx.global::<Db>().get()?;
        Message::delete(message_id, conn)?;
        state.update(cx, |data, _cx| {
            if let Ok(data) = data {
                data.delete_message(message_id);
            }
        });
        Ok(())
    }
}

pub(crate) fn init(window: &mut Window, cx: &mut Context<HomeView>) {
    let chat_data = cx.new(|cx| ChatDataInner::new(window, cx));
    let _subscriptions = vec![cx.subscribe_in(&chat_data, window, ChatData::subscribe_in)];
    cx.set_global(ChatData {
        data: chat_data,
        _subscriptions,
    });
}

#[cfg(test)]
mod tests {
    use super::ChatDataInner;
    use crate::database::{Content, Conversation, Folder, Message, Role, Status};
    use time::OffsetDateTime;

    fn now() -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }

    fn message(id: i32, conversation_id: i32) -> Message {
        Message {
            id,
            conversation_id,
            conversation_path: format!("/conversation/{conversation_id}"),
            role: Role::User,
            content: Content::Text(format!("message {id}")),
            status: Status::Normal,
            created_time: now(),
            updated_time: now(),
            start_time: now(),
            end_time: now(),
        }
    }

    fn conversation(id: i32, folder_id: Option<i32>) -> Conversation {
        Conversation {
            id,
            path: format!("/conversation/{id}"),
            folder_id,
            title: format!("Conversation {id}"),
            icon: "🤖".to_string(),
            created_time: now(),
            updated_time: now(),
            info: None,
            messages: vec![],
            template_id: 1,
        }
    }

    fn folder(id: i32, parent_id: Option<i32>) -> Folder {
        Folder {
            id,
            name: format!("Folder {id}"),
            path: format!("/folder/{id}"),
            parent_id,
            created_time: now(),
            updated_time: now(),
            conversations: vec![],
            folders: vec![],
        }
    }

    fn empty_chat_data() -> ChatDataInner {
        ChatDataInner {
            conversations: vec![],
            folders: vec![],
            tabs: vec![],
            active_tab: None,
        }
    }

    #[test]
    fn add_folder_places_into_parent() {
        let mut data = empty_chat_data();
        let mut root = folder(1, None);
        root.folders.push(folder(2, Some(1)));
        data.folders.push(root);

        let new_folder = folder(3, Some(2));
        data.add_folder(new_folder);

        let parent = ChatDataInner::get_folder(&mut data.folders, 2).unwrap();
        assert!(parent.folders.iter().any(|f| f.id == 3));
    }

    #[test]
    fn add_folder_to_root_when_no_parent() {
        let mut data = empty_chat_data();
        data.add_folder(folder(1, None));
        assert_eq!(data.folders.len(), 1);
        assert_eq!(data.folders[0].id, 1);
    }

    #[test]
    fn add_conversation_places_into_folder_or_root() {
        let mut data = empty_chat_data();
        data.folders.push(folder(1, None));

        data.add_conversation(conversation(1, Some(1)));
        data.add_conversation(conversation(2, None));

        let parent = ChatDataInner::get_folder(&mut data.folders, 1).unwrap();
        assert_eq!(parent.conversations.len(), 1);
        assert_eq!(parent.conversations[0].id, 1);
        assert_eq!(data.conversations.len(), 1);
        assert_eq!(data.conversations[0].id, 2);
    }

    #[test]
    fn get_conversation_recurses_through_folders() {
        let mut data = empty_chat_data();
        let mut root = folder(1, None);
        let mut child = folder(2, Some(1));
        child.conversations.push(conversation(3, Some(2)));
        root.folders.push(child);
        data.folders.push(root);

        let found = ChatDataInner::get_conversation(&data.folders, &data.conversations, 3);
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, 3);
    }

    #[test]
    fn get_conversation_mut_allows_updates() {
        let mut data = empty_chat_data();
        let mut root = folder(1, None);
        root.conversations.push(conversation(2, Some(1)));
        data.folders.push(root);

        if let Some(conversation) =
            ChatDataInner::get_conversation_mut(&mut data.folders, &mut data.conversations, 2)
        {
            conversation.title = "Updated".to_string();
        }

        let found = ChatDataInner::get_conversation(&data.folders, &data.conversations, 2);
        assert_eq!(found.unwrap().title, "Updated");
    }

    #[test]
    fn add_message_and_replace_message_updates_conversation() {
        let mut data = empty_chat_data();
        let mut root = folder(1, None);
        root.conversations.push(conversation(2, Some(1)));
        data.folders.push(root);

        data.add_message(2, message(10, 2));
        data.add_message(2, message(11, 2));
        let before = ChatDataInner::get_conversation(&data.folders, &data.conversations, 2)
            .unwrap()
            .messages
            .len();
        assert_eq!(before, 2);

        let mut updated = message(11, 2);
        updated.content = Content::Text("updated".to_string());
        data.replace_message(2, updated.clone());

        let found = ChatDataInner::get_conversation(&data.folders, &data.conversations, 2)
            .unwrap()
            .messages
            .iter()
            .find(|msg| msg.id == 11)
            .unwrap();
        assert_eq!(found.content, updated.content);

        data.replace_message(2, message(12, 2));
        let after = ChatDataInner::get_conversation(&data.folders, &data.conversations, 2)
            .unwrap()
            .messages
            .len();
        assert_eq!(after, 3);
    }

    #[test]
    fn delete_conversation_removes_nested_conversation() {
        let mut data = empty_chat_data();
        let mut root = folder(1, None);
        let mut child = folder(2, Some(1));
        child.conversations.push(conversation(3, Some(2)));
        root.folders.push(child);
        data.folders.push(root);

        data.delete_conversation(3);
        let found = ChatDataInner::get_conversation(&data.folders, &data.conversations, 3);
        assert!(found.is_none());
    }

    #[test]
    fn delete_folder_removes_nested_folder() {
        let mut data = empty_chat_data();
        let mut root = folder(1, None);
        root.folders.push(folder(2, Some(1)));
        data.folders.push(root);

        data.delete_folder(2);
        let found = ChatDataInner::get_folder(&mut data.folders, 2);
        assert!(found.is_none());
    }

    #[test]
    fn move_item_supports_forward_and_backward_moves() {
        let mut values = vec![1, 2, 3, 4];
        ChatDataInner::move_item(&mut values, 0, Some(1));
        assert_eq!(values, vec![2, 1, 3, 4]);

        ChatDataInner::move_item(&mut values, 3, Some(1));
        assert_eq!(values, vec![2, 4, 1, 3]);
    }
}

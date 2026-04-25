use crate::{
    database::{Content, Conversation, Db, Folder, Message, Role, Status},
    errors::AiChatResult,
    foundation::search::field_matches_query,
};
use gpui::*;
use std::collections::BTreeSet;
use time::OffsetDateTime;

pub struct ChatDataInner {
    pub(crate) conversations: Vec<Conversation>,
    pub(crate) folders: Vec<Folder>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationSearchResult {
    pub id: i32,
    pub title: String,
    pub icon: String,
    pub info: Option<String>,
    pub folder_path: Vec<String>,
}

impl ConversationSearchResult {
    pub(crate) fn path_label(&self, root_label: &str) -> String {
        if self.folder_path.is_empty() {
            root_label.to_string()
        } else {
            self.folder_path.join(" / ")
        }
    }
}

#[derive(Debug, Clone)]
pub struct AddConversationMessage {
    pub provider: String,
    pub role: Role,
    pub content: Content,
    pub send_content: serde_json::Value,
    pub status: Status,
    pub error: Option<String>,
}

// Bootstraps chat data from the database-backed tree.
impl ChatDataInner {
    pub(crate) fn new(
        _window: &mut Window,
        cx: &mut Context<AiChatResult<Self>>,
    ) -> AiChatResult<Self> {
        let conn = &mut cx.global::<Db>().get()?;
        let conversations = Conversation::query_without_folder(conn)?;
        let folders = Folder::query(conn)?;
        Ok(Self {
            conversations,
            folders,
        })
    }
    pub(crate) fn first_conversation(&self) -> Option<&Conversation> {
        self.conversations.first()
    }

    pub(crate) fn search_conversations(&self, query: &str) -> Vec<ConversationSearchResult> {
        let query = query.trim().to_lowercase();
        let mut results = Vec::new();
        Self::collect_conversation_results(&self.conversations, &[], query.as_str(), &mut results);
        Self::collect_folder_conversation_results(&self.folders, &[], query.as_str(), &mut results);
        results
    }

    fn collect_folder_conversation_results(
        folders: &[Folder],
        parent_path: &[String],
        query: &str,
        results: &mut Vec<ConversationSearchResult>,
    ) {
        for folder in folders {
            let mut folder_path = parent_path.to_vec();
            folder_path.push(folder.name.clone());
            Self::collect_conversation_results(&folder.conversations, &folder_path, query, results);
            Self::collect_folder_conversation_results(
                &folder.folders,
                &folder_path,
                query,
                results,
            );
        }
    }

    fn collect_conversation_results(
        conversations: &[Conversation],
        folder_path: &[String],
        query: &str,
        results: &mut Vec<ConversationSearchResult>,
    ) {
        for conversation in conversations {
            if conversation_matches_query(conversation, folder_path, query) {
                results.push(ConversationSearchResult {
                    id: conversation.id,
                    title: conversation.title.clone(),
                    icon: conversation.icon.clone(),
                    info: conversation.info.clone(),
                    folder_path: folder_path.to_vec(),
                });
            }
        }
    }
}

fn conversation_matches_query(
    conversation: &Conversation,
    folder_path: &[String],
    query: &str,
) -> bool {
    if query.is_empty() {
        return true;
    }

    field_matches_query(&conversation.title, query)
        || conversation
            .info
            .as_deref()
            .is_some_and(|info| field_matches_query(info, query))
        || folder_path
            .iter()
            .any(|folder_name| field_matches_query(folder_name, query))
        || field_matches_query(&conversation.path, query)
}

// Traverses the folder tree and mutates folder or conversation placement.
impl ChatDataInner {
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
    pub(crate) fn add_folder(&mut self, new_folder: Folder) {
        if let Some(parent_id) = new_folder.parent_id {
            if let Some(parent) = ChatDataInner::get_folder(&mut self.folders, parent_id) {
                parent.folders.push(new_folder);
            }
        } else {
            self.folders.push(new_folder);
        }
    }
    pub(crate) fn add_conversation(&mut self, new_conversation: Conversation) {
        if let Some(parent_id) = new_conversation.folder_id {
            if let Some(parent) = ChatDataInner::get_folder(&mut self.folders, parent_id) {
                parent.conversations.push(new_conversation);
            }
        } else {
            self.conversations.push(new_conversation);
        }
    }
    fn take_folder(folders: &mut Vec<Folder>, id: i32) -> Option<Folder> {
        if let Some(index) = folders.iter().position(|folder| folder.id == id) {
            return Some(folders.remove(index));
        }
        for folder in folders.iter_mut() {
            if let Some(found) = Self::take_folder(&mut folder.folders, id) {
                return Some(found);
            }
        }
        None
    }
    fn take_conversation(
        folders: &mut [Folder],
        conversations: &mut Vec<Conversation>,
        conversation_id: i32,
    ) -> Option<Conversation> {
        if let Some(index) = conversations
            .iter()
            .position(|conversation| conversation.id == conversation_id)
        {
            return Some(conversations.remove(index));
        }
        for folder in folders {
            if let Some(found) = Self::take_conversation(
                &mut folder.folders,
                &mut folder.conversations,
                conversation_id,
            ) {
                return Some(found);
            }
        }
        None
    }
    fn folder_contains_descendant(folders: &[Folder], folder_id: i32, target_id: i32) -> bool {
        folders.iter().any(|folder| {
            (folder.id == folder_id && Self::folder_subtree_contains(&folder.folders, target_id))
                || Self::folder_contains_descendant(&folder.folders, folder_id, target_id)
        })
    }
    fn folder_subtree_contains(folders: &[Folder], target_id: i32) -> bool {
        folders.iter().any(|folder| {
            folder.id == target_id || Self::folder_subtree_contains(&folder.folders, target_id)
        })
    }
    pub(crate) fn move_conversation(
        &mut self,
        conversation_id: i32,
        _target_folder_id: Option<i32>,
        updated: Conversation,
    ) {
        if Self::take_conversation(&mut self.folders, &mut self.conversations, conversation_id)
            .is_none()
        {
            return;
        }
        self.add_conversation(updated);
    }
    pub(crate) fn move_folder(
        &mut self,
        folder_id: i32,
        target_parent_id: Option<i32>,
        updated: Folder,
    ) {
        if target_parent_id == Some(folder_id) {
            return;
        }
        if let Some(target_parent_id) = target_parent_id
            && Self::folder_contains_descendant(&self.folders, folder_id, target_parent_id)
        {
            return;
        }
        if Self::take_folder(&mut self.folders, folder_id).is_none() {
            return;
        }
        self.add_folder(updated);
    }

    pub(crate) fn update_folder(&mut self, id: i32, updated: Folder) {
        if Self::take_folder(&mut self.folders, id).is_none() {
            return;
        }
        self.add_folder(updated);
    }

    pub(crate) fn update_conversation(&mut self, id: i32, updated: Conversation) {
        if Self::take_conversation(&mut self.folders, &mut self.conversations, id).is_none() {
            return;
        }
        self.add_conversation(updated);
    }
}

// Resolves conversations and manages tab ordering and activation.
impl ChatDataInner {
    fn find_folder(folders: &[Folder], id: i32) -> Option<&Folder> {
        for folder in folders {
            if folder.id == id {
                return Some(folder);
            }
            if let Some(folder) = Self::find_folder(&folder.folders, id) {
                return Some(folder);
            }
        }
        None
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
    pub(crate) fn conversation(&self, conversation_id: i32) -> Option<&Conversation> {
        Self::get_conversation(&self.folders, &self.conversations, conversation_id)
    }

    pub(crate) fn folder(&self, folder_id: i32) -> Option<&Folder> {
        Self::find_folder(&self.folders, folder_id)
    }

    pub(crate) fn folder_ids(&self) -> BTreeSet<i32> {
        let mut ids = BTreeSet::new();
        Self::collect_folder_ids(&self.folders, &mut ids);
        ids
    }

    #[cfg(test)]
    fn move_item<T>(items: &mut Vec<T>, from_ix: usize, to_ix: Option<usize>) {
        let item = items.remove(from_ix);
        match to_ix {
            Some(target_ix) => items.insert(target_ix.min(items.len()), item),
            None => items.push(item),
        }
    }

    fn collect_folder_ids(folders: &[Folder], ids: &mut BTreeSet<i32>) {
        for folder in folders {
            ids.insert(folder.id);
            Self::collect_folder_ids(&folder.folders, ids);
        }
    }
}

// Keeps the in-memory tree aligned after destructive mutations.
impl ChatDataInner {
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
    pub(crate) fn delete_conversation(&mut self, conversation_id: i32) {
        Self::__delete_conversation(&mut self.folders, &mut self.conversations, conversation_id);
    }

    pub(crate) fn delete_folder(&mut self, folder_id: i32) {
        Self::__delete_folder(&mut self.folders, folder_id);
    }
    fn __delete_folder(folders: &mut Vec<Folder>, folder_id: i32) {
        if let Some(index) = folders.iter().position(|f| f.id == folder_id) {
            folders.remove(index);
        }
        for folder in folders.iter_mut() {
            Self::__delete_folder(&mut folder.folders, folder_id);
        }
    }
    pub(crate) fn conversation_messages(&self, conversation_id: i32) -> Option<&[Message]> {
        Self::get_conversation(&self.folders, &self.conversations, conversation_id)
            .map(|conversation| conversation.messages.as_slice())
    }
    pub(crate) fn conversation_message_at(
        &self,
        conversation_id: i32,
        index: usize,
    ) -> Option<Message> {
        self.conversation_messages(conversation_id)?
            .get(index)
            .cloned()
    }
    pub(crate) fn add_message(&mut self, conversation_id: i32, message: Message) {
        if let Some(conversation) =
            Self::get_conversation_mut(&mut self.folders, &mut self.conversations, conversation_id)
        {
            conversation.messages.push(message);
        }
    }
    pub(crate) fn message(&self, conversation_id: i32, message_id: i32) -> Option<Message> {
        Self::get_conversation(&self.folders, &self.conversations, conversation_id).and_then(
            |conversation| {
                conversation
                    .messages
                    .iter()
                    .find(|message| message.id == message_id)
                    .cloned()
            },
        )
    }
    pub(crate) fn update_message(
        &mut self,
        conversation_id: i32,
        message_id: i32,
        update: impl FnOnce(&mut Message),
    ) -> bool {
        let Some(conversation) =
            Self::get_conversation_mut(&mut self.folders, &mut self.conversations, conversation_id)
        else {
            return false;
        };
        let Some(message) = conversation
            .messages
            .iter_mut()
            .find(|message| message.id == message_id)
        else {
            return false;
        };
        update(message);
        true
    }
    pub(crate) fn update_message_content(
        &mut self,
        message_id: i32,
        content: Content,
        updated_time: OffsetDateTime,
    ) -> bool {
        let mut content = Some(content);
        self.update_message_by_id(message_id, |message| {
            if let Some(content) = content.take() {
                message.content = content;
                message.updated_time = updated_time;
            }
        })
    }
    fn update_message_by_id(&mut self, message_id: i32, update: impl FnOnce(&mut Message)) -> bool {
        let mut update = Some(update);
        for conversation in &mut self.conversations {
            if let Some(message) = conversation
                .messages
                .iter_mut()
                .find(|message| message.id == message_id)
            {
                if let Some(update) = update.take() {
                    update(message);
                }
                return true;
            }
        }
        Self::__update_message_by_id(&mut self.folders, message_id, &mut update)
    }
    fn __update_message_by_id<F>(
        folders: &mut [Folder],
        message_id: i32,
        update: &mut Option<F>,
    ) -> bool
    where
        F: FnOnce(&mut Message),
    {
        for folder in folders {
            for conversation in &mut folder.conversations {
                if let Some(message) = conversation
                    .messages
                    .iter_mut()
                    .find(|message| message.id == message_id)
                {
                    if let Some(update) = update.take() {
                        update(message);
                    }
                    return true;
                }
            }
            if Self::__update_message_by_id(&mut folder.folders, message_id, update) {
                return true;
            }
        }
        false
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
    pub(crate) fn clear_conversation_messages(&mut self, conversation_id: i32) {
        if let Some(conversation) =
            Self::get_conversation_mut(&mut self.folders, &mut self.conversations, conversation_id)
        {
            conversation.messages.clear();
        }
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
            provider: "OpenAI".to_string(),
            role: Role::User,
            content: Content::new(format!("message {id}")),
            send_content: serde_json::json!({}),
            status: Status::Normal,
            created_time: now(),
            updated_time: now(),
            start_time: now(),
            end_time: now(),
            error: None,
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
        updated.content = Content::new("updated");
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
    fn message_helpers_read_and_update_nested_message() {
        let mut data = empty_chat_data();
        let mut root = folder(1, None);
        root.conversations.push(conversation(2, Some(1)));
        data.folders.push(root);
        data.add_message(2, message(11, 2));

        let found = data.message(2, 11).expect("message should exist");
        assert_eq!(found.id, 11);

        let updated = data.update_message(2, 11, |message| {
            message.content = Content::new("updated");
        });
        assert!(updated);
        assert_eq!(
            data.message(2, 11).expect("message should exist").content,
            Content::new("updated")
        );
        assert!(!data.update_message(2, 99, |_| {}));
    }

    #[test]
    fn update_message_content_finds_messages_by_id_across_tree() {
        let mut data = empty_chat_data();
        data.conversations.push(conversation(1, None));
        let mut root = folder(2, None);
        let mut child = folder(3, Some(2));
        child.conversations.push(conversation(4, Some(3)));
        root.folders.push(child);
        data.folders.push(root);
        data.add_message(1, message(10, 1));
        data.add_message(4, message(20, 4));

        let nested_time = now();
        assert!(data.update_message_content(20, Content::new("updated nested"), nested_time));
        let nested = data.message(4, 20).expect("message should exist");
        assert_eq!(nested.content, Content::new("updated nested"));
        assert_eq!(nested.updated_time, nested_time);

        let root_time = now();
        assert!(data.update_message_content(10, Content::new("updated root"), root_time));
        let root = data.message(1, 10).expect("message should exist");
        assert_eq!(root.content, Content::new("updated root"));
        assert_eq!(root.updated_time, root_time);
        assert!(!data.update_message_content(99, Content::new("missing"), now()));
    }

    #[test]
    fn clear_conversation_messages_only_resets_target_conversation() {
        let mut data = empty_chat_data();
        data.conversations.push(conversation(1, None));
        data.conversations.push(conversation(2, None));
        data.add_message(1, message(10, 1));
        data.add_message(1, message(11, 1));
        data.add_message(2, message(20, 2));

        data.clear_conversation_messages(1);

        assert_eq!(
            data.conversation_messages(1)
                .expect("conversation 1 should exist")
                .len(),
            0
        );
        assert_eq!(
            data.conversation_messages(2)
                .expect("conversation 2 should exist")
                .len(),
            1
        );
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
    fn move_conversation_from_root_to_folder() {
        let mut data = empty_chat_data();
        data.conversations.push(conversation(1, None));
        data.folders.push(folder(2, None));

        data.move_conversation(1, Some(2), conversation(1, Some(2)));

        assert!(data.conversations.is_empty());
        let folder = ChatDataInner::get_folder(&mut data.folders, 2).unwrap();
        assert_eq!(folder.conversations.len(), 1);
        assert_eq!(folder.conversations[0].folder_id, Some(2));
    }

    #[test]
    fn move_conversation_from_folder_to_root() {
        let mut data = empty_chat_data();
        let mut root = folder(1, None);
        root.conversations.push(conversation(2, Some(1)));
        data.folders.push(root);

        data.move_conversation(2, None, conversation(2, None));

        assert_eq!(data.conversations.len(), 1);
        assert_eq!(data.conversations[0].folder_id, None);
        let folder = ChatDataInner::get_folder(&mut data.folders, 1).unwrap();
        assert!(folder.conversations.is_empty());
    }

    #[test]
    fn move_folder_under_other_folder_preserves_children() {
        let mut data = empty_chat_data();
        let mut folder_one = folder(1, None);
        let mut child = folder(3, Some(1));
        child.conversations.push(conversation(4, Some(3)));
        folder_one.folders.push(child);
        data.folders.push(folder_one);
        data.folders.push(folder(2, None));

        let mut updated_child = folder(3, Some(2));
        updated_child.path = "/Folder 2/Folder 3".to_string();
        updated_child.conversations.push(conversation(4, Some(3)));

        data.move_folder(3, Some(2), updated_child);

        let destination = ChatDataInner::get_folder(&mut data.folders, 2).unwrap();
        assert_eq!(destination.folders.len(), 1);
        assert_eq!(destination.folders[0].id, 3);
        assert_eq!(destination.folders[0].conversations.len(), 1);
        let source = ChatDataInner::get_folder(&mut data.folders, 1).unwrap();
        assert!(source.folders.is_empty());
    }

    #[test]
    fn move_folder_rejects_descendant_parent() {
        let mut data = empty_chat_data();
        let mut root = folder(1, None);
        root.folders.push(folder(2, Some(1)));
        data.folders.push(root);

        data.move_folder(1, Some(2), folder(1, Some(2)));

        assert_eq!(data.folders.len(), 1);
        let root = ChatDataInner::get_folder(&mut data.folders, 1).unwrap();
        assert_eq!(root.parent_id, None);
    }

    #[test]
    fn update_folder_replaces_nested_folder_in_place() {
        let mut data = empty_chat_data();
        let mut root = folder(1, None);
        root.folders.push(folder(2, Some(1)));
        data.folders.push(root);

        let mut updated = folder(2, Some(1));
        updated.name = "Renamed Folder".to_string();
        updated.path = "/folder/1/renamed-folder".to_string();

        data.update_folder(2, updated);

        let folder = ChatDataInner::get_folder(&mut data.folders, 2).unwrap();
        assert_eq!(folder.name, "Renamed Folder");
        assert_eq!(folder.path, "/folder/1/renamed-folder");
    }

    #[test]
    fn update_conversation_replaces_nested_conversation_in_place() {
        let mut data = empty_chat_data();
        let mut root = folder(1, None);
        root.conversations.push(conversation(2, Some(1)));
        data.folders.push(root);

        let mut updated = conversation(2, Some(1));
        updated.title = "Renamed Conversation".to_string();
        updated.path = "/folder/1/renamed-conversation".to_string();
        updated.info = Some("updated info".to_string());

        data.update_conversation(2, updated);

        let conversation =
            ChatDataInner::get_conversation(&data.folders, &data.conversations, 2).unwrap();
        assert_eq!(conversation.title, "Renamed Conversation");
        assert_eq!(conversation.path, "/folder/1/renamed-conversation");
        assert_eq!(conversation.info.as_deref(), Some("updated info"));
    }

    #[test]
    fn folder_lookup_recurses_through_nested_tree() {
        let mut data = empty_chat_data();
        let mut root = folder(1, None);
        let child = folder(2, Some(1));
        root.folders.push(child);
        data.folders.push(root);

        let folder = data.folder(2).expect("folder should exist");
        assert_eq!(folder.id, 2);
    }

    #[test]
    fn search_conversations_matches_root_title_info_and_pinyin() {
        let mut data = empty_chat_data();
        let mut naming = conversation(1, None);
        naming.title = "命名助手".to_string();
        naming.info = Some("生成更好的名字".to_string());
        data.conversations.push(naming);
        data.conversations.push(conversation(2, None));

        assert_eq!(data.search_conversations("命名")[0].id, 1);
        assert_eq!(data.search_conversations("mmzs")[0].id, 1);
        assert_eq!(data.search_conversations("shengcheng")[0].id, 1);
    }

    #[test]
    fn search_conversations_recurses_into_nested_folders() {
        let mut data = empty_chat_data();
        let mut root = folder(1, None);
        root.name = "工作".to_string();
        let mut child = folder(2, Some(1));
        child.name = "归档".to_string();
        child.conversations.push(conversation(3, Some(2)));
        root.folders.push(child);
        data.folders.push(root);

        let results = data.search_conversations("gd");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, 3);
        assert_eq!(results[0].path_label("Root"), "工作 / 归档".to_string());
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

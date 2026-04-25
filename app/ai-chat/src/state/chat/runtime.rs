use super::{AddConversationMessage, ChatDataInner};
use crate::{
    database::{Conversation, Db, Folder, Message, NewConversation, NewFolder, NewMessage},
    errors::AiChatResult,
    features::home::HomeView,
    foundation::i18n::I18n,
    state::WorkspaceStore,
};
use gpui::*;
use gpui_component::{
    WindowExt,
    notification::{Notification, NotificationType},
};
use std::ops::Deref;
use tracing::{Level, event};

#[derive(Debug)]
pub enum ChatDataEvent {
    AddConversation {
        name: SharedString,
        icon: SharedString,
        info: Option<SharedString>,
        parent_id: Option<i32>,
        initial_messages: Option<Vec<AddConversationMessage>>,
    },
    AddFolder {
        name: SharedString,
        parent_id: Option<i32>,
    },
    UpdateFolder {
        id: i32,
        name: SharedString,
    },
    UpdateConversation {
        id: i32,
        title: SharedString,
        icon: SharedString,
        info: Option<SharedString>,
    },
    MoveConversation {
        conversation_id: i32,
        target_folder_id: Option<i32>,
    },
    MoveFolder {
        folder_id: i32,
        target_parent_id: Option<i32>,
    },
    DeleteMessage(i32),
    ClearConversationMessages(i32),
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

struct AddConversationInput<'a> {
    title: &'a str,
    icon: &'a str,
    info: Option<&'a str>,
    folder_id: Option<i32>,
    initial_messages: Option<&'a [AddConversationMessage]>,
}

// Subscribes UI views to chat-data events and dispatches them to handlers.
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
            update_folder_failed,
            update_conversation_failed,
            delete_message_failed,
            delete_conversation_failed,
            delete_folder_failed,
            move_conversation_failed,
            move_folder_failed,
        ) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("notify-add-conversation-failed").into(),
                i18n.t("notify-add-folder-failed").into(),
                i18n.t("notify-update-folder-failed").into(),
                i18n.t("notify-update-conversation-failed").into(),
                i18n.t("notify-delete-message-failed").into(),
                i18n.t("notify-delete-conversation-failed").into(),
                i18n.t("notify-delete-folder-failed").into(),
                i18n.t("notify-move-conversation-failed").into(),
                i18n.t("notify-move-folder-failed").into(),
            )
        };
        match chat_data_event {
            ChatDataEvent::AddConversation {
                name,
                icon,
                info,
                parent_id,
                initial_messages,
            } => Self::handle_event_result(
                Self::add_conversation(
                    state,
                    AddConversationInput {
                        title: name,
                        icon,
                        info: info.as_ref().map(|x| x.as_str()),
                        folder_id: *parent_id,
                        initial_messages: initial_messages.as_deref(),
                    },
                    cx,
                ),
                window,
                add_conversation_failed,
                "add conversation",
                cx,
            ),
            ChatDataEvent::AddFolder { name, parent_id } => Self::handle_event_result(
                Self::add_folder(state, name, *parent_id, cx),
                window,
                add_folder_failed,
                "add folder",
                cx,
            ),
            ChatDataEvent::UpdateFolder { id, name } => Self::handle_event_result(
                Self::update_folder(state, *id, name, cx),
                window,
                update_folder_failed,
                "update folder",
                cx,
            ),
            ChatDataEvent::UpdateConversation {
                id,
                title,
                icon,
                info,
            } => Self::handle_event_result(
                Self::update_conversation(
                    state,
                    *id,
                    title,
                    icon,
                    info.as_ref().map(|x| x.as_str()),
                    cx,
                ),
                window,
                update_conversation_failed,
                "update conversation",
                cx,
            ),
            ChatDataEvent::MoveConversation {
                conversation_id,
                target_folder_id,
            } => Self::handle_event_result(
                Self::move_conversation(state, *conversation_id, *target_folder_id, cx),
                window,
                move_conversation_failed,
                "move conversation",
                cx,
            ),
            ChatDataEvent::MoveFolder {
                folder_id,
                target_parent_id,
            } => Self::handle_event_result(
                Self::move_folder(state, *folder_id, *target_parent_id, cx),
                window,
                move_folder_failed,
                "move folder",
                cx,
            ),
            ChatDataEvent::DeleteMessage(message_id) => Self::handle_event_result(
                Self::delete_message(state, *message_id, cx),
                window,
                delete_message_failed,
                "delete message",
                cx,
            ),
            ChatDataEvent::ClearConversationMessages(conversation_id) => Self::handle_event_result(
                Self::clear_conversation_messages(state, *conversation_id, cx),
                window,
                delete_message_failed,
                "clear conversation messages",
                cx,
            ),
            ChatDataEvent::DeleteConversation(conversation_id) => Self::handle_event_result(
                Self::delete_conversation(state, *conversation_id, cx),
                window,
                delete_conversation_failed,
                "delete conversation",
                cx,
            ),
            ChatDataEvent::DeleteFolder(folder_id) => Self::handle_event_result(
                Self::delete_folder(state, *folder_id, cx),
                window,
                delete_folder_failed,
                "delete folder",
                cx,
            ),
        }
    }

    fn handle_event_result(
        result: AiChatResult<()>,
        window: &mut Window,
        title: SharedString,
        action: &'static str,
        cx: &mut Context<HomeView>,
    ) {
        if let Err(err) = result {
            window.push_notification(
                Notification::new()
                    .title(title)
                    .message(SharedString::from(err.to_string()))
                    .with_type(NotificationType::Error),
                cx,
            );
            event!(Level::ERROR, "{action} error:{err:?}");
        }
    }
}

// Persists folder and conversation changes before updating in-memory state.
impl ChatData {
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
        input: AddConversationInput<'_>,
        cx: &mut Context<HomeView>,
    ) -> AiChatResult<()> {
        let new_conversation = NewConversation {
            title: input.title,
            folder_id: input.folder_id,
            icon: input.icon,
            info: input.info,
        };
        let conn = &mut cx.global::<Db>().get()?;
        let mut conversation = Conversation::insert(new_conversation, conn)?;
        if let Some(initial_messages) = input.initial_messages {
            for initial_message in initial_messages {
                let mut new_message = NewMessage::new(
                    conversation.id,
                    &initial_message.provider,
                    initial_message.role,
                    &initial_message.content,
                    &initial_message.send_content,
                    initial_message.status,
                );
                if let Some(error) = initial_message.error.as_ref() {
                    new_message = new_message.with_error(error);
                }
                let message = Message::insert(new_message, conn)?;
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
}

// Persists folder, conversation, and message deletions or moves.
impl ChatData {
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
        cx.global::<WorkspaceStore>()
            .deref()
            .clone()
            .update(cx, |workspace, cx| {
                workspace.remove_conversation_tab(id, cx);
            });
        Ok(())
    }
    fn move_conversation(
        state: &Entity<AiChatResult<ChatDataInner>>,
        conversation_id: i32,
        target_folder_id: Option<i32>,
        cx: &mut Context<HomeView>,
    ) -> AiChatResult<()> {
        let conn = &mut cx.global::<Db>().get()?;
        let conversation = Conversation::move_to_folder(conversation_id, target_folder_id, conn)?;
        state.update(cx, |data, _cx| {
            if let Ok(data) = data {
                data.move_conversation(conversation_id, target_folder_id, conversation);
            }
        });
        Ok(())
    }
    fn move_folder(
        state: &Entity<AiChatResult<ChatDataInner>>,
        folder_id: i32,
        target_parent_id: Option<i32>,
        cx: &mut Context<HomeView>,
    ) -> AiChatResult<()> {
        let conn = &mut cx.global::<Db>().get()?;
        let folder = Folder::move_to_parent(folder_id, target_parent_id, conn)?;
        state.update(cx, |data, _cx| {
            if let Ok(data) = data {
                data.move_folder(folder_id, target_parent_id, folder);
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
        cx.global::<WorkspaceStore>()
            .deref()
            .clone()
            .update(cx, |workspace, cx| {
                workspace.sanitize_open_folders(cx);
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
    fn clear_conversation_messages(
        state: &Entity<AiChatResult<ChatDataInner>>,
        conversation_id: i32,
        cx: &mut Context<HomeView>,
    ) -> AiChatResult<()> {
        let conn = &mut cx.global::<Db>().get()?;
        Message::delete_by_conversation_id(conversation_id, conn)?;
        state.update(cx, |data, _cx| {
            if let Ok(data) = data {
                data.clear_conversation_messages(conversation_id);
            }
        });
        Ok(())
    }

    fn update_folder(
        state: &Entity<AiChatResult<ChatDataInner>>,
        id: i32,
        name: &str,
        cx: &mut Context<HomeView>,
    ) -> AiChatResult<()> {
        let conn = &mut cx.global::<Db>().get()?;
        let folder = Folder::update_name(id, name, conn)?;
        state.update(cx, |data, _cx| {
            if let Ok(data) = data {
                data.update_folder(id, folder);
            }
        });
        Ok(())
    }

    fn update_conversation(
        state: &Entity<AiChatResult<ChatDataInner>>,
        id: i32,
        title: &str,
        icon: &str,
        info: Option<&str>,
        cx: &mut Context<HomeView>,
    ) -> AiChatResult<()> {
        let conn = &mut cx.global::<Db>().get()?;
        let conversation = Conversation::update(id, title, icon, info, conn)?;
        state.update(cx, |data, _cx| {
            if let Ok(data) = data {
                data.update_conversation(id, conversation.clone());
            }
        });
        cx.global::<WorkspaceStore>()
            .deref()
            .clone()
            .update(cx, |workspace, cx| {
                workspace.sync_conversation_metadata(&conversation, cx);
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

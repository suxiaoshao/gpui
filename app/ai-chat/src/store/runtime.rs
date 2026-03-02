use super::{AddConversationMessage, ChatDataInner};
use crate::{
    database::{Conversation, Db, Folder, Message, NewConversation, NewFolder, NewMessage},
    errors::AiChatResult,
    i18n::I18n,
    views::home::HomeView,
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

struct AddConversationInput<'a> {
    title: &'a str,
    icon: &'a str,
    info: Option<&'a str>,
    folder_id: Option<i32>,
    template_id: i32,
    initial_messages: Option<&'a [AddConversationMessage]>,
}

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
                AddConversationInput {
                    title: name,
                    icon,
                    info: info.as_ref().map(|x| x.as_str()),
                    folder_id: *parent_id,
                    template_id: *template,
                    initial_messages: initial_messages.as_deref(),
                },
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
        input: AddConversationInput<'_>,
        cx: &mut Context<HomeView>,
    ) -> AiChatResult<()> {
        let new_conversation = NewConversation {
            title: input.title,
            folder_id: input.folder_id,
            icon: input.icon,
            info: input.info,
            template_id: input.template_id,
        };
        let conn = &mut cx.global::<Db>().get()?;
        let mut conversation = Conversation::insert(new_conversation, conn)?;
        if let Some(initial_messages) = input.initial_messages {
            for initial_message in initial_messages {
                let message = Message::insert(
                    NewMessage::new(
                        conversation.id,
                        initial_message.role,
                        initial_message.content.clone(),
                        initial_message.send_content.clone(),
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

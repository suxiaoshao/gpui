use super::utils::{deserialize_offset_date_time, serialize_offset_date_time};
use crate::{
    components::message::MessageViewExt,
    database::{
        Db, Role, Status,
        model::{SqlConversation, SqlMessage, SqlNewMessage},
    },
    errors::{AiChatError, AiChatResult},
    i18n::I18n,
    store::{ChatData, ChatDataEvent},
    views::message_preview::{MessagePreview, MessagePreviewExt},
};
use fluent_bundle::FluentArgs;
use diesel::SqliteConnection;
use gpui::{App, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};
use gpui_component::Root;
use serde::{Deserialize, Serialize};
use std::ops::{AddAssign, Deref};
use time::OffsetDateTime;
use tracing::{Level, event};

#[derive(Debug, Serialize, Clone, Deserialize, PartialEq, Eq)]
pub struct TemporaryMessage {
    pub id: usize,
    pub role: Role,
    pub content: Content,
    pub send_content: serde_json::Value,
    pub status: Status,
    #[serde(
        rename = "createdTime",
        serialize_with = "serialize_offset_date_time",
        deserialize_with = "deserialize_offset_date_time"
    )]
    pub created_time: OffsetDateTime,
    #[serde(
        rename = "updatedTime",
        serialize_with = "serialize_offset_date_time",
        deserialize_with = "deserialize_offset_date_time"
    )]
    pub updated_time: OffsetDateTime,
    #[serde(
        rename = "startTime",
        serialize_with = "serialize_offset_date_time",
        deserialize_with = "deserialize_offset_date_time"
    )]
    pub start_time: OffsetDateTime,
    #[serde(
        rename = "endTime",
        serialize_with = "serialize_offset_date_time",
        deserialize_with = "deserialize_offset_date_time"
    )]
    pub end_time: OffsetDateTime,
}

#[derive(Debug, Serialize, Clone, Deserialize, PartialEq, Eq)]
#[serde(tag = "tag", content = "value", rename_all = "camelCase")]
pub enum Content {
    Text(String),
    Extension {
        source: String,
        #[serde(rename = "extensionName")]
        extension_name: String,
        content: String,
    },
}

impl AddAssign<String> for Content {
    fn add_assign(&mut self, rhs: String) {
        match self {
            Content::Text(text) => {
                *text += &rhs;
            }
            Content::Extension { source, .. } => {
                *source += &rhs;
            }
        }
    }
}

impl AddAssign<&str> for Content {
    fn add_assign(&mut self, rhs: &str) {
        match self {
            Content::Text(text) => {
                *text += rhs;
            }
            Content::Extension { source, .. } => {
                *source += rhs;
            }
        }
    }
}

impl Content {
    pub(crate) fn send_content(&self) -> &str {
        match self {
            Content::Text(content) => content,
            Content::Extension { content, .. } => content,
        }
    }
}

#[cfg(test)]
mod content_tests {
    use super::Content;

    #[test]
    fn add_assign_appends_text_content() {
        let mut content = Content::Text("hello".to_string());
        content += " world";
        assert_eq!(content, Content::Text("hello world".to_string()));
    }

    #[test]
    fn add_assign_appends_extension_source() {
        let mut content = Content::Extension {
            source: "src".to_string(),
            extension_name: "ext".to_string(),
            content: "payload".to_string(),
        };
        content += " more";
        assert_eq!(
            content,
            Content::Extension {
                source: "src more".to_string(),
                extension_name: "ext".to_string(),
                content: "payload".to_string(),
            }
        );
    }

    #[test]
    fn send_content_uses_extension_payload() {
        let content = Content::Extension {
            source: "src".to_string(),
            extension_name: "ext".to_string(),
            content: "payload".to_string(),
        };
        assert_eq!(content.send_content(), "payload");
    }
}

#[derive(Debug, Serialize, Clone, Deserialize, PartialEq, Eq)]
pub struct Message {
    pub id: i32,
    #[serde(rename = "conversationId")]
    pub conversation_id: i32,
    #[serde(rename = "conversationPath")]
    pub conversation_path: String,
    pub role: Role,
    pub content: Content,
    pub status: Status,
    #[serde(
        rename = "createdTime",
        serialize_with = "serialize_offset_date_time",
        deserialize_with = "deserialize_offset_date_time"
    )]
    pub created_time: OffsetDateTime,
    #[serde(
        rename = "updatedTime",
        serialize_with = "serialize_offset_date_time",
        deserialize_with = "deserialize_offset_date_time"
    )]
    pub updated_time: OffsetDateTime,
    #[serde(
        rename = "startTime",
        serialize_with = "serialize_offset_date_time",
        deserialize_with = "deserialize_offset_date_time"
    )]
    pub start_time: OffsetDateTime,
    #[serde(
        rename = "endTime",
        serialize_with = "serialize_offset_date_time",
        deserialize_with = "deserialize_offset_date_time"
    )]
    pub end_time: OffsetDateTime,
}

impl MessageViewExt for Message {
    type Id = i32;

    fn role(&self) -> &Role {
        &self.role
    }

    fn content(&self) -> &Content {
        &self.content
    }

    fn id(&self) -> Self::Id {
        self.id
    }

    fn status(&self) -> &Status {
        &self.status
    }

    fn open_view_by_id(id: Self::Id, _window: &mut gpui::Window, cx: &mut gpui::App) {
        let message = match cx.global::<Db>().get() {
            Ok(mut conn) => match Message::find(id, &mut conn) {
                Ok(message) => message,
                Err(err) => {
                    event!(Level::ERROR, "find message failed: {}", err);
                    return;
                }
            },
            Err(err) => {
                event!(Level::ERROR, "get db failed: {}", err);
                return;
            }
        };
        let title = {
            let i18n = cx.global::<I18n>();
            let mut args = FluentArgs::new();
            args.set("id", message.id as i64);
            i18n.t_with_args("message-preview-title", &args)
        };
        match cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(800.), px(600.)),
                    cx,
                ))),
                titlebar: Some(TitlebarOptions {
                    title: Some(title.into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                let message_view = cx.new(|cx| MessagePreview::new(message.clone(), window, cx));
                cx.new(|cx| Root::new(message_view, window, cx))
            },
        ) {
            Ok(_) => {}
            Err(err) => {
                event!(Level::ERROR, "open message view window: {}", err);
            }
        };
    }

    fn delete_message_by_id(message_id: Self::Id, _window: &mut gpui::Window, cx: &mut App) {
        let chat_data = cx.global::<ChatData>().deref().clone();
        chat_data.update(cx, move |_this, cx| {
            cx.emit(ChatDataEvent::DeleteMessage(message_id));
        });
    }
}

impl MessagePreviewExt for Message {
    fn on_update_content(&self, content: Content, cx: &mut App) -> AiChatResult<()> {
        let conn = &mut cx.global::<Db>().get()?;
        Message::update_content(self.id, &content, conn)?;
        Ok(())
    }
}

impl TryFrom<SqlMessage> for Message {
    type Error = AiChatError;

    fn try_from(value: SqlMessage) -> Result<Self, Self::Error> {
        Ok(Message {
            id: value.id,
            conversation_id: value.conversation_id,
            conversation_path: value.conversation_path,
            role: value.role.parse()?,
            content: serde_json::from_str(&value.content)?,
            status: value.status.parse()?,
            created_time: value.created_time,
            updated_time: value.updated_time,
            start_time: value.start_time,
            end_time: value.end_time,
        })
    }
}

#[derive(Deserialize, Debug)]
pub struct NewMessage {
    pub conversation_id: i32,
    pub role: Role,
    pub content: Content,
    pub send_content: serde_json::Value,
    pub status: Status,
}

impl NewMessage {
    pub fn new(
        conversation_id: i32,
        role: Role,
        content: Content,
        send_content: serde_json::Value,
        status: Status,
    ) -> Self {
        Self {
            conversation_id,
            role,
            content,
            send_content,
            status,
        }
    }
}

impl Message {
    pub fn insert(
        NewMessage {
            conversation_id,
            role,
            content,
            send_content: _,
            status,
        }: NewMessage,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<Message> {
        conn.immediate_transaction(|conn| {
            let time = OffsetDateTime::now_utc();
            let SqlConversation { path, .. } = SqlConversation::find(conversation_id, conn)?;

            let new_message = SqlNewMessage {
                conversation_id,
                conversation_path: path,
                role: role.to_string(),
                content: serde_json::to_string(&content)?,
                // send_content,
                status: status.to_string(),
                created_time: time,
                updated_time: time,
                start_time: time,
                end_time: time,
            };
            let message = new_message.insert(conn)?;
            Message::try_from(message)
        })
    }
    pub fn insert_many(
        messages: Vec<TemporaryMessage>,
        path: String,
        conversation_id: i32,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<()> {
        let messages = messages
            .into_iter()
            .map(
                |TemporaryMessage {
                     role,
                     content,
                     send_content: _,
                     created_time,
                     updated_time,
                     start_time,
                     end_time,
                     status,
                     ..
                 }| {
                    Ok(SqlNewMessage {
                        conversation_id,
                        conversation_path: path.clone(),
                        role: role.to_string(),
                        content: serde_json::to_string(&content)?,
                        // send_content,
                        status: status.to_string(),
                        created_time,
                        updated_time,
                        start_time,
                        end_time,
                    })
                },
            )
            .collect::<Result<Vec<_>, AiChatError>>()?;
        SqlNewMessage::insert_many(&messages, conn)?;
        Ok(())
    }
    pub fn messages_by_conversation_id(
        conversation_id: i32,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<Vec<Message>> {
        let messages = SqlMessage::query_by_conversation_id(conversation_id, conn)?;
        messages
            .into_iter()
            .map(TryFrom::try_from)
            .collect::<AiChatResult<_>>()
    }
    pub fn add_content(
        id: i32,
        new_content: String,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<()> {
        let time = OffsetDateTime::now_utc();
        let SqlMessage { content, .. } = SqlMessage::find(id, conn)?;
        let mut content = serde_json::from_str::<Content>(&content)?;
        content += new_content;
        SqlMessage::add_content(id, serde_json::to_string(&content)?, time, conn)?;
        Ok(())
    }
    pub fn update_status(id: i32, status: Status, conn: &mut SqliteConnection) -> AiChatResult<()> {
        let time = OffsetDateTime::now_utc();
        SqlMessage::update_status(id, status, time, conn)?;
        Ok(())
    }
    pub fn find(id: i32, conn: &mut SqliteConnection) -> AiChatResult<Message> {
        let message = SqlMessage::find(id, conn)?;
        Message::try_from(message)
    }
    pub fn update_path(
        old_path_pre: &str,
        new_path_pre: &str,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<()> {
        let update_list = SqlMessage::find_by_path_pre(old_path_pre, conn)?;
        let time = OffsetDateTime::now_utc();
        update_list.into_iter().try_for_each(
            |SqlMessage {
                 id,
                 conversation_path,
                 ..
             }| {
                SqlMessage::update_path(
                    id,
                    conversation_path,
                    old_path_pre,
                    new_path_pre,
                    time,
                    conn,
                )?;
                Ok::<(), AiChatError>(())
            },
        )?;
        Ok(())
    }
    pub fn delete(id: i32, conn: &mut SqliteConnection) -> AiChatResult<()> {
        SqlMessage::delete(id, conn)?;
        Ok(())
    }
    pub fn update_content(
        id: i32,
        content: &Content,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<()> {
        let time = OffsetDateTime::now_utc();
        SqlMessage::update_content(id, serde_json::to_string(content)?, time, conn)?;
        Ok(())
    }
}

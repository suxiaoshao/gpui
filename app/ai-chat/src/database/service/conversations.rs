use diesel::SqliteConnection;
use time::OffsetDateTime;

use crate::{
    database::{
        Message,
        model::{
            SqlConversation, SqlFolder, SqlMessage, SqlNewConversation, SqlUpdateConversation,
        },
    },
    errors::{AiChatError, AiChatResult},
};

use super::utils::serialize_offset_date_time;

#[derive(serde::Serialize, Clone, Debug)]
pub struct Conversation {
    pub id: i32,
    pub path: String,
    #[serde(rename = "folderId")]
    pub folder_id: Option<i32>,
    pub title: String,
    pub icon: String,
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
    pub info: Option<String>,
    pub messages: Vec<Message>,
}

#[derive(serde::Deserialize, Debug)]
pub struct NewConversation<'a> {
    pub title: &'a str,
    #[serde(rename = "folderId")]
    pub folder_id: Option<i32>,
    pub icon: &'a str,
    pub info: Option<&'a str>,
}

impl Conversation {
    pub fn find(id: i32, conn: &mut SqliteConnection) -> AiChatResult<Self> {
        let sql_conversation = SqlConversation::find(id, conn)?;
        Self::from_sql_conversation(sql_conversation, conn)
    }
    pub fn insert(
        NewConversation {
            title,
            folder_id,
            icon,
            info,
        }: NewConversation,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<Self> {
        let time = OffsetDateTime::now_utc();
        let folder = folder_id
            .map(|folder_id| SqlFolder::find(folder_id, conn))
            .transpose()?;
        let path = match folder {
            Some(folder) => format!("{}/{}", folder.path, title),
            None => format!("/{title}"),
        };
        if SqlFolder::path_exists(&path, conn)? {
            return Err(AiChatError::FolderPathExists(path));
        }
        if SqlConversation::path_exists(&path, conn)? {
            return Err(AiChatError::ConversationPathExists(path));
        }
        let sql_new = SqlNewConversation {
            title,
            path,
            folder_id,
            icon,
            info,
            created_time: time,
            updated_time: time,
        };
        let conversation = sql_new.insert(conn)?;
        Self::from_sql_conversation(conversation, conn)
    }
    /// 获取没有文件夹的会话
    pub fn query_without_folder(conn: &mut SqliteConnection) -> AiChatResult<Vec<Conversation>> {
        let data = SqlConversation::query_without_folder(conn)?;
        data.into_iter()
            .map(|sql_conversation| Self::from_sql_conversation(sql_conversation, conn))
            .collect::<AiChatResult<_>>()
    }
    fn from_sql_conversation(
        SqlConversation {
            id,
            path,
            folder_id,
            title,
            icon,
            created_time,
            updated_time,
            info,
        }: SqlConversation,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<Conversation> {
        let messages = Message::messages_by_conversation_id(id, conn)?;
        Ok(Conversation {
            id,
            path,
            folder_id,
            title,
            icon,
            created_time,
            updated_time,
            info,
            messages,
        })
    }
    pub fn find_by_folder_id(
        folder_id: i32,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<Vec<Conversation>> {
        let data = SqlConversation::find_by_folder_id(folder_id, conn)?;
        data.into_iter()
            .map(|sql_conversation| Self::from_sql_conversation(sql_conversation, conn))
            .collect::<AiChatResult<_>>()
    }
    pub fn delete_by_id(id: i32, conn: &mut SqliteConnection) -> AiChatResult<()> {
        conn.immediate_transaction(|conn| {
            SqlMessage::delete_by_conversation_id(id, conn)?;
            SqlConversation::delete_by_id(id, conn)?;
            Ok::<(), AiChatError>(())
        })?;
        Ok(())
    }

    pub fn move_to_folder(
        id: i32,
        target_folder_id: Option<i32>,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<Self> {
        conn.immediate_transaction(|conn| {
            let conversation = SqlConversation::find(id, conn)?;
            if conversation.folder_id == target_folder_id {
                return Self::from_sql_conversation(conversation, conn);
            }

            let target_folder = target_folder_id
                .map(|folder_id| SqlFolder::find(folder_id, conn))
                .transpose()?;
            let new_path = match target_folder {
                Some(ref folder) => format!("{}/{}", folder.path, conversation.title),
                None => format!("/{}", conversation.title),
            };
            if SqlFolder::path_exists(&new_path, conn)? {
                return Err(AiChatError::FolderPathExists(new_path));
            }
            if SqlConversation::path_exists(&new_path, conn)? {
                return Err(AiChatError::ConversationPathExists(new_path));
            }

            let time = OffsetDateTime::now_utc();
            SqlUpdateConversation::move_folder(id, target_folder_id, &new_path, time, conn)?;
            SqlMessage::move_folder(id, &new_path, time, conn)?;

            Self::find(id, conn)
        })
    }
}

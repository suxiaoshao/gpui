use super::utils::serialize_offset_date_time;
use crate::{
    database::{
        Conversation,
        model::{SqlConversation, SqlFolder, SqlMessage, SqlNewFolder},
    },
    errors::{AiChatError, AiChatResult},
};
use diesel::SqliteConnection;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Serialize, Debug)]
pub struct Folder {
    pub id: i32,
    pub name: String,
    pub path: String,
    #[serde(rename = "parentId")]
    pub parent_id: Option<i32>,
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
    pub conversations: Vec<Conversation>,
    pub folders: Vec<Folder>,
}

impl Folder {
    pub fn insert(
        NewFolder { name, parent_id }: NewFolder,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<Self> {
        let now = OffsetDateTime::now_utc();
        let parent_folder = parent_id
            .map(|folder_id| SqlFolder::find(folder_id, conn))
            .transpose()?;
        let path = match parent_folder {
            Some(parent_folder) => format!("{}/{}", parent_folder.path, name),
            None => format!("/{name}"),
        };
        if SqlFolder::path_exists(&path, conn)? {
            return Err(AiChatError::FolderPathExists(path));
        }
        if SqlConversation::path_exists(&path, conn)? {
            return Err(AiChatError::ConversationPathExists(path));
        }
        let new_folder = SqlNewFolder {
            name,
            path,
            parent_id,
            created_time: now,
            updated_time: now,
        };
        let new_folder = new_folder.insert(conn)?;
        let folder = Self::from_sql_folder(new_folder, conn)?;
        Ok(folder)
    }
    pub fn query(conn: &mut SqliteConnection) -> AiChatResult<Vec<Self>> {
        let sql_folders = SqlFolder::query(conn)?;
        sql_folders
            .into_iter()
            .map(|sql_folder| Self::from_sql_folder(sql_folder, conn))
            .collect()
    }
    fn from_sql_folder(
        SqlFolder {
            id,
            path,
            name,
            parent_id,
            created_time,
            updated_time,
        }: SqlFolder,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<Self> {
        let conversations = Conversation::find_by_folder_id(id, conn)?;
        let folders = Self::find_by_parent_id(id, conn)?;
        Ok(Self {
            id,
            name,
            path,
            parent_id,
            created_time,
            updated_time,
            conversations,
            folders,
        })
    }
    fn find_by_parent_id(parent_id: i32, conn: &mut SqliteConnection) -> AiChatResult<Vec<Self>> {
        let sql_folders = SqlFolder::query_by_parent_id(parent_id, conn)?;
        sql_folders
            .into_iter()
            .map(|sql_folder| Self::from_sql_folder(sql_folder, conn))
            .collect()
    }
    pub fn delete_by_id(id: i32, conn: &mut SqliteConnection) -> AiChatResult<()> {
        conn.immediate_transaction::<_, AiChatError, _>(|conn| {
            let folder = SqlFolder::find(id, conn)?;
            SqlMessage::delete_by_path(&folder.path, conn)?;
            SqlConversation::delete_by_path(&folder.path, conn)?;
            SqlFolder::delete_by_path(&folder.path, conn)?;
            SqlFolder::delete_by_id(id, conn)?;
            Ok(())
        })?;
        Ok(())
    }
}

#[derive(Deserialize)]
pub struct NewFolder<'a> {
    name: &'a str,
    #[serde(rename = "parentId")]
    parent_id: Option<i32>,
}

impl<'a> NewFolder<'a> {
    pub fn new(name: &'a str, parent_id: Option<i32>) -> Self {
        Self { name, parent_id }
    }
}

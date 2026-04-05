use super::utils::serialize_offset_date_time;
use crate::{
    database::{
        Conversation,
        model::{
            SqlConversation, SqlFolder, SqlMessage, SqlNewFolder, SqlUpdateConversation,
            SqlUpdateFolder,
        },
    },
    errors::{AiChatError, AiChatResult},
};
use diesel::SqliteConnection;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Serialize, Clone, Debug)]
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

// Creates folders and rebuilds nested folder trees from persisted data.
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
}

// Deletes folders and rewrites descendant paths when folders move.
impl Folder {
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

    pub fn update_name(id: i32, name: &str, conn: &mut SqliteConnection) -> AiChatResult<Self> {
        conn.immediate_transaction::<_, AiChatError, _>(|conn| {
            let folder = SqlFolder::find(id, conn)?;
            if folder.name == name {
                return Self::from_sql_folder(folder, conn);
            }

            let new_path = if let Some(parent_id) = folder.parent_id {
                let parent = SqlFolder::find(parent_id, conn)?;
                format!("{}/{}", parent.path, name)
            } else {
                format!("/{name}")
            };

            if SqlFolder::path_exists(&new_path, conn)? {
                return Err(AiChatError::FolderPathExists(new_path));
            }
            if SqlConversation::path_exists(&new_path, conn)? {
                return Err(AiChatError::ConversationPathExists(new_path));
            }

            let old_path = folder.path.clone();
            let time = OffsetDateTime::now_utc();

            SqlUpdateFolder {
                id,
                name,
                path: new_path.clone(),
                parent_id: folder.parent_id,
                updated_time: time,
            }
            .update(conn)?;

            for child in SqlFolder::find_by_path_pre(&old_path, conn)? {
                SqlUpdateFolder::from_new_path(&child, &old_path, &new_path, time).update(conn)?;
            }

            for conversation in SqlConversation::find_by_path_pre(&old_path, conn)? {
                SqlUpdateConversation::from_new_path(&conversation, &old_path, &new_path, time)
                    .update(conn)?;
            }

            for message in SqlMessage::find_by_path_pre(&old_path, conn)? {
                SqlMessage::update_path(
                    message.id,
                    message.conversation_path,
                    &old_path,
                    &new_path,
                    time,
                    conn,
                )?;
            }

            Self::from_sql_folder(SqlFolder::find(id, conn)?, conn)
        })
    }

    pub fn move_to_parent(
        id: i32,
        new_parent_id: Option<i32>,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<Self> {
        conn.immediate_transaction::<_, AiChatError, _>(|conn| {
            let folder = SqlFolder::find(id, conn)?;
            if folder.parent_id == new_parent_id {
                return Self::from_sql_folder(folder, conn);
            }

            if new_parent_id == Some(id) {
                return Err(AiChatError::InvalidFolderMove(format!(
                    "folder {id} cannot move into itself"
                )));
            }

            let parent = new_parent_id
                .map(|parent_id| SqlFolder::find(parent_id, conn))
                .transpose()?;
            if let Some(parent) = parent.as_ref()
                && (parent.path == folder.path
                    || parent.path.starts_with(&format!("{}/", folder.path)))
            {
                return Err(AiChatError::InvalidFolderMove(format!(
                    "folder {id} cannot move into descendant {}",
                    parent.id
                )));
            }

            let new_path = match parent {
                Some(ref parent) => format!("{}/{}", parent.path, folder.name),
                None => format!("/{}", folder.name),
            };
            if SqlFolder::path_exists(&new_path, conn)? {
                return Err(AiChatError::FolderPathExists(new_path));
            }
            if SqlConversation::path_exists(&new_path, conn)? {
                return Err(AiChatError::ConversationPathExists(new_path));
            }

            let old_path = folder.path.clone();
            let time = OffsetDateTime::now_utc();
            SqlUpdateFolder::move_folder(id, new_parent_id, &new_path, time, conn)?;

            for child in SqlFolder::find_by_path_pre(&old_path, conn)? {
                SqlUpdateFolder::from_new_path(&child, &old_path, &new_path, time).update(conn)?;
            }
            for conversation in SqlConversation::find_by_path_pre(&old_path, conn)? {
                SqlUpdateConversation::from_new_path(&conversation, &old_path, &new_path, time)
                    .update(conn)?;
            }
            for message in SqlMessage::find_by_path_pre(&old_path, conn)? {
                SqlMessage::update_path(
                    message.id,
                    message.conversation_path,
                    &old_path,
                    &new_path,
                    time,
                    conn,
                )?;
            }

            Self::from_sql_folder(SqlFolder::find(id, conn)?, conn)
        })
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

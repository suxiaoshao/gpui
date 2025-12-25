use diesel::SqliteConnection;
use gpui::SharedString;
use gpui_component::tree::TreeItem;
use pinyin::ToPinyin;
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

#[derive(serde::Serialize)]
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
    #[serde(rename = "templateId")]
    pub template_id: i32,
}

impl From<&Conversation> for TreeItem {
    fn from(value: &Conversation) -> Self {
        TreeItem::new(
            SharedString::from(format!("conversation-tree-item-{}", value.id)),
            value.title.clone(),
        )
    }
}

#[derive(serde::Deserialize, Debug)]
pub struct NewConversation {
    pub title: String,
    #[serde(rename = "folderId")]
    pub folder_id: Option<i32>,
    pub icon: String,
    pub info: Option<String>,
    #[serde(rename = "templateId")]
    pub template_id: i32,
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
            template_id,
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
            template_id,
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
            template_id,
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
            template_id,
        })
    }
    pub fn update(
        id: i32,
        NewConversation {
            title,
            folder_id,
            icon,
            info,
            template_id,
        }: NewConversation,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<()> {
        let time = OffsetDateTime::now_utc();
        let folder = folder_id
            .map(|folder_id| SqlFolder::find(folder_id, conn))
            .transpose()?;
        let old_conversation = SqlConversation::find(id, conn)?;
        let path = match folder {
            Some(folder) => format!("{}/{}", folder.path, title),
            None => format!("/{title}"),
        };
        if SqlFolder::path_exists(&path, conn)? {
            return Err(AiChatError::FolderPathExists(path));
        }
        let path_updated = old_conversation.path != path;
        if path_updated && SqlConversation::path_exists(&path, conn)? {
            return Err(AiChatError::ConversationPathExists(path));
        }
        let update = SqlUpdateConversation {
            id,
            path: path.clone(),
            title,
            folder_id,
            icon,
            updated_time: time,
            info,
            template_id,
        };
        conn.immediate_transaction(|conn| {
            update.update(conn)?;
            if path_updated {
                SqlMessage::move_folder(id, &path, time, conn)?;
            }
            Ok::<(), AiChatError>(())
        })?;
        Ok(())
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
    pub fn update_path(
        old_path_pre: &str,
        new_path_pre: &str,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<()> {
        let update_list = SqlConversation::find_by_path_pre(old_path_pre, conn)?;
        let time = OffsetDateTime::now_utc();
        update_list
            .into_iter()
            .map(|old| SqlUpdateConversation::from_new_path(old, old_path_pre, new_path_pre, time))
            .try_for_each(|update| {
                update.update(conn)?;
                Ok::<(), AiChatError>(())
            })?;
        Ok(())
    }
    pub fn move_folder(
        conversation_id: i32,
        new_folder_id: Option<i32>,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<()> {
        let SqlConversation {
            folder_id: old_folder_id,
            mut path,
            ..
        } = SqlConversation::find(conversation_id, conn)?;
        let old_path_pre = match old_folder_id {
            Some(folder_id) => {
                let SqlFolder { path, .. } = SqlFolder::find(folder_id, conn)?;
                path
            }
            _ => "/".to_string(),
        };
        let new_path_pre = match new_folder_id {
            Some(new_folder_id) => {
                let SqlFolder { path, .. } = SqlFolder::find(new_folder_id, conn)?;
                path
            }
            None => "/".to_string(),
        };
        path.replace_range(0..old_path_pre.len(), &new_path_pre);
        conn.immediate_transaction::<_, AiChatError, _>(|conn| {
            let time = OffsetDateTime::now_utc();
            SqlUpdateConversation::move_folder(conversation_id, new_folder_id, &path, time, conn)?;
            SqlMessage::move_folder(conversation_id, &path, time, conn)?;
            Ok(())
        })?;
        Ok(())
    }
    pub fn clear(id: i32, conn: &mut SqliteConnection) -> AiChatResult<()> {
        SqlMessage::delete_by_conversation_id(id, conn)?;
        Ok(())
    }
    pub fn search(query: &str, conn: &mut SqliteConnection) -> AiChatResult<Vec<Conversation>> {
        let all_conversations = SqlConversation::get_all(conn)?;
        let data = all_conversations
            .into_iter()
            .filter(|SqlConversation { title, info, .. }| {
                title.contains(query)
                    || get_chinese_str(title).contains(query)
                    || info.as_ref().is_some_and(|info| info.contains(query))
                    || get_chinese_str(info.as_ref().unwrap_or(&"".to_string())).contains(query)
            })
            .map(|sql_conversation| Self::from_sql_conversation(sql_conversation, conn))
            .collect::<AiChatResult<_>>()?;
        Ok(data)
    }
}

fn get_chinese_str(data: &str) -> String {
    data.chars()
        .map(|x| {
            x.to_pinyin()
                .map(|x| x.plain().to_string())
                .unwrap_or(x.to_string())
        })
        .fold("".to_string(), |acc, x| acc + &x)
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_pinyin() {
        let data = "我爱你";
        assert_eq!(get_chinese_str(data), "woaini");
        let data = "编写 issue 详情";
        assert_eq!(get_chinese_str(data), "bianxie issue xiangqing");
    }
}

/*
 * @Author: suxiaoshao suxiaoshao@gmail.com
 * @Date: 2024-04-28 04:23:22
 * @LastEditors: suxiaoshao suxiaoshao@gmail.com
 * @LastEditTime: 2024-05-16 04:24:23
 * @FilePath: /tauri/packages/ChatGPT/src-tauri/src/store/service/conversation_templates.rs
 */
use crate::{
    database::{
        Role,
        model::{
            SqlConversationTemplate, SqlNewConversationTemplate, SqlUpdateConversationTemplate,
        },
    },
    errors::AiChatResult,
};
use diesel::SqliteConnection;
use gpui_component::select::SelectItem;
use time::OffsetDateTime;

use super::utils::{deserialize_offset_date_time, serialize_offset_date_time};

#[derive(serde::Serialize, Clone, serde::Deserialize)]
pub struct ConversationTemplatePrompt {
    pub prompt: String,
    pub role: Role,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct ConversationTemplate {
    pub id: i32,
    pub name: String,
    pub icon: String,
    pub description: Option<String>,
    pub prompts: Vec<ConversationTemplatePrompt>,
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
}

impl SelectItem for ConversationTemplate {
    type Value = i32;

    fn title(&self) -> gpui::SharedString {
        self.name.clone().into()
    }

    fn value(&self) -> &Self::Value {
        &self.id
    }
}

impl ConversationTemplate {
    pub fn find(id: i32, conn: &mut SqliteConnection) -> AiChatResult<Self> {
        let SqlConversationTemplate {
            id,
            name,
            icon,
            created_time,
            updated_time,
            description,
            prompts,
        } = SqlConversationTemplate::find(id, conn)?;
        Ok(Self {
            id,
            name,
            icon,
            created_time,
            updated_time,
            description,
            prompts: serde_json::from_value(prompts)?,
        })
    }
    pub fn all(conn: &mut SqliteConnection) -> AiChatResult<Vec<Self>> {
        let sql_conversation_templates = SqlConversationTemplate::all(conn)?;
        let mut conversation_templates = Vec::new();
        for SqlConversationTemplate {
            id,
            name,
            icon,
            created_time,
            updated_time,
            description,
            prompts,
        } in sql_conversation_templates
        {
            conversation_templates.push(Self {
                id,
                name,
                icon,
                created_time,
                updated_time,
                description,
                prompts: serde_json::from_value(prompts)?,
            });
        }
        Ok(conversation_templates)
    }
    pub fn update(
        NewConversationTemplate {
            name,
            icon,
            description,
            prompts,
        }: NewConversationTemplate,
        id: i32,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<()> {
        let time = OffsetDateTime::now_utc();
        conn.immediate_transaction(|conn| {
            // Update the conversation template
            let sql_new = SqlUpdateConversationTemplate {
                id,
                name,
                icon,
                updated_time: time,
                description,
                prompts: serde_json::to_value(prompts)?,
            };
            sql_new.update(conn)?;
            Ok(())
        })
    }
    pub fn delete(id: i32, conn: &mut SqliteConnection) -> AiChatResult<()> {
        conn.immediate_transaction(|conn| {
            SqlConversationTemplate::delete_by_id(id, conn)?;
            Ok(())
        })
    }
}

#[derive(serde::Deserialize)]
pub struct NewConversationTemplate {
    pub name: String,
    pub icon: String,
    pub description: Option<String>,
    pub prompts: Vec<ConversationTemplatePrompt>,
}

impl NewConversationTemplate {
    pub fn insert(self, conn: &mut SqliteConnection) -> AiChatResult<i32> {
        let NewConversationTemplate {
            name,
            icon,
            description,
            prompts,
        } = self;
        let time = OffsetDateTime::now_utc();
        conn.immediate_transaction(|conn| {
            // Insert the new conversation template
            let sql_new = SqlNewConversationTemplate {
                name,
                icon,
                created_time: time,
                updated_time: time,
                description,
                prompts: serde_json::to_value(prompts)?,
            };
            let SqlConversationTemplate { id, .. } = sql_new.insert(conn)?;
            Ok(id)
        })
    }
}

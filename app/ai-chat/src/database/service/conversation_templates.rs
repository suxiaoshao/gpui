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
    foundation::search::field_matches_query,
    llm::CapabilityRequirement,
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
    #[serde(rename = "requiredCapabilities", default)]
    pub required_capabilities: Vec<CapabilityRequirement>,
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

    fn matches(&self, query: &str) -> bool {
        self.matches_search_query(query)
    }

    fn value(&self) -> &Self::Value {
        &self.id
    }
}

impl ConversationTemplate {
    pub fn matches_search_query(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return true;
        }

        field_matches_query(&self.name, &query)
            || self
                .description
                .as_deref()
                .is_some_and(|description| field_matches_query(description, &query))
    }

    pub fn find(id: i32, conn: &mut SqliteConnection) -> AiChatResult<Self> {
        let SqlConversationTemplate {
            id,
            name,
            icon,
            created_time,
            updated_time,
            description,
            prompts,
            required_capabilities,
        } = SqlConversationTemplate::find(id, conn)?;
        Ok(Self {
            id,
            name,
            icon,
            created_time,
            updated_time,
            description,
            prompts: serde_json::from_value(prompts)?,
            required_capabilities: serde_json::from_value(required_capabilities)?,
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
            required_capabilities,
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
                required_capabilities: serde_json::from_value(required_capabilities)?,
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
            required_capabilities,
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
                required_capabilities: serde_json::to_value(required_capabilities)?,
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
    #[serde(rename = "requiredCapabilities", default)]
    pub required_capabilities: Vec<CapabilityRequirement>,
}

impl NewConversationTemplate {
    pub fn insert(self, conn: &mut SqliteConnection) -> AiChatResult<i32> {
        let NewConversationTemplate {
            name,
            icon,
            description,
            prompts,
            required_capabilities,
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
                required_capabilities: serde_json::to_value(required_capabilities)?,
            };
            let SqlConversationTemplate { id, .. } = sql_new.insert(conn)?;
            Ok(id)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{ConversationTemplate, NewConversationTemplate};
    use crate::{
        database::{CREATE_TABLE_SQL, ConversationTemplatePrompt, Role},
        llm::CapabilityRequirement,
    };
    use diesel::{Connection, SqliteConnection, connection::SimpleConnection};
    use time::OffsetDateTime;

    fn setup_conn() -> anyhow::Result<SqliteConnection> {
        let mut conn = SqliteConnection::establish(":memory:")?;
        conn.batch_execute(CREATE_TABLE_SQL)?;
        Ok(conn)
    }

    fn template(name: &str, description: Option<&str>) -> ConversationTemplate {
        ConversationTemplate {
            id: 1,
            name: name.to_string(),
            icon: "🤖".to_string(),
            description: description.map(ToString::to_string),
            prompts: vec![ConversationTemplatePrompt {
                prompt: "hello".to_string(),
                role: Role::User,
            }],
            required_capabilities: Vec::new(),
            created_time: OffsetDateTime::UNIX_EPOCH,
            updated_time: OffsetDateTime::UNIX_EPOCH,
        }
    }

    #[test]
    fn matches_search_query_supports_name_pinyin() {
        let template = template("命名助手", Some("生成更好的名字"));

        assert!(template.matches_search_query("mingming"));
        assert!(template.matches_search_query("mmzs"));
    }

    #[test]
    fn matches_search_query_supports_description_pinyin() {
        let template = template("总结", Some("生成更好的名字"));

        assert!(template.matches_search_query("shengcheng"));
        assert!(template.matches_search_query("ghdmz"));
    }

    #[test]
    fn required_capabilities_roundtrip_insert_find_update() -> anyhow::Result<()> {
        let mut conn = setup_conn()?;
        let prompts = vec![ConversationTemplatePrompt {
            prompt: "hello".to_string(),
            role: Role::User,
        }];
        let id = NewConversationTemplate {
            name: "Vision".to_string(),
            icon: "🖼️".to_string(),
            description: None,
            prompts: prompts.clone(),
            required_capabilities: vec![
                CapabilityRequirement::ImageInput,
                CapabilityRequirement::Reasoning,
            ],
        }
        .insert(&mut conn)?;

        let inserted = ConversationTemplate::find(id, &mut conn)?;
        assert_eq!(
            inserted.required_capabilities,
            vec![
                CapabilityRequirement::ImageInput,
                CapabilityRequirement::Reasoning,
            ]
        );

        ConversationTemplate::update(
            NewConversationTemplate {
                name: "Tool".to_string(),
                icon: "🛠️".to_string(),
                description: Some("tool template".to_string()),
                prompts,
                required_capabilities: vec![CapabilityRequirement::ToolCalling],
            },
            id,
            &mut conn,
        )?;
        let updated = ConversationTemplate::find(id, &mut conn)?;
        assert_eq!(
            updated.required_capabilities,
            vec![CapabilityRequirement::ToolCalling]
        );
        Ok(())
    }
}

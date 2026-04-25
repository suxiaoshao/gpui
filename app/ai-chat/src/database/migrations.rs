use crate::{
    database::{
        CREATE_TABLE_SQL, Content, ConversationTemplatePrompt, Mode, Role, Status,
        model::{SqlConversation, SqlConversationTemplate, SqlFolder, SqlMessage},
    },
    errors::AiChatResult,
    llm::{Message as FetchMessage, provider_by_name},
};
use diesel::{SqliteConnection, connection::SimpleConnection};
use std::collections::HashMap;
use time::OffsetDateTime;

pub(super) fn v1_to_v3(
    v1_conn: &mut SqliteConnection,
    target_conn: &mut SqliteConnection,
) -> AiChatResult<()> {
    migrate_legacy_store(
        LegacyData {
            folders: SqlFolder::all(v1_conn)?,
            templates: v1::SqlConversationTemplateV1::all(v1_conn)?
                .into_iter()
                .map(LegacyTemplate::try_from)
                .collect::<AiChatResult<_>>()?,
            conversations: v2::SqlConversationV2::all(v1_conn)?
                .into_iter()
                .map(Into::into)
                .collect(),
            messages: LegacyMessageSource::V1(
                v1::SqlMessageV1::all(v1_conn)?
                    .into_iter()
                    .map(Into::into)
                    .collect(),
            ),
        },
        target_conn,
    )
}

pub(super) fn v1_to_v5(
    v1_conn: &mut SqliteConnection,
    target_conn: &mut SqliteConnection,
) -> AiChatResult<()> {
    v1_to_v3(v1_conn, target_conn)
}

pub(super) fn v2_to_v3(
    v2_conn: &mut SqliteConnection,
    target_conn: &mut SqliteConnection,
) -> AiChatResult<()> {
    migrate_legacy_store(
        LegacyData {
            folders: SqlFolder::all(v2_conn)?,
            templates: v2::SqlConversationTemplateV2::all(v2_conn)?
                .into_iter()
                .map(Into::into)
                .collect(),
            conversations: v2::SqlConversationV2::all(v2_conn)?
                .into_iter()
                .map(Into::into)
                .collect(),
            messages: LegacyMessageSource::V2(
                v2::SqlMessageV2::all(v2_conn)?
                    .into_iter()
                    .map(Into::into)
                    .collect(),
            ),
        },
        target_conn,
    )
}

pub(super) fn v2_to_v5(
    v2_conn: &mut SqliteConnection,
    target_conn: &mut SqliteConnection,
) -> AiChatResult<()> {
    v2_to_v3(v2_conn, target_conn)
}

pub(super) fn v3_to_v5(
    v3_conn: &mut SqliteConnection,
    target_conn: &mut SqliteConnection,
) -> AiChatResult<()> {
    target_conn.immediate_transaction(|target_conn| {
        target_conn.batch_execute(CREATE_TABLE_SQL)?;
        SqlFolder::migration_save(SqlFolder::all(v3_conn)?, target_conn)?;
        SqlConversationTemplate::migration_save(
            SqlConversationTemplate::all(v3_conn)?,
            target_conn,
        )?;
        SqlConversation::migration_save(SqlConversation::get_all(v3_conn)?, target_conn)?;
        SqlMessage::migration_save(
            build_v3_migrated_messages(v3::SqlMessageV3::all(v3_conn)?)?,
            target_conn,
        )?;
        Ok(())
    })
}

pub(super) fn v4_to_v5(
    v4_conn: &mut SqliteConnection,
    target_conn: &mut SqliteConnection,
) -> AiChatResult<()> {
    target_conn.immediate_transaction(|target_conn| {
        target_conn.batch_execute(CREATE_TABLE_SQL)?;
        SqlFolder::migration_save(SqlFolder::all(v4_conn)?, target_conn)?;
        SqlConversationTemplate::migration_save(
            SqlConversationTemplate::all(v4_conn)?,
            target_conn,
        )?;
        SqlConversation::migration_save(SqlConversation::get_all(v4_conn)?, target_conn)?;
        SqlMessage::migration_save(SqlMessage::all(v4_conn)?, target_conn)?;
        Ok(())
    })
}

fn migrate_legacy_store(data: LegacyData, target_conn: &mut SqliteConnection) -> AiChatResult<()> {
    let LegacyData {
        folders,
        templates,
        conversations,
        messages,
    } = data;
    target_conn.immediate_transaction(|target_conn| {
        target_conn.batch_execute(CREATE_TABLE_SQL)?;

        let migrated_messages = match messages {
            LegacyMessageSource::V1(messages) => {
                build_v1_migrated_messages(&templates, &conversations, messages)?
            }
            LegacyMessageSource::V2(messages) => {
                build_v2_migrated_messages(&templates, &conversations, messages)?
            }
        };

        SqlFolder::migration_save(folders, target_conn)?;
        SqlConversationTemplate::migration_save(
            templates.iter().cloned().map(Into::into).collect(),
            target_conn,
        )?;
        SqlConversation::migration_save(
            conversations.iter().cloned().map(Into::into).collect(),
            target_conn,
        )?;
        SqlMessage::migration_save(migrated_messages, target_conn)?;
        Ok(())
    })
}

struct LegacyData {
    folders: Vec<SqlFolder>,
    templates: Vec<LegacyTemplate>,
    conversations: Vec<LegacyConversation>,
    messages: LegacyMessageSource,
}

enum LegacyMessageSource {
    V1(Vec<LegacyMessageV1>),
    V2(Vec<LegacyMessageV2>),
}

#[derive(Clone, serde::Deserialize)]
#[serde(tag = "tag", content = "value", rename_all = "camelCase")]
enum LegacyContent {
    Text(String),
    WebSearch {
        text: String,
        citations: Vec<crate::database::UrlCitation>,
    },
}

impl From<LegacyContent> for Content {
    fn from(value: LegacyContent) -> Self {
        match value {
            LegacyContent::Text(text) => Content::new(text),
            LegacyContent::WebSearch { text, citations } => {
                Content::with_citations(text, citations)
            }
        }
    }
}

#[derive(Clone)]
struct LegacyTemplate {
    id: i32,
    name: String,
    icon: String,
    description: Option<String>,
    mode: Mode,
    provider: String,
    template: serde_json::Value,
    prompts: serde_json::Value,
    created_time: OffsetDateTime,
    updated_time: OffsetDateTime,
}

impl From<LegacyTemplate> for SqlConversationTemplate {
    fn from(value: LegacyTemplate) -> Self {
        Self {
            id: value.id,
            name: value.name,
            icon: value.icon,
            description: value.description,
            prompts: value.prompts,
            created_time: value.created_time,
            updated_time: value.updated_time,
        }
    }
}

#[derive(Clone)]
struct LegacyConversation {
    id: i32,
    folder_id: Option<i32>,
    path: String,
    title: String,
    icon: String,
    created_time: OffsetDateTime,
    updated_time: OffsetDateTime,
    info: Option<String>,
    template_id: i32,
}

impl From<LegacyConversation> for SqlConversation {
    fn from(value: LegacyConversation) -> Self {
        Self {
            id: value.id,
            folder_id: value.folder_id,
            path: value.path,
            title: value.title,
            icon: value.icon,
            created_time: value.created_time,
            updated_time: value.updated_time,
            info: value.info,
        }
    }
}

#[derive(Clone)]
struct LegacyMessageV1 {
    id: i32,
    conversation_id: i32,
    conversation_path: String,
    role: String,
    content: String,
    status: String,
    created_time: OffsetDateTime,
    updated_time: OffsetDateTime,
    start_time: OffsetDateTime,
    end_time: OffsetDateTime,
}

#[derive(Clone)]
struct LegacyMessageV2 {
    id: i32,
    conversation_id: i32,
    conversation_path: String,
    role: String,
    content: String,
    send_content: serde_json::Value,
    status: String,
    created_time: OffsetDateTime,
    updated_time: OffsetDateTime,
    start_time: OffsetDateTime,
    end_time: OffsetDateTime,
    error: Option<String>,
}

fn normalize_legacy_provider_name(name: &str) -> &str {
    match name {
        "OpenAI Stream" | "OpenAI" => "OpenAI",
        _ => name,
    }
}

fn normalize_legacy_content_str(content: &str) -> AiChatResult<Content> {
    if let Ok(content) = serde_json::from_str::<Content>(content) {
        return Ok(content);
    }
    if let Ok(content) = serde_json::from_str::<LegacyContent>(content) {
        return Ok(content.into());
    }
    Ok(Content::new(content))
}

fn build_v1_migrated_messages(
    templates: &[LegacyTemplate],
    conversations: &[LegacyConversation],
    messages: Vec<LegacyMessageV1>,
) -> AiChatResult<Vec<SqlMessage>> {
    let templates_by_id = templates
        .iter()
        .map(|template| (template.id, template))
        .collect::<HashMap<_, _>>();
    let conversations_by_id = conversations
        .iter()
        .map(|conversation| (conversation.id, conversation.template_id))
        .collect::<HashMap<_, _>>();
    let mut messages_by_conversation = HashMap::<i32, Vec<LegacyMessageV1>>::new();
    for message in messages {
        messages_by_conversation
            .entry(message.conversation_id)
            .or_default()
            .push(message);
    }

    let mut migrated = Vec::new();
    for (conversation_id, mut conversation_messages) in messages_by_conversation {
        conversation_messages.sort_by_key(|message| (message.created_time, message.id));
        let Some(template_id) = conversations_by_id.get(&conversation_id) else {
            continue;
        };
        let Some(template) = templates_by_id.get(template_id) else {
            continue;
        };
        migrated.extend(build_conversation_messages(
            template,
            conversation_messages,
        )?);
    }
    migrated.sort_by_key(|message| (message.conversation_id, message.created_time, message.id));
    Ok(migrated)
}

fn build_conversation_messages(
    template: &LegacyTemplate,
    messages: Vec<LegacyMessageV1>,
) -> AiChatResult<Vec<SqlMessage>> {
    let provider = normalize_legacy_provider_name(&template.provider).to_string();
    let mut history = Vec::<FetchMessage>::new();
    let mut current_round_payload = None::<serde_json::Value>;
    let mut last_payload = None::<serde_json::Value>;
    let mut migrated = Vec::with_capacity(messages.len());

    for message in messages {
        let role: Role = message.role.parse()?;
        let status: Status = message.status.parse()?;
        let content = normalize_legacy_content_str(&message.content)?;
        let send_text = content.send_content().to_string();
        let send_content = if role == Role::Assistant {
            current_round_payload
                .clone()
                .or_else(|| last_payload.clone())
                .unwrap_or(build_request_payload(template, &history, role, &send_text)?)
        } else {
            let payload = build_request_payload(template, &history, role, &send_text)?;
            current_round_payload = Some(payload.clone());
            last_payload = Some(payload.clone());
            payload
        };

        migrated.push(SqlMessage {
            id: message.id,
            conversation_id: message.conversation_id,
            conversation_path: message.conversation_path.clone(),
            provider: provider.clone(),
            role: message.role.clone(),
            content: serde_json::to_value(content)?,
            send_content,
            status: message.status.clone(),
            created_time: message.created_time,
            updated_time: message.updated_time,
            start_time: message.start_time,
            end_time: message.end_time,
            error: None,
        });

        if status == Status::Normal {
            history.push(FetchMessage::new(role, send_text));
        }
        if role == Role::Assistant {
            current_round_payload = None;
        }
    }
    Ok(migrated)
}

fn build_v2_migrated_messages(
    templates: &[LegacyTemplate],
    conversations: &[LegacyConversation],
    messages: Vec<LegacyMessageV2>,
) -> AiChatResult<Vec<SqlMessage>> {
    let templates_by_id = templates
        .iter()
        .map(|template| (template.id, template))
        .collect::<HashMap<_, _>>();
    let conversations_by_id = conversations
        .iter()
        .map(|conversation| (conversation.id, conversation.template_id))
        .collect::<HashMap<_, _>>();

    messages
        .into_iter()
        .map(|message| {
            let template_id = conversations_by_id
                .get(&message.conversation_id)
                .copied()
                .unwrap_or_default();
            let provider = templates_by_id
                .get(&template_id)
                .map(|template| normalize_legacy_provider_name(&template.provider).to_string())
                .unwrap_or_else(|| "OpenAI".to_string());
            let content = normalize_legacy_content_str(&message.content)?;
            Ok(SqlMessage {
                id: message.id,
                conversation_id: message.conversation_id,
                conversation_path: message.conversation_path,
                provider,
                role: message.role,
                content: serde_json::to_value(content)?,
                send_content: message.send_content,
                status: message.status,
                created_time: message.created_time,
                updated_time: message.updated_time,
                start_time: message.start_time,
                end_time: message.end_time,
                error: message.error,
            })
        })
        .collect()
}

fn build_v3_migrated_messages(messages: Vec<v3::SqlMessageV3>) -> AiChatResult<Vec<SqlMessage>> {
    messages
        .into_iter()
        .map(|message| {
            let content = normalize_legacy_content_str(&message.content)?;
            Ok(SqlMessage {
                id: message.id,
                conversation_id: message.conversation_id,
                conversation_path: message.conversation_path,
                provider: message.provider,
                role: message.role,
                content: serde_json::to_value(content)?,
                send_content: message.send_content,
                status: message.status,
                created_time: message.created_time,
                updated_time: message.updated_time,
                start_time: message.start_time,
                end_time: message.end_time,
                error: message.error,
            })
        })
        .collect()
}

fn build_request_payload(
    template: &LegacyTemplate,
    history_messages: &[FetchMessage],
    role: Role,
    send_text: &str,
) -> AiChatResult<serde_json::Value> {
    let prompts =
        serde_json::from_value::<Vec<ConversationTemplatePrompt>>(template.prompts.clone())?
            .into_iter()
            .map(|prompt| FetchMessage::new(prompt.role, prompt.prompt))
            .collect::<Vec<_>>();

    let history = match template.mode {
        Mode::Contextual => history_messages.to_vec(),
        Mode::Single => Vec::new(),
        Mode::AssistantOnly => history_messages
            .iter()
            .filter(|message| message.role == Role::Assistant)
            .cloned()
            .collect(),
    };

    let mut request_messages = prompts;
    request_messages.extend(history);
    request_messages.push(FetchMessage::new(role, send_text.to_string()));

    let mut request_template = template.template.clone();
    request_template["stream"] = serde_json::Value::Bool(template.provider == "OpenAI Stream");
    provider_by_name(normalize_legacy_provider_name(&template.provider))?
        .request_body(&request_template, request_messages)
}

pub(super) mod v1 {
    use crate::errors::AiChatResult;
    use diesel::prelude::*;
    use time::OffsetDateTime;

    diesel::table! {
        conversation_templates (id) {
            id -> Integer,
            name -> Text,
            icon -> Text,
            description -> Nullable<Text>,
            mode -> Text,
            adapter -> Text,
            template -> Text,
            prompts -> Text,
            created_time -> TimestamptzSqlite,
            updated_time -> TimestamptzSqlite,
        }
    }

    diesel::table! {
        messages (id) {
            id -> Integer,
            conversation_id -> Integer,
            conversation_path -> Text,
            role -> Text,
            content -> Text,
            status -> Text,
            created_time -> TimestamptzSqlite,
            updated_time -> TimestamptzSqlite,
            start_time -> TimestamptzSqlite,
            end_time -> TimestamptzSqlite,
        }
    }

    #[derive(Debug, Queryable)]
    pub(crate) struct SqlConversationTemplateV1 {
        pub id: i32,
        pub name: String,
        pub icon: String,
        pub description: Option<String>,
        pub mode: String,
        pub adapter: String,
        pub template: String,
        pub prompts: String,
        pub created_time: OffsetDateTime,
        pub updated_time: OffsetDateTime,
    }

    impl SqlConversationTemplateV1 {
        pub fn all(conn: &mut SqliteConnection) -> AiChatResult<Vec<Self>> {
            conversation_templates::table
                .load::<Self>(conn)
                .map_err(|e| e.into())
        }
    }

    #[derive(Debug, Queryable)]
    pub(crate) struct SqlMessageV1 {
        pub id: i32,
        pub conversation_id: i32,
        pub conversation_path: String,
        pub role: String,
        pub content: String,
        pub status: String,
        pub created_time: OffsetDateTime,
        pub updated_time: OffsetDateTime,
        pub start_time: OffsetDateTime,
        pub end_time: OffsetDateTime,
    }

    impl SqlMessageV1 {
        pub fn all(conn: &mut SqliteConnection) -> AiChatResult<Vec<Self>> {
            messages::table.load::<Self>(conn).map_err(|e| e.into())
        }
    }
}

pub(super) mod v2 {
    use crate::errors::AiChatResult;
    use diesel::prelude::*;
    use time::OffsetDateTime;

    diesel::table! {
        conversation_templates (id) {
            id -> Integer,
            name -> Text,
            icon -> Text,
            description -> Nullable<Text>,
            mode -> Text,
            adapter -> Text,
            template -> Json,
            prompts -> Json,
            created_time -> TimestamptzSqlite,
            updated_time -> TimestamptzSqlite,
        }
    }

    diesel::table! {
        conversations (id) {
            id -> Integer,
            folder_id -> Nullable<Integer>,
            path -> Text,
            title -> Text,
            icon -> Text,
            created_time -> TimestamptzSqlite,
            updated_time -> TimestamptzSqlite,
            info -> Nullable<Text>,
            template_id -> Integer,
        }
    }

    diesel::table! {
        messages (id) {
            id -> Integer,
            conversation_id -> Integer,
            conversation_path -> Text,
            role -> Text,
            content -> Text,
            send_content -> Json,
            status -> Text,
            created_time -> TimestamptzSqlite,
            updated_time -> TimestamptzSqlite,
            start_time -> TimestamptzSqlite,
            end_time -> TimestamptzSqlite,
            error -> Nullable<Text>,
        }
    }

    #[derive(Debug, Queryable)]
    pub(crate) struct SqlConversationTemplateV2 {
        pub id: i32,
        pub name: String,
        pub icon: String,
        pub description: Option<String>,
        pub mode: String,
        pub adapter: String,
        pub template: serde_json::Value,
        pub prompts: serde_json::Value,
        pub created_time: OffsetDateTime,
        pub updated_time: OffsetDateTime,
    }

    impl SqlConversationTemplateV2 {
        pub fn all(conn: &mut SqliteConnection) -> AiChatResult<Vec<Self>> {
            conversation_templates::table
                .load::<Self>(conn)
                .map_err(|e| e.into())
        }
    }

    #[derive(Debug, Queryable)]
    pub(crate) struct SqlConversationV2 {
        pub id: i32,
        pub folder_id: Option<i32>,
        pub path: String,
        pub title: String,
        pub icon: String,
        pub created_time: OffsetDateTime,
        pub updated_time: OffsetDateTime,
        pub info: Option<String>,
        pub template_id: i32,
    }

    impl SqlConversationV2 {
        pub fn all(conn: &mut SqliteConnection) -> AiChatResult<Vec<Self>> {
            conversations::table
                .load::<Self>(conn)
                .map_err(|e| e.into())
        }
    }

    #[derive(Debug, Queryable)]
    pub(crate) struct SqlMessageV2 {
        pub id: i32,
        pub conversation_id: i32,
        pub conversation_path: String,
        pub role: String,
        pub content: String,
        pub send_content: serde_json::Value,
        pub status: String,
        pub created_time: OffsetDateTime,
        pub updated_time: OffsetDateTime,
        pub start_time: OffsetDateTime,
        pub end_time: OffsetDateTime,
        pub error: Option<String>,
    }

    impl SqlMessageV2 {
        pub fn all(conn: &mut SqliteConnection) -> AiChatResult<Vec<Self>> {
            messages::table.load::<Self>(conn).map_err(|e| e.into())
        }
    }
}

pub(super) mod v3 {
    use crate::errors::AiChatResult;
    use diesel::prelude::*;
    use time::OffsetDateTime;

    diesel::table! {
        messages (id) {
            id -> Integer,
            conversation_id -> Integer,
            conversation_path -> Text,
            provider -> Text,
            role -> Text,
            content -> Text,
            send_content -> Json,
            status -> Text,
            created_time -> TimestamptzSqlite,
            updated_time -> TimestamptzSqlite,
            start_time -> TimestamptzSqlite,
            end_time -> TimestamptzSqlite,
            error -> Nullable<Text>,
        }
    }

    #[derive(Debug, Queryable)]
    pub(crate) struct SqlMessageV3 {
        pub id: i32,
        pub conversation_id: i32,
        pub conversation_path: String,
        pub provider: String,
        pub role: String,
        pub content: String,
        pub send_content: serde_json::Value,
        pub status: String,
        pub created_time: OffsetDateTime,
        pub updated_time: OffsetDateTime,
        pub start_time: OffsetDateTime,
        pub end_time: OffsetDateTime,
        pub error: Option<String>,
    }

    impl SqlMessageV3 {
        pub fn all(conn: &mut SqliteConnection) -> AiChatResult<Vec<Self>> {
            messages::table.load::<Self>(conn).map_err(|e| e.into())
        }
    }
}

impl TryFrom<v1::SqlConversationTemplateV1> for LegacyTemplate {
    type Error = crate::errors::AiChatError;

    fn try_from(value: v1::SqlConversationTemplateV1) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value.id,
            name: value.name,
            icon: value.icon,
            description: value.description,
            mode: value.mode.parse()?,
            provider: value.adapter,
            template: serde_json::from_str(&value.template)?,
            prompts: serde_json::from_str(&value.prompts)?,
            created_time: value.created_time,
            updated_time: value.updated_time,
        })
    }
}

impl From<v2::SqlConversationTemplateV2> for LegacyTemplate {
    fn from(value: v2::SqlConversationTemplateV2) -> Self {
        Self {
            id: value.id,
            name: value.name,
            icon: value.icon,
            description: value.description,
            mode: value.mode.parse().unwrap_or(Mode::Contextual),
            provider: value.adapter,
            template: value.template,
            prompts: value.prompts,
            created_time: value.created_time,
            updated_time: value.updated_time,
        }
    }
}

impl From<v2::SqlConversationV2> for LegacyConversation {
    fn from(value: v2::SqlConversationV2) -> Self {
        Self {
            id: value.id,
            folder_id: value.folder_id,
            path: value.path,
            title: value.title,
            icon: value.icon,
            created_time: value.created_time,
            updated_time: value.updated_time,
            info: value.info,
            template_id: value.template_id,
        }
    }
}

impl From<v1::SqlMessageV1> for LegacyMessageV1 {
    fn from(value: v1::SqlMessageV1) -> Self {
        Self {
            id: value.id,
            conversation_id: value.conversation_id,
            conversation_path: value.conversation_path,
            role: value.role,
            content: value.content,
            status: value.status,
            created_time: value.created_time,
            updated_time: value.updated_time,
            start_time: value.start_time,
            end_time: value.end_time,
        }
    }
}

impl From<v2::SqlMessageV2> for LegacyMessageV2 {
    fn from(value: v2::SqlMessageV2) -> Self {
        Self {
            id: value.id,
            conversation_id: value.conversation_id,
            conversation_path: value.conversation_path,
            role: value.role,
            content: value.content,
            send_content: value.send_content,
            status: value.status,
            created_time: value.created_time,
            updated_time: value.updated_time,
            start_time: value.start_time,
            end_time: value.end_time,
            error: value.error,
        }
    }
}

#[cfg(test)]
mod tests;

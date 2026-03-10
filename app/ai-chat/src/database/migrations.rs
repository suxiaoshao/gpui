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
        let send_text = message_send_text(&message.content)?;
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
            content: message.content.clone(),
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
            Ok(SqlMessage {
                id: message.id,
                conversation_id: message.conversation_id,
                conversation_path: message.conversation_path,
                provider,
                role: message.role,
                content: message.content,
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

fn message_send_text(content: &str) -> AiChatResult<String> {
    Ok(match serde_json::from_str::<Content>(content) {
        Ok(content) => content.send_content().to_string(),
        Err(_) => content.to_string(),
    })
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
mod tests {
    use super::{v1_to_v3, v2_to_v3};
    use crate::database::model::{SqlConversation, SqlConversationTemplate, SqlMessage};
    use diesel::{
        Connection, RunQueryDsl, SqliteConnection, connection::SimpleConnection, sql_query,
    };
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    const V1_CREATE_TABLE_SQL: &str = r#"
create table folders
(
    id           INTEGER primary key autoincrement not null,
    name         TEXT                              not null,
    path         TEXT                              not null,
    parent_id    INTEGER,
    created_time DateTime                          not null,
    updated_time DateTime                          not null,
    unique (name, parent_id),
    unique (path),
    foreign key (parent_id) references folders (id)
);

CREATE TABLE conversation_templates
(
    id           Integer PRIMARY KEY AUTOINCREMENT not null,
    name         TEXT                              NOT NULL,
    icon         TEXT                              not null,
    description  TEXT,
    mode         TEXT                              not null check ( mode in ('contextual', 'single', 'assistant-only') ) default 'contextual',
    adapter      TEXT                              NOT NULL,
    template     TEXT                              NOT NULL,
    prompts      TEXT                              NOT NULL,
    created_time DateTime                          not null,
    updated_time DateTime                          not null
);

create table conversations
(
    id           INTEGER primary key autoincrement not null,
    folder_id    INTEGER,
    path         TEXT                              not null,
    title        TEXT                              not null,
    icon         TEXT                              not null,
    created_time DateTime                          not null,
    updated_time DateTime                          not null,
    info         TEXT,
    template_id  INTEGER                           not null,
    foreign key (folder_id) references folders (id),
    FOREIGN KEY (template_id) REFERENCES conversation_templates (id),
    unique (path)
);

create table messages
(
    id                INTEGER primary key autoincrement not null,
    conversation_id   INTEGER                           not null,
    conversation_path TEXT                              not null,
    role              TEXT                              not null check ( role in ('developer', 'user', 'assistant') ),
    content           TEXT                              not null,
    status            TEXT                              not null check ( status in ('normal', 'hidden', 'loading', 'paused', 'error') ),
    created_time      DateTime                          not null,
    updated_time      DateTime                          not null,
    start_time        DateTime                          not null,
    end_time          DateTime                          not null,
    foreign key (conversation_id) references conversations (id)
);
"#;

    const V2_CREATE_TABLE_SQL: &str =
        include_str!("../../migrations/2025-12-23-141452-0000_create_tables/up.sql");

    static TEMP_DB_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_db_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let counter = TEMP_DB_COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        std::env::temp_dir().join(format!(
            "gpui-ai-chat-{name}-{pid}-{unique}-{counter}.sqlite3"
        ))
    }

    fn template_json() -> &'static str {
        r#"{"model":"gpt-4o","stream":false,"temperature":1.0,"top_p":1.0,"n":1,"max_completion_tokens":null,"presence_penalty":0.0,"frequency_penalty":0.0}"#
    }

    fn insert_seed_data(
        conn: &mut SqliteConnection,
        with_send_content: bool,
    ) -> anyhow::Result<()> {
        sql_query(format!(
            "insert into conversation_templates (id, name, icon, description, mode, adapter, template, prompts, created_time, updated_time)
             values (1, 'base', '🤖', null, 'contextual', 'OpenAI', '{}', '[]', '2026-01-01 00:00:00+00:00', '2026-01-01 00:00:00+00:00')",
            template_json()
        ))
        .execute(conn)?;
        sql_query(
            "insert into conversations (id, folder_id, path, title, icon, created_time, updated_time, info, template_id)
             values (1, null, '/默认', '默认', '🤖', '2026-01-01 00:00:00+00:00', '2026-01-01 00:00:00+00:00', null, 1)",
        )
        .execute(conn)?;
        if with_send_content {
            sql_query(
                "insert into messages (id, conversation_id, conversation_path, role, content, send_content, status, created_time, updated_time, start_time, end_time, error)
                 values
                 (1, 1, '/默认', 'user', '{\"tag\":\"text\",\"value\":\"hello\"}', '{\"model\":\"gpt-4o\",\"stream\":false}', 'normal', '2026-01-01 00:00:00+00:00', '2026-01-01 00:00:00+00:00', '2026-01-01 00:00:00+00:00', '2026-01-01 00:00:00+00:00', null),
                 (2, 1, '/默认', 'assistant', '{\"tag\":\"text\",\"value\":\"world\"}', '{\"model\":\"gpt-4o\",\"stream\":false}', 'normal', '2026-01-01 00:00:01+00:00', '2026-01-01 00:00:01+00:00', '2026-01-01 00:00:01+00:00', '2026-01-01 00:00:01+00:00', null)",
            )
            .execute(conn)?;
        } else {
            sql_query(
                "insert into messages (id, conversation_id, conversation_path, role, content, status, created_time, updated_time, start_time, end_time)
                 values
                 (1, 1, '/默认', 'user', '{\"tag\":\"text\",\"value\":\"hello\"}', 'normal', '2026-01-01 00:00:00+00:00', '2026-01-01 00:00:00+00:00', '2026-01-01 00:00:00+00:00', '2026-01-01 00:00:00+00:00'),
                 (2, 1, '/默认', 'assistant', '{\"tag\":\"text\",\"value\":\"world\"}', 'normal', '2026-01-01 00:00:01+00:00', '2026-01-01 00:00:01+00:00', '2026-01-01 00:00:01+00:00', '2026-01-01 00:00:01+00:00')",
            )
            .execute(conn)?;
        }
        Ok(())
    }

    #[test]
    fn v1_migration_backfills_send_content_from_request_logic() -> anyhow::Result<()> {
        let v1_path = temp_db_path("v1");
        let v3_path = temp_db_path("v3");
        let mut v1_conn = SqliteConnection::establish(v1_path.to_str().expect("v1 path"))?;
        let mut v3_conn = SqliteConnection::establish(v3_path.to_str().expect("v3 path"))?;

        v1_conn.batch_execute(V1_CREATE_TABLE_SQL)?;
        insert_seed_data(&mut v1_conn, false)?;

        v1_to_v3(&mut v1_conn, &mut v3_conn)?;

        let templates = SqlConversationTemplate::all(&mut v3_conn)?;
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].prompts, serde_json::json!([]));

        let conversations = SqlConversation::get_all(&mut v3_conn)?;
        assert_eq!(conversations.len(), 1);

        let messages = SqlMessage::all(&mut v3_conn)?;
        assert_eq!(messages.len(), 2);
        assert!(messages.iter().all(|message| message.error.is_none()));
        assert!(messages.iter().all(|message| message.provider == "OpenAI"));
        let first_send_content = &messages[0].send_content;
        let second_send_content = &messages[1].send_content;
        assert_eq!(first_send_content["model"], "gpt-4o");
        assert_eq!(first_send_content["input"][0]["content"], "hello");
        assert_eq!(second_send_content, first_send_content);

        drop(v1_conn);
        drop(v3_conn);
        let _ = fs::remove_file(v1_path);
        let _ = fs::remove_file(v3_path);
        Ok(())
    }

    #[test]
    fn v2_migration_keeps_send_content_and_adds_provider() -> anyhow::Result<()> {
        let v2_path = temp_db_path("v2");
        let v3_path = temp_db_path("v3");
        let mut v2_conn = SqliteConnection::establish(v2_path.to_str().expect("v2 path"))?;
        let mut v3_conn = SqliteConnection::establish(v3_path.to_str().expect("v3 path"))?;

        v2_conn.batch_execute(V2_CREATE_TABLE_SQL)?;
        insert_seed_data(&mut v2_conn, true)?;

        v2_to_v3(&mut v2_conn, &mut v3_conn)?;

        let messages = SqlMessage::all(&mut v3_conn)?;
        assert_eq!(messages.len(), 2);
        assert!(messages.iter().all(|message| message.provider == "OpenAI"));
        assert_eq!(messages[0].send_content["model"], "gpt-4o");

        drop(v2_conn);
        drop(v3_conn);
        let _ = fs::remove_file(v2_path);
        let _ = fs::remove_file(v3_path);
        Ok(())
    }
}

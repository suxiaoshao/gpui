use super::{v1_to_v5, v2_to_v5, v3_to_v5, v4_to_v5};
use crate::database::model::{
    SqlConversation, SqlConversationTemplate, SqlGlobalShortcutBinding, SqlMessage,
};
use diesel::{Connection, RunQueryDsl, SqliteConnection, connection::SimpleConnection, sql_query};
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
    include_str!("../../../migrations/2025-12-23-141452-0000_create_tables/up.sql");
const V3_CREATE_TABLE_SQL: &str =
    include_str!("../../../migrations/2026-03-08-000000_create_tables_v3/up.sql");
const V4_CREATE_TABLE_SQL: &str =
    include_str!("../../../migrations/2026-03-15-000000_create_tables_v4/up.sql");

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

fn insert_seed_data(conn: &mut SqliteConnection, with_send_content: bool) -> anyhow::Result<()> {
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
    let v5_path = temp_db_path("v5");
    let mut v1_conn = SqliteConnection::establish(v1_path.to_str().expect("v1 path"))?;
    let mut v5_conn = SqliteConnection::establish(v5_path.to_str().expect("v5 path"))?;

    v1_conn.batch_execute(V1_CREATE_TABLE_SQL)?;
    insert_seed_data(&mut v1_conn, false)?;

    v1_to_v5(&mut v1_conn, &mut v5_conn)?;

    let templates = SqlConversationTemplate::all(&mut v5_conn)?;
    assert_eq!(templates.len(), 1);
    assert_eq!(templates[0].prompts, serde_json::json!([]));

    let conversations = SqlConversation::get_all(&mut v5_conn)?;
    assert_eq!(conversations.len(), 1);

    let messages = SqlMessage::all(&mut v5_conn)?;
    assert_eq!(messages.len(), 2);
    assert!(messages.iter().all(|message| message.error.is_none()));
    assert!(messages.iter().all(|message| message.provider == "OpenAI"));
    assert!(SqlGlobalShortcutBinding::all(&mut v5_conn)?.is_empty());
    let first_send_content = &messages[0].send_content;
    let second_send_content = &messages[1].send_content;
    assert_eq!(first_send_content["model"], "gpt-4o");
    assert_eq!(first_send_content["input"][0]["content"], "hello");
    assert_eq!(second_send_content, first_send_content);

    drop(v1_conn);
    drop(v5_conn);
    let _ = fs::remove_file(v1_path);
    let _ = fs::remove_file(v5_path);
    Ok(())
}

#[test]
fn v2_migration_keeps_send_content_and_adds_provider() -> anyhow::Result<()> {
    let v2_path = temp_db_path("v2");
    let v5_path = temp_db_path("v5");
    let mut v2_conn = SqliteConnection::establish(v2_path.to_str().expect("v2 path"))?;
    let mut v5_conn = SqliteConnection::establish(v5_path.to_str().expect("v5 path"))?;

    v2_conn.batch_execute(V2_CREATE_TABLE_SQL)?;
    insert_seed_data(&mut v2_conn, true)?;

    v2_to_v5(&mut v2_conn, &mut v5_conn)?;

    let messages = SqlMessage::all(&mut v5_conn)?;
    assert_eq!(messages.len(), 2);
    assert!(messages.iter().all(|message| message.provider == "OpenAI"));
    assert_eq!(messages[0].send_content["model"], "gpt-4o");
    assert!(SqlGlobalShortcutBinding::all(&mut v5_conn)?.is_empty());

    drop(v2_conn);
    drop(v5_conn);
    let _ = fs::remove_file(v2_path);
    let _ = fs::remove_file(v5_path);
    Ok(())
}

#[test]
fn v3_migration_converts_legacy_content_enum_to_json_struct_in_v5() -> anyhow::Result<()> {
    let v3_path = temp_db_path("v3-source");
    let v5_path = temp_db_path("v5-target");
    let mut v3_conn = SqliteConnection::establish(v3_path.to_str().expect("v3 path"))?;
    let mut v5_conn = SqliteConnection::establish(v5_path.to_str().expect("v5 path"))?;

    v3_conn.batch_execute(V3_CREATE_TABLE_SQL)?;
    sql_query(
            "insert into conversation_templates (id, name, icon, description, prompts, created_time, updated_time)
             values (1, 'base', '🤖', null, '[]', '2026-01-01 00:00:00+00:00', '2026-01-01 00:00:00+00:00')",
        )
        .execute(&mut v3_conn)?;
    sql_query(
            "insert into conversations (id, folder_id, path, title, icon, created_time, updated_time, info)
             values (1, null, '/默认', '默认', '🤖', '2026-01-01 00:00:00+00:00', '2026-01-01 00:00:00+00:00', null)",
        )
        .execute(&mut v3_conn)?;
    sql_query(
            "insert into messages (id, conversation_id, conversation_path, provider, role, content, send_content, status, created_time, updated_time, start_time, end_time, error)
             values (1, 1, '/默认', 'OpenAI', 'assistant', '{\"tag\":\"webSearch\",\"value\":{\"text\":\"hello\",\"citations\":[{\"title\":\"Example\",\"url\":\"https://example.com\"}]}}', '{\"model\":\"gpt-4o\"}', 'normal', '2026-01-01 00:00:00+00:00', '2026-01-01 00:00:00+00:00', '2026-01-01 00:00:00+00:00', '2026-01-01 00:00:00+00:00', null)",
        )
        .execute(&mut v3_conn)?;

    v3_to_v5(&mut v3_conn, &mut v5_conn)?;

    let messages = SqlMessage::all(&mut v5_conn)?;
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content["text"], "hello");
    assert!(messages[0].content.get("reasoningSummary").is_none());
    assert_eq!(
        messages[0].content["citations"][0]["url"],
        "https://example.com"
    );
    assert!(SqlGlobalShortcutBinding::all(&mut v5_conn)?.is_empty());

    drop(v3_conn);
    drop(v5_conn);
    let _ = fs::remove_file(v3_path);
    let _ = fs::remove_file(v5_path);
    Ok(())
}

#[test]
fn v4_migration_creates_empty_global_shortcut_table() -> anyhow::Result<()> {
    let v4_path = temp_db_path("v4-source");
    let v5_path = temp_db_path("v5-target");
    let mut v4_conn = SqliteConnection::establish(v4_path.to_str().expect("v4 path"))?;
    let mut v5_conn = SqliteConnection::establish(v5_path.to_str().expect("v5 path"))?;

    v4_conn.batch_execute(V4_CREATE_TABLE_SQL)?;
    sql_query(
            "insert into folders (id, name, path, parent_id, created_time, updated_time)
             values (1, '默认', '/默认', null, '2026-01-01 00:00:00+00:00', '2026-01-01 00:00:00+00:00')",
        )
        .execute(&mut v4_conn)?;
    sql_query(
            "insert into conversation_templates (id, name, icon, description, prompts, created_time, updated_time)
             values (1, 'base', '🤖', null, '[]', '2026-01-01 00:00:00+00:00', '2026-01-01 00:00:00+00:00')",
        )
        .execute(&mut v4_conn)?;
    sql_query(
            "insert into conversations (id, folder_id, path, title, icon, created_time, updated_time, info)
             values (1, 1, '/默认/会话', '默认', '🤖', '2026-01-01 00:00:00+00:00', '2026-01-01 00:00:00+00:00', null)",
        )
        .execute(&mut v4_conn)?;
    sql_query(
            "insert into messages (id, conversation_id, conversation_path, provider, role, content, send_content, status, created_time, updated_time, start_time, end_time, error)
             values (1, 1, '/默认/会话', 'OpenAI', 'user', '{\"text\":\"hello\"}', '{\"model\":\"gpt-4o\"}', 'normal', '2026-01-01 00:00:00+00:00', '2026-01-01 00:00:00+00:00', '2026-01-01 00:00:00+00:00', '2026-01-01 00:00:00+00:00', null)",
        )
        .execute(&mut v4_conn)?;

    v4_to_v5(&mut v4_conn, &mut v5_conn)?;

    assert_eq!(SqlConversationTemplate::all(&mut v5_conn)?.len(), 1);
    assert_eq!(SqlConversation::get_all(&mut v5_conn)?.len(), 1);
    assert_eq!(SqlMessage::all(&mut v5_conn)?.len(), 1);
    assert!(SqlGlobalShortcutBinding::all(&mut v5_conn)?.is_empty());

    drop(v4_conn);
    drop(v5_conn);
    let _ = fs::remove_file(v4_path);
    let _ = fs::remove_file(v5_path);
    Ok(())
}

use crate::{
    APP_NAME,
    database::model::{SqlConversationTemplate, SqlNewConversation, SqlNewConversationTemplate},
    errors::{AiChatError, AiChatResult},
};
use diesel::{
    Connection, SqliteConnection,
    connection::SimpleConnection,
    r2d2::{ConnectionManager, Pool},
};
use gpui::App;
use std::{ops::Deref, path::Path};
use time::OffsetDateTime;
use tracing::{Level, event};

mod migrations;
mod model;
mod schema;
mod service;
mod types;

pub use service::{
    Content, Conversation, ConversationTemplate, ConversationTemplatePrompt, Folder, Message,
    NewConversation, NewConversationTemplate, NewFolder, NewMessage, UrlCitation,
};
pub use types::{Mode, Role, Status};

const DATABASE_FILE_V1: &str = "history.sqlite3";
const DATABASE_FILE_V2: &str = "history_v2.sqlite3";
const DATABASE_FILE_V3: &str = "history_v3.sqlite3";
const CREATE_TABLE_SQL: &str =
    include_str!("../migrations/2026-03-08-000000_create_tables_v3/up.sql");

pub(crate) type DbConn = Pool<ConnectionManager<SqliteConnection>>;

pub(crate) struct Db(DbConn);

impl gpui::Global for Db {}

impl Deref for Db {
    type Target = DbConn;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub(crate) fn init_store(cx: &mut App) {
    let conn = match establish_connection() {
        Ok(conn) => conn,
        Err(err) => {
            event!(Level::ERROR, "init_store failed: {}", err);
            return;
        }
    };
    cx.set_global(Db(conn));
}

fn establish_connection() -> AiChatResult<DbConn> {
    let data_dir = get_data_dir()?;
    create_data_dir(&data_dir)?;
    StoreVersion::new(&data_dir)?.migration()
}

enum StoreVersion {
    None(DbConn),
    V1 {
        conn: DbConn,
        v1_db: SqliteConnection,
    },
    V2 {
        conn: DbConn,
        v2_path: std::path::PathBuf,
    },
    V3(DbConn),
}

impl StoreVersion {
    fn new(data_dir: &Path) -> AiChatResult<Self> {
        let v1_path = data_dir.join(DATABASE_FILE_V1);
        let v2_path = data_dir.join(DATABASE_FILE_V2);
        let v3_path = data_dir.join(DATABASE_FILE_V3);
        match (v1_path.exists(), v2_path.exists(), v3_path.exists()) {
            (_, _, true) => Ok(Self::V3(get_dbconn(&v3_path)?)),
            (_, true, false) => Ok(Self::V2 {
                conn: get_dbconn(&v3_path)?,
                v2_path,
            }),
            (true, false, false) => Ok(Self::V1 {
                conn: get_dbconn(&v3_path)?,
                v1_db: SqliteConnection::establish(v1_path.to_str().ok_or(AiChatError::DbPath)?)?,
            }),
            _ => Ok(Self::None(get_dbconn(&v3_path)?)),
        }
    }

    fn migration(self) -> AiChatResult<DbConn> {
        match self {
            Self::None(conn) => {
                event!(Level::INFO, "initialize database v3");
                let mut db = conn.get()?;
                init_tables(&mut db)?;
                Ok(conn)
            }
            Self::V1 { conn, mut v1_db } => {
                event!(Level::INFO, "migrate database from v1 to v3");
                let v3_db = &mut conn.get()?;
                if let Err(err) = migrations::v1_to_v3(&mut v1_db, v3_db) {
                    event!(Level::ERROR, "database migration v1 -> v3 failed: {}", err);
                    init_tables(v3_db)?;
                }
                Ok(conn)
            }
            Self::V2 { conn, v2_path } => {
                event!(Level::INFO, "migrate database from v2 to v3");
                let mut v2_db =
                    SqliteConnection::establish(v2_path.to_str().ok_or(AiChatError::DbPath)?)?;
                let v3_db = &mut conn.get()?;
                if let Err(err) = migrations::v2_to_v3(&mut v2_db, v3_db) {
                    event!(Level::ERROR, "database migration v2 -> v3 failed: {}", err);
                    init_tables(v3_db)?;
                }
                Ok(conn)
            }
            Self::V3(conn) => Ok(conn),
        }
    }
}

fn get_data_dir() -> AiChatResult<std::path::PathBuf> {
    Ok(dirs_next::config_dir()
        .ok_or(AiChatError::DbPath)?
        .join(APP_NAME))
}

fn create_data_dir(data_dir: &Path) -> AiChatResult<()> {
    if !data_dir.exists() {
        std::fs::create_dir_all(data_dir)?;
    }
    Ok(())
}

fn get_dbconn(db_path: &Path) -> AiChatResult<DbConn> {
    let url = db_path.to_str().ok_or(AiChatError::DbPath)?;
    let manager = ConnectionManager::<SqliteConnection>::new(url);
    let pool = Pool::builder().test_on_check_out(true).build(manager)?;
    Ok(pool)
}

fn init_tables(conn: &mut SqliteConnection) -> AiChatResult<()> {
    conn.immediate_transaction(|conn| {
        conn.batch_execute(CREATE_TABLE_SQL)?;
        let default_conversation_template = SqlNewConversationTemplate::default()?;
        let SqlConversationTemplate { .. } = default_conversation_template.insert(conn)?;
        let now = OffsetDateTime::now_utc();
        let default_conversation = SqlNewConversation {
            title: "默认",
            path: "/默认".to_string(),
            folder_id: None,
            icon: "🤖",
            info: None,
            created_time: now,
            updated_time: now,
        };
        default_conversation.insert(conn)?;
        Ok(())
    })
}

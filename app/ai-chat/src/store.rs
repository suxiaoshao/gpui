use crate::{
    APP_NAME,
    errors::{AiChatError, AiChatResult},
};
use diesel::{
    SqliteConnection,
    connection::SimpleConnection,
    r2d2::{ConnectionManager, Pool},
};
use gpui::App;
use std::{ops::Deref, path::PathBuf};
use tracing::{Level, event};

mod model;
mod schema;
mod service;
mod types;

pub use service::{
    Content, Conversation, ConversationTemplate, Folder, Message, NewConversation,
    NewConversationTemplate, NewFolder, NewMessage, deserialize_offset_date_time,
    serialize_offset_date_time,
};
pub use types::{Mode, Role, Status};

static DATABASE_FILE: &str = "history.sqlite";

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
    let url_path = get_data_url()?;
    let not_exists = check_data_file(&url_path)?;
    let url = url_path.to_str().ok_or(AiChatError::DbPath)?;
    let manager = ConnectionManager::<SqliteConnection>::new(url);
    let pool = Pool::builder().test_on_check_out(true).build(manager)?;
    if not_exists {
        create_tables(&pool)?;
    }
    Ok(pool)
}

fn get_data_url() -> AiChatResult<PathBuf> {
    let data_path = dirs_next::config_dir()
        .ok_or(AiChatError::DbPath)?
        .join(APP_NAME)
        .join(DATABASE_FILE);
    Ok(data_path)
}

fn check_data_file(url: &PathBuf) -> AiChatResult<bool> {
    use std::fs::File;
    if !url.exists() {
        std::fs::create_dir_all(url.parent().ok_or(AiChatError::DbPath)?)?;
        File::create(url)?;
        return Ok(true);
    }
    Ok(false)
}

fn create_tables(conn: &DbConn) -> AiChatResult<()> {
    let conn = &mut conn.get()?;
    conn.batch_execute(include_str!(
        "../migrations/2025-12-23-141452-0000_create_tables/up.sql"
    ))?;
    Ok(())
}

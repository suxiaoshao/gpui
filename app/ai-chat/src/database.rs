use diesel::{
    SqliteConnection,
    r2d2::{ConnectionManager, Pool},
};
use std::ops::Deref;

mod bootstrap;
mod migrations;
mod model;
mod schema;
mod service;
mod types;

pub(crate) use service::field_matches_query;
pub use service::{
    Content, Conversation, ConversationTemplate, ConversationTemplatePrompt, Folder, Message,
    NewConversation, NewConversationTemplate, NewFolder, NewMessage, UrlCitation,
};
#[allow(unused_imports)]
pub use service::{GlobalShortcutBinding, NewGlobalShortcutBinding, UpdateGlobalShortcutBinding};
pub use types::{Mode, Role, ShortcutInputSource, Status};
const CREATE_TABLE_SQL: &str =
    include_str!("../migrations/2026-03-20-000000_create_tables_v5/up.sql");

pub(crate) type DbConn = Pool<ConnectionManager<SqliteConnection>>;

pub(crate) struct Db(DbConn);

impl gpui::Global for Db {}

impl Deref for Db {
    type Target = DbConn;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
pub(crate) use bootstrap::init_store;

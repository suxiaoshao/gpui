use crate::{database::schema::global_shortcut_bindings, errors::AiChatResult};
use diesel::prelude::*;
use time::OffsetDateTime;

#[derive(Insertable)]
#[diesel(table_name = global_shortcut_bindings)]
pub struct SqlNewGlobalShortcutBinding<'a> {
    pub(in super::super) name: &'a str,
    pub(in super::super) hotkey: &'a str,
    pub(in super::super) enabled: bool,
    pub(in super::super) template_id: Option<i32>,
    pub(in super::super) provider_name: &'a str,
    pub(in super::super) model_id: &'a str,
    pub(in super::super) mode: &'a str,
    pub(in super::super) request_template: &'a serde_json::Value,
    pub(in super::super) input_source: &'a str,
    pub(in super::super) created_time: OffsetDateTime,
    pub(in super::super) updated_time: OffsetDateTime,
}

impl SqlNewGlobalShortcutBinding<'_> {
    pub fn insert(&self, conn: &mut SqliteConnection) -> AiChatResult<SqlGlobalShortcutBinding> {
        diesel::insert_into(global_shortcut_bindings::table)
            .values(self)
            .get_result(conn)
            .map_err(Into::into)
    }
}

#[derive(Debug, Queryable, Insertable)]
#[diesel(table_name = global_shortcut_bindings)]
pub struct SqlGlobalShortcutBinding {
    pub(in super::super) id: i32,
    pub(in super::super) name: String,
    pub(in super::super) hotkey: String,
    pub(in super::super) enabled: bool,
    pub(in super::super) template_id: Option<i32>,
    pub(in super::super) provider_name: String,
    pub(in super::super) model_id: String,
    pub(in super::super) mode: String,
    pub(in super::super) request_template: serde_json::Value,
    pub(in super::super) input_source: String,
    pub(in super::super) created_time: OffsetDateTime,
    pub(in super::super) updated_time: OffsetDateTime,
}

impl SqlGlobalShortcutBinding {
    pub fn find(id: i32, conn: &mut SqliteConnection) -> AiChatResult<Self> {
        global_shortcut_bindings::table
            .find(id)
            .first(conn)
            .map_err(Into::into)
    }

    pub fn all(conn: &mut SqliteConnection) -> AiChatResult<Vec<Self>> {
        global_shortcut_bindings::table
            .order(global_shortcut_bindings::updated_time.desc())
            .load(conn)
            .map_err(Into::into)
    }

    pub fn migration_save(
        data: Vec<SqlGlobalShortcutBinding>,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<()> {
        diesel::insert_into(global_shortcut_bindings::table)
            .values(data)
            .execute(conn)?;
        Ok(())
    }

    pub fn delete_by_id(id: i32, conn: &mut SqliteConnection) -> AiChatResult<()> {
        diesel::delete(global_shortcut_bindings::table.find(id)).execute(conn)?;
        Ok(())
    }
}

#[derive(AsChangeset, Identifiable)]
#[diesel(table_name = global_shortcut_bindings)]
pub struct SqlUpdateGlobalShortcutBinding<'a> {
    pub(in super::super) id: i32,
    pub(in super::super) name: &'a str,
    pub(in super::super) hotkey: &'a str,
    pub(in super::super) enabled: bool,
    pub(in super::super) template_id: Option<i32>,
    pub(in super::super) provider_name: &'a str,
    pub(in super::super) model_id: &'a str,
    pub(in super::super) mode: &'a str,
    pub(in super::super) request_template: &'a serde_json::Value,
    pub(in super::super) input_source: &'a str,
    pub(in super::super) updated_time: OffsetDateTime,
}

impl SqlUpdateGlobalShortcutBinding<'_> {
    pub fn update(&self, conn: &mut SqliteConnection) -> AiChatResult<()> {
        diesel::update(global_shortcut_bindings::table.find(self.id))
            .set(self)
            .execute(conn)?;
        Ok(())
    }
}

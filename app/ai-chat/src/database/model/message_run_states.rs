use crate::{database::schema::message_run_states, errors::AiChatResult};
use diesel::prelude::*;
use time::OffsetDateTime;

#[derive(Insertable)]
#[diesel(table_name = message_run_states)]
pub struct SqlNewMessageRunState<'a> {
    pub(in super::super) message_id: i32,
    pub(in super::super) provider: &'a str,
    pub(in super::super) run_id: Option<&'a str>,
    pub(in super::super) output_item_ids: &'a serde_json::Value,
    pub(in super::super) continuation_metadata: &'a serde_json::Value,
    pub(in super::super) request_body: &'a serde_json::Value,
    pub(in super::super) usage: Option<&'a serde_json::Value>,
    pub(in super::super) model: Option<&'a str>,
    pub(in super::super) settings: Option<&'a serde_json::Value>,
    pub(in super::super) created_time: OffsetDateTime,
    pub(in super::super) updated_time: OffsetDateTime,
}

impl SqlNewMessageRunState<'_> {
    pub fn insert(&self, conn: &mut SqliteConnection) -> AiChatResult<()> {
        diesel::insert_into(message_run_states::table)
            .values(self)
            .execute(conn)?;
        Ok(())
    }
}

#[allow(dead_code)]
#[derive(Debug, Queryable)]
pub struct SqlMessageRunState {
    pub message_id: i32,
    pub provider: String,
    pub run_id: Option<String>,
    pub output_item_ids: serde_json::Value,
    pub continuation_metadata: serde_json::Value,
    pub request_body: serde_json::Value,
    pub usage: Option<serde_json::Value>,
    pub model: Option<String>,
    pub settings: Option<serde_json::Value>,
    pub created_time: OffsetDateTime,
    pub updated_time: OffsetDateTime,
}

impl SqlMessageRunState {
    pub fn find_by_message_id(
        message_id: i32,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<Option<Self>> {
        message_run_states::table
            .filter(message_run_states::message_id.eq(message_id))
            .first::<Self>(conn)
            .optional()
            .map_err(Into::into)
    }

    pub fn delete_by_message_id(message_id: i32, conn: &mut SqliteConnection) -> AiChatResult<()> {
        diesel::delete(
            message_run_states::table.filter(message_run_states::message_id.eq(message_id)),
        )
        .execute(conn)?;
        Ok(())
    }

    #[cfg(test)]
    pub fn all(conn: &mut SqliteConnection) -> AiChatResult<Vec<Self>> {
        message_run_states::table
            .load::<Self>(conn)
            .map_err(Into::into)
    }
}

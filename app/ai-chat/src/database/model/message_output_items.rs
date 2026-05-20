use crate::{database::schema::message_output_items, errors::AiChatResult};
use diesel::prelude::*;
use time::OffsetDateTime;

#[derive(Insertable)]
#[diesel(table_name = message_output_items)]
pub struct SqlNewMessageOutputItem<'a> {
    pub(in super::super) message_id: i32,
    pub(in super::super) sequence: i32,
    pub(in super::super) item_kind: &'a str,
    pub(in super::super) provider_item_id: Option<&'a str>,
    pub(in super::super) status: &'a str,
    pub(in super::super) payload: &'a serde_json::Value,
    pub(in super::super) created_time: OffsetDateTime,
    pub(in super::super) updated_time: OffsetDateTime,
}

impl SqlNewMessageOutputItem<'_> {
    pub fn insert_many(data: &[Self], conn: &mut SqliteConnection) -> AiChatResult<()> {
        if data.is_empty() {
            return Ok(());
        }
        diesel::insert_into(message_output_items::table)
            .values(data)
            .execute(conn)?;
        Ok(())
    }
}

#[allow(dead_code)]
#[derive(Debug, Queryable)]
pub struct SqlMessageOutputItem {
    pub id: i32,
    pub message_id: i32,
    pub sequence: i32,
    pub item_kind: String,
    pub provider_item_id: Option<String>,
    pub status: String,
    pub payload: serde_json::Value,
    pub created_time: OffsetDateTime,
    pub updated_time: OffsetDateTime,
}

impl SqlMessageOutputItem {
    #[cfg(test)]
    pub fn query_by_message_id(
        message_id: i32,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<Vec<Self>> {
        message_output_items::table
            .filter(message_output_items::message_id.eq(message_id))
            .order(message_output_items::sequence.asc())
            .load::<Self>(conn)
            .map_err(Into::into)
    }

    pub fn delete_by_message_id(message_id: i32, conn: &mut SqliteConnection) -> AiChatResult<()> {
        diesel::delete(
            message_output_items::table.filter(message_output_items::message_id.eq(message_id)),
        )
        .execute(conn)?;
        Ok(())
    }

    #[cfg(test)]
    pub fn all(conn: &mut SqliteConnection) -> AiChatResult<Vec<Self>> {
        message_output_items::table
            .order((
                message_output_items::message_id.asc(),
                message_output_items::sequence.asc(),
            ))
            .load::<Self>(conn)
            .map_err(Into::into)
    }
}

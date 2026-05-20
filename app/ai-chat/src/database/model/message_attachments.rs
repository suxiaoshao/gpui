use crate::{database::schema::message_attachments, errors::AiChatResult};
use diesel::prelude::*;
use time::OffsetDateTime;

#[derive(Insertable)]
#[diesel(table_name = message_attachments)]
pub struct SqlNewMessageAttachment<'a> {
    pub(in super::super) message_id: i32,
    pub(in super::super) attachment_id: &'a str,
    pub(in super::super) kind: &'a str,
    pub(in super::super) mime_type: Option<&'a str>,
    pub(in super::super) name: Option<&'a str>,
    pub(in super::super) metadata: &'a serde_json::Value,
    pub(in super::super) external_uri: Option<&'a str>,
    pub(in super::super) path: Option<&'a str>,
    pub(in super::super) sha256: Option<&'a str>,
    pub(in super::super) created_time: OffsetDateTime,
    pub(in super::super) updated_time: OffsetDateTime,
}

impl SqlNewMessageAttachment<'_> {
    pub fn insert_many(data: &[Self], conn: &mut SqliteConnection) -> AiChatResult<()> {
        if data.is_empty() {
            return Ok(());
        }
        diesel::insert_into(message_attachments::table)
            .values(data)
            .execute(conn)?;
        Ok(())
    }
}

#[allow(dead_code)]
#[derive(Debug, Queryable)]
pub struct SqlMessageAttachment {
    pub id: i32,
    pub message_id: i32,
    pub attachment_id: String,
    pub kind: String,
    pub mime_type: Option<String>,
    pub name: Option<String>,
    pub metadata: serde_json::Value,
    pub external_uri: Option<String>,
    pub path: Option<String>,
    pub sha256: Option<String>,
    pub created_time: OffsetDateTime,
    pub updated_time: OffsetDateTime,
}

impl SqlMessageAttachment {
    #[cfg(test)]
    pub fn query_by_message_id(
        message_id: i32,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<Vec<Self>> {
        message_attachments::table
            .filter(message_attachments::message_id.eq(message_id))
            .order(message_attachments::id.asc())
            .load::<Self>(conn)
            .map_err(Into::into)
    }

    pub fn delete_by_message_id(message_id: i32, conn: &mut SqliteConnection) -> AiChatResult<()> {
        diesel::delete(
            message_attachments::table.filter(message_attachments::message_id.eq(message_id)),
        )
        .execute(conn)?;
        Ok(())
    }

    #[cfg(test)]
    pub fn all(conn: &mut SqliteConnection) -> AiChatResult<Vec<Self>> {
        message_attachments::table
            .order((
                message_attachments::message_id.asc(),
                message_attachments::id.asc(),
            ))
            .load::<Self>(conn)
            .map_err(Into::into)
    }
}

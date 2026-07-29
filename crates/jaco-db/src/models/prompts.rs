use super::*;

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = prompts)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub(crate) struct SqlPromptRow {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) content: String,
    pub(crate) enabled: bool,
    pub(crate) sort_order: i32,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = prompts)]
pub(crate) struct SqlNewPromptRow {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) content: String,
    pub(crate) enabled: bool,
    pub(crate) sort_order: i32,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) updated_at: OffsetDateTime,
}

impl TryFrom<SqlPromptRow> for PromptRecord {
    type Error = DbError;

    fn try_from(row: SqlPromptRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            name: row.name,
            content: PromptContent { text: row.content },
            enabled: row.enabled,
            sort_order: row.sort_order,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

use super::*;

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = projects)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub(crate) struct SqlProjectRow {
    pub(crate) id: String,
    pub(crate) path: String,
    pub(crate) display_name: String,
    pub(crate) kind: String,
    pub(crate) pinned: bool,
    pub(crate) removed: bool,
    pub(crate) metadata_json: Value,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) updated_at: OffsetDateTime,
    pub(crate) last_opened_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = projects)]
pub(crate) struct SqlNewProjectRow {
    pub(crate) id: String,
    pub(crate) path: String,
    pub(crate) display_name: String,
    pub(crate) kind: String,
    pub(crate) pinned: bool,
    pub(crate) removed: bool,
    pub(crate) metadata_json: Value,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) updated_at: OffsetDateTime,
    pub(crate) last_opened_at: Option<OffsetDateTime>,
}

impl TryFrom<SqlProjectRow> for ProjectRecord {
    type Error = DbError;

    fn try_from(row: SqlProjectRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            path: row.path,
            display_name: row.display_name,
            kind: db_label_parse(row.kind)?,
            pinned: row.pinned,
            removed: row.removed,
            metadata: from_json(row.metadata_json)?,
            created_at: row.created_at,
            updated_at: row.updated_at,
            last_opened_at: row.last_opened_at,
        })
    }
}

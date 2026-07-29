use super::*;

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = schema_migrations)]
pub(crate) struct SqlNewSchemaMigrationRow {
    pub(crate) name: String,
    pub(crate) executed_at: OffsetDateTime,
}

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = schema_metadata)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub(crate) struct SqlSchemaMetadataRow {
    pub(crate) id: String,
    pub(crate) schema_version: i32,
    pub(crate) created_app_version: Option<String>,
    pub(crate) last_opened_app_version: Option<String>,
    pub(crate) payload_json: Value,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = schema_metadata)]
pub(crate) struct SqlNewSchemaMetadataRow {
    pub(crate) id: String,
    pub(crate) schema_version: i32,
    pub(crate) created_app_version: Option<String>,
    pub(crate) last_opened_app_version: Option<String>,
    pub(crate) payload_json: Value,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) updated_at: OffsetDateTime,
}

impl TryFrom<SqlSchemaMetadataRow> for SchemaMetadataRecord {
    type Error = DbError;

    fn try_from(row: SqlSchemaMetadataRow) -> Result<Self> {
        if row.id != "default" {
            return Err(DbError::Invariant(format!(
                "unexpected schema metadata id {}",
                row.id
            )));
        }
        Ok(Self {
            schema_version: row.schema_version,
            created_app_version: row.created_app_version,
            last_opened_app_version: row.last_opened_app_version,
            payload: from_json(row.payload_json)?,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

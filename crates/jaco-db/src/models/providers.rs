use super::*;

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = providers)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub(crate) struct SqlProviderRow {
    pub(crate) id: String,
    pub(crate) kind: String,
    pub(crate) display_name: String,
    pub(crate) enabled: bool,
    pub(crate) settings_json: Value,
    pub(crate) secret_refs_json: Value,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = providers)]
pub(crate) struct SqlNewProviderRow {
    pub(crate) id: String,
    pub(crate) kind: String,
    pub(crate) display_name: String,
    pub(crate) enabled: bool,
    pub(crate) settings_json: Value,
    pub(crate) secret_refs_json: Value,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = provider_models)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub(crate) struct SqlProviderModelRow {
    pub(crate) id: String,
    pub(crate) provider_id: String,
    pub(crate) model_id: String,
    pub(crate) display_name: Option<String>,
    pub(crate) enabled: bool,
    pub(crate) capabilities_json: Value,
    pub(crate) metadata_json: Value,
    pub(crate) fetched_at: OffsetDateTime,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) updated_at: OffsetDateTime,
    pub(crate) pricing_json: Option<Value>,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = provider_models)]
pub(crate) struct SqlNewProviderModelRow {
    pub(crate) id: String,
    pub(crate) provider_id: String,
    pub(crate) model_id: String,
    pub(crate) display_name: Option<String>,
    pub(crate) enabled: bool,
    pub(crate) capabilities_json: Value,
    pub(crate) metadata_json: Value,
    pub(crate) fetched_at: OffsetDateTime,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) updated_at: OffsetDateTime,
    pub(crate) pricing_json: Option<Value>,
}

impl TryFrom<SqlProviderRow> for ProviderRecord {
    type Error = DbError;

    fn try_from(row: SqlProviderRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            kind: row.kind,
            display_name: row.display_name,
            enabled: row.enabled,
            settings: from_json(row.settings_json)?,
            secret_refs: from_json(row.secret_refs_json)?,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

impl TryFrom<SqlProviderModelRow> for ProviderModelRecord {
    type Error = DbError;

    fn try_from(row: SqlProviderModelRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            provider_id: row.provider_id,
            model_id: row.model_id,
            display_name: row.display_name,
            enabled: row.enabled,
            capabilities: from_json(row.capabilities_json)?,
            metadata: from_json(row.metadata_json)?,
            fetched_at: row.fetched_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
            pricing: from_json_opt(row.pricing_json)?,
        })
    }
}

use super::*;

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = shortcuts)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub(crate) struct SqlShortcutRow {
    pub(crate) id: String,
    pub(crate) hotkey: String,
    pub(crate) enabled: bool,
    pub(crate) prompt_id: Option<String>,
    pub(crate) provider_id: Option<String>,
    pub(crate) model_id: Option<String>,
    pub(crate) input_source: String,
    pub(crate) action_json: Value,
    pub(crate) settings_snapshot_json: Value,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = shortcuts)]
pub(crate) struct SqlNewShortcutRow {
    pub(crate) id: String,
    pub(crate) hotkey: String,
    pub(crate) enabled: bool,
    pub(crate) prompt_id: Option<String>,
    pub(crate) provider_id: Option<String>,
    pub(crate) model_id: Option<String>,
    pub(crate) input_source: String,
    pub(crate) action_json: Value,
    pub(crate) settings_snapshot_json: Value,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) updated_at: OffsetDateTime,
}

impl TryFrom<SqlShortcutRow> for ShortcutRecord {
    type Error = DbError;

    fn try_from(row: SqlShortcutRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            hotkey: row.hotkey,
            enabled: row.enabled,
            prompt_id: row.prompt_id,
            provider_id: row.provider_id,
            model_id: row.model_id,
            input_source: db_label_parse(row.input_source)?,
            action: from_json(row.action_json)?,
            settings_snapshot: from_json(row.settings_snapshot_json)?,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

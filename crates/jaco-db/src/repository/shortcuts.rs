use super::*;

impl FreshRepository {
    pub fn insert_shortcut(&self, input: NewShortcut) -> Result<ShortcutRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let now = now_string()?;
            let row = SqlNewShortcutRow {
                id: new_id(),
                hotkey: input.hotkey,
                enabled: input.enabled,
                prompt_id: input.prompt_id,
                provider_id: input.provider_id,
                model_id: input.model_id,
                input_source: db_label(&input.input_source)?,
                action_json: to_json(&input.action)?,
                settings_snapshot_json: to_json(&input.settings_snapshot)?,
                created_at: now,
                updated_at: now,
            };
            diesel::insert_into(shortcuts::table)
                .values(&row)
                .returning(SqlShortcutRow::as_returning())
                .get_result::<SqlShortcutRow>(conn)?
                .try_into()
        })
    }

    pub fn get_shortcut(&self, id: &str) -> Result<Option<ShortcutRecord>> {
        let mut conn = self.conn()?;
        shortcut_row(&mut conn, id)?
            .map(TryInto::try_into)
            .transpose()
    }

    pub fn update_shortcut(&self, id: &str, input: UpdateShortcut) -> Result<ShortcutRecord> {
        let mut conn = self.conn()?;
        diesel::update(shortcuts::table.find(id))
            .set((
                shortcuts::hotkey.eq(input.hotkey),
                shortcuts::enabled.eq(input.enabled),
                shortcuts::prompt_id.eq(input.prompt_id),
                shortcuts::provider_id.eq(input.provider_id),
                shortcuts::model_id.eq(input.model_id),
                shortcuts::input_source.eq(db_label(&input.input_source)?),
                shortcuts::action_json.eq(to_json(&input.action)?),
                shortcuts::settings_snapshot_json.eq(to_json(&input.settings_snapshot)?),
                shortcuts::updated_at.eq(now_string()?),
            ))
            .returning(SqlShortcutRow::as_returning())
            .get_result::<SqlShortcutRow>(&mut conn)?
            .try_into()
    }

    pub fn set_shortcut_enabled(&self, id: &str, enabled: bool) -> Result<ShortcutRecord> {
        let mut conn = self.conn()?;
        diesel::update(shortcuts::table.find(id))
            .set((
                shortcuts::enabled.eq(enabled),
                shortcuts::updated_at.eq(now_string()?),
            ))
            .returning(SqlShortcutRow::as_returning())
            .get_result::<SqlShortcutRow>(&mut conn)?
            .try_into()
    }

    pub fn delete_shortcut(&self, id: &str) -> Result<usize> {
        let mut conn = self.conn()?;
        Ok(diesel::delete(shortcuts::table.find(id)).execute(&mut conn)?)
    }

    pub fn list_shortcuts(&self) -> Result<Vec<ShortcutRecord>> {
        let mut conn = self.conn()?;
        shortcuts::table
            .order(shortcuts::created_at.asc())
            .select(SqlShortcutRow::as_select())
            .load::<SqlShortcutRow>(&mut conn)?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }
}

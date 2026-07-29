use super::*;

impl FreshRepository {
    pub fn insert_prompt(&self, input: NewPrompt) -> Result<PromptRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let now = now_string()?;
            let row = SqlNewPromptRow {
                id: new_id(),
                name: input.name,
                content: input.content.text,
                enabled: input.enabled,
                sort_order: input.sort_order,
                created_at: now,
                updated_at: now,
            };
            diesel::insert_into(prompts::table)
                .values(&row)
                .returning(SqlPromptRow::as_returning())
                .get_result::<SqlPromptRow>(conn)?
                .try_into()
        })
    }

    pub fn get_prompt(&self, id: &str) -> Result<Option<PromptRecord>> {
        let mut conn = self.conn()?;
        prompt_row(&mut conn, id)?
            .map(TryInto::try_into)
            .transpose()
    }

    pub fn list_prompts(&self) -> Result<Vec<PromptRecord>> {
        let mut conn = self.conn()?;
        prompts::table
            .order((
                prompts::sort_order.asc(),
                prompts::name.asc(),
                prompts::created_at.asc(),
            ))
            .select(SqlPromptRow::as_select())
            .load::<SqlPromptRow>(&mut conn)?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }

    pub fn update_prompt(&self, id: &str, input: UpdatePrompt) -> Result<PromptRecord> {
        let mut conn = self.conn()?;
        diesel::update(prompts::table.find(id))
            .set((
                prompts::name.eq(input.name),
                prompts::content.eq(input.content.text),
                prompts::enabled.eq(input.enabled),
                prompts::sort_order.eq(input.sort_order),
                prompts::updated_at.eq(now_string()?),
            ))
            .returning(SqlPromptRow::as_returning())
            .get_result::<SqlPromptRow>(&mut conn)?
            .try_into()
    }

    pub fn delete_prompt(&self, id: &str) -> Result<usize> {
        let mut conn = self.conn()?;
        Ok(diesel::delete(prompts::table.find(id)).execute(&mut conn)?)
    }
}

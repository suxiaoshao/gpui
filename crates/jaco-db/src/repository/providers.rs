use super::*;

impl FreshRepository {
    pub fn insert_provider(&self, input: NewProvider) -> Result<ProviderRecord> {
        self.insert_provider_with_id(new_id(), input)
    }

    pub fn insert_provider_with_id(
        &self,
        id: ProviderId,
        input: NewProvider,
    ) -> Result<ProviderRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let now = now_string()?;
            let row = SqlNewProviderRow {
                id,
                kind: input.kind,
                display_name: input.display_name,
                enabled: input.enabled,
                settings_json: to_json(&input.settings)?,
                secret_refs_json: to_json(&input.secret_refs)?,
                created_at: now,
                updated_at: now,
            };
            diesel::insert_into(providers::table)
                .values(&row)
                .returning(SqlProviderRow::as_returning())
                .get_result::<SqlProviderRow>(conn)?
                .try_into()
        })
    }

    pub fn get_provider(&self, id: &str) -> Result<Option<ProviderRecord>> {
        let mut conn = self.conn()?;
        provider_row(&mut conn, id)?
            .map(TryInto::try_into)
            .transpose()
    }

    pub fn list_providers(&self) -> Result<Vec<ProviderRecord>> {
        let mut conn = self.conn()?;
        providers::table
            .order((providers::display_name.asc(), providers::kind.asc()))
            .select(SqlProviderRow::as_select())
            .load::<SqlProviderRow>(&mut conn)?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }

    pub fn update_provider(&self, id: &str, input: UpdateProvider) -> Result<ProviderRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            diesel::update(providers::table.find(id))
                .set((
                    providers::display_name.eq(input.display_name),
                    providers::enabled.eq(input.enabled),
                    providers::settings_json.eq(to_json(&input.settings)?),
                    providers::secret_refs_json.eq(to_json(&input.secret_refs)?),
                    providers::updated_at.eq(now_string()?),
                ))
                .returning(SqlProviderRow::as_returning())
                .get_result::<SqlProviderRow>(conn)?
                .try_into()
        })
    }

    pub fn delete_provider(&self, id: &str) -> Result<usize> {
        let mut conn = self.conn()?;
        Ok(diesel::delete(providers::table.find(id)).execute(&mut conn)?)
    }

    pub fn upsert_provider_model(&self, input: NewProviderModel) -> Result<ProviderModelRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let now = now_string()?;
            let row = SqlNewProviderModelRow {
                id: new_id(),
                provider_id: input.provider_id,
                model_id: input.model_id,
                display_name: input.display_name,
                enabled: input.enabled,
                capabilities_json: to_json(&input.capabilities)?,
                metadata_json: to_json(&input.metadata)?,
                fetched_at: now,
                created_at: now,
                updated_at: now,
            };
            diesel::insert_into(provider_models::table)
                .values(&row)
                .on_conflict((provider_models::provider_id, provider_models::model_id))
                .do_update()
                .set((
                    provider_models::display_name.eq(excluded(provider_models::display_name)),
                    provider_models::capabilities_json
                        .eq(excluded(provider_models::capabilities_json)),
                    provider_models::metadata_json.eq(excluded(provider_models::metadata_json)),
                    provider_models::fetched_at.eq(excluded(provider_models::fetched_at)),
                    provider_models::updated_at.eq(excluded(provider_models::updated_at)),
                ))
                .returning(SqlProviderModelRow::as_returning())
                .get_result::<SqlProviderModelRow>(conn)?
                .try_into()
        })
    }

    pub fn replace_fetched_provider_models(
        &self,
        provider_id: &str,
        models: Vec<NewProviderModel>,
    ) -> Result<Vec<ProviderModelRecord>> {
        let model_ids = models
            .iter()
            .map(|model| {
                if model.provider_id != provider_id {
                    return Err(DbError::Invariant(format!(
                        "provider model {} belongs to provider {}, expected {}",
                        model.model_id, model.provider_id, provider_id
                    )));
                }
                Ok(model.model_id.clone())
            })
            .collect::<Result<Vec<_>>>()?;
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let delete_query = diesel::delete(
                provider_models::table.filter(provider_models::provider_id.eq(provider_id)),
            );
            if model_ids.is_empty() {
                delete_query.execute(conn)?;
            } else {
                delete_query
                    .filter(provider_models::model_id.ne_all(&model_ids))
                    .execute(conn)?;
            }
            let mut records = Vec::with_capacity(models.len());
            for input in models {
                let now = now_string()?;
                let row = SqlNewProviderModelRow {
                    id: new_id(),
                    provider_id: input.provider_id,
                    model_id: input.model_id,
                    display_name: input.display_name,
                    enabled: input.enabled,
                    capabilities_json: to_json(&input.capabilities)?,
                    metadata_json: to_json(&input.metadata)?,
                    fetched_at: now,
                    created_at: now,
                    updated_at: now,
                };
                let record = diesel::insert_into(provider_models::table)
                    .values(&row)
                    .on_conflict((provider_models::provider_id, provider_models::model_id))
                    .do_update()
                    .set((
                        provider_models::display_name.eq(excluded(provider_models::display_name)),
                        provider_models::capabilities_json
                            .eq(excluded(provider_models::capabilities_json)),
                        provider_models::metadata_json.eq(excluded(provider_models::metadata_json)),
                        provider_models::fetched_at.eq(excluded(provider_models::fetched_at)),
                        provider_models::updated_at.eq(excluded(provider_models::updated_at)),
                    ))
                    .returning(SqlProviderModelRow::as_returning())
                    .get_result::<SqlProviderModelRow>(conn)?
                    .try_into()?;
                records.push(record);
            }
            Ok(records)
        })
    }

    pub fn get_provider_model(
        &self,
        provider_id: &str,
        model_id: &str,
    ) -> Result<Option<ProviderModelRecord>> {
        let mut conn = self.conn()?;
        provider_model_row(&mut conn, provider_id, model_id)?
            .map(TryInto::try_into)
            .transpose()
    }

    pub fn list_provider_models(&self, provider_id: &str) -> Result<Vec<ProviderModelRecord>> {
        let mut conn = self.conn()?;
        provider_models::table
            .filter(provider_models::provider_id.eq(provider_id))
            .order((
                provider_models::display_name.asc(),
                provider_models::model_id.asc(),
            ))
            .select(SqlProviderModelRow::as_select())
            .load::<SqlProviderModelRow>(&mut conn)?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }

    pub fn set_provider_model_enabled(
        &self,
        provider_id: &str,
        model_id: &str,
        enabled: bool,
    ) -> Result<ProviderModelRecord> {
        let mut conn = self.conn()?;
        diesel::update(
            provider_models::table
                .filter(provider_models::provider_id.eq(provider_id))
                .filter(provider_models::model_id.eq(model_id)),
        )
        .set((
            provider_models::enabled.eq(enabled),
            provider_models::updated_at.eq(now_string()?),
        ))
        .returning(SqlProviderModelRow::as_returning())
        .get_result::<SqlProviderModelRow>(&mut conn)?
        .try_into()
    }

    pub fn delete_provider_model(&self, provider_id: &str, model_id: &str) -> Result<usize> {
        let mut conn = self.conn()?;
        Ok(diesel::delete(
            provider_models::table
                .filter(provider_models::provider_id.eq(provider_id))
                .filter(provider_models::model_id.eq(model_id)),
        )
        .execute(&mut conn)?)
    }
}

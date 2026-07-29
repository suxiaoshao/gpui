use super::*;

impl FreshRepository {
    pub fn insert_project(&self, input: NewProject) -> Result<ProjectRecord> {
        let mut conn = self.conn()?;
        conn.immediate_transaction(|conn| {
            let now = now_string()?;
            let row = SqlNewProjectRow {
                id: new_id(),
                path: input.path,
                display_name: input.display_name,
                kind: db_label(&input.kind)?,
                pinned: input.pinned,
                removed: input.removed,
                metadata_json: to_json(&input.metadata)?,
                created_at: now,
                updated_at: now,
                last_opened_at: None,
            };
            diesel::insert_into(projects::table)
                .values(&row)
                .returning(SqlProjectRow::as_returning())
                .get_result::<SqlProjectRow>(conn)?
                .try_into()
        })
    }

    pub fn get_project(&self, id: &str) -> Result<Option<ProjectRecord>> {
        let mut conn = self.conn()?;
        project_row(&mut conn, id)?
            .map(TryInto::try_into)
            .transpose()
    }

    pub fn get_project_by_path(&self, path: &str) -> Result<Option<ProjectRecord>> {
        let mut conn = self.conn()?;
        projects::table
            .filter(projects::path.eq(path))
            .select(SqlProjectRow::as_select())
            .first::<SqlProjectRow>(&mut conn)
            .optional()?
            .map(TryInto::try_into)
            .transpose()
    }

    pub fn list_projects(&self) -> Result<Vec<ProjectRecord>> {
        let mut conn = self.conn()?;
        projects::table
            .order((projects::display_name.asc(), projects::path.asc()))
            .select(SqlProjectRow::as_select())
            .load::<SqlProjectRow>(&mut conn)?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }

    pub fn list_visible_projects(&self) -> Result<Vec<ProjectRecord>> {
        let mut conn = self.conn()?;
        projects::table
            .filter(projects::removed.eq(false))
            .order((projects::display_name.asc(), projects::path.asc()))
            .select(SqlProjectRow::as_select())
            .load::<SqlProjectRow>(&mut conn)?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }

    pub fn list_sidebar_projects(&self) -> Result<Vec<ProjectRecord>> {
        let mut conn = self.conn()?;
        let normal = db_label(&ProjectKind::Normal)?;
        projects::table
            .filter(projects::kind.eq(normal))
            .filter(projects::removed.eq(false))
            .order((projects::display_name.asc(), projects::path.asc()))
            .select(SqlProjectRow::as_select())
            .load::<SqlProjectRow>(&mut conn)?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }

    pub fn update_project_metadata(
        &self,
        id: &str,
        metadata: ProjectMetadata,
    ) -> Result<ProjectRecord> {
        let mut conn = self.conn()?;
        diesel::update(projects::table.find(id))
            .set((
                projects::metadata_json.eq(to_json(&metadata)?),
                projects::updated_at.eq(now_string()?),
            ))
            .returning(SqlProjectRow::as_returning())
            .get_result::<SqlProjectRow>(&mut conn)?
            .try_into()
    }

    pub fn rename_project(&self, id: &str, display_name: String) -> Result<ProjectRecord> {
        let mut conn = self.conn()?;
        diesel::update(projects::table.find(id))
            .set((
                projects::display_name.eq(display_name),
                projects::updated_at.eq(now_string()?),
            ))
            .returning(SqlProjectRow::as_returning())
            .get_result::<SqlProjectRow>(&mut conn)?
            .try_into()
    }

    pub fn set_project_removed(&self, id: &str, removed: bool) -> Result<ProjectRecord> {
        let mut conn = self.conn()?;
        diesel::update(projects::table.find(id))
            .set((
                projects::removed.eq(removed),
                projects::updated_at.eq(now_string()?),
            ))
            .returning(SqlProjectRow::as_returning())
            .get_result::<SqlProjectRow>(&mut conn)?
            .try_into()
    }

    pub fn set_project_pinned(&self, id: &str, pinned: bool) -> Result<ProjectRecord> {
        let mut conn = self.conn()?;
        diesel::update(projects::table.find(id))
            .set((
                projects::pinned.eq(pinned),
                projects::updated_at.eq(now_string()?),
            ))
            .returning(SqlProjectRow::as_returning())
            .get_result::<SqlProjectRow>(&mut conn)?
            .try_into()
    }
}

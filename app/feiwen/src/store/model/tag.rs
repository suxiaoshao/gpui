use crate::{errors::FeiwenResult, store::service::Tag};

use super::super::schema::{novel_tag, tag};
use diesel::dsl::count;
use diesel::{prelude::*, QueryDsl};

#[derive(Queryable, Insertable, Debug)]
#[diesel(table_name = tag)]
pub(crate) struct TagModel {
    pub(crate) id: Option<i32>,
    pub(crate) name: String,
}

impl TagModel {
    pub(crate) fn all_tags(conn: &mut SqliteConnection) -> FeiwenResult<Vec<Self>> {
        let data = tag::table
            .inner_join(novel_tag::table.on(tag::name.eq(novel_tag::tag_id)))
            .select((tag::id, tag::name))
            .filter(tag::id.is_not_null())
            .group_by(tag::name)
            .order(count(novel_tag::tag_id).desc())
            .load(conn)?;
        Ok(data)
    }
    pub(crate) fn save(tags: Vec<TagModel>, conn: &mut SqliteConnection) -> FeiwenResult<()> {
        diesel::insert_or_ignore_into(tag::table)
            .values(tags)
            .execute(conn)?;
        Ok(())
    }
}

impl From<&Tag> for TagModel {
    fn from(url: &Tag) -> Self {
        Self {
            id: url.id,
            name: url.name.clone(),
        }
    }
}

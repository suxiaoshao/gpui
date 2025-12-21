use super::super::schema::novel;
use crate::{
    errors::FeiwenResult,
    store::{
        service::Novel,
        types::{Author, Title},
    },
};
use diesel::prelude::*;
use diesel::{QueryDsl, SqliteConnection};

#[derive(Insertable, Queryable)]
#[diesel(table_name = novel)]
pub(crate) struct NovelModel {
    pub(crate) id: i32,
    pub(crate) name: String,
    pub(crate) desc: String,
    pub(crate) is_limit: bool,
    pub(crate) latest_chapter_name: String,
    pub(crate) latest_chapter_id: i32,
    pub(crate) word_count: i32,
    pub(crate) read_count: i32,
    pub(crate) reply_count: i32,
    pub(crate) author_id: Option<i32>,
    pub(crate) author_name: String,
}

impl NovelModel {
    pub(crate) fn query(conn: &mut SqliteConnection) -> FeiwenResult<Vec<NovelModel>> {
        use super::super::schema::novel::dsl;
        let data = dsl::novel.load::<Self>(conn)?;
        Ok(data)
    }
    pub(crate) fn save(self, conn: &mut SqliteConnection) -> FeiwenResult<()> {
        diesel::insert_or_ignore_into(novel::table)
            .values(self)
            .execute(conn)?;
        Ok(())
    }
    pub(crate) fn count(conn: &mut SqliteConnection) -> FeiwenResult<i64> {
        let count = novel::dsl::novel.count().get_result(conn)?;
        Ok(count)
    }
}

impl From<Novel> for NovelModel {
    fn from(value: Novel) -> Self {
        let (author_id, author_name) = match value.author {
            Author::Anonymous(name) => (None, name),
            Author::Known(Title { name, id }) => (Some(id), name),
        };
        Self {
            id: value.title.id,
            name: value.title.name,
            desc: value.desc,
            is_limit: value.is_limit,
            latest_chapter_name: value.latest_chapter.name,
            latest_chapter_id: value.latest_chapter.id,
            author_id,
            author_name,
            word_count: value.count.word_count,
            read_count: value.count.read_count,
            reply_count: value.count.reply_count,
        }
    }
}

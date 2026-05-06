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
    pub(crate) read_count: Option<i32>,
    pub(crate) reply_count: Option<i32>,
    pub(crate) author_id: Option<i32>,
    pub(crate) author_name: String,
}

impl NovelModel {
    pub(crate) fn query(conn: &mut SqliteConnection) -> FeiwenResult<Vec<NovelModel>> {
        use super::super::schema::novel::dsl;
        let data = dsl::novel.load::<Self>(conn)?;
        Ok(data)
    }
    pub(crate) fn save(mut self, conn: &mut SqliteConnection) -> FeiwenResult<()> {
        use super::super::schema::novel::dsl;

        if let Some((read_count, reply_count)) = dsl::novel
            .find(self.id)
            .select((dsl::read_count, dsl::reply_count))
            .first::<(Option<i32>, Option<i32>)>(conn)
            .optional()?
        {
            if self.read_count.is_none() {
                self.read_count = read_count;
            }
            if self.reply_count.is_none() {
                self.reply_count = reply_count;
            }
        }

        diesel::insert_into(novel::table)
            .values(&self)
            .on_conflict(novel::id)
            .do_update()
            .set((
                novel::name.eq(&self.name),
                novel::desc.eq(&self.desc),
                novel::is_limit.eq(self.is_limit),
                novel::latest_chapter_name.eq(&self.latest_chapter_name),
                novel::latest_chapter_id.eq(self.latest_chapter_id),
                novel::word_count.eq(self.word_count),
                novel::read_count.eq(self.read_count),
                novel::reply_count.eq(self.reply_count),
                novel::author_id.eq(self.author_id),
                novel::author_name.eq(&self.author_name),
            ))
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

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::{Connection, connection::SimpleConnection};

    fn connection() -> SqliteConnection {
        let mut conn = SqliteConnection::establish(":memory:").unwrap();
        conn.batch_execute(
            r#"
            CREATE TABLE novel
            (
                id                  integer NOT NULL PRIMARY key,
                name                text    NOT NULL,
                desc                text    NOT NULL,
                is_limit            boolean NOT NULL,
                latest_chapter_name text    NOT NULL,
                latest_chapter_id   integer NOT NULL,
                word_count          integer NOT NULL,
                read_count          integer,
                reply_count         integer,
                author_id           integer,
                author_name         text    not null
            );
            "#,
        )
        .unwrap();
        conn
    }

    fn model(id: i32, read_count: Option<i32>, reply_count: Option<i32>) -> NovelModel {
        NovelModel {
            id,
            name: format!("name-{id}"),
            desc: "desc".to_owned(),
            is_limit: false,
            latest_chapter_name: "chapter".to_owned(),
            latest_chapter_id: 10,
            word_count: 1000,
            read_count,
            reply_count,
            author_id: Some(1),
            author_name: "author".to_owned(),
        }
    }

    #[test]
    fn save_preserves_existing_counts_when_new_counts_are_missing() {
        let mut conn = connection();
        model(1, Some(12), Some(34)).save(&mut conn).unwrap();

        let mut updated = model(1, None, None);
        updated.name = "updated".to_owned();
        updated.author_id = None;
        updated.save(&mut conn).unwrap();

        let row = novel::table.find(1).first::<NovelModel>(&mut conn).unwrap();
        assert_eq!(row.name, "updated");
        assert_eq!(row.author_id, None);
        assert_eq!(row.read_count, Some(12));
        assert_eq!(row.reply_count, Some(34));
    }

    #[test]
    fn save_allows_missing_counts_for_new_novel() {
        let mut conn = connection();
        model(1, None, None).save(&mut conn).unwrap();

        let row = novel::table.find(1).first::<NovelModel>(&mut conn).unwrap();
        assert_eq!(row.read_count, None);
        assert_eq!(row.reply_count, None);
    }
}

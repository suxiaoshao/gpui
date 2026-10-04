use duckdb::{Connection, Error as DuckdbError, params};

use crate::{
    errors::FeiwenResult,
    query::{NovelRecord, QuerySpec, query_records},
    types::{Author, NovelCount, Title},
};

use super::Tag;

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Novel {
    pub title: Title,
    pub author: Author,
    pub latest_chapter: Title,
    pub desc: gpui_kit::SharedString,
    pub count: NovelCount,
    pub tags: std::collections::HashSet<Tag>,
    pub is_limit: bool,
}

impl Novel {
    pub fn save(self, conn: &mut Connection) -> FeiwenResult<()> {
        let old_counts = load_existing_counts(conn, self.title.id)?;
        let read_count = self
            .count
            .read_count
            .or_else(|| old_counts.and_then(|counts| counts.0));
        let reply_count = self
            .count
            .reply_count
            .or_else(|| old_counts.and_then(|counts| counts.1));
        let (author_id, author_name) = match &self.author {
            Author::Known(author) => (Some(author.id), author.name.clone()),
            Author::Anonymous(name) => (None, name.clone()),
        };

        let tx = conn.transaction()?;
        tx.execute(
            r#"
            INSERT INTO novel (
                id,
                name,
                "desc",
                is_limit,
                latest_chapter_name,
                latest_chapter_id,
                word_count,
                read_count,
                reply_count,
                author_id,
                author_name
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT (id) DO UPDATE SET
                name = excluded.name,
                "desc" = excluded."desc",
                is_limit = excluded.is_limit,
                latest_chapter_name = excluded.latest_chapter_name,
                latest_chapter_id = excluded.latest_chapter_id,
                word_count = excluded.word_count,
                read_count = excluded.read_count,
                reply_count = excluded.reply_count,
                author_id = excluded.author_id,
                author_name = excluded.author_name
            "#,
            params![
                self.title.id,
                self.title.name.as_ref(),
                self.desc.as_ref(),
                self.is_limit,
                self.latest_chapter.name.as_ref(),
                self.latest_chapter.id,
                self.count.word_count,
                read_count,
                reply_count,
                author_id,
                author_name.as_ref(),
            ],
        )?;

        for tag in &self.tags {
            tx.execute(
                "INSERT INTO tag (id, name) VALUES (?, ?) ON CONFLICT DO NOTHING",
                params![tag.id, tag.name.as_ref()],
            )?;
            tx.execute(
                "INSERT INTO novel_tag (novel_id, tag_id) VALUES (?, ?) ON CONFLICT DO NOTHING",
                params![self.title.id, tag.name.as_ref()],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    pub fn count(conn: &Connection) -> FeiwenResult<i64> {
        conn.query_row("SELECT count(*) FROM novel", [], |row| row.get(0))
            .map_err(Into::into)
    }

    pub fn query(spec: &QuerySpec, conn: &Connection) -> FeiwenResult<Vec<Novel>> {
        Ok(query_records(conn, spec)?
            .into_iter()
            .map(NovelRecord::into_novel)
            .collect())
    }
}

fn load_existing_counts(
    conn: &Connection,
    novel_id: i32,
) -> FeiwenResult<Option<(Option<i32>, Option<i32>)>> {
    match conn.query_row(
        "SELECT read_count, reply_count FROM novel WHERE id = ?",
        params![novel_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ) {
        Ok(counts) => Ok(Some(counts)),
        Err(DuckdbError::QueryReturnedNoRows) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

impl NovelRecord {
    fn into_novel(self) -> Novel {
        let author = match self.author_id {
            Some(author_id) => Author::Known(Title {
                name: self.author_name.into(),
                id: author_id,
            }),
            None => Author::Anonymous(self.author_name.into()),
        };

        Novel {
            desc: self.desc.into(),
            is_limit: self.is_limit,
            title: Title {
                name: self.title.into(),
                id: self.id,
            },
            author,
            latest_chapter: Title {
                name: self.latest_chapter_name.into(),
                id: self.latest_chapter_id,
            },
            count: NovelCount {
                word_count: self.word_count,
                read_count: self.read_count,
                reply_count: self.reply_count,
            },
            tags: self
                .tags
                .into_iter()
                .map(|name| {
                    let id = self.tag_ids.get(&name).copied().flatten();
                    Tag {
                        name: name.into(),
                        id,
                    }
                })
                .collect(),
        }
    }
}

impl Novel {
    pub fn new(
        title: Title,
        author: Author,
        latest_chapter: Title,
        desc: gpui_kit::SharedString,
        count: NovelCount,
        tags: std::collections::HashSet<Tag>,
        is_limit: bool,
    ) -> Self {
        Self {
            title,
            author,
            latest_chapter,
            desc,
            count,
            tags,
            is_limit,
        }
    }
}

use crate::{FeiwenResult, query::AuthorRef, service::Tag};
use duckdb::Connection;
use std::collections::HashMap;

#[derive(Clone, Default, PartialEq, Eq)]
pub struct CatalogOptions {
    tags: Vec<(i32, String)>,
    authors: Vec<(AuthorRef, String)>,
}
impl CatalogOptions {
    pub fn tags(&self) -> &[(i32, String)] {
        &self.tags
    }
    pub fn authors(&self) -> &[(AuthorRef, String)] {
        &self.authors
    }
    pub(crate) fn load(conn: &Connection) -> FeiwenResult<Self> {
        let tags = Tag::tags_with_id(conn)?
            .into_iter()
            .map(|tag| (tag.id, tag.name))
            .collect();
        let mut stmt = conn
            .prepare("SELECT author_id, author_name FROM novel GROUP BY author_id, author_name")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, Option<i32>>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut authors = HashMap::new();
        for row in rows {
            let (id, name) = row?;
            let author = id.map_or_else(|| AuthorRef::Name(name.clone()), AuthorRef::Id);
            authors.entry(author).or_insert(name);
        }
        let mut authors: Vec<_> = authors.into_iter().collect();
        authors.sort_by(|(left_id, left_name), (right_id, right_name)| {
            left_name.cmp(right_name).then_with(|| {
                fn key(id: &AuthorRef) -> (u8, i32, &str) {
                    match id {
                        AuthorRef::Id(ix) => (0, *ix, ""),
                        AuthorRef::Name(name) => (1, 0, name.as_str()),
                    }
                }
                key(left_id).cmp(&key(right_id))
            })
        });
        Ok(Self { tags, authors })
    }
}

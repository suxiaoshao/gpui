use duckdb::Connection;

use crate::errors::FeiwenResult;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Tag {
    pub name: gpui_kit::SharedString,
    pub id: Option<i32>,
}

#[derive(Clone)]
pub(crate) struct TagWithId {
    pub name: String,
    pub id: i32,
}

impl Tag {
    pub(crate) fn tags_with_id(conn: &Connection) -> FeiwenResult<Vec<TagWithId>> {
        let mut stmt = conn.prepare(
            "\
            SELECT tag.id, tag.name \
            FROM tag \
            INNER JOIN novel_tag ON tag.name = novel_tag.tag_id \
            WHERE tag.id IS NOT NULL \
            GROUP BY tag.id, tag.name \
            ORDER BY count(novel_tag.tag_id) DESC, tag.name ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(TagWithId {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

impl Tag {
    pub fn new(name: gpui_kit::SharedString, id: Option<i32>) -> Self {
        Self { name, id }
    }
}

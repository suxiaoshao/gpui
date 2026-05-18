use duckdb::Connection;
use gpui::{IntoElement, ParentElement, RenderOnce};
use gpui_component::tag::Tag as TagComponent;

use crate::errors::FeiwenResult;

#[derive(Debug, Clone, PartialEq, Eq, Hash, IntoElement)]
pub(crate) struct Tag {
    pub(crate) name: String,
    pub(crate) id: Option<i32>,
}

impl RenderOnce for Tag {
    fn render(self, _window: &mut gpui::Window, _cx: &mut gpui::App) -> impl gpui::IntoElement {
        TagComponent::primary().child(self.name)
    }
}

#[derive(Clone)]
pub(crate) struct TagWithId {
    pub(crate) name: String,
    pub(crate) id: i32,
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

use diesel::SqliteConnection;
use gpui::{IntoElement, ParentElement, RenderOnce};
use gpui_component::tag::Tag as TagComponent;

use crate::{errors::FeiwenResult, store::model::TagModel};

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

impl From<TagModel> for Tag {
    fn from(value: TagModel) -> Self {
        Self {
            name: value.name,
            id: value.id,
        }
    }
}

impl Tag {
    pub(crate) fn tags_with_id(conn: &mut SqliteConnection) -> FeiwenResult<Vec<TagWithId>> {
        let tags = TagModel::all_tags(conn)?;
        let tags = tags
            .into_iter()
            .filter_map(|tag| match tag.id {
                Some(id) => Some(TagWithId { name: tag.name, id }),
                None => None,
            })
            .collect::<Vec<TagWithId>>();
        Ok(tags)
    }
}

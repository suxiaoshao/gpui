use diesel::SqliteConnection;

use crate::{errors::FeiwenResult, store::model::TagModel};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Tag {
    pub(crate) name: String,
    pub(crate) id: Option<i32>,
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

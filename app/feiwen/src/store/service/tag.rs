use diesel::SqliteConnection;

use crate::{errors::FeiwenResult, store::model::TagModel};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Tag {
    pub(crate) name: String,
    pub(crate) id: Option<i32>,
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
    pub(crate) fn tags(conn: &mut SqliteConnection) -> FeiwenResult<Vec<Self>> {
        let tags = TagModel::all_tags(conn)?;
        let tags = tags
            .into_iter()
            .map(|tag| tag.into())
            .collect::<Vec<Self>>();
        Ok(tags)
    }
}

use std::collections::HashSet;

use diesel::SqliteConnection;

use crate::{
    errors::{FeiwenError, FeiwenResult},
    store::{
        model::{NovelModel, NovelTagModel, TagModel},
        types::{Author, NovelCount, Title},
    },
};

use super::Tag;

#[derive(Debug, Clone)]
pub(crate) struct Novel {
    pub(crate) title: Title,
    pub(crate) author: Author,
    pub(crate) latest_chapter: Title,
    pub(crate) desc: String,
    pub(crate) count: NovelCount,
    pub(crate) tags: HashSet<Tag>,
    pub(crate) is_limit: bool,
}

impl Novel {
    pub(crate) fn save(self, conn: &mut SqliteConnection) -> FeiwenResult<()> {
        let tags = self
            .tags
            .iter()
            .map(|tag| tag.into())
            .collect::<Vec<TagModel>>();
        let novel_tags = self
            .tags
            .iter()
            .map(|Tag { name, .. }| NovelTagModel {
                novel_id: self.title.id,
                tag_id: name.clone(),
            })
            .collect::<Vec<NovelTagModel>>();
        let novel = NovelModel::from(self);
        conn.immediate_transaction::<_, FeiwenError, _>(|conn| {
            novel.save(conn)?;
            TagModel::save(tags, conn)?;
            NovelTagModel::save(novel_tags, conn)?;
            Ok(())
        })?;
        Ok(())
    }
    pub(crate) fn count(conn: &mut SqliteConnection) -> FeiwenResult<i64> {
        NovelModel::count(conn)
    }
}

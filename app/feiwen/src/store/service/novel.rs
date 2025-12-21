use diesel::SqliteConnection;
use gpui::{IntoElement, ParentElement, RenderOnce, Styled, div};
use gpui_component::{ActiveTheme, StyledExt, label::Label};
use std::collections::{HashMap, HashSet};

use crate::{
    errors::{FeiwenError, FeiwenResult},
    store::{
        model::{NovelModel, NovelTagModel, TagModel},
        types::{Author, NovelCount, Title},
    },
};

use super::Tag;

#[derive(Debug, Clone, IntoElement)]
pub(crate) struct Novel {
    pub(crate) title: Title,
    pub(crate) author: Author,
    pub(crate) latest_chapter: Title,
    pub(crate) desc: String,
    pub(crate) count: NovelCount,
    pub(crate) tags: HashSet<Tag>,
    pub(crate) is_limit: bool,
}

impl RenderOnce for Novel {
    fn render(self, _window: &mut gpui::Window, cx: &mut gpui::App) -> impl gpui::IntoElement {
        div()
            .child(Label::new(self.title.name).text_lg())
            .child(match self.author {
                Author::Anonymous(name) => div().child(Label::new(name)),
                Author::Known(title) => div().child(Label::new(title.name)),
            })
            .child(
                Label::new(self.desc)
                    .font_light()
                    .text_color(cx.theme().secondary_foreground),
            )
            .child(div().flex().gap_2().children(self.tags))
    }
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
    pub(crate) fn search(
        query: &str,
        tags: &HashSet<String>,
        conn: &mut SqliteConnection,
    ) -> FeiwenResult<Vec<Novel>> {
        let all_novels = NovelModel::query(conn)?;
        let all_tags = TagModel::all_tags(conn)?
            .into_iter()
            .filter_map(|TagModel { id, name }| id.map(|id| (name, id)))
            .collect::<HashMap<_, _>>();
        let all_novel_tags = NovelTagModel::get_all(conn)?.into_iter().fold(
            HashMap::new(),
            |mut map, NovelTagModel { novel_id, tag_id }| {
                map.entry(novel_id)
                    .or_insert_with(HashSet::new)
                    .insert(tag_id);
                map
            },
        );
        let data = all_novels
            .into_iter()
            .filter(
                |NovelModel {
                     author_name, name, ..
                 }| author_name.contains(query) || name.contains(query),
            )
            .filter_map(
                |NovelModel {
                     id,
                     name,
                     desc,
                     is_limit,
                     latest_chapter_name,
                     latest_chapter_id,
                     word_count,
                     read_count,
                     reply_count,
                     author_id,
                     author_name,
                 }| {
                    if let Some(novel_tags) = all_novel_tags.get(&id)
                        && novel_tags.is_superset(tags)
                    {
                        let author = match author_id {
                            Some(author_id) => Author::Known(Title {
                                name: author_name,
                                id: author_id,
                            }),
                            None => Author::Anonymous(author_name),
                        };
                        let title = Title { name, id };
                        let latest_chapter = Title {
                            name: latest_chapter_name,
                            id: latest_chapter_id,
                        };
                        let count = NovelCount {
                            word_count,
                            read_count,
                            reply_count,
                        };
                        let tags = novel_tags
                            .iter()
                            .map(|tag_name| Tag {
                                name: tag_name.clone(),
                                id: all_tags.get(tag_name).copied(),
                            })
                            .collect::<HashSet<_>>();
                        Some(Novel {
                            desc,
                            is_limit,
                            title,
                            author,
                            latest_chapter,
                            count,
                            tags,
                        })
                    } else {
                        None
                    }
                },
            )
            .collect();
        Ok(data)
    }
}

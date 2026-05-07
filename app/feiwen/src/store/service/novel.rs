use diesel::SqliteConnection;
use gpui::{IntoElement, ParentElement, RenderOnce, Styled, div};
use gpui_component::{ActiveTheme, StyledExt, label::Label};
use std::collections::{HashMap, HashSet};

use crate::{
    errors::{FeiwenError, FeiwenResult},
    store::{
        model::{NovelModel, NovelTagModel, TagModel},
        query::{FilterExpr, NovelRecord, Predicate, QuerySpec, TagsPredicate, TextField, TextOp},
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
    #[allow(dead_code)]
    pub(crate) fn search(
        query: &str,
        tags: &HashSet<String>,
        conn: &mut SqliteConnection,
    ) -> FeiwenResult<Vec<Novel>> {
        let mut filters = Vec::new();
        if !query.is_empty() {
            filters.push(FilterExpr::Any(vec![
                FilterExpr::Predicate(Predicate::Text {
                    field: TextField::Title,
                    op: TextOp::Contains,
                    value: query.to_owned(),
                }),
                FilterExpr::Predicate(Predicate::Text {
                    field: TextField::AuthorName,
                    op: TextOp::Contains,
                    value: query.to_owned(),
                }),
            ]));
        }
        if !tags.is_empty() {
            filters.push(FilterExpr::Predicate(Predicate::Tags(
                TagsPredicate::ContainsAll(tags.clone()),
            )));
        }
        Self::query(
            &QuerySpec {
                filter: FilterExpr::All(filters),
                sorts: Vec::new(),
            },
            conn,
        )
    }

    pub(crate) fn query(spec: &QuerySpec, conn: &mut SqliteConnection) -> FeiwenResult<Vec<Novel>> {
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
        let records = all_novels
            .into_iter()
            .map(|novel| novel_record(novel, &all_novel_tags))
            .collect::<Vec<_>>();
        let data = spec
            .apply(records)
            .into_iter()
            .map(|record| record.into_novel(&all_tags))
            .collect();
        Ok(data)
    }
}

fn novel_record(novel: NovelModel, all_novel_tags: &HashMap<i32, HashSet<String>>) -> NovelRecord {
    let NovelModel {
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
    } = novel;

    NovelRecord {
        id,
        title: name,
        desc,
        is_limit,
        latest_chapter_name,
        latest_chapter_id,
        word_count,
        read_count,
        reply_count,
        author_id,
        author_name,
        tags: all_novel_tags.get(&id).cloned().unwrap_or_default(),
    }
}

impl NovelRecord {
    fn into_novel(self, all_tags: &HashMap<String, i32>) -> Novel {
        let author = match self.author_id {
            Some(author_id) => Author::Known(Title {
                name: self.author_name,
                id: author_id,
            }),
            None => Author::Anonymous(self.author_name),
        };
        let tags = self
            .tags
            .iter()
            .map(|tag_name| Tag {
                name: tag_name.clone(),
                id: all_tags.get(tag_name).copied(),
            })
            .collect::<HashSet<_>>();

        Novel {
            desc: self.desc,
            is_limit: self.is_limit,
            title: Title {
                name: self.title,
                id: self.id,
            },
            author,
            latest_chapter: Title {
                name: self.latest_chapter_name,
                id: self.latest_chapter_id,
            },
            count: NovelCount {
                word_count: self.word_count,
                read_count: self.read_count,
                reply_count: self.reply_count,
            },
            tags,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::{Connection, connection::SimpleConnection};

    fn connection() -> SqliteConnection {
        let mut conn = SqliteConnection::establish(":memory:").unwrap();
        conn.batch_execute(include_str!(
            "../../../migrations/2022-05-15-162950_novel/up.sql"
        ))
        .unwrap();
        conn.batch_execute(include_str!(
            "../../../migrations/2022-05-15-163112_tag/up.sql"
        ))
        .unwrap();
        conn.batch_execute(include_str!(
            "../../../migrations/2022-05-16-064913_novel_tag/up.sql"
        ))
        .unwrap();
        conn
    }

    fn novel(id: i32, title: &str, author_name: &str, tags: &[&str]) -> Novel {
        Novel {
            title: Title {
                name: title.to_owned(),
                id,
            },
            author: Author::Known(Title {
                name: author_name.to_owned(),
                id,
            }),
            latest_chapter: Title {
                name: "chapter".to_owned(),
                id: id * 10,
            },
            desc: "desc".to_owned(),
            count: NovelCount {
                word_count: 1000,
                read_count: Some(100),
                reply_count: Some(10),
            },
            tags: tags
                .iter()
                .map(|name| Tag {
                    name: (*name).to_owned(),
                    id: Some(id),
                })
                .collect(),
            is_limit: false,
        }
    }

    #[test]
    fn search_keeps_quick_query_and_selected_tags_compatibility() {
        let mut conn = connection();
        novel(1, "Rust 入门", "张三", &["rust", "systems"])
            .save(&mut conn)
            .unwrap();
        novel(2, "Python 入门", "Rust 作者", &["python"])
            .save(&mut conn)
            .unwrap();
        novel(3, "Rust 进阶", "李四", &["rust"])
            .save(&mut conn)
            .unwrap();

        let tags = HashSet::from(["systems".to_owned()]);
        let results = Novel::search("Rust", &tags, &mut conn).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title.id, 1);
    }
}

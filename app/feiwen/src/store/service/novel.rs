use duckdb::{Connection, Error as DuckdbError, params};
use gpui::{IntoElement, ParentElement, RenderOnce, Styled, div};
use gpui_component::{ActiveTheme, StyledExt, label::Label};

use crate::{
    errors::FeiwenResult,
    store::{
        query::{NovelRecord, QuerySpec, query_records},
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
    pub(crate) tags: std::collections::HashSet<Tag>,
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
    pub(crate) fn save(self, conn: &mut Connection) -> FeiwenResult<()> {
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
                self.title.name,
                self.desc,
                self.is_limit,
                self.latest_chapter.name,
                self.latest_chapter.id,
                self.count.word_count,
                read_count,
                reply_count,
                author_id,
                author_name,
            ],
        )?;

        for tag in &self.tags {
            tx.execute(
                "INSERT INTO tag (id, name) VALUES (?, ?) ON CONFLICT DO NOTHING",
                params![tag.id, tag.name],
            )?;
            tx.execute(
                "INSERT INTO novel_tag (novel_id, tag_id) VALUES (?, ?) ON CONFLICT DO NOTHING",
                params![self.title.id, tag.name],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    pub(crate) fn count(conn: &Connection) -> FeiwenResult<i64> {
        conn.query_row("SELECT count(*) FROM novel", [], |row| row.get(0))
            .map_err(Into::into)
    }

    pub(crate) fn query(spec: &QuerySpec, conn: &Connection) -> FeiwenResult<Vec<Novel>> {
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
                name: self.author_name,
                id: author_id,
            }),
            None => Author::Anonymous(self.author_name),
        };

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
            tags: self
                .tags
                .into_iter()
                .map(|name| {
                    let id = self.tag_ids.get(&name).copied().flatten();
                    Tag { name, id }
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::store::{
        initialize_schema,
        query::{
            AuthorPredicate, AuthorRef, BoolField, FilterExpr, NumberField, Predicate,
            SortDirection, SortExpr, SortSpec, TagsPredicate, TextField, TextOp,
        },
    };

    fn connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize_schema(&conn).unwrap();
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

    fn save_novel(
        conn: &mut Connection,
        id: i32,
        title: &str,
        tags: &[&str],
        is_limit: bool,
        reply_count: Option<i32>,
    ) {
        let mut novel = novel(id, title, &format!("author-{id}"), tags);
        novel.is_limit = is_limit;
        novel.count.reply_count = reply_count;
        novel.save(conn).unwrap();
    }

    fn sorted_ids(results: Vec<Novel>) -> Vec<i32> {
        results.into_iter().map(|novel| novel.title.id).collect()
    }

    #[test]
    fn save_upserts_and_preserves_existing_counts_when_missing() {
        let mut conn = connection();
        save_novel(&mut conn, 1, "first", &["rust"], false, Some(10));

        let mut updated = novel(1, "updated", "author-1", &["rust", "gpui"]);
        updated.count.read_count = None;
        updated.count.reply_count = None;
        updated.save(&mut conn).unwrap();

        let (name, read_count, reply_count) = conn
            .query_row(
                "SELECT name, read_count, reply_count FROM novel WHERE id = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<i32>>(1)?,
                        row.get::<_, Option<i32>>(2)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(name, "updated");
        assert_eq!(read_count, Some(100));
        assert_eq!(reply_count, Some(10));

        let tags = Novel::query(
            &QuerySpec {
                filter: FilterExpr::default(),
                sorts: Vec::new(),
            },
            &conn,
        )
        .unwrap()
        .remove(0)
        .tags;
        assert!(tags.iter().any(|tag| tag.name == "rust"));
        assert!(tags.iter().any(|tag| tag.name == "gpui"));
    }

    #[test]
    fn query_pushes_tag_limit_and_reply_sort_semantics() {
        let mut conn = connection();
        save_novel(
            &mut conn,
            1,
            "match low",
            &["BL", "年下", "完结"],
            true,
            Some(10),
        );
        save_novel(
            &mut conn,
            2,
            "match high",
            &["BL", "年下", "完结"],
            true,
            Some(30),
        );
        save_novel(
            &mut conn,
            3,
            "match missing",
            &["BL", "年下", "完结"],
            true,
            None,
        );
        save_novel(
            &mut conn,
            4,
            "not limited",
            &["BL", "年下", "完结"],
            false,
            Some(99),
        );
        save_novel(&mut conn, 5, "missing tag", &["BL", "完结"], true, Some(88));

        let spec = QuerySpec {
            filter: FilterExpr::All(vec![
                FilterExpr::Predicate(Predicate::Tags(TagsPredicate::ContainsAll(HashSet::from(
                    ["BL".to_owned(), "年下".to_owned(), "完结".to_owned()],
                )))),
                FilterExpr::Predicate(Predicate::Bool {
                    field: BoolField::IsLimit,
                    value: true,
                }),
            ]),
            sorts: vec![SortSpec {
                expr: SortExpr::Number(NumberField::ReplyCount),
                direction: SortDirection::Desc,
            }],
        };

        let results = Novel::query(&spec, &conn).unwrap();

        assert_eq!(sorted_ids(results.clone()), vec![2, 1, 3]);
        assert!(
            results[0]
                .tags
                .iter()
                .any(|tag| tag.name == "BL" && tag.id == Some(1))
        );
    }

    #[test]
    fn query_loads_tags_for_large_result_sets() {
        let mut conn = connection();
        let total = 501;
        for id in 1..=total {
            let tag = format!("tag-{id}");
            save_novel(
                &mut conn,
                id as i32,
                &format!("novel-{id}"),
                &[tag.as_str()],
                false,
                Some(id as i32),
            );
        }

        let results = Novel::query(
            &QuerySpec {
                filter: FilterExpr::default(),
                sorts: vec![SortSpec {
                    expr: SortExpr::Number(NumberField::NovelId),
                    direction: SortDirection::Asc,
                }],
            },
            &conn,
        )
        .unwrap();

        assert_eq!(results.len(), total);
        for id in 1..=total {
            assert!(
                results[id - 1]
                    .tags
                    .iter()
                    .any(|tag| tag.name == format!("tag-{id}"))
            );
            assert!(
                results[id - 1]
                    .tags
                    .iter()
                    .any(|tag| tag.name == format!("tag-{id}") && tag.id == Some(id as i32))
            );
        }
    }

    #[test]
    fn query_tag_set_relations_match_current_semantics() {
        let mut conn = connection();
        save_novel(&mut conn, 1, "empty", &[], false, Some(1));
        save_novel(&mut conn, 2, "rust", &["rust"], false, Some(2));
        save_novel(&mut conn, 3, "rust gpui", &["rust", "gpui"], false, Some(3));

        let query_ids = |predicate: TagsPredicate, conn: &Connection| {
            let spec = QuerySpec {
                filter: FilterExpr::Predicate(Predicate::Tags(predicate)),
                sorts: vec![SortSpec {
                    expr: SortExpr::Number(NumberField::NovelId),
                    direction: SortDirection::Asc,
                }],
            };
            sorted_ids(Novel::query(&spec, conn).unwrap())
        };

        assert_eq!(
            query_ids(
                TagsPredicate::Intersects(HashSet::from(["gpui".to_owned()])),
                &conn
            ),
            vec![3]
        );
        assert_eq!(
            query_ids(
                TagsPredicate::ContainsAll(HashSet::from(["rust".to_owned()])),
                &conn
            ),
            vec![2, 3]
        );
        assert_eq!(
            query_ids(
                TagsPredicate::ContainedBy(HashSet::from(["rust".to_owned()])),
                &conn
            ),
            vec![1, 2]
        );
        assert_eq!(
            query_ids(
                TagsPredicate::Equals(HashSet::from(["rust".to_owned(), "gpui".to_owned()])),
                &conn
            ),
            vec![3]
        );
        assert_eq!(
            query_ids(TagsPredicate::Equals(HashSet::new()), &conn),
            vec![1]
        );
        assert_eq!(query_ids(TagsPredicate::IsEmpty, &conn), vec![1]);
        assert_eq!(query_ids(TagsPredicate::IsNotEmpty, &conn), vec![2, 3]);
    }

    #[test]
    fn query_or_and_not_tag_filters_keep_full_result_sets() {
        let mut conn = connection();
        save_novel(&mut conn, 1, "rare", &["rare"], false, Some(1));
        save_novel(&mut conn, 2, "other", &["other"], false, Some(2));

        let spec = QuerySpec {
            filter: FilterExpr::Any(vec![
                FilterExpr::Predicate(Predicate::Tags(TagsPredicate::ContainsAll(HashSet::from(
                    ["rare".to_owned()],
                )))),
                FilterExpr::Predicate(Predicate::Text {
                    field: TextField::Title,
                    op: TextOp::Equals,
                    value: "other".to_owned(),
                }),
            ]),
            sorts: vec![SortSpec {
                expr: SortExpr::Number(NumberField::NovelId),
                direction: SortDirection::Asc,
            }],
        };

        assert_eq!(sorted_ids(Novel::query(&spec, &conn).unwrap()), vec![1, 2]);

        let spec = QuerySpec {
            filter: FilterExpr::Not(Box::new(FilterExpr::Predicate(Predicate::Tags(
                TagsPredicate::ContainsAll(HashSet::from(["rare".to_owned()])),
            )))),
            sorts: vec![SortSpec {
                expr: SortExpr::Number(NumberField::NovelId),
                direction: SortDirection::Asc,
            }],
        };

        assert_eq!(sorted_ids(Novel::query(&spec, &conn).unwrap()), vec![2]);
    }

    #[test]
    fn query_author_not_in_keeps_anonymous_authors_when_ids_do_not_match() {
        let mut conn = connection();
        novel(1, "known", "known-author", &["tag"])
            .save(&mut conn)
            .unwrap();
        let mut anonymous = novel(2, "anonymous", "anonymous-author", &["tag"]);
        anonymous.author = Author::Anonymous("anonymous-author".to_owned());
        anonymous.save(&mut conn).unwrap();

        let spec = QuerySpec {
            filter: FilterExpr::Predicate(Predicate::Author(AuthorPredicate::NotIn(vec![
                AuthorRef::Id(1),
            ]))),
            sorts: vec![SortSpec {
                expr: SortExpr::Number(NumberField::NovelId),
                direction: SortDirection::Asc,
            }],
        };

        assert_eq!(sorted_ids(Novel::query(&spec, &conn).unwrap()), vec![2]);
    }

    #[test]
    fn query_treats_text_author_and_tag_values_as_plain_values() {
        let mut conn = connection();
        let injection = "' OR 1=1 --";
        save_novel(&mut conn, 1, "safe", &["tag"], false, Some(1));
        save_novel(&mut conn, 2, injection, &["tag', 1) --"], false, Some(2));

        let spec = QuerySpec {
            filter: FilterExpr::Predicate(Predicate::Text {
                field: TextField::Title,
                op: TextOp::Equals,
                value: injection.to_owned(),
            }),
            sorts: Vec::new(),
        };
        assert_eq!(sorted_ids(Novel::query(&spec, &conn).unwrap()), vec![2]);

        let spec = QuerySpec {
            filter: FilterExpr::Predicate(Predicate::Tags(TagsPredicate::Intersects(
                HashSet::from(["tag', 1) --".to_owned()]),
            ))),
            sorts: Vec::new(),
        };
        assert_eq!(sorted_ids(Novel::query(&spec, &conn).unwrap()), vec![2]);

        let mut anonymous = novel(3, "anonymous", "author' OR 1=1 --", &["tag"]);
        anonymous.author = Author::Anonymous("author' OR 1=1 --".to_owned());
        anonymous.save(&mut conn).unwrap();

        let spec = QuerySpec {
            filter: FilterExpr::Predicate(Predicate::Author(AuthorPredicate::Is(AuthorRef::Name(
                "author' OR 1=1 --".to_owned(),
            )))),
            sorts: Vec::new(),
        };
        assert_eq!(sorted_ids(Novel::query(&spec, &conn).unwrap()), vec![3]);
    }
}

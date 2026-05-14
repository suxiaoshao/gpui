use diesel::{QueryableByName, RunQueryDsl, SqliteConnection, sql_types};
use gpui::{IntoElement, ParentElement, RenderOnce, Styled, div};
use gpui_component::{ActiveTheme, StyledExt, label::Label};
use std::collections::{HashMap, HashSet};

use crate::{
    errors::{FeiwenError, FeiwenResult},
    store::{
        model::{NovelModel, NovelTagModel, TagModel},
        query::{NovelQueryPlan, QuerySpec, QuerySql},
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

#[derive(QueryableByName)]
struct NovelRow {
    #[diesel(sql_type = sql_types::Integer)]
    id: i32,
    #[diesel(sql_type = sql_types::Text)]
    name: String,
    #[diesel(sql_type = sql_types::Text)]
    desc: String,
    #[diesel(sql_type = sql_types::Bool)]
    is_limit: bool,
    #[diesel(sql_type = sql_types::Text)]
    latest_chapter_name: String,
    #[diesel(sql_type = sql_types::Integer)]
    latest_chapter_id: i32,
    #[diesel(sql_type = sql_types::Integer)]
    word_count: i32,
    #[diesel(sql_type = sql_types::Nullable<sql_types::Integer>)]
    read_count: Option<i32>,
    #[diesel(sql_type = sql_types::Nullable<sql_types::Integer>)]
    reply_count: Option<i32>,
    #[diesel(sql_type = sql_types::Nullable<sql_types::Integer>)]
    author_id: Option<i32>,
    #[diesel(sql_type = sql_types::Text)]
    author_name: String,
}

#[derive(QueryableByName)]
struct NovelTagRow {
    #[diesel(sql_type = sql_types::Integer)]
    novel_id: i32,
    #[diesel(sql_type = sql_types::Text)]
    name: String,
    #[diesel(sql_type = sql_types::Nullable<sql_types::Integer>)]
    id: Option<i32>,
}

#[derive(QueryableByName)]
struct TagCountRow {
    #[diesel(sql_type = sql_types::Text)]
    tag_id: String,
    #[diesel(sql_type = sql_types::BigInt)]
    tag_count: i64,
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

    pub(crate) fn query(spec: &QuerySpec, conn: &mut SqliteConnection) -> FeiwenResult<Vec<Novel>> {
        let rows = query_novel_rows(spec, conn)?;
        let tags = load_tags_for_rows(&rows, conn)?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let row_tags = tags.get(&row.id).cloned().unwrap_or_default();
                row.into_novel(row_tags)
            })
            .collect())
    }
}

fn query_novel_rows(spec: &QuerySpec, conn: &mut SqliteConnection) -> FeiwenResult<Vec<NovelRow>> {
    let plan = query_plan(spec, conn)?;
    Ok(spec
        .query_sql_with_plan("n", &plan)
        .load::<NovelRow>(conn)?)
}

fn query_plan(spec: &QuerySpec, conn: &mut SqliteConnection) -> FeiwenResult<NovelQueryPlan> {
    let candidates = spec.tag_anchor_candidates();
    if candidates.is_empty() {
        return Ok(NovelQueryPlan::default());
    }

    let tag_counts = load_tag_counts(&candidates, conn)?;
    Ok(NovelQueryPlan::from_tag_counts(candidates, &tag_counts))
}

fn load_tag_counts(
    tags: &HashSet<String>,
    conn: &mut SqliteConnection,
) -> FeiwenResult<HashMap<String, i64>> {
    if tags.is_empty() {
        return Ok(HashMap::new());
    }

    let mut sql = QuerySql::new();
    sql.push_sql(
        "\
        SELECT tag_id, COUNT(*) AS tag_count \
        FROM novel_tag \
        WHERE tag_id IN (",
    );
    let mut tags = tags.iter().collect::<Vec<_>>();
    tags.sort();
    for (ix, tag) in tags.into_iter().enumerate() {
        if ix > 0 {
            sql.push_sql(", ");
        }
        sql.push_text(tag);
    }
    sql.push_sql(") GROUP BY tag_id");

    Ok(sql
        .load::<TagCountRow>(conn)?
        .into_iter()
        .map(|row| (row.tag_id, row.tag_count))
        .collect())
}

fn load_tags_for_rows(
    rows: &[NovelRow],
    conn: &mut SqliteConnection,
) -> FeiwenResult<HashMap<i32, HashSet<Tag>>> {
    if rows.is_empty() {
        return Ok(HashMap::new());
    }

    let mut sql = QuerySql::new();
    sql.push_sql(
        "\
        SELECT \
            nt.novel_id, \
            nt.tag_id AS name, \
            tag.id \
        FROM novel_tag nt \
        LEFT JOIN tag ON tag.name = nt.tag_id \
        WHERE nt.novel_id IN (",
    );
    for (ix, row) in rows.iter().enumerate() {
        if ix > 0 {
            sql.push_sql(", ");
        }
        sql.push_i32(row.id);
    }
    sql.push_sql(")");

    let rows = sql.load::<NovelTagRow>(conn)?;
    let mut tags = HashMap::new();
    for row in rows {
        tags.entry(row.novel_id)
            .or_insert_with(HashSet::new)
            .insert(Tag {
                name: row.name,
                id: row.id,
            });
    }
    Ok(tags)
}

impl NovelRow {
    fn into_novel(self, tags: HashSet<Tag>) -> Novel {
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
                name: self.name,
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
    use crate::store::query::{
        AuthorPredicate, AuthorRef, FilterExpr, Predicate, TagsPredicate, TextField, TextOp,
    };
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
        conn.batch_execute(include_str!(
            "../../../migrations/2026-05-06-000001_nullable_novel_counts/up.sql"
        ))
        .unwrap();
        conn.batch_execute(include_str!(
            "../../../migrations/2026-05-13-000001_query_indexes/up.sql"
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

    fn save_novel(
        conn: &mut SqliteConnection,
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
                    field: crate::store::query::BoolField::IsLimit,
                    value: true,
                }),
            ]),
            sorts: vec![crate::store::query::SortSpec {
                expr: crate::store::query::SortExpr::Number(
                    crate::store::query::NumberField::ReplyCount,
                ),
                direction: crate::store::query::SortDirection::Desc,
            }],
        };

        let results = Novel::query(&spec, &mut conn).unwrap();

        assert_eq!(
            results
                .iter()
                .map(|novel| novel.title.id)
                .collect::<Vec<_>>(),
            vec![2, 1, 3]
        );
        assert!(
            results[0]
                .tags
                .iter()
                .any(|tag| tag.name == "BL" && tag.id == Some(1))
        );
    }

    #[test]
    fn query_tag_set_relations_match_current_semantics() {
        let mut conn = connection();
        save_novel(&mut conn, 1, "empty", &[], false, Some(1));
        save_novel(&mut conn, 2, "rust", &["rust"], false, Some(2));
        save_novel(&mut conn, 3, "rust gpui", &["rust", "gpui"], false, Some(3));

        let query_ids = |predicate: TagsPredicate, conn: &mut SqliteConnection| {
            let spec = QuerySpec {
                filter: FilterExpr::Predicate(Predicate::Tags(predicate)),
                sorts: vec![crate::store::query::SortSpec {
                    expr: crate::store::query::SortExpr::Number(
                        crate::store::query::NumberField::NovelId,
                    ),
                    direction: crate::store::query::SortDirection::Asc,
                }],
            };
            Novel::query(&spec, conn)
                .unwrap()
                .into_iter()
                .map(|novel| novel.title.id)
                .collect::<Vec<_>>()
        };

        assert_eq!(
            query_ids(
                TagsPredicate::Intersects(HashSet::from(["gpui".to_owned()])),
                &mut conn
            ),
            vec![3]
        );
        assert_eq!(
            query_ids(
                TagsPredicate::ContainsAll(HashSet::from(["rust".to_owned()])),
                &mut conn
            ),
            vec![2, 3]
        );
        assert_eq!(
            query_ids(
                TagsPredicate::ContainedBy(HashSet::from(["rust".to_owned()])),
                &mut conn
            ),
            vec![1, 2]
        );
        assert_eq!(
            query_ids(
                TagsPredicate::Equals(HashSet::from(["rust".to_owned(), "gpui".to_owned()])),
                &mut conn
            ),
            vec![3]
        );
        assert_eq!(
            query_ids(TagsPredicate::Equals(HashSet::new()), &mut conn),
            vec![1]
        );
        assert_eq!(query_ids(TagsPredicate::IsEmpty, &mut conn), vec![1]);
        assert_eq!(query_ids(TagsPredicate::IsNotEmpty, &mut conn), vec![2, 3]);
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
            sorts: vec![crate::store::query::SortSpec {
                expr: crate::store::query::SortExpr::Number(
                    crate::store::query::NumberField::NovelId,
                ),
                direction: crate::store::query::SortDirection::Asc,
            }],
        };

        let results = Novel::query(&spec, &mut conn).unwrap();
        assert_eq!(
            results
                .iter()
                .map(|novel| novel.title.id)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );

        let spec = QuerySpec {
            filter: FilterExpr::Not(Box::new(FilterExpr::Predicate(Predicate::Tags(
                TagsPredicate::ContainsAll(HashSet::from(["rare".to_owned()])),
            )))),
            sorts: vec![crate::store::query::SortSpec {
                expr: crate::store::query::SortExpr::Number(
                    crate::store::query::NumberField::NovelId,
                ),
                direction: crate::store::query::SortDirection::Asc,
            }],
        };

        let results = Novel::query(&spec, &mut conn).unwrap();
        assert_eq!(
            results
                .iter()
                .map(|novel| novel.title.id)
                .collect::<Vec<_>>(),
            vec![2]
        );
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
            filter: FilterExpr::Predicate(Predicate::Author(
                crate::store::query::AuthorPredicate::NotIn(vec![
                    crate::store::query::AuthorRef::Id(1),
                ]),
            )),
            sorts: vec![crate::store::query::SortSpec {
                expr: crate::store::query::SortExpr::Number(
                    crate::store::query::NumberField::NovelId,
                ),
                direction: crate::store::query::SortDirection::Asc,
            }],
        };

        let results = Novel::query(&spec, &mut conn).unwrap();

        assert_eq!(
            results
                .into_iter()
                .map(|novel| novel.title.id)
                .collect::<Vec<_>>(),
            vec![2]
        );
    }

    #[test]
    fn query_binds_text_author_and_tag_values_without_injection() {
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
        let results = Novel::query(&spec, &mut conn).unwrap();
        assert_eq!(
            results
                .iter()
                .map(|novel| novel.title.id)
                .collect::<Vec<_>>(),
            vec![2]
        );

        let spec = QuerySpec {
            filter: FilterExpr::Predicate(Predicate::Tags(TagsPredicate::Intersects(
                HashSet::from(["tag', 1) --".to_owned()]),
            ))),
            sorts: Vec::new(),
        };
        let results = Novel::query(&spec, &mut conn).unwrap();
        assert_eq!(
            results
                .iter()
                .map(|novel| novel.title.id)
                .collect::<Vec<_>>(),
            vec![2]
        );

        let mut anonymous = novel(3, "anonymous", "author' OR 1=1 --", &["tag"]);
        anonymous.author = Author::Anonymous("author' OR 1=1 --".to_owned());
        anonymous.save(&mut conn).unwrap();

        let spec = QuerySpec {
            filter: FilterExpr::Predicate(Predicate::Author(AuthorPredicate::Is(AuthorRef::Name(
                "author' OR 1=1 --".to_owned(),
            )))),
            sorts: Vec::new(),
        };
        let results = Novel::query(&spec, &mut conn).unwrap();
        assert_eq!(
            results
                .iter()
                .map(|novel| novel.title.id)
                .collect::<Vec<_>>(),
            vec![3]
        );
    }
}

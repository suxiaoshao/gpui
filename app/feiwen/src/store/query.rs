use std::collections::{HashMap, HashSet};

use polars::prelude::*;

const ID: &str = "id";
const TITLE: &str = "name";
const DESCRIPTION: &str = "desc";
const IS_LIMIT: &str = "is_limit";
const LATEST_CHAPTER_NAME: &str = "latest_chapter_name";
const LATEST_CHAPTER_ID: &str = "latest_chapter_id";
const WORD_COUNT: &str = "word_count";
const READ_COUNT: &str = "read_count";
const REPLY_COUNT: &str = "reply_count";
const AUTHOR_ID: &str = "author_id";
const AUTHOR_NAME: &str = "author_name";
const TAGS: &str = "tags";

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct QuerySpec {
    pub(crate) filter: FilterExpr,
    pub(crate) sorts: Vec<SortSpec>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FilterExpr {
    All(Vec<FilterExpr>),
    Any(Vec<FilterExpr>),
    Not(Box<FilterExpr>),
    Predicate(Predicate),
}

impl Default for FilterExpr {
    fn default() -> Self {
        Self::All(Vec::new())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Predicate {
    Text {
        field: TextField,
        op: TextOp,
        value: String,
    },
    Number {
        field: NumberField,
        op: NumberOp,
    },
    Bool {
        field: BoolField,
        value: bool,
    },
    Tags(TagsPredicate),
    Author(AuthorPredicate),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextField {
    Title,
    Description,
    LatestChapter,
    AuthorName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextOp {
    Contains,
    StartsWith,
    EndsWith,
    Equals,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NumberField {
    NovelId,
    LatestChapterId,
    WordCount,
    ReadCount,
    ReplyCount,
    AuthorId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NumberOp {
    Eq(i32),
    Ne(i32),
    Lt(i32),
    Lte(i32),
    Gt(i32),
    Gte(i32),
    Between { min: i32, max: i32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BoolField {
    IsLimit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TagsPredicate {
    Intersects(HashSet<String>),
    ContainsAll(HashSet<String>),
    ContainedBy(HashSet<String>),
    Equals(HashSet<String>),
    IsEmpty,
    IsNotEmpty,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum AuthorRef {
    Id(i32),
    Name(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AuthorPredicate {
    Is(AuthorRef),
    In(Vec<AuthorRef>),
    NotIn(Vec<AuthorRef>),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SortSpec {
    pub(crate) expr: SortExpr,
    pub(crate) direction: SortDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SortDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SortExpr {
    Number(NumberField),
    Text(TextField),
    Bool(BoolField),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NovelRecord {
    pub(crate) id: i32,
    pub(crate) title: String,
    pub(crate) desc: String,
    pub(crate) is_limit: bool,
    pub(crate) latest_chapter_name: String,
    pub(crate) latest_chapter_id: i32,
    pub(crate) word_count: i32,
    pub(crate) read_count: Option<i32>,
    pub(crate) reply_count: Option<i32>,
    pub(crate) author_id: Option<i32>,
    pub(crate) author_name: String,
    pub(crate) tags: Vec<String>,
    pub(crate) tag_ids: HashMap<String, Option<i32>>,
}

#[derive(Debug, Clone)]
pub(crate) struct NovelQueryDataset {
    frame: DataFrame,
    records: HashMap<i32, NovelRecord>,
}

impl NovelQueryDataset {
    pub(crate) fn new(records: Vec<NovelRecord>) -> PolarsResult<Self> {
        let mut ids = Vec::with_capacity(records.len());
        let mut titles = Vec::with_capacity(records.len());
        let mut descriptions = Vec::with_capacity(records.len());
        let mut is_limits = Vec::with_capacity(records.len());
        let mut latest_chapter_names = Vec::with_capacity(records.len());
        let mut latest_chapter_ids = Vec::with_capacity(records.len());
        let mut word_counts = Vec::with_capacity(records.len());
        let mut read_counts = Vec::with_capacity(records.len());
        let mut reply_counts = Vec::with_capacity(records.len());
        let mut author_ids = Vec::with_capacity(records.len());
        let mut author_names = Vec::with_capacity(records.len());
        let mut tag_lists = Vec::with_capacity(records.len());
        let mut by_id = HashMap::with_capacity(records.len());

        for record in records {
            ids.push(record.id);
            titles.push(record.title.clone());
            descriptions.push(record.desc.clone());
            is_limits.push(record.is_limit);
            latest_chapter_names.push(record.latest_chapter_name.clone());
            latest_chapter_ids.push(record.latest_chapter_id);
            word_counts.push(record.word_count);
            read_counts.push(record.read_count);
            reply_counts.push(record.reply_count);
            author_ids.push(record.author_id);
            author_names.push(record.author_name.clone());
            tag_lists.push(Series::new(PlSmallStr::EMPTY, record.tags.as_slice()));
            by_id.insert(record.id, record);
        }

        let tags: ListChunked = tag_lists.into_iter().map(Some).collect();
        let mut tags = tags.into_series();
        tags.rename(TAGS.into());

        let height = by_id.len();
        let frame = DataFrame::new(
            height,
            vec![
                Series::new(ID.into(), ids).into(),
                Series::new(TITLE.into(), titles).into(),
                Series::new(DESCRIPTION.into(), descriptions).into(),
                Series::new(IS_LIMIT.into(), is_limits).into(),
                Series::new(LATEST_CHAPTER_NAME.into(), latest_chapter_names).into(),
                Series::new(LATEST_CHAPTER_ID.into(), latest_chapter_ids).into(),
                Series::new(WORD_COUNT.into(), word_counts).into(),
                Series::new(READ_COUNT.into(), read_counts).into(),
                Series::new(REPLY_COUNT.into(), reply_counts).into(),
                Series::new(AUTHOR_ID.into(), author_ids).into(),
                Series::new(AUTHOR_NAME.into(), author_names).into(),
                tags.into(),
            ],
        )?;

        Ok(Self {
            frame,
            records: by_id,
        })
    }

    pub(crate) fn query(&self, spec: &QuerySpec) -> PolarsResult<Vec<NovelRecord>> {
        let mut frame = self.frame.clone().lazy().filter(spec.filter.expr()?);
        if !spec.sorts.is_empty() {
            let sort_exprs = spec
                .sorts
                .iter()
                .map(|sort| sort.expr.expr())
                .collect::<Vec<_>>();
            let descending = spec
                .sorts
                .iter()
                .map(|sort| matches!(sort.direction, SortDirection::Desc))
                .collect::<Vec<_>>();
            frame = frame.sort_by_exprs(
                sort_exprs,
                SortMultipleOptions::default()
                    .with_order_descending_multi(descending)
                    .with_nulls_last(true)
                    .with_maintain_order(true),
            );
        }

        let ids = frame.select([col(ID)]).collect()?;
        let ids = ids.column(ID)?.i32()?;
        let mut records = Vec::with_capacity(ids.len());
        for id in ids.into_no_null_iter() {
            if let Some(record) = self.records.get(&id) {
                records.push(record.clone());
            }
        }
        Ok(records)
    }
}

impl QuerySpec {
    pub(crate) fn filter_count(&self) -> usize {
        self.filter.predicate_count()
    }

    pub(crate) fn sort_count(&self) -> usize {
        self.sorts.len()
    }
}

impl FilterExpr {
    fn predicate_count(&self) -> usize {
        match self {
            FilterExpr::All(filters) | FilterExpr::Any(filters) => {
                filters.iter().map(Self::predicate_count).sum()
            }
            FilterExpr::Not(filter) => filter.predicate_count(),
            FilterExpr::Predicate(_) => 1,
        }
    }

    fn expr(&self) -> PolarsResult<Expr> {
        match self {
            FilterExpr::All(filters) => combine_filters(filters, true),
            FilterExpr::Any(filters) => combine_filters(filters, false),
            FilterExpr::Not(filter) => Ok(filter.expr()?.not()),
            FilterExpr::Predicate(predicate) => predicate.expr(),
        }
    }
}

impl Predicate {
    fn expr(&self) -> PolarsResult<Expr> {
        match self {
            Predicate::Text { field, op, value } => {
                let field = col(field.column());
                let value = lit(value.as_str());
                Ok(match op {
                    TextOp::Contains => field.str().contains_literal(value),
                    TextOp::StartsWith => field.str().starts_with(value),
                    TextOp::EndsWith => field.str().ends_with(value),
                    TextOp::Equals => field.eq(value),
                })
            }
            Predicate::Number { field, op } => {
                let field = col(field.column());
                Ok(match op {
                    NumberOp::Eq(value) => field.eq(lit(*value)),
                    NumberOp::Ne(value) => field.neq(lit(*value)),
                    NumberOp::Lt(value) => field.lt(lit(*value)),
                    NumberOp::Lte(value) => field.lt_eq(lit(*value)),
                    NumberOp::Gt(value) => field.gt(lit(*value)),
                    NumberOp::Gte(value) => field.gt_eq(lit(*value)),
                    NumberOp::Between { min, max } => {
                        field.clone().gt_eq(lit(*min)).and(field.lt_eq(lit(*max)))
                    }
                })
            }
            Predicate::Bool { field, value } => Ok(col(field.column()).eq(lit(*value))),
            Predicate::Tags(predicate) => predicate.expr(),
            Predicate::Author(predicate) => predicate.expr(),
        }
    }
}

impl TagsPredicate {
    fn expr(&self) -> PolarsResult<Expr> {
        let tags = col(TAGS);
        Ok(match self {
            TagsPredicate::Intersects(values) => tags
                .list()
                .set_intersection(string_list_lit(values)?)
                .list()
                .len()
                .gt(lit(0u32)),
            TagsPredicate::ContainsAll(values) => string_list_lit(values)?
                .list()
                .set_difference(tags)
                .list()
                .len()
                .eq(lit(0u32)),
            TagsPredicate::ContainedBy(values) => tags
                .list()
                .set_difference(string_list_lit(values)?)
                .list()
                .len()
                .eq(lit(0u32)),
            TagsPredicate::Equals(values) => tags
                .clone()
                .list()
                .set_difference(string_list_lit(values)?)
                .list()
                .len()
                .eq(lit(0u32))
                .and(
                    string_list_lit(values)?
                        .list()
                        .set_difference(tags)
                        .list()
                        .len()
                        .eq(lit(0u32)),
                ),
            TagsPredicate::IsEmpty => tags.list().len().eq(lit(0u32)),
            TagsPredicate::IsNotEmpty => tags.list().len().gt(lit(0u32)),
        })
    }
}

impl AuthorPredicate {
    fn expr(&self) -> PolarsResult<Expr> {
        match self {
            AuthorPredicate::Is(author) => author.expr(),
            AuthorPredicate::In(authors) => combine_authors(authors, false),
            AuthorPredicate::NotIn(authors) => Ok(combine_authors(authors, false)?.not()),
        }
    }
}

impl AuthorRef {
    fn expr(&self) -> PolarsResult<Expr> {
        Ok(match self {
            AuthorRef::Id(id) => col(AUTHOR_ID)
                .is_not_null()
                .and(col(AUTHOR_ID).eq(lit(*id))),
            AuthorRef::Name(name) => col(AUTHOR_ID)
                .is_null()
                .and(col(AUTHOR_NAME).eq(lit(name.as_str()))),
        })
    }
}

impl TextField {
    fn column(self) -> &'static str {
        match self {
            TextField::Title => TITLE,
            TextField::Description => DESCRIPTION,
            TextField::LatestChapter => LATEST_CHAPTER_NAME,
            TextField::AuthorName => AUTHOR_NAME,
        }
    }
}

impl NumberField {
    fn column(self) -> &'static str {
        match self {
            NumberField::NovelId => ID,
            NumberField::LatestChapterId => LATEST_CHAPTER_ID,
            NumberField::WordCount => WORD_COUNT,
            NumberField::ReadCount => READ_COUNT,
            NumberField::ReplyCount => REPLY_COUNT,
            NumberField::AuthorId => AUTHOR_ID,
        }
    }
}

impl BoolField {
    fn column(self) -> &'static str {
        match self {
            BoolField::IsLimit => IS_LIMIT,
        }
    }
}

impl SortExpr {
    fn expr(&self) -> Expr {
        match self {
            SortExpr::Number(field) => col(field.column()),
            SortExpr::Text(field) => col(field.column()),
            SortExpr::Bool(field) => col(field.column()),
        }
    }
}

fn combine_filters(filters: &[FilterExpr], all: bool) -> PolarsResult<Expr> {
    let mut filters = filters.iter();
    let Some(first) = filters.next() else {
        return Ok(lit(all));
    };
    let mut expr = first.expr()?;
    for filter in filters {
        expr = if all {
            expr.and(filter.expr()?)
        } else {
            expr.or(filter.expr()?)
        };
    }
    Ok(expr)
}

fn combine_authors(authors: &[AuthorRef], all: bool) -> PolarsResult<Expr> {
    let mut authors = authors.iter();
    let Some(first) = authors.next() else {
        return Ok(lit(all));
    };
    let mut expr = first.expr()?;
    for author in authors {
        expr = expr.or(author.expr()?);
    }
    Ok(expr)
}

fn string_list_lit(values: &HashSet<String>) -> PolarsResult<Expr> {
    let mut values = values.iter().map(String::as_str).collect::<Vec<_>>();
    values.sort();
    let series = Series::new(PlSmallStr::EMPTY, values);
    Ok(lit(series.implode()?.into_series()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(values: &[&str]) -> HashSet<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    fn record(id: i32) -> NovelRecord {
        NovelRecord {
            id,
            title: format!("Rust novel {id}"),
            desc: "systems programming guide".to_owned(),
            is_limit: false,
            latest_chapter_name: format!("chapter {id}"),
            latest_chapter_id: id * 10,
            word_count: id * 1000,
            read_count: Some(id * 100),
            reply_count: Some(id * 10),
            author_id: Some(id),
            author_name: format!("author-{id}"),
            tags: vec!["rust".to_owned(), "systems".to_owned()],
            tag_ids: HashMap::from([
                ("rust".to_owned(), Some(1)),
                ("systems".to_owned(), Some(2)),
            ]),
        }
    }

    fn query_ids(spec: QuerySpec, records: Vec<NovelRecord>) -> Vec<i32> {
        NovelQueryDataset::new(records)
            .unwrap()
            .query(&spec)
            .unwrap()
            .into_iter()
            .map(|record| record.id)
            .collect()
    }

    #[test]
    fn nested_boolean_filters_match_expected_records() {
        let spec = QuerySpec {
            filter: FilterExpr::All(vec![
                FilterExpr::Any(vec![
                    FilterExpr::Predicate(Predicate::Text {
                        field: TextField::Title,
                        op: TextOp::Contains,
                        value: "novel".to_owned(),
                    }),
                    FilterExpr::Predicate(Predicate::Text {
                        field: TextField::Description,
                        op: TextOp::Contains,
                        value: "missing".to_owned(),
                    }),
                ]),
                FilterExpr::Not(Box::new(FilterExpr::Predicate(Predicate::Bool {
                    field: BoolField::IsLimit,
                    value: true,
                }))),
            ]),
            sorts: Vec::new(),
        };

        let mut limited = record(2);
        limited.is_limit = true;

        assert_eq!(query_ids(spec, vec![record(1), limited]), vec![1]);
        assert_eq!(
            query_ids(
                QuerySpec {
                    filter: FilterExpr::All(Vec::new()),
                    sorts: Vec::new(),
                },
                vec![record(1)]
            ),
            vec![1]
        );
        assert!(
            query_ids(
                QuerySpec {
                    filter: FilterExpr::Any(Vec::new()),
                    sorts: Vec::new(),
                },
                vec![record(1)]
            )
            .is_empty()
        );
    }

    #[test]
    fn text_predicates_cover_all_text_operators() {
        for (op, value) in [
            (TextOp::Contains, "novel"),
            (TextOp::StartsWith, "Rust"),
            (TextOp::EndsWith, "1"),
            (TextOp::Equals, "Rust novel 1"),
        ] {
            let spec = QuerySpec {
                filter: FilterExpr::Predicate(Predicate::Text {
                    field: TextField::Title,
                    op,
                    value: value.to_owned(),
                }),
                sorts: Vec::new(),
            };
            assert_eq!(query_ids(spec, vec![record(1)]), vec![1]);
        }
    }

    #[test]
    fn number_predicates_cover_comparisons_and_missing_values() {
        let mut row = record(2);
        row.read_count = None;

        for op in [
            NumberOp::Eq(2000),
            NumberOp::Ne(1000),
            NumberOp::Lt(3000),
            NumberOp::Lte(2000),
            NumberOp::Gt(1000),
            NumberOp::Gte(2000),
            NumberOp::Between {
                min: 1000,
                max: 3000,
            },
        ] {
            let spec = QuerySpec {
                filter: FilterExpr::Predicate(Predicate::Number {
                    field: NumberField::WordCount,
                    op,
                }),
                sorts: Vec::new(),
            };
            assert_eq!(query_ids(spec, vec![row.clone()]), vec![2]);
        }

        let spec = QuerySpec {
            filter: FilterExpr::Predicate(Predicate::Number {
                field: NumberField::ReadCount,
                op: NumberOp::Gt(0),
            }),
            sorts: Vec::new(),
        };
        assert!(query_ids(spec, vec![row]).is_empty());
    }

    #[test]
    fn tags_predicates_use_polars_list_set_semantics() {
        let empty = NovelRecord {
            tags: Vec::new(),
            ..record(1)
        };
        let rust = NovelRecord {
            id: 2,
            tags: vec!["rust".to_owned()],
            ..record(2)
        };
        let rust_gpui = NovelRecord {
            id: 3,
            tags: vec!["rust".to_owned(), "gpui".to_owned()],
            ..record(3)
        };
        let records = vec![empty, rust, rust_gpui];

        let query = |predicate| {
            query_ids(
                QuerySpec {
                    filter: FilterExpr::Predicate(Predicate::Tags(predicate)),
                    sorts: vec![SortSpec {
                        expr: SortExpr::Number(NumberField::NovelId),
                        direction: SortDirection::Asc,
                    }],
                },
                records.clone(),
            )
        };

        assert_eq!(query(TagsPredicate::Intersects(set(&["gpui"]))), vec![3]);
        assert_eq!(
            query(TagsPredicate::ContainsAll(set(&["rust"]))),
            vec![2, 3]
        );
        assert_eq!(
            query(TagsPredicate::ContainedBy(set(&["rust"]))),
            vec![1, 2]
        );
        assert_eq!(
            query(TagsPredicate::Equals(set(&["rust", "gpui"]))),
            vec![3]
        );
        assert_eq!(query(TagsPredicate::Equals(HashSet::new())), vec![1]);
        assert_eq!(query(TagsPredicate::IsEmpty), vec![1]);
        assert_eq!(query(TagsPredicate::IsNotEmpty), vec![2, 3]);
    }

    #[test]
    fn author_predicates_prefer_id_and_cover_anonymous_names() {
        let known = record(1);
        let mut anonymous = record(2);
        anonymous.author_id = None;
        anonymous.author_name = "匿名作者".to_owned();

        for predicate in [
            AuthorPredicate::Is(AuthorRef::Id(1)),
            AuthorPredicate::In(vec![AuthorRef::Id(1)]),
            AuthorPredicate::NotIn(vec![AuthorRef::Id(9)]),
        ] {
            let spec = QuerySpec {
                filter: FilterExpr::Predicate(Predicate::Author(predicate)),
                sorts: Vec::new(),
            };
            assert_eq!(query_ids(spec, vec![known.clone()]), vec![1]);
        }

        let spec = QuerySpec {
            filter: FilterExpr::Predicate(Predicate::Author(AuthorPredicate::Is(AuthorRef::Name(
                "匿名作者".to_owned(),
            )))),
            sorts: Vec::new(),
        };
        assert_eq!(query_ids(spec, vec![anonymous]), vec![2]);

        let spec = QuerySpec {
            filter: FilterExpr::Predicate(Predicate::Author(AuthorPredicate::Is(AuthorRef::Name(
                "author-1".to_owned(),
            )))),
            sorts: Vec::new(),
        };
        assert!(query_ids(spec, vec![known]).is_empty());
    }

    #[test]
    fn sorting_supports_fields_priority_and_missing_last() {
        let mut one = record(1);
        let mut two = record(2);
        let mut three = record(3);
        one.author_name = "b".to_owned();
        two.author_name = "a".to_owned();
        three.author_name = "a".to_owned();
        three.read_count = None;

        let spec = QuerySpec {
            filter: FilterExpr::default(),
            sorts: vec![
                SortSpec {
                    expr: SortExpr::Text(TextField::AuthorName),
                    direction: SortDirection::Asc,
                },
                SortSpec {
                    expr: SortExpr::Number(NumberField::ReadCount),
                    direction: SortDirection::Desc,
                },
            ],
        };

        assert_eq!(query_ids(spec, vec![one, two, three]), vec![2, 3, 1]);
    }
}

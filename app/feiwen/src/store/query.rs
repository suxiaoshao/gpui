use std::{collections::HashMap, io};

use duckdb::{
    Connection, Error as DuckdbError, params_from_iter,
    types::{Type, Value},
};

use crate::errors::FeiwenResult;

const ID: &str = "id";
const TITLE: &str = "name";
const DESCRIPTION: &str = "\"desc\"";
const IS_LIMIT: &str = "is_limit";
const LATEST_CHAPTER_NAME: &str = "latest_chapter_name";
const LATEST_CHAPTER_ID: &str = "latest_chapter_id";
const WORD_COUNT: &str = "word_count";
const READ_COUNT: &str = "read_count";
const REPLY_COUNT: &str = "reply_count";
const AUTHOR_ID: &str = "author_id";
const AUTHOR_NAME: &str = "author_name";

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
    Intersects(std::collections::HashSet<String>),
    ContainsAll(std::collections::HashSet<String>),
    ContainedBy(std::collections::HashSet<String>),
    Equals(std::collections::HashSet<String>),
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

struct QueryStatement {
    sql: String,
    params: Vec<Value>,
}

struct QueryBuilder {
    params: Vec<Value>,
}

impl QuerySpec {
    pub(crate) fn filter_count(&self) -> usize {
        self.filter.predicate_count()
    }

    pub(crate) fn sort_count(&self) -> usize {
        self.sorts.len()
    }
}

pub(crate) fn query_records(conn: &Connection, spec: &QuerySpec) -> FeiwenResult<Vec<NovelRecord>> {
    let statement = build_query(spec);
    let mut prepared = conn.prepare(&statement.sql)?;
    let rows = prepared.query_map(params_from_iter(statement.params.iter()), |row| {
        let tags = string_list(row.get(11)?, 11)?;
        let tag_ids = optional_i32_list(row.get(12)?, 12)?;
        let tag_ids = tags
            .iter()
            .enumerate()
            .map(|(index, tag)| (tag.clone(), tag_ids.get(index).copied().flatten()))
            .collect::<HashMap<_, _>>();

        Ok(NovelRecord {
            id: row.get(0)?,
            title: row.get(1)?,
            desc: row.get(2)?,
            is_limit: row.get(3)?,
            latest_chapter_name: row.get(4)?,
            latest_chapter_id: row.get(5)?,
            word_count: row.get(6)?,
            read_count: row.get(7)?,
            reply_count: row.get(8)?,
            author_id: row.get(9)?,
            author_name: row.get(10)?,
            tags,
            tag_ids,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn build_query(spec: &QuerySpec) -> QueryStatement {
    let mut builder = QueryBuilder { params: Vec::new() };
    let filter = builder.filter(&spec.filter);
    let order_by = builder.order_by(&spec.sorts);
    QueryStatement {
        sql: format!(
            r#"
            WITH novel_query AS (
                SELECT
                    n.id,
                    n.name,
                    n."desc",
                    n.is_limit,
                    n.latest_chapter_name,
                    n.latest_chapter_id,
                    n.word_count,
                    n.read_count,
                    n.reply_count,
                    n.author_id,
                    n.author_name,
                    COALESCE(
                        list(nt.tag_id ORDER BY nt.tag_id) FILTER (WHERE nt.tag_id IS NOT NULL),
                        []::VARCHAR[]
                    ) AS tags,
                    COALESCE(
                        list(tag.id ORDER BY nt.tag_id) FILTER (WHERE nt.tag_id IS NOT NULL),
                        []::INTEGER[]
                    ) AS tag_ids
                FROM novel n
                LEFT JOIN novel_tag nt ON nt.novel_id = n.id
                LEFT JOIN tag ON tag.name = nt.tag_id
                GROUP BY
                    n.id,
                    n.name,
                    n."desc",
                    n.is_limit,
                    n.latest_chapter_name,
                    n.latest_chapter_id,
                    n.word_count,
                    n.read_count,
                    n.reply_count,
                    n.author_id,
                    n.author_name
            )
            SELECT
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
                author_name,
                tags,
                tag_ids
            FROM novel_query
            WHERE {filter}
            {order_by}
            "#,
        ),
        params: builder.params,
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
}

impl QueryBuilder {
    fn filter(&mut self, filter: &FilterExpr) -> String {
        match filter {
            FilterExpr::All(filters) => self.combine(filters, "AND", true),
            FilterExpr::Any(filters) => self.combine(filters, "OR", false),
            FilterExpr::Not(filter) => format!("NOT ({})", self.filter(filter)),
            FilterExpr::Predicate(predicate) => self.predicate(predicate),
        }
    }

    fn combine(&mut self, filters: &[FilterExpr], operator: &str, empty_value: bool) -> String {
        if filters.is_empty() {
            return bool_sql(empty_value).to_owned();
        }
        filters
            .iter()
            .map(|filter| format!("({})", self.filter(filter)))
            .collect::<Vec<_>>()
            .join(&format!(" {operator} "))
    }

    fn predicate(&mut self, predicate: &Predicate) -> String {
        match predicate {
            Predicate::Text { field, op, value } => self.text(*field, *op, value),
            Predicate::Number { field, op } => self.number(*field, *op),
            Predicate::Bool { field, value } => {
                let param = self.push_bool(*value);
                format!("{} = {param}", field.column())
            }
            Predicate::Tags(predicate) => self.tags(predicate),
            Predicate::Author(predicate) => self.author(predicate),
        }
    }

    fn text(&mut self, field: TextField, op: TextOp, value: &str) -> String {
        let column = field.column();
        let param = self.push_text(value);
        match op {
            TextOp::Contains => format!("contains({column}, {param})"),
            TextOp::StartsWith => format!("starts_with({column}, {param})"),
            TextOp::EndsWith => format!("ends_with({column}, {param})"),
            TextOp::Equals => format!("{column} = {param}"),
        }
    }

    fn number(&mut self, field: NumberField, op: NumberOp) -> String {
        let column = field.column();
        match op {
            NumberOp::Eq(value) => format!("{column} = {}", self.push_i32(value)),
            NumberOp::Ne(value) => format!("{column} <> {}", self.push_i32(value)),
            NumberOp::Lt(value) => format!("{column} < {}", self.push_i32(value)),
            NumberOp::Lte(value) => format!("{column} <= {}", self.push_i32(value)),
            NumberOp::Gt(value) => format!("{column} > {}", self.push_i32(value)),
            NumberOp::Gte(value) => format!("{column} >= {}", self.push_i32(value)),
            NumberOp::Between { min, max } => {
                let min = self.push_i32(min);
                let max = self.push_i32(max);
                format!("{column} BETWEEN {min} AND {max}")
            }
        }
    }

    fn tags(&mut self, predicate: &TagsPredicate) -> String {
        match predicate {
            TagsPredicate::Intersects(values) if values.is_empty() => "FALSE".to_owned(),
            TagsPredicate::ContainsAll(values) if values.is_empty() => "TRUE".to_owned(),
            TagsPredicate::ContainedBy(values) if values.is_empty() => {
                "length(tags) = 0".to_owned()
            }
            TagsPredicate::Equals(values) if values.is_empty() => "length(tags) = 0".to_owned(),
            TagsPredicate::Intersects(values) => {
                let values = self.string_list(values);
                format!("list_has_any(tags, {values})")
            }
            TagsPredicate::ContainsAll(values) => {
                let values = self.string_list(values);
                format!("list_has_all(tags, {values})")
            }
            TagsPredicate::ContainedBy(values) => {
                let values = self.string_list(values);
                format!("list_has_all({values}, tags)")
            }
            TagsPredicate::Equals(values) => {
                let left = self.string_list(values);
                let right = self.string_list(values);
                format!("list_has_all(tags, {left}) AND list_has_all({right}, tags)")
            }
            TagsPredicate::IsEmpty => "length(tags) = 0".to_owned(),
            TagsPredicate::IsNotEmpty => "length(tags) > 0".to_owned(),
        }
    }

    fn author(&mut self, predicate: &AuthorPredicate) -> String {
        match predicate {
            AuthorPredicate::Is(author) => self.author_ref(author),
            AuthorPredicate::In(authors) => self.combine_authors(authors, false),
            AuthorPredicate::NotIn(authors) => {
                format!("NOT ({})", self.combine_authors(authors, false))
            }
        }
    }

    fn combine_authors(&mut self, authors: &[AuthorRef], empty_value: bool) -> String {
        if authors.is_empty() {
            return bool_sql(empty_value).to_owned();
        }
        authors
            .iter()
            .map(|author| format!("({})", self.author_ref(author)))
            .collect::<Vec<_>>()
            .join(" OR ")
    }

    fn author_ref(&mut self, author: &AuthorRef) -> String {
        match author {
            AuthorRef::Id(id) => {
                let id = self.push_i32(*id);
                format!("{AUTHOR_ID} IS NOT NULL AND {AUTHOR_ID} = {id}")
            }
            AuthorRef::Name(name) => {
                let name = self.push_text(name);
                format!("{AUTHOR_ID} IS NULL AND {AUTHOR_NAME} = {name}")
            }
        }
    }

    fn order_by(&mut self, sorts: &[SortSpec]) -> String {
        if sorts.is_empty() {
            return "ORDER BY id ASC".to_owned();
        }
        let mut sort_sql = sorts
            .iter()
            .map(|sort| {
                format!(
                    "{} {} NULLS LAST",
                    sort.expr.column(),
                    match sort.direction {
                        SortDirection::Asc => "ASC",
                        SortDirection::Desc => "DESC",
                    }
                )
            })
            .collect::<Vec<_>>();
        if !sorts
            .iter()
            .any(|sort| matches!(sort.expr, SortExpr::Number(NumberField::NovelId)))
        {
            sort_sql.push("id ASC".to_owned());
        }
        format!("ORDER BY {}", sort_sql.join(", "))
    }

    fn string_list(&mut self, values: &std::collections::HashSet<String>) -> String {
        let mut values = values.iter().map(String::as_str).collect::<Vec<_>>();
        values.sort();
        let params = values
            .into_iter()
            .map(|value| self.push_text(value))
            .collect::<Vec<_>>()
            .join(", ");
        format!("list_value({params})")
    }

    fn push_i32(&mut self, value: i32) -> String {
        self.params.push(Value::Int(value));
        "?".to_owned()
    }

    fn push_bool(&mut self, value: bool) -> String {
        self.params.push(Value::Boolean(value));
        "?".to_owned()
    }

    fn push_text(&mut self, value: &str) -> String {
        self.params.push(Value::Text(value.to_owned()));
        "?".to_owned()
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
    fn column(&self) -> &'static str {
        match self {
            SortExpr::Number(field) => field.column(),
            SortExpr::Text(field) => field.column(),
            SortExpr::Bool(field) => field.column(),
        }
    }
}

fn bool_sql(value: bool) -> &'static str {
    if value { "TRUE" } else { "FALSE" }
}

fn string_list(value: Value, index: usize) -> duckdb::Result<Vec<String>> {
    let values = match value {
        Value::List(values) | Value::Array(values) => values,
        Value::Null => return Ok(Vec::new()),
        value => {
            return Err(conversion_error(
                index,
                format!("expected string list: {value:?}"),
            ));
        }
    };
    values
        .into_iter()
        .map(|value| match value {
            Value::Text(value) => Ok(value),
            Value::Null => Err(conversion_error(index, "unexpected null tag name")),
            value => Err(conversion_error(
                index,
                format!("expected string list item: {value:?}"),
            )),
        })
        .collect()
}

fn optional_i32_list(value: Value, index: usize) -> duckdb::Result<Vec<Option<i32>>> {
    let values = match value {
        Value::List(values) | Value::Array(values) => values,
        Value::Null => return Ok(Vec::new()),
        value => {
            return Err(conversion_error(
                index,
                format!("expected int list: {value:?}"),
            ));
        }
    };
    values
        .into_iter()
        .map(|value| optional_i32(value, index))
        .collect()
}

fn optional_i32(value: Value, index: usize) -> duckdb::Result<Option<i32>> {
    match value {
        Value::Null => Ok(None),
        Value::Int(value) => Ok(Some(value)),
        Value::BigInt(value) => i32::try_from(value)
            .map(Some)
            .map_err(|err| conversion_error(index, err.to_string())),
        value => Err(conversion_error(
            index,
            format!("expected int list item: {value:?}"),
        )),
    }
}

fn conversion_error(index: usize, message: impl Into<String>) -> DuckdbError {
    DuckdbError::FromSqlConversionFailure(
        index,
        Type::List(Box::new(Type::Any)),
        Box::new(io::Error::new(io::ErrorKind::InvalidData, message.into())),
    )
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use duckdb::params;

    use super::*;
    use crate::store::initialize_schema;

    fn set(values: &[&str]) -> HashSet<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    fn connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize_schema(&conn).unwrap();
        conn
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

    fn insert_records(conn: &Connection, records: &[NovelRecord]) {
        for record in records {
            conn.execute(
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
                "#,
                params![
                    record.id,
                    record.title,
                    record.desc,
                    record.is_limit,
                    record.latest_chapter_name,
                    record.latest_chapter_id,
                    record.word_count,
                    record.read_count,
                    record.reply_count,
                    record.author_id,
                    record.author_name,
                ],
            )
            .unwrap();

            for tag in &record.tags {
                let tag_id = record.tag_ids.get(tag).copied().flatten();
                conn.execute(
                    "INSERT INTO tag (id, name) VALUES (?, ?) ON CONFLICT DO NOTHING",
                    params![tag_id, tag],
                )
                .unwrap();
                conn.execute(
                    "INSERT INTO novel_tag (novel_id, tag_id) VALUES (?, ?) ON CONFLICT DO NOTHING",
                    params![record.id, tag],
                )
                .unwrap();
            }
        }
    }

    fn query_ids(spec: QuerySpec, records: Vec<NovelRecord>) -> Vec<i32> {
        let conn = connection();
        insert_records(&conn, &records);
        query_records(&conn, &spec)
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
    fn tags_predicates_use_duckdb_list_set_semantics() {
        let empty = NovelRecord {
            tags: Vec::new(),
            tag_ids: HashMap::new(),
            ..record(1)
        };
        let rust = NovelRecord {
            id: 2,
            tags: vec!["rust".to_owned()],
            tag_ids: HashMap::from([("rust".to_owned(), Some(1))]),
            ..record(2)
        };
        let rust_gpui = NovelRecord {
            id: 3,
            tags: vec!["rust".to_owned(), "gpui".to_owned()],
            tag_ids: HashMap::from([("rust".to_owned(), Some(1)), ("gpui".to_owned(), Some(3))]),
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

    #[test]
    fn injection_shaped_strings_are_plain_values() {
        let injection = "' OR 1=1 --";
        let plain = record(1);
        let mut injected_title = record(2);
        injected_title.title = injection.to_owned();
        injected_title.tags = vec!["tag', 1) --".to_owned()];
        injected_title.tag_ids = HashMap::from([("tag', 1) --".to_owned(), Some(9))]);
        let mut anonymous = record(3);
        anonymous.author_id = None;
        anonymous.author_name = "author' OR 1=1 --".to_owned();

        let spec = QuerySpec {
            filter: FilterExpr::Predicate(Predicate::Text {
                field: TextField::Title,
                op: TextOp::Equals,
                value: injection.to_owned(),
            }),
            sorts: Vec::new(),
        };
        assert_eq!(
            query_ids(
                spec,
                vec![plain.clone(), injected_title.clone(), anonymous.clone()]
            ),
            vec![2]
        );

        let spec = QuerySpec {
            filter: FilterExpr::Predicate(Predicate::Tags(TagsPredicate::Intersects(set(&[
                "tag', 1) --",
            ])))),
            sorts: Vec::new(),
        };
        assert_eq!(
            query_ids(spec, vec![plain.clone(), injected_title, anonymous.clone()]),
            vec![2]
        );

        let spec = QuerySpec {
            filter: FilterExpr::Predicate(Predicate::Author(AuthorPredicate::Is(AuthorRef::Name(
                "author' OR 1=1 --".to_owned(),
            )))),
            sorts: Vec::new(),
        };
        assert_eq!(query_ids(spec, vec![plain, anonymous]), vec![3]);
    }
}

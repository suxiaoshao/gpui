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
    IsNot(AuthorRef),
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
            AuthorPredicate::IsNot(author) => format!("NOT ({})", self.author_ref(author)),
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

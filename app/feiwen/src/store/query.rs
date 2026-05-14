#[cfg(test)]
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use diesel::{
    query_builder::{AstPass, Query, QueryFragment, QueryId},
    result::QueryResult,
    sql_types::{Bool, Integer, Text, Untyped},
    sqlite::Sqlite,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct QuerySql {
    parts: Vec<SqlPart>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct NovelQueryPlan {
    anchor: Option<TagAnchor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TagAnchor {
    tag: String,
}

#[derive(Debug, Clone, PartialEq)]
enum SqlPart {
    Raw(String),
    Text(String),
    Integer(i32),
    Bool(bool),
}

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
#[cfg(test)]
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
    pub(crate) tags: HashSet<String>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg(test)]
enum SortValue {
    Number(f64),
    Text(String),
    Bool(bool),
}

impl QuerySpec {
    #[cfg(test)]
    pub(crate) fn apply(&self, mut records: Vec<NovelRecord>) -> Vec<NovelRecord> {
        records.retain(|record| self.filter.matches(record));
        apply_sorts(&mut records, &self.sorts);
        records
    }

    pub(crate) fn filter_count(&self) -> usize {
        self.filter.predicate_count()
    }

    pub(crate) fn sort_count(&self) -> usize {
        self.sorts.len()
    }

    pub(crate) fn tag_anchor_candidates(&self) -> HashSet<String> {
        let mut tags = HashSet::new();
        self.filter.collect_tag_anchor_candidates(&mut tags);
        tags
    }

    pub(crate) fn query_sql_with_plan(&self, alias: &str, plan: &NovelQueryPlan) -> QuerySql {
        let mut sql = QuerySql::new();
        sql.push_sql(
            "\
        SELECT \
            n.id, \
            n.name, \
            n.desc, \
            n.is_limit, \
            n.latest_chapter_name, \
            n.latest_chapter_id, \
            n.word_count, \
            n.read_count, \
            n.reply_count, \
            n.author_id, \
            n.author_name ",
        );
        if let Some(anchor) = plan.tag_anchor() {
            sql.push_sql(
                "\
        FROM novel_tag t0 INDEXED BY idx_novel_tag_tag_id_novel_id \
        CROSS JOIN novel n \
        WHERE t0.tag_id = ",
            );
            sql.push_text(anchor);
            sql.push_sql(" AND ");
            sql.push_sql(alias);
            sql.push_sql(".id = t0.novel_id AND ");
        } else {
            sql.push_sql(
                "\
        FROM novel n \
        WHERE ",
            );
        }
        self.filter.push_sql(alias, &mut sql);
        self.push_order_sql(alias, &mut sql);
        sql
    }

    fn push_order_sql(&self, alias: &str, sql: &mut QuerySql) {
        if self.sorts.is_empty() {
            return;
        }
        sql.push_sql(" ORDER BY ");
        let mut first = true;
        for sort in &self.sorts {
            if !first {
                sql.push_sql(", ");
            }
            first = false;

            let expr = sort.expr.sql(alias);
            let direction = match sort.direction {
                SortDirection::Asc => "ASC",
                SortDirection::Desc => "DESC",
            };
            sql.push_sql("(");
            sql.push_sql(&expr);
            sql.push_sql(") IS NULL ASC, (");
            sql.push_sql(&expr);
            sql.push_sql(") ");
            sql.push_sql(direction);
        }
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

    fn push_sql(&self, alias: &str, sql: &mut QuerySql) {
        match self {
            FilterExpr::All(filters) => {
                if filters.is_empty() {
                    sql.push_sql("1");
                } else {
                    join_sql(filters, " AND ", alias, sql);
                }
            }
            FilterExpr::Any(filters) => {
                if filters.is_empty() {
                    sql.push_sql("0");
                } else {
                    join_sql(filters, " OR ", alias, sql);
                }
            }
            FilterExpr::Not(filter) => {
                sql.push_sql("NOT (");
                filter.push_sql(alias, sql);
                sql.push_sql(")");
            }
            FilterExpr::Predicate(predicate) => predicate.push_sql(alias, sql),
        }
    }

    fn collect_tag_anchor_candidates(&self, tags: &mut HashSet<String>) {
        match self {
            FilterExpr::All(filters) => {
                for filter in filters {
                    filter.collect_tag_anchor_candidates(tags);
                }
            }
            FilterExpr::Predicate(Predicate::Tags(
                TagsPredicate::ContainsAll(values) | TagsPredicate::Equals(values),
            )) => {
                tags.extend(values.iter().cloned());
            }
            FilterExpr::Any(_) | FilterExpr::Not(_) | FilterExpr::Predicate(_) => {}
        }
    }

    #[cfg(test)]
    fn matches(&self, record: &NovelRecord) -> bool {
        match self {
            FilterExpr::All(filters) => filters.iter().all(|filter| filter.matches(record)),
            FilterExpr::Any(filters) => filters.iter().any(|filter| filter.matches(record)),
            FilterExpr::Not(filter) => !filter.matches(record),
            FilterExpr::Predicate(predicate) => predicate.matches(record),
        }
    }
}

impl Predicate {
    fn push_sql(&self, alias: &str, sql: &mut QuerySql) {
        match self {
            Predicate::Text { field, op, value } => {
                let field = field.sql(alias);
                match op {
                    TextOp::Contains => {
                        sql.push_sql("instr(");
                        sql.push_sql(field);
                        sql.push_sql(", ");
                        sql.push_text(value);
                        sql.push_sql(") > 0");
                    }
                    TextOp::StartsWith => {
                        sql.push_sql("substr(");
                        sql.push_sql(field);
                        sql.push_sql(", 1, length(");
                        sql.push_text(value);
                        sql.push_sql(")) = ");
                        sql.push_text(value);
                    }
                    TextOp::EndsWith => {
                        sql.push_sql("substr(");
                        sql.push_sql(field);
                        sql.push_sql(", -length(");
                        sql.push_text(value);
                        sql.push_sql(")) = ");
                        sql.push_text(value);
                    }
                    TextOp::Equals => {
                        sql.push_sql(field);
                        sql.push_sql(" = ");
                        sql.push_text(value);
                    }
                }
            }
            Predicate::Number { field, op } => {
                let field = field.sql(alias);
                match op {
                    NumberOp::Eq(value) => push_number_compare(sql, &field, "=", *value),
                    NumberOp::Ne(value) => push_number_compare(sql, &field, "<>", *value),
                    NumberOp::Lt(value) => push_number_compare(sql, &field, "<", *value),
                    NumberOp::Lte(value) => push_number_compare(sql, &field, "<=", *value),
                    NumberOp::Gt(value) => push_number_compare(sql, &field, ">", *value),
                    NumberOp::Gte(value) => push_number_compare(sql, &field, ">=", *value),
                    NumberOp::Between { min, max } => push_number_between(sql, &field, *min, *max),
                }
            }
            Predicate::Bool { field, value } => {
                sql.push_sql(field.sql(alias));
                sql.push_sql(" = ");
                sql.push_bool(*value);
            }
            Predicate::Tags(predicate) => predicate.push_sql(alias, sql),
            Predicate::Author(predicate) => predicate.push_sql(alias, sql),
        }
    }

    #[cfg(test)]
    fn matches(&self, record: &NovelRecord) -> bool {
        match self {
            Predicate::Text { field, op, value } => match op {
                TextOp::Contains => record.text(*field).contains(value),
                TextOp::StartsWith => record.text(*field).starts_with(value),
                TextOp::EndsWith => record.text(*field).ends_with(value),
                TextOp::Equals => record.text(*field) == value,
            },
            Predicate::Number { field, op } => {
                record.number(*field).is_some_and(|value| match op {
                    NumberOp::Eq(target) => value == *target,
                    NumberOp::Ne(target) => value != *target,
                    NumberOp::Lt(target) => value < *target,
                    NumberOp::Lte(target) => value <= *target,
                    NumberOp::Gt(target) => value > *target,
                    NumberOp::Gte(target) => value >= *target,
                    NumberOp::Between { min, max } => value >= *min && value <= *max,
                })
            }
            Predicate::Bool { field, value } => record.bool(*field) == *value,
            Predicate::Tags(predicate) => predicate.matches(&record.tags),
            Predicate::Author(predicate) => predicate.matches(record),
        }
    }
}

impl TagsPredicate {
    fn push_sql(&self, alias: &str, sql: &mut QuerySql) {
        match self {
            TagsPredicate::Intersects(values) => push_tag_exists_sql(alias, values, false, sql),
            TagsPredicate::ContainsAll(values) => {
                if values.is_empty() {
                    sql.push_sql("1");
                } else {
                    for (ix, value) in sorted_values(values).into_iter().enumerate() {
                        if ix > 0 {
                            sql.push_sql(" AND ");
                        }
                        sql.push_sql("EXISTS (SELECT 1 FROM novel_tag nt WHERE nt.novel_id = ");
                        sql.push_sql(alias);
                        sql.push_sql(".id AND nt.tag_id = ");
                        sql.push_text(value);
                        sql.push_sql(")");
                    }
                }
            }
            TagsPredicate::ContainedBy(values) => {
                if values.is_empty() {
                    sql.push_sql("NOT EXISTS (SELECT 1 FROM novel_tag nt WHERE nt.novel_id = ");
                    sql.push_sql(alias);
                    sql.push_sql(".id)");
                } else {
                    sql.push_sql("NOT EXISTS (SELECT 1 FROM novel_tag nt WHERE nt.novel_id = ");
                    sql.push_sql(alias);
                    sql.push_sql(".id AND nt.tag_id NOT IN (");
                    push_text_list(values, sql);
                    sql.push_sql("))");
                }
            }
            TagsPredicate::Equals(values) => {
                sql.push_sql("(");
                TagsPredicate::ContainsAll(values.clone()).push_sql(alias, sql);
                sql.push_sql(") AND (");
                TagsPredicate::ContainedBy(values.clone()).push_sql(alias, sql);
                sql.push_sql(")");
            }
            TagsPredicate::IsEmpty => {
                sql.push_sql("NOT EXISTS (SELECT 1 FROM novel_tag nt WHERE nt.novel_id = ");
                sql.push_sql(alias);
                sql.push_sql(".id)");
            }
            TagsPredicate::IsNotEmpty => {
                sql.push_sql("EXISTS (SELECT 1 FROM novel_tag nt WHERE nt.novel_id = ");
                sql.push_sql(alias);
                sql.push_sql(".id)");
            }
        }
    }

    #[cfg(test)]
    fn matches(&self, tags: &HashSet<String>) -> bool {
        match self {
            TagsPredicate::Intersects(values) => !tags.is_disjoint(values),
            TagsPredicate::ContainsAll(values) => tags.is_superset(values),
            TagsPredicate::ContainedBy(values) => tags.is_subset(values),
            TagsPredicate::Equals(values) => tags == values,
            TagsPredicate::IsEmpty => tags.is_empty(),
            TagsPredicate::IsNotEmpty => !tags.is_empty(),
        }
    }
}

impl AuthorPredicate {
    fn push_sql(&self, alias: &str, sql: &mut QuerySql) {
        match self {
            AuthorPredicate::Is(author) => author.push_sql(alias, sql),
            AuthorPredicate::In(authors) => {
                if authors.is_empty() {
                    sql.push_sql("0");
                } else {
                    join_author_sql(authors, " OR ", alias, sql);
                }
            }
            AuthorPredicate::NotIn(authors) => {
                if authors.is_empty() {
                    sql.push_sql("1");
                } else {
                    sql.push_sql("NOT (");
                    join_author_sql(authors, " OR ", alias, sql);
                    sql.push_sql(")");
                }
            }
        }
    }

    #[cfg(test)]
    fn matches(&self, record: &NovelRecord) -> bool {
        match self {
            AuthorPredicate::Is(author) => author.matches(record),
            AuthorPredicate::In(authors) => authors.iter().any(|author| author.matches(record)),
            AuthorPredicate::NotIn(authors) => !authors.iter().any(|author| author.matches(record)),
        }
    }
}

impl AuthorRef {
    fn push_sql(&self, alias: &str, sql: &mut QuerySql) {
        match self {
            AuthorRef::Id(id) => {
                sql.push_sql("(");
                sql.push_sql(alias);
                sql.push_sql(".author_id IS NOT NULL AND ");
                sql.push_sql(alias);
                sql.push_sql(".author_id = ");
                sql.push_i32(*id);
                sql.push_sql(")");
            }
            AuthorRef::Name(name) => {
                sql.push_sql("(");
                sql.push_sql(alias);
                sql.push_sql(".author_id IS NULL AND ");
                sql.push_sql(alias);
                sql.push_sql(".author_name = ");
                sql.push_text(name);
                sql.push_sql(")");
            }
        }
    }

    #[cfg(test)]
    fn matches(&self, record: &NovelRecord) -> bool {
        match self {
            AuthorRef::Id(id) => record.author_id == Some(*id),
            AuthorRef::Name(name) => record.author_id.is_none() && record.author_name == *name,
        }
    }
}

impl TextField {
    fn sql(self, alias: &str) -> String {
        let column = match self {
            TextField::Title => "name",
            TextField::Description => "desc",
            TextField::LatestChapter => "latest_chapter_name",
            TextField::AuthorName => "author_name",
        };
        format!("{alias}.{column}")
    }
}

impl NumberField {
    fn sql(self, alias: &str) -> String {
        let column = match self {
            NumberField::NovelId => "id",
            NumberField::LatestChapterId => "latest_chapter_id",
            NumberField::WordCount => "word_count",
            NumberField::ReadCount => "read_count",
            NumberField::ReplyCount => "reply_count",
            NumberField::AuthorId => "author_id",
        };
        format!("{alias}.{column}")
    }
}

impl BoolField {
    fn sql(self, alias: &str) -> String {
        match self {
            BoolField::IsLimit => format!("{alias}.is_limit"),
        }
    }
}

#[cfg(test)]
impl NovelRecord {
    fn text(&self, field: TextField) -> &str {
        match field {
            TextField::Title => &self.title,
            TextField::Description => &self.desc,
            TextField::LatestChapter => &self.latest_chapter_name,
            TextField::AuthorName => &self.author_name,
        }
    }

    fn number(&self, field: NumberField) -> Option<i32> {
        match field {
            NumberField::NovelId => Some(self.id),
            NumberField::LatestChapterId => Some(self.latest_chapter_id),
            NumberField::WordCount => Some(self.word_count),
            NumberField::ReadCount => self.read_count,
            NumberField::ReplyCount => self.reply_count,
            NumberField::AuthorId => self.author_id,
        }
    }

    fn bool(&self, field: BoolField) -> bool {
        match field {
            BoolField::IsLimit => self.is_limit,
        }
    }
}

#[cfg(test)]
fn apply_sorts(records: &mut [NovelRecord], sorts: &[SortSpec]) {
    for sort in sorts.iter().rev() {
        records.sort_by(
            |left, right| match (sort.expr.eval(left), sort.expr.eval(right)) {
                (Some(left), Some(right)) => {
                    let ordering = left.cmp(&right);
                    match sort.direction {
                        SortDirection::Asc => ordering,
                        SortDirection::Desc => ordering.reverse(),
                    }
                }
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            },
        );
    }
}

#[cfg(test)]
impl SortValue {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (SortValue::Number(left), SortValue::Number(right)) => {
                left.partial_cmp(right).unwrap_or(Ordering::Equal)
            }
            (SortValue::Text(left), SortValue::Text(right)) => left.cmp(right),
            (SortValue::Bool(left), SortValue::Bool(right)) => left.cmp(right),
            (left, right) => left.kind_order().cmp(&right.kind_order()),
        }
    }

    fn kind_order(&self) -> u8 {
        match self {
            SortValue::Number(_) => 0,
            SortValue::Text(_) => 1,
            SortValue::Bool(_) => 2,
        }
    }
}

impl SortExpr {
    fn sql(&self, alias: &str) -> String {
        match self {
            SortExpr::Number(field) => field.sql(alias),
            SortExpr::Text(field) => field.sql(alias),
            SortExpr::Bool(field) => field.sql(alias),
        }
    }

    #[cfg(test)]
    fn eval(&self, record: &NovelRecord) -> Option<SortValue> {
        match self {
            SortExpr::Number(field) => record
                .number(*field)
                .map(|value| SortValue::Number(value as f64)),
            SortExpr::Text(field) => Some(SortValue::Text(record.text(*field).to_owned())),
            SortExpr::Bool(field) => Some(SortValue::Bool(record.bool(*field))),
        }
    }
}

impl NovelQueryPlan {
    pub(crate) fn from_tag_counts(
        candidates: HashSet<String>,
        tag_counts: &HashMap<String, i64>,
    ) -> Self {
        let anchor = candidates
            .into_iter()
            .min_by(|left, right| {
                let left_count = tag_counts.get(left).copied().unwrap_or(0);
                let right_count = tag_counts.get(right).copied().unwrap_or(0);
                left_count.cmp(&right_count).then_with(|| left.cmp(right))
            })
            .map(|tag| TagAnchor { tag });

        Self { anchor }
    }

    fn tag_anchor(&self) -> Option<&str> {
        self.anchor.as_ref().map(|anchor| anchor.tag.as_str())
    }
}

impl QuerySql {
    pub(crate) fn new() -> Self {
        Self { parts: Vec::new() }
    }

    pub(crate) fn push_sql(&mut self, sql: impl AsRef<str>) {
        self.parts.push(SqlPart::Raw(sql.as_ref().to_owned()));
    }

    pub(crate) fn push_text(&mut self, value: impl AsRef<str>) {
        self.parts.push(SqlPart::Text(value.as_ref().to_owned()));
    }

    pub(crate) fn push_i32(&mut self, value: i32) {
        self.parts.push(SqlPart::Integer(value));
    }

    pub(crate) fn push_bool(&mut self, value: bool) {
        self.parts.push(SqlPart::Bool(value));
    }

    #[cfg(test)]
    fn sql_with_placeholders(&self) -> String {
        self.parts
            .iter()
            .map(|part| match part {
                SqlPart::Raw(sql) => sql.as_str(),
                SqlPart::Text(_) | SqlPart::Integer(_) | SqlPart::Bool(_) => "?",
            })
            .collect()
    }

    #[cfg(test)]
    fn bind_count(&self) -> usize {
        self.parts
            .iter()
            .filter(|part| !matches!(part, SqlPart::Raw(_)))
            .count()
    }
}

impl Query for QuerySql {
    type SqlType = Untyped;
}

impl QueryId for QuerySql {
    type QueryId = ();

    const HAS_STATIC_QUERY_ID: bool = false;
}

impl<Conn> diesel::RunQueryDsl<Conn> for QuerySql {}

impl QueryFragment<Sqlite> for QuerySql {
    fn walk_ast<'b>(&'b self, mut out: AstPass<'_, 'b, Sqlite>) -> QueryResult<()> {
        out.unsafe_to_cache_prepared();
        for part in &self.parts {
            match part {
                SqlPart::Raw(sql) => out.push_sql(sql),
                SqlPart::Text(value) => out.push_bind_param::<Text, _>(value)?,
                SqlPart::Integer(value) => out.push_bind_param::<Integer, _>(value)?,
                SqlPart::Bool(value) => out.push_bind_param::<Bool, _>(value)?,
            }
        }
        Ok(())
    }
}

fn join_sql(filters: &[FilterExpr], separator: &str, alias: &str, sql: &mut QuerySql) {
    for (ix, filter) in filters.iter().enumerate() {
        if ix > 0 {
            sql.push_sql(separator);
        }
        sql.push_sql("(");
        filter.push_sql(alias, sql);
        sql.push_sql(")");
    }
}

fn join_author_sql(authors: &[AuthorRef], separator: &str, alias: &str, sql: &mut QuerySql) {
    for (ix, author) in authors.iter().enumerate() {
        if ix > 0 {
            sql.push_sql(separator);
        }
        sql.push_sql("(");
        author.push_sql(alias, sql);
        sql.push_sql(")");
    }
}

fn push_number_compare(sql: &mut QuerySql, field: &str, op: &str, value: i32) {
    sql.push_sql("(");
    sql.push_sql(field);
    sql.push_sql(" IS NOT NULL AND ");
    sql.push_sql(field);
    sql.push_sql(" ");
    sql.push_sql(op);
    sql.push_sql(" ");
    sql.push_i32(value);
    sql.push_sql(")");
}

fn push_number_between(sql: &mut QuerySql, field: &str, min: i32, max: i32) {
    sql.push_sql("(");
    sql.push_sql(field);
    sql.push_sql(" IS NOT NULL AND ");
    sql.push_sql(field);
    sql.push_sql(" >= ");
    sql.push_i32(min);
    sql.push_sql(" AND ");
    sql.push_sql(field);
    sql.push_sql(" <= ");
    sql.push_i32(max);
    sql.push_sql(")");
}

fn push_tag_exists_sql(alias: &str, values: &HashSet<String>, negated: bool, sql: &mut QuerySql) {
    if values.is_empty() {
        sql.push_sql(if negated { "1" } else { "0" });
        return;
    }
    let op = if negated { "NOT IN" } else { "IN" };
    sql.push_sql("EXISTS (SELECT 1 FROM novel_tag nt WHERE nt.novel_id = ");
    sql.push_sql(alias);
    sql.push_sql(".id AND nt.tag_id ");
    sql.push_sql(op);
    sql.push_sql(" (");
    push_text_list(values, sql);
    sql.push_sql("))");
}

fn push_text_list(values: &HashSet<String>, sql: &mut QuerySql) {
    for (ix, value) in sorted_values(values).into_iter().enumerate() {
        if ix > 0 {
            sql.push_sql(", ");
        }
        sql.push_text(value);
    }
}

fn sorted_values(values: &HashSet<String>) -> Vec<&String> {
    let mut values = values.iter().collect::<Vec<_>>();
    values.sort();
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(values: &[&str]) -> HashSet<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    fn counts(values: &[(&str, i64)]) -> HashMap<String, i64> {
        values
            .iter()
            .map(|(tag, count)| ((*tag).to_owned(), *count))
            .collect()
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
            tags: set(&["rust", "systems"]),
        }
    }

    #[test]
    fn nested_boolean_filters_match_expected_records() {
        let spec = QuerySpec {
            filter: FilterExpr::All(vec![
                FilterExpr::Any(vec![
                    FilterExpr::Predicate(Predicate::Text {
                        field: TextField::Title,
                        op: TextOp::Contains,
                        value: "Rust".to_owned(),
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
        let results = spec.apply(vec![record(1), limited]);

        assert_eq!(
            results
                .into_iter()
                .map(|record| record.id)
                .collect::<Vec<_>>(),
            vec![1]
        );
        assert!(FilterExpr::All(Vec::new()).matches(&record(1)));
        assert!(!FilterExpr::Any(Vec::new()).matches(&record(1)));
    }

    #[test]
    fn text_predicates_cover_all_text_operators() {
        let row = record(1);
        for (op, value) in [
            (TextOp::Contains, "novel"),
            (TextOp::StartsWith, "Rust"),
            (TextOp::EndsWith, "1"),
            (TextOp::Equals, "Rust novel 1"),
        ] {
            assert!(
                Predicate::Text {
                    field: TextField::Title,
                    op,
                    value: value.to_owned(),
                }
                .matches(&row)
            );
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
            assert!(
                Predicate::Number {
                    field: NumberField::WordCount,
                    op,
                }
                .matches(&row)
            );
        }

        assert!(
            !Predicate::Number {
                field: NumberField::ReadCount,
                op: NumberOp::Gt(0),
            }
            .matches(&row)
        );
    }

    #[test]
    fn tags_predicates_cover_set_relations() {
        let row = record(1);

        for predicate in [
            TagsPredicate::Intersects(set(&["rust"])),
            TagsPredicate::ContainsAll(set(&["rust", "systems"])),
            TagsPredicate::ContainedBy(set(&["rust", "systems", "extra"])),
            TagsPredicate::Equals(set(&["rust", "systems"])),
            TagsPredicate::IsNotEmpty,
        ] {
            assert!(Predicate::Tags(predicate).matches(&row));
        }

        assert!(!Predicate::Tags(TagsPredicate::IsEmpty).matches(&row));
    }

    #[test]
    fn author_predicates_prefer_id_and_cover_anonymous_names() {
        let known = record(1);
        let mut anonymous = record(2);
        anonymous.author_id = None;
        anonymous.author_name = "匿名作者".to_owned();

        assert!(Predicate::Author(AuthorPredicate::Is(AuthorRef::Id(1))).matches(&known));
        assert!(Predicate::Author(AuthorPredicate::In(vec![AuthorRef::Id(1)])).matches(&known));
        assert!(Predicate::Author(AuthorPredicate::NotIn(vec![AuthorRef::Id(9)])).matches(&known));
        assert!(
            Predicate::Text {
                field: TextField::AuthorName,
                op: TextOp::Contains,
                value: "author".to_owned(),
            }
            .matches(&known)
        );
        assert!(
            Predicate::Author(AuthorPredicate::Is(AuthorRef::Name("匿名作者".to_owned())))
                .matches(&anonymous)
        );
        assert!(
            !Predicate::Author(AuthorPredicate::Is(AuthorRef::Name("author-1".to_owned())))
                .matches(&known)
        );
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

        let results = spec.apply(vec![one, two, three]);
        assert_eq!(
            results
                .into_iter()
                .map(|record| record.id)
                .collect::<Vec<_>>(),
            vec![2, 3, 1]
        );

        assert_eq!(
            SortExpr::Bool(BoolField::IsLimit).eval(&record(1)),
            Some(SortValue::Bool(false))
        );
    }

    #[test]
    fn tag_anchor_plan_selects_rarest_positive_and_tag() {
        let spec = QuerySpec {
            filter: FilterExpr::All(vec![FilterExpr::Predicate(Predicate::Tags(
                TagsPredicate::ContainsAll(set(&["BL", "年下", "完结"])),
            ))]),
            sorts: Vec::new(),
        };

        let plan = NovelQueryPlan::from_tag_counts(
            spec.tag_anchor_candidates(),
            &counts(&[("BL", 81726), ("年下", 4745), ("完结", 52395)]),
        );

        assert_eq!(plan.tag_anchor(), Some("年下"));
    }

    #[test]
    fn tag_anchor_plan_supports_equals_but_not_empty_sets() {
        let equals = QuerySpec {
            filter: FilterExpr::Predicate(Predicate::Tags(TagsPredicate::Equals(set(&[
                "rust", "gpui",
            ])))),
            sorts: Vec::new(),
        };
        let plan = NovelQueryPlan::from_tag_counts(
            equals.tag_anchor_candidates(),
            &counts(&[("rust", 10), ("gpui", 2)]),
        );
        assert_eq!(plan.tag_anchor(), Some("gpui"));

        for predicate in [
            TagsPredicate::ContainsAll(HashSet::new()),
            TagsPredicate::Equals(HashSet::new()),
        ] {
            let spec = QuerySpec {
                filter: FilterExpr::Predicate(Predicate::Tags(predicate)),
                sorts: Vec::new(),
            };
            let plan =
                NovelQueryPlan::from_tag_counts(spec.tag_anchor_candidates(), &HashMap::new());
            assert_eq!(plan.tag_anchor(), None);
        }
    }

    #[test]
    fn tag_anchor_candidates_do_not_cross_or_or_not_boundaries() {
        let spec = QuerySpec {
            filter: FilterExpr::All(vec![
                FilterExpr::Any(vec![FilterExpr::Predicate(Predicate::Tags(
                    TagsPredicate::ContainsAll(set(&["rare"])),
                ))]),
                FilterExpr::Not(Box::new(FilterExpr::Predicate(Predicate::Tags(
                    TagsPredicate::Equals(set(&["excluded"])),
                )))),
            ]),
            sorts: Vec::new(),
        };

        assert!(spec.tag_anchor_candidates().is_empty());
    }

    #[test]
    fn query_sql_with_tag_anchor_uses_indexed_cross_join_and_binds_anchor() {
        let injection = "tag', 1) --";
        let spec = QuerySpec {
            filter: FilterExpr::Predicate(Predicate::Tags(TagsPredicate::ContainsAll(set(&[
                injection,
            ])))),
            sorts: Vec::new(),
        };
        let plan = NovelQueryPlan::from_tag_counts(spec.tag_anchor_candidates(), &HashMap::new());

        let sql = spec.query_sql_with_plan("n", &plan);
        let text = sql.sql_with_placeholders();

        assert!(text.contains("FROM novel_tag t0 INDEXED BY idx_novel_tag_tag_id_novel_id"));
        assert!(text.contains("CROSS JOIN novel n"));
        assert!(text.contains("WHERE t0.tag_id = ? AND n.id = t0.novel_id AND EXISTS"));
        assert!(!text.contains(injection));
        assert_eq!(sql.bind_count(), 2);
    }

    #[test]
    fn query_sql_uses_binds_for_user_controlled_values() {
        let injection = "' OR 1=1 --";
        let spec = QuerySpec {
            filter: FilterExpr::All(vec![
                FilterExpr::Predicate(Predicate::Text {
                    field: TextField::Title,
                    op: TextOp::Contains,
                    value: injection.to_owned(),
                }),
                FilterExpr::Predicate(Predicate::Author(AuthorPredicate::Is(AuthorRef::Name(
                    "author', 1) --".to_owned(),
                )))),
                FilterExpr::Predicate(Predicate::Tags(TagsPredicate::Intersects(set(&[
                    "tag', 1) --",
                ])))),
            ]),
            sorts: vec![SortSpec {
                expr: SortExpr::Number(NumberField::ReplyCount),
                direction: SortDirection::Desc,
            }],
        };

        let sql = spec.query_sql_with_plan("n", &NovelQueryPlan::default());
        let text = sql.sql_with_placeholders();

        assert!(!text.contains(injection));
        assert!(!text.contains("author', 1) --"));
        assert!(!text.contains("tag', 1) --"));
        assert!(text.contains("instr(n.name, ?) > 0"));
        assert!(text.contains("n.author_name = ?"));
        assert!(text.contains("nt.tag_id IN (?)"));
        assert!(text.contains("ORDER BY (n.reply_count) IS NULL ASC, (n.reply_count) DESC"));
        assert_eq!(sql.bind_count(), 3);
    }

    #[test]
    fn query_sql_uses_binds_for_repeated_text_and_numeric_values() {
        let spec = QuerySpec {
            filter: FilterExpr::All(vec![
                FilterExpr::Predicate(Predicate::Text {
                    field: TextField::LatestChapter,
                    op: TextOp::StartsWith,
                    value: "chapter".to_owned(),
                }),
                FilterExpr::Predicate(Predicate::Number {
                    field: NumberField::WordCount,
                    op: NumberOp::Between { min: 10, max: 20 },
                }),
                FilterExpr::Predicate(Predicate::Bool {
                    field: BoolField::IsLimit,
                    value: true,
                }),
            ]),
            sorts: Vec::new(),
        };

        let sql = spec.query_sql_with_plan("n", &NovelQueryPlan::default());
        let text = sql.sql_with_placeholders();

        assert!(text.contains("substr(n.latest_chapter_name, 1, length(?)) = ?"));
        assert!(text.contains("n.word_count >= ? AND n.word_count <= ?"));
        assert!(text.contains("n.is_limit = ?"));
        assert_eq!(sql.bind_count(), 5);
    }
}

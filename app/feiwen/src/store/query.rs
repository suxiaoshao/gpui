#[cfg(test)]
use std::cmp::Ordering;
use std::collections::HashSet;

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
    #[allow(dead_code)]
    Constant(f64),
    #[allow(dead_code)]
    Add(Box<SortExpr>, Box<SortExpr>),
    #[allow(dead_code)]
    Sub(Box<SortExpr>, Box<SortExpr>),
    #[allow(dead_code)]
    Mul(Box<SortExpr>, Box<SortExpr>),
    #[allow(dead_code)]
    Div(Box<SortExpr>, Box<SortExpr>),
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

    pub(crate) fn where_sql(&self, alias: &str) -> String {
        self.filter.sql(alias)
    }

    pub(crate) fn order_sql(&self, alias: &str) -> String {
        if self.sorts.is_empty() {
            return String::new();
        }
        let parts = self
            .sorts
            .iter()
            .flat_map(|sort| {
                let expr = sort.expr.sql(alias);
                let direction = match sort.direction {
                    SortDirection::Asc => "ASC",
                    SortDirection::Desc => "DESC",
                };
                [
                    format!("({expr}) IS NULL ASC"),
                    format!("({expr}) {direction}"),
                ]
            })
            .collect::<Vec<_>>();
        format!(" ORDER BY {}", parts.join(", "))
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

    fn sql(&self, alias: &str) -> String {
        match self {
            FilterExpr::All(filters) => {
                if filters.is_empty() {
                    "1".to_owned()
                } else {
                    join_sql(filters, " AND ", alias)
                }
            }
            FilterExpr::Any(filters) => {
                if filters.is_empty() {
                    "0".to_owned()
                } else {
                    join_sql(filters, " OR ", alias)
                }
            }
            FilterExpr::Not(filter) => format!("NOT ({})", filter.sql(alias)),
            FilterExpr::Predicate(predicate) => predicate.sql(alias),
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
    fn sql(&self, alias: &str) -> String {
        match self {
            Predicate::Text { field, op, value } => {
                let field = field.sql(alias);
                let value = sql_string(value);
                match op {
                    TextOp::Contains => format!("instr({field}, {value}) > 0"),
                    TextOp::StartsWith => format!("substr({field}, 1, length({value})) = {value}"),
                    TextOp::EndsWith => format!("substr({field}, -length({value})) = {value}"),
                    TextOp::Equals => format!("{field} = {value}"),
                }
            }
            Predicate::Number { field, op } => {
                let field = field.sql(alias);
                match op {
                    NumberOp::Eq(value) => format!("({field} IS NOT NULL AND {field} = {value})"),
                    NumberOp::Ne(value) => format!("({field} IS NOT NULL AND {field} <> {value})"),
                    NumberOp::Lt(value) => format!("({field} IS NOT NULL AND {field} < {value})"),
                    NumberOp::Lte(value) => {
                        format!("({field} IS NOT NULL AND {field} <= {value})")
                    }
                    NumberOp::Gt(value) => format!("({field} IS NOT NULL AND {field} > {value})"),
                    NumberOp::Gte(value) => {
                        format!("({field} IS NOT NULL AND {field} >= {value})")
                    }
                    NumberOp::Between { min, max } => {
                        format!("({field} IS NOT NULL AND {field} >= {min} AND {field} <= {max})")
                    }
                }
            }
            Predicate::Bool { field, value } => {
                format!("{} = {}", field.sql(alias), if *value { 1 } else { 0 })
            }
            Predicate::Tags(predicate) => predicate.sql(alias),
            Predicate::Author(predicate) => predicate.sql(alias),
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
    fn sql(&self, alias: &str) -> String {
        match self {
            TagsPredicate::Intersects(values) => tag_exists_sql(alias, values, false),
            TagsPredicate::ContainsAll(values) => values
                .iter()
                .map(|value| {
                    format!(
                        "EXISTS (SELECT 1 FROM novel_tag nt WHERE nt.novel_id = {alias}.id AND nt.tag_id = {})",
                        sql_string(value)
                    )
                })
                .collect::<Vec<_>>()
                .join(" AND ")
                .if_empty("1"),
            TagsPredicate::ContainedBy(values) => {
                if values.is_empty() {
                    format!(
                        "NOT EXISTS (SELECT 1 FROM novel_tag nt WHERE nt.novel_id = {alias}.id)"
                    )
                } else {
                    let values = sql_string_list(values);
                    format!(
                        "NOT EXISTS (SELECT 1 FROM novel_tag nt WHERE nt.novel_id = {alias}.id AND nt.tag_id NOT IN ({values}))"
                    )
                }
            }
            TagsPredicate::Equals(values) => {
                let contains_all = TagsPredicate::ContainsAll(values.clone()).sql(alias);
                let contained_by = TagsPredicate::ContainedBy(values.clone()).sql(alias);
                format!("({contains_all}) AND ({contained_by})")
            }
            TagsPredicate::IsEmpty => {
                format!("NOT EXISTS (SELECT 1 FROM novel_tag nt WHERE nt.novel_id = {alias}.id)")
            }
            TagsPredicate::IsNotEmpty => {
                format!("EXISTS (SELECT 1 FROM novel_tag nt WHERE nt.novel_id = {alias}.id)")
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
    fn sql(&self, alias: &str) -> String {
        match self {
            AuthorPredicate::Is(author) => author.sql(alias),
            AuthorPredicate::In(authors) => join_author_sql(authors, " OR ", alias).if_empty("0"),
            AuthorPredicate::NotIn(authors) => {
                let sql = join_author_sql(authors, " OR ", alias);
                if sql.is_empty() {
                    "1".to_owned()
                } else {
                    format!("NOT ({sql})")
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
    fn sql(&self, alias: &str) -> String {
        match self {
            AuthorRef::Id(id) => {
                format!("({alias}.author_id IS NOT NULL AND {alias}.author_id = {id})")
            }
            AuthorRef::Name(name) => {
                format!(
                    "({alias}.author_id IS NULL AND {alias}.author_name = {})",
                    sql_string(name)
                )
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
            SortExpr::Constant(value) => value.to_string(),
            SortExpr::Add(left, right) => format!("({} + {})", left.sql(alias), right.sql(alias)),
            SortExpr::Sub(left, right) => format!("({} - {})", left.sql(alias), right.sql(alias)),
            SortExpr::Mul(left, right) => format!("({} * {})", left.sql(alias), right.sql(alias)),
            SortExpr::Div(left, right) => format!(
                "CASE WHEN ({}) = 0 THEN NULL ELSE (({}) * 1.0 / ({})) END",
                right.sql(alias),
                left.sql(alias),
                right.sql(alias)
            ),
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
            SortExpr::Constant(value) => Some(SortValue::Number(*value)),
            SortExpr::Add(left, right) => Some(SortValue::Number(
                left.eval_number(record)? + right.eval_number(record)?,
            )),
            SortExpr::Sub(left, right) => Some(SortValue::Number(
                left.eval_number(record)? - right.eval_number(record)?,
            )),
            SortExpr::Mul(left, right) => Some(SortValue::Number(
                left.eval_number(record)? * right.eval_number(record)?,
            )),
            SortExpr::Div(left, right) => {
                let right = right.eval_number(record)?;
                if right == 0. {
                    return None;
                }
                Some(SortValue::Number(left.eval_number(record)? / right))
            }
        }
    }

    #[cfg(test)]
    fn eval_number(&self, record: &NovelRecord) -> Option<f64> {
        match self.eval(record)? {
            SortValue::Number(value) => Some(value),
            SortValue::Text(_) | SortValue::Bool(_) => None,
        }
    }
}

fn join_sql(filters: &[FilterExpr], separator: &str, alias: &str) -> String {
    filters
        .iter()
        .map(|filter| format!("({})", filter.sql(alias)))
        .collect::<Vec<_>>()
        .join(separator)
}

fn join_author_sql(authors: &[AuthorRef], separator: &str, alias: &str) -> String {
    authors
        .iter()
        .map(|author| format!("({})", author.sql(alias)))
        .collect::<Vec<_>>()
        .join(separator)
}

fn tag_exists_sql(alias: &str, values: &HashSet<String>, negated: bool) -> String {
    if values.is_empty() {
        return if negated { "1" } else { "0" }.to_owned();
    }
    let values = sql_string_list(values);
    let op = if negated { "NOT IN" } else { "IN" };
    format!(
        "EXISTS (SELECT 1 FROM novel_tag nt WHERE nt.novel_id = {alias}.id AND nt.tag_id {op} ({values}))"
    )
}

fn sql_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn sql_string_list(values: &HashSet<String>) -> String {
    let mut values = values
        .iter()
        .map(|value| sql_string(value))
        .collect::<Vec<_>>();
    values.sort();
    values.join(", ")
}

trait IfEmpty {
    fn if_empty(self, fallback: &str) -> String;
}

impl IfEmpty for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.to_owned()
        } else {
            self
        }
    }
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
    fn sorting_supports_fields_math_priority_and_missing_last() {
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
                    expr: SortExpr::Div(
                        Box::new(SortExpr::Number(NumberField::ReadCount)),
                        Box::new(SortExpr::Number(NumberField::WordCount)),
                    ),
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

        let divide_by_zero = SortExpr::Div(
            Box::new(SortExpr::Number(NumberField::ReadCount)),
            Box::new(SortExpr::Constant(0.)),
        );
        assert_eq!(divide_by_zero.eval(&record(1)), None);

        assert_eq!(
            SortExpr::Sub(
                Box::new(SortExpr::Number(NumberField::ReadCount)),
                Box::new(SortExpr::Number(NumberField::ReplyCount)),
            )
            .eval(&record(1)),
            Some(SortValue::Number(90.))
        );
        assert_eq!(
            SortExpr::Add(
                Box::new(SortExpr::Number(NumberField::ReplyCount)),
                Box::new(SortExpr::Constant(5.)),
            )
            .eval(&record(1)),
            Some(SortValue::Number(15.))
        );
        assert_eq!(
            SortExpr::Mul(
                Box::new(SortExpr::Number(NumberField::ReplyCount)),
                Box::new(SortExpr::Constant(2.)),
            )
            .eval(&record(1)),
            Some(SortValue::Number(20.))
        );
        assert_eq!(
            SortExpr::Bool(BoolField::IsLimit).eval(&record(1)),
            Some(SortValue::Bool(false))
        );
    }
}

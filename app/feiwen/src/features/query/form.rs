use std::collections::HashSet;

use gpui_form::{
    FormSchema, ValidationDynamicItemsPath, ValidationDynamicPath, ValidationMessage,
    ValidationRequest, ValidationSink, Validator,
};

use super::advanced::options::{
    AuthorRelation, BoolRelation, FieldKind, GroupRelation, NumberRelation, SortField,
    TagsRelation, TextRelation,
};
#[cfg(test)]
use crate::store::query::SortExpr;
use crate::store::query::{
    AuthorPredicate, AuthorRef, BoolField, FilterExpr, NumberOp, Predicate, QuerySpec,
    SortDirection, SortSpec, TagsPredicate, TextField, TextOp,
};

#[derive(Clone, Debug, PartialEq, FormSchema)]
pub(crate) struct QueryDraft {
    #[form(child)]
    pub(crate) filters: FilterGroupDraft,
    #[form(items)]
    pub(crate) sorts: Vec<SortDraft>,
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
pub(crate) struct FilterGroupDraft {
    pub(crate) relation: GroupRelation,
    pub(crate) negated: bool,
    #[form(items)]
    pub(crate) children: Vec<FilterNodeDraft>,
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
pub(crate) struct FilterNodeDraft {
    #[form(child)]
    pub(crate) kind: FilterNodeKind,
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
pub(crate) enum FilterNodeKind {
    Condition(FilterConditionDraft),
    Group(FilterGroupDraft),
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
pub(crate) struct FilterConditionDraft {
    pub(crate) negated: bool,
    #[form(child)]
    pub(crate) field: ConditionField,
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
pub(crate) enum ConditionField {
    Unselected,
    Text(TextConditionDraft),
    Number(NumberConditionDraft),
    Bool(BoolConditionDraft),
    Tags(TagsConditionDraft),
    Author(AuthorConditionDraft),
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
pub(crate) struct TextConditionDraft {
    pub(crate) field: FieldKind,
    pub(crate) relation: Option<TextRelation>,
    pub(crate) value: String,
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
pub(crate) struct NumberConditionDraft {
    pub(crate) field: FieldKind,
    pub(crate) relation: Option<NumberRelation>,
    pub(crate) single: String,
    pub(crate) min: String,
    pub(crate) max: String,
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
pub(crate) struct BoolConditionDraft {
    pub(crate) relation: Option<BoolRelation>,
    pub(crate) value: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
pub(crate) struct TagsConditionDraft {
    pub(crate) relation: Option<TagsRelation>,
    pub(crate) values: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
pub(crate) struct AuthorConditionDraft {
    pub(crate) relation: Option<AuthorRelation>,
    pub(crate) text: String,
    pub(crate) single: Option<AuthorRef>,
    pub(crate) multiple: Vec<AuthorRef>,
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
pub(crate) struct SortDraft {
    pub(crate) field: Option<SortField>,
    pub(crate) direction: Option<SortDirection>,
}

impl Default for QueryDraft {
    fn default() -> Self {
        Self {
            filters: FilterGroupDraft {
                relation: GroupRelation::All,
                negated: false,
                children: vec![FilterNodeDraft::condition()],
            },
            sorts: Vec::new(),
        }
    }
}

impl FilterNodeDraft {
    pub(crate) fn condition() -> Self {
        Self {
            kind: FilterNodeKind::Condition(FilterConditionDraft {
                negated: false,
                field: ConditionField::Unselected,
            }),
        }
    }
}

impl ConditionField {
    pub(crate) fn for_field(field: FieldKind) -> Self {
        match field {
            FieldKind::Title | FieldKind::Description | FieldKind::LatestChapterTitle => {
                Self::Text(TextConditionDraft {
                    field,
                    relation: None,
                    value: String::new(),
                })
            }
            FieldKind::WordCount | FieldKind::ReadCount | FieldKind::ReplyCount => {
                Self::Number(NumberConditionDraft {
                    field,
                    relation: None,
                    single: String::new(),
                    min: String::new(),
                    max: String::new(),
                })
            }
            FieldKind::IsLimit => Self::Bool(BoolConditionDraft {
                relation: None,
                value: Some(false),
            }),
            FieldKind::Tags => Self::Tags(TagsConditionDraft {
                relation: None,
                values: Vec::new(),
            }),
            FieldKind::Author => Self::Author(AuthorConditionDraft {
                relation: None,
                text: String::new(),
                single: None,
                multiple: Vec::new(),
            }),
        }
    }

    pub(crate) fn field(&self) -> Option<FieldKind> {
        match self {
            Self::Unselected => None,
            Self::Text(value) => Some(value.field),
            Self::Number(value) => Some(value.field),
            Self::Bool(_) => Some(FieldKind::IsLimit),
            Self::Tags(_) => Some(FieldKind::Tags),
            Self::Author(_) => Some(FieldKind::Author),
        }
    }
}

pub(crate) struct QueryDraftValidator;

impl Validator<QueryDraft> for QueryDraftValidator {
    fn validate(
        &self,
        request: ValidationRequest<'_, QueryDraft>,
        out: &mut ValidationSink<'_, QueryDraft>,
    ) {
        let root_children = QueryDraft::ROOT
            .then(QueryDraft::FILTERS)
            .then(FilterGroupDraft::CHILDREN);
        let root_nodes = request.items(&root_children);
        validate_nodes(request.clone(), root_nodes, out);

        let sort_items = QueryDraft::ROOT.then(QueryDraft::SORTS);
        for sort in request.items(&sort_items) {
            let field = sort.then(SortDraft::FIELD);
            if request.try_get(&field).is_ok_and(Option::is_none) {
                out.at(field).error(
                    "sort_field_required",
                    ValidationMessage::key("query-validation-sort-field"),
                );
            }
        }
    }
}

fn validate_nodes(
    request: ValidationRequest<'_, QueryDraft>,
    nodes: Vec<gpui_form::ValidationItemPath<'_, QueryDraft, FilterNodeDraft>>,
    out: &mut ValidationSink<'_, QueryDraft>,
) {
    for node in nodes {
        let kind = node.then(FilterNodeDraft::KIND);
        let Ok(value) = request.try_get(&kind) else {
            continue;
        };
        match value {
            FilterNodeKind::Condition(_) => {
                let Ok(Some(condition)) = kind.case(FilterNodeKind::CONDITION).resolve(&request)
                else {
                    continue;
                };
                validate_condition(request.clone(), condition, out);
            }
            FilterNodeKind::Group(_) => {
                let Ok(Some(group)) = kind.case(FilterNodeKind::GROUP).resolve(&request) else {
                    continue;
                };
                let children: ValidationDynamicItemsPath<'_, QueryDraft, FilterNodeDraft> =
                    group.then(FilterGroupDraft::CHILDREN);
                let Ok(children) = request.try_items(&children) else {
                    continue;
                };
                validate_nodes(request.clone(), children, out);
            }
        }
    }
}

fn validate_condition(
    request: ValidationRequest<'_, QueryDraft>,
    condition: ValidationDynamicPath<'_, QueryDraft, FilterConditionDraft>,
    out: &mut ValidationSink<'_, QueryDraft>,
) {
    let field = condition.then(FilterConditionDraft::FIELD);
    let Ok(value) = request.try_get(&field) else {
        return;
    };
    match value {
        ConditionField::Unselected => out.at(field).error(
            "condition_field_required",
            ValidationMessage::key("query-validation-field"),
        ),
        ConditionField::Text(value) => {
            let Ok(Some(path)) = field.case(ConditionField::TEXT).resolve(&request) else {
                return;
            };
            if value.relation.is_none() {
                out.at(path.clone().then(TextConditionDraft::RELATION))
                    .error(
                        "condition_relation_required",
                        ValidationMessage::key("query-validation-relation"),
                    );
                return;
            }
            if value.value.trim().is_empty() {
                out.at(path.then(TextConditionDraft::VALUE)).error(
                    "condition_value_required",
                    ValidationMessage::key("query-validation-text"),
                );
            }
        }
        ConditionField::Number(value) => {
            let Ok(Some(path)) = field.case(ConditionField::NUMBER).resolve(&request) else {
                return;
            };
            let Some(relation) = value.relation else {
                out.at(path.then(NumberConditionDraft::RELATION)).error(
                    "condition_relation_required",
                    ValidationMessage::key("query-validation-relation"),
                );
                return;
            };
            if relation == NumberRelation::Between {
                validate_number(
                    &value.min,
                    path.clone().then(NumberConditionDraft::MIN),
                    out,
                );
                validate_number(
                    &value.max,
                    path.clone().then(NumberConditionDraft::MAX),
                    out,
                );
                if let (Ok(min), Ok(max)) = (
                    value.min.trim().parse::<i32>(),
                    value.max.trim().parse::<i32>(),
                ) && min > max
                {
                    out.at(path.then(NumberConditionDraft::MAX)).error(
                        "condition_range_reversed",
                        ValidationMessage::key("query-validation-range"),
                    );
                }
            } else {
                validate_number(&value.single, path.then(NumberConditionDraft::SINGLE), out);
            }
        }
        ConditionField::Bool(value) => {
            let Ok(Some(path)) = field.case(ConditionField::BOOL).resolve(&request) else {
                return;
            };
            if value.relation.is_none() {
                out.at(path.clone().then(BoolConditionDraft::RELATION))
                    .error(
                        "condition_relation_required",
                        ValidationMessage::key("query-validation-relation"),
                    );
                return;
            }
            if value.value.is_none() {
                out.at(path.then(BoolConditionDraft::VALUE)).error(
                    "condition_value_required",
                    ValidationMessage::key("query-validation-value"),
                );
            }
        }
        ConditionField::Tags(value) => {
            let Ok(Some(path)) = field.case(ConditionField::TAGS).resolve(&request) else {
                return;
            };
            let Some(relation) = value.relation else {
                out.at(path.then(TagsConditionDraft::RELATION)).error(
                    "condition_relation_required",
                    ValidationMessage::key("query-validation-relation"),
                );
                return;
            };
            if relation.needs_value() && value.values.is_empty() {
                out.at(path.then(TagsConditionDraft::VALUES)).error(
                    "condition_value_required",
                    ValidationMessage::key("query-validation-selection"),
                );
            }
        }
        ConditionField::Author(value) => {
            let Ok(Some(path)) = field.case(ConditionField::AUTHOR).resolve(&request) else {
                return;
            };
            let Some(relation) = value.relation else {
                out.at(path.then(AuthorConditionDraft::RELATION)).error(
                    "condition_relation_required",
                    ValidationMessage::key("query-validation-relation"),
                );
                return;
            };
            match relation {
                AuthorRelation::NameContains
                | AuthorRelation::NameStartsWith
                | AuthorRelation::NameEndsWith
                | AuthorRelation::NameEquals
                    if value.text.trim().is_empty() =>
                {
                    out.at(path.then(AuthorConditionDraft::TEXT)).error(
                        "condition_value_required",
                        ValidationMessage::key("query-validation-text"),
                    );
                }
                AuthorRelation::Is | AuthorRelation::IsNot if value.single.is_none() => {
                    out.at(path.then(AuthorConditionDraft::SINGLE)).error(
                        "condition_value_required",
                        ValidationMessage::key("query-validation-selection"),
                    );
                }
                AuthorRelation::In | AuthorRelation::NotIn if value.multiple.is_empty() => {
                    out.at(path.then(AuthorConditionDraft::MULTIPLE)).error(
                        "condition_value_required",
                        ValidationMessage::key("query-validation-selection"),
                    );
                }
                _ => {}
            }
        }
    }
}

fn validate_number(
    value: &str,
    path: ValidationDynamicPath<'_, QueryDraft, String>,
    out: &mut ValidationSink<'_, QueryDraft>,
) {
    if value.trim().parse::<i32>().is_err() {
        out.at(path).error(
            "condition_number_invalid",
            ValidationMessage::key("query-validation-number"),
        );
    }
}

impl QueryDraft {
    pub(crate) fn to_spec(&self) -> Result<QuerySpec, String> {
        Ok(QuerySpec {
            filter: self.filters.to_expr()?,
            sorts: self
                .sorts
                .iter()
                .map(SortDraft::to_spec)
                .collect::<Result<_, _>>()?,
        })
    }

    #[cfg(test)]
    pub(crate) fn from_spec(spec: &QuerySpec) -> Self {
        Self {
            filters: FilterGroupDraft::from_expr(&spec.filter),
            sorts: spec.sorts.iter().map(SortDraft::from_spec).collect(),
        }
    }
}

impl FilterGroupDraft {
    fn to_expr(&self) -> Result<FilterExpr, String> {
        let children = self
            .children
            .iter()
            .map(FilterNodeDraft::to_expr)
            .collect::<Result<Vec<_>, _>>()?;
        let expr = match self.relation {
            GroupRelation::All => FilterExpr::All(children),
            GroupRelation::Any => FilterExpr::Any(children),
        };
        Ok(if self.negated {
            FilterExpr::Not(Box::new(expr))
        } else {
            expr
        })
    }

    #[cfg(test)]
    fn from_expr(expr: &FilterExpr) -> Self {
        let (negated, expr) = match expr {
            FilterExpr::Not(inner)
                if matches!(inner.as_ref(), FilterExpr::All(_) | FilterExpr::Any(_)) =>
            {
                (true, inner.as_ref())
            }
            _ => (false, expr),
        };
        let (relation, children): (GroupRelation, &[FilterExpr]) = match expr {
            FilterExpr::All(children) => (GroupRelation::All, children.as_slice()),
            FilterExpr::Any(children) => (GroupRelation::Any, children.as_slice()),
            other => (GroupRelation::All, std::slice::from_ref(other)),
        };
        Self {
            relation,
            negated,
            children: children.iter().map(FilterNodeDraft::from_expr).collect(),
        }
    }
}

impl FilterNodeDraft {
    fn to_expr(&self) -> Result<FilterExpr, String> {
        match &self.kind {
            FilterNodeKind::Condition(condition) => condition.to_expr(),
            FilterNodeKind::Group(group) => group.to_expr(),
        }
    }

    #[cfg(test)]
    fn from_expr(expr: &FilterExpr) -> Self {
        if matches!(expr, FilterExpr::All(_) | FilterExpr::Any(_))
            || matches!(expr, FilterExpr::Not(inner) if matches!(inner.as_ref(), FilterExpr::All(_) | FilterExpr::Any(_)))
        {
            return Self {
                kind: FilterNodeKind::Group(FilterGroupDraft::from_expr(expr)),
            };
        }
        let (negated, expr) = match expr {
            FilterExpr::Not(inner) => (true, inner.as_ref()),
            _ => (false, expr),
        };
        Self {
            kind: FilterNodeKind::Condition(FilterConditionDraft {
                negated,
                field: ConditionField::from_expr(expr),
            }),
        }
    }
}

impl FilterConditionDraft {
    fn to_expr(&self) -> Result<FilterExpr, String> {
        let expr = self.field.to_expr()?;
        Ok(if self.negated {
            FilterExpr::Not(Box::new(expr))
        } else {
            expr
        })
    }
}

impl ConditionField {
    fn to_expr(&self) -> Result<FilterExpr, String> {
        let predicate = match self {
            Self::Unselected => return Err("请选择字段".to_owned()),
            Self::Text(value) => Predicate::Text {
                field: value
                    .field
                    .text_field()
                    .ok_or_else(|| "字段与文本条件不匹配".to_owned())?,
                op: match value.relation.ok_or_else(|| "请选择条件".to_owned())? {
                    TextRelation::Contains => TextOp::Contains,
                    TextRelation::StartsWith => TextOp::StartsWith,
                    TextRelation::EndsWith => TextOp::EndsWith,
                    TextRelation::Equals => TextOp::Equals,
                },
                value: non_empty(&value.value, "请输入文本")?,
            },
            Self::Number(value) => Predicate::Number {
                field: value
                    .field
                    .number_field()
                    .ok_or_else(|| "字段与数字条件不匹配".to_owned())?,
                op: value.to_op()?,
            },
            Self::Bool(value) => {
                value.relation.ok_or_else(|| "请选择条件".to_owned())?;
                Predicate::Bool {
                    field: BoolField::IsLimit,
                    value: value.value.ok_or_else(|| "请选择有效项".to_owned())?,
                }
            }
            Self::Tags(value) => Predicate::Tags(value.to_predicate()?),
            Self::Author(value) => return value.to_expr(),
        };
        Ok(FilterExpr::Predicate(predicate))
    }

    #[cfg(test)]
    fn from_expr(expr: &FilterExpr) -> Self {
        match expr {
            FilterExpr::Predicate(Predicate::Text { field, op, value }) => {
                if *field == TextField::AuthorName {
                    return Self::Author(AuthorConditionDraft {
                        relation: Some(match op {
                            TextOp::Contains => AuthorRelation::NameContains,
                            TextOp::StartsWith => AuthorRelation::NameStartsWith,
                            TextOp::EndsWith => AuthorRelation::NameEndsWith,
                            TextOp::Equals => AuthorRelation::NameEquals,
                        }),
                        text: value.clone(),
                        single: None,
                        multiple: Vec::new(),
                    });
                }
                Self::Text(TextConditionDraft {
                    field: match field {
                        TextField::Title => FieldKind::Title,
                        TextField::Description => FieldKind::Description,
                        TextField::LatestChapter => FieldKind::LatestChapterTitle,
                        TextField::AuthorName => unreachable!(),
                    },
                    relation: Some(match op {
                        TextOp::Contains => TextRelation::Contains,
                        TextOp::StartsWith => TextRelation::StartsWith,
                        TextOp::EndsWith => TextRelation::EndsWith,
                        TextOp::Equals => TextRelation::Equals,
                    }),
                    value: value.clone(),
                })
            }
            FilterExpr::Predicate(Predicate::Number { field, op }) => {
                let (relation, single, min, max) = NumberConditionDraft::from_op(*op);
                Self::Number(NumberConditionDraft {
                    field: match field {
                        crate::store::query::NumberField::WordCount => FieldKind::WordCount,
                        crate::store::query::NumberField::ReadCount => FieldKind::ReadCount,
                        crate::store::query::NumberField::ReplyCount => FieldKind::ReplyCount,
                        _ => FieldKind::WordCount,
                    },
                    relation: Some(relation),
                    single,
                    min,
                    max,
                })
            }
            FilterExpr::Predicate(Predicate::Bool { value, .. }) => {
                Self::Bool(BoolConditionDraft {
                    relation: Some(BoolRelation::Is),
                    value: Some(*value),
                })
            }
            FilterExpr::Predicate(Predicate::Tags(predicate)) => {
                Self::Tags(TagsConditionDraft::from_predicate(predicate))
            }
            FilterExpr::Predicate(Predicate::Author(predicate)) => {
                Self::Author(AuthorConditionDraft::from_predicate(predicate))
            }
            _ => Self::Unselected,
        }
    }
}

impl NumberConditionDraft {
    fn to_op(&self) -> Result<NumberOp, String> {
        let relation = self.relation.ok_or_else(|| "请选择条件".to_owned())?;
        if relation == NumberRelation::Between {
            let min = parse_i32(&self.min)?;
            let max = parse_i32(&self.max)?;
            if min > max {
                return Err("最大值必须大于或等于最小值".to_owned());
            }
            return Ok(NumberOp::Between { min, max });
        }
        let value = parse_i32(&self.single)?;
        Ok(match relation {
            NumberRelation::Eq => NumberOp::Eq(value),
            NumberRelation::Ne => NumberOp::Ne(value),
            NumberRelation::Lt => NumberOp::Lt(value),
            NumberRelation::Lte => NumberOp::Lte(value),
            NumberRelation::Gt => NumberOp::Gt(value),
            NumberRelation::Gte => NumberOp::Gte(value),
            NumberRelation::Between => unreachable!(),
        })
    }

    #[cfg(test)]
    fn from_op(op: NumberOp) -> (NumberRelation, String, String, String) {
        match op {
            NumberOp::Eq(value) => (
                NumberRelation::Eq,
                value.to_string(),
                String::new(),
                String::new(),
            ),
            NumberOp::Ne(value) => (
                NumberRelation::Ne,
                value.to_string(),
                String::new(),
                String::new(),
            ),
            NumberOp::Lt(value) => (
                NumberRelation::Lt,
                value.to_string(),
                String::new(),
                String::new(),
            ),
            NumberOp::Lte(value) => (
                NumberRelation::Lte,
                value.to_string(),
                String::new(),
                String::new(),
            ),
            NumberOp::Gt(value) => (
                NumberRelation::Gt,
                value.to_string(),
                String::new(),
                String::new(),
            ),
            NumberOp::Gte(value) => (
                NumberRelation::Gte,
                value.to_string(),
                String::new(),
                String::new(),
            ),
            NumberOp::Between { min, max } => (
                NumberRelation::Between,
                String::new(),
                min.to_string(),
                max.to_string(),
            ),
        }
    }
}

impl TagsConditionDraft {
    fn to_predicate(&self) -> Result<TagsPredicate, String> {
        let relation = self.relation.ok_or_else(|| "请选择条件".to_owned())?;
        let values = self.values.iter().cloned().collect::<HashSet<_>>();
        if relation.needs_value() && values.is_empty() {
            return Err("请选择至少一项".to_owned());
        }
        Ok(match relation {
            TagsRelation::Intersects => TagsPredicate::Intersects(values),
            TagsRelation::ContainsAll => TagsPredicate::ContainsAll(values),
            TagsRelation::ContainedBy => TagsPredicate::ContainedBy(values),
            TagsRelation::Equals => TagsPredicate::Equals(values),
            TagsRelation::IsEmpty => TagsPredicate::IsEmpty,
            TagsRelation::IsNotEmpty => TagsPredicate::IsNotEmpty,
        })
    }

    #[cfg(test)]
    fn from_predicate(predicate: &TagsPredicate) -> Self {
        let (relation, values) = match predicate {
            TagsPredicate::Intersects(values) => (TagsRelation::Intersects, values),
            TagsPredicate::ContainsAll(values) => (TagsRelation::ContainsAll, values),
            TagsPredicate::ContainedBy(values) => (TagsRelation::ContainedBy, values),
            TagsPredicate::Equals(values) => (TagsRelation::Equals, values),
            TagsPredicate::IsEmpty => {
                return Self {
                    relation: Some(TagsRelation::IsEmpty),
                    values: Vec::new(),
                };
            }
            TagsPredicate::IsNotEmpty => {
                return Self {
                    relation: Some(TagsRelation::IsNotEmpty),
                    values: Vec::new(),
                };
            }
        };
        let mut values = values.iter().cloned().collect::<Vec<_>>();
        values.sort();
        Self {
            relation: Some(relation),
            values,
        }
    }
}

impl AuthorConditionDraft {
    fn to_expr(&self) -> Result<FilterExpr, String> {
        let relation = self.relation.ok_or_else(|| "请选择条件".to_owned())?;
        match relation {
            AuthorRelation::NameContains
            | AuthorRelation::NameStartsWith
            | AuthorRelation::NameEndsWith
            | AuthorRelation::NameEquals => Ok(FilterExpr::Predicate(Predicate::Text {
                field: TextField::AuthorName,
                op: match relation {
                    AuthorRelation::NameContains => TextOp::Contains,
                    AuthorRelation::NameStartsWith => TextOp::StartsWith,
                    AuthorRelation::NameEndsWith => TextOp::EndsWith,
                    AuthorRelation::NameEquals => TextOp::Equals,
                    _ => unreachable!(),
                },
                value: non_empty(&self.text, "请输入文本")?,
            })),
            AuthorRelation::Is | AuthorRelation::IsNot => {
                let author = self
                    .single
                    .clone()
                    .ok_or_else(|| "请选择有效作者".to_owned())?;
                Ok(FilterExpr::Predicate(Predicate::Author(
                    if relation == AuthorRelation::IsNot {
                        AuthorPredicate::IsNot(author)
                    } else {
                        AuthorPredicate::Is(author)
                    },
                )))
            }
            AuthorRelation::In | AuthorRelation::NotIn => {
                if self.multiple.is_empty() {
                    return Err("请选择至少一项".to_owned());
                }
                Ok(FilterExpr::Predicate(Predicate::Author(
                    if relation == AuthorRelation::In {
                        AuthorPredicate::In(self.multiple.clone())
                    } else {
                        AuthorPredicate::NotIn(self.multiple.clone())
                    },
                )))
            }
        }
    }

    #[cfg(test)]
    fn from_predicate(predicate: &AuthorPredicate) -> Self {
        match predicate {
            AuthorPredicate::Is(author) => Self {
                relation: Some(AuthorRelation::Is),
                text: String::new(),
                single: Some(author.clone()),
                multiple: Vec::new(),
            },
            AuthorPredicate::IsNot(author) => Self {
                relation: Some(AuthorRelation::IsNot),
                text: String::new(),
                single: Some(author.clone()),
                multiple: Vec::new(),
            },
            AuthorPredicate::In(authors) => Self {
                relation: Some(AuthorRelation::In),
                text: String::new(),
                single: None,
                multiple: authors.clone(),
            },
            AuthorPredicate::NotIn(authors) => Self {
                relation: Some(AuthorRelation::NotIn),
                text: String::new(),
                single: None,
                multiple: authors.clone(),
            },
        }
    }
}

impl SortDraft {
    fn to_spec(&self) -> Result<SortSpec, String> {
        Ok(SortSpec {
            expr: self
                .field
                .ok_or_else(|| "请选择排序字段".to_owned())?
                .sort_expr(),
            direction: self.direction.ok_or_else(|| "请选择排序方向".to_owned())?,
        })
    }

    #[cfg(test)]
    fn from_spec(spec: &SortSpec) -> Self {
        Self {
            field: Some(match spec.expr {
                SortExpr::Text(TextField::Title) => SortField::Title,
                SortExpr::Text(TextField::AuthorName) => SortField::AuthorName,
                SortExpr::Number(crate::store::query::NumberField::NovelId) => SortField::NovelId,
                SortExpr::Number(crate::store::query::NumberField::LatestChapterId) => {
                    SortField::LatestChapterId
                }
                SortExpr::Text(TextField::LatestChapter) => SortField::LatestChapterTitle,
                SortExpr::Text(TextField::Description) => {
                    unreachable!("query form does not produce description sort expressions")
                }
                SortExpr::Number(crate::store::query::NumberField::WordCount) => {
                    SortField::WordCount
                }
                SortExpr::Number(crate::store::query::NumberField::ReadCount) => {
                    SortField::ReadCount
                }
                SortExpr::Number(crate::store::query::NumberField::ReplyCount) => {
                    SortField::ReplyCount
                }
                SortExpr::Number(crate::store::query::NumberField::AuthorId) => SortField::AuthorId,
                SortExpr::Bool(BoolField::IsLimit) => SortField::IsLimit,
            }),
            direction: Some(spec.direction),
        }
    }
}

fn non_empty(value: &str, message: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        Err(message.to_owned())
    } else {
        Ok(value.to_owned())
    }
}

fn parse_i32(value: &str) -> Result<i32, String> {
    value
        .trim()
        .parse()
        .map_err(|_| "请输入有效数字".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{AppContext as _, TestAppContext};
    use gpui_form::{Form, ResolveError, ValidationTrigger};

    #[test]
    fn recursive_query_draft_round_trips_through_query_spec() {
        let draft = QueryDraft {
            filters: FilterGroupDraft {
                relation: GroupRelation::Any,
                negated: true,
                children: vec![FilterNodeDraft {
                    kind: FilterNodeKind::Group(FilterGroupDraft {
                        relation: GroupRelation::All,
                        negated: false,
                        children: vec![FilterNodeDraft {
                            kind: FilterNodeKind::Condition(FilterConditionDraft {
                                negated: false,
                                field: ConditionField::Text(TextConditionDraft {
                                    field: FieldKind::Title,
                                    relation: Some(TextRelation::Contains),
                                    value: "测试".to_owned(),
                                }),
                            }),
                        }],
                    }),
                }],
            },
            sorts: vec![SortDraft {
                field: Some(SortField::Title),
                direction: Some(SortDirection::Desc),
            }],
        };
        let spec = draft.to_spec().unwrap();
        assert_eq!(QueryDraft::from_spec(&spec), draft);
    }

    #[test]
    fn author_is_not_and_condition_negation_round_trip_without_changing_the_form_shape() {
        let draft = QueryDraft {
            filters: FilterGroupDraft {
                relation: GroupRelation::All,
                negated: false,
                children: vec![FilterNodeDraft {
                    kind: FilterNodeKind::Condition(FilterConditionDraft {
                        negated: true,
                        field: ConditionField::Author(AuthorConditionDraft {
                            relation: Some(AuthorRelation::IsNot),
                            text: String::new(),
                            single: Some(AuthorRef::Id(42)),
                            multiple: Vec::new(),
                        }),
                    }),
                }],
            },
            sorts: Vec::new(),
        };

        let spec = draft.to_spec().unwrap();
        assert_eq!(QueryDraft::from_spec(&spec), draft);
    }

    #[gpui::test]
    fn relation_changes_preserve_all_number_operands(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let form = cx.new(|_| {
                Form::new(QueryDraft {
                    filters: FilterGroupDraft {
                        relation: GroupRelation::All,
                        negated: false,
                        children: vec![FilterNodeDraft {
                            kind: FilterNodeKind::Condition(FilterConditionDraft {
                                negated: false,
                                field: ConditionField::Number(NumberConditionDraft {
                                    field: FieldKind::WordCount,
                                    relation: Some(NumberRelation::Eq),
                                    single: "100".to_owned(),
                                    min: "10".to_owned(),
                                    max: "200".to_owned(),
                                }),
                            }),
                        }],
                    },
                    sorts: Vec::new(),
                })
            });
            let children = QueryDraft::ROOT
                .then(QueryDraft::FILTERS)
                .then(FilterGroupDraft::CHILDREN);
            let item = children.items(&form, cx).remove(0);
            let condition = item
                .then(FilterNodeDraft::KIND)
                .case(FilterNodeKind::CONDITION)
                .resolve(&form, cx)
                .unwrap()
                .unwrap();
            let number = condition
                .then(FilterConditionDraft::FIELD)
                .case(ConditionField::NUMBER)
                .resolve(&form, cx)
                .unwrap()
                .unwrap();

            number
                .clone()
                .then(NumberConditionDraft::RELATION)
                .try_set(&form, Some(NumberRelation::Between), cx)
                .unwrap();
            assert_eq!(
                number
                    .clone()
                    .then(NumberConditionDraft::SINGLE)
                    .try_get(&form, cx)
                    .unwrap(),
                "100"
            );
            assert_eq!(
                number
                    .clone()
                    .then(NumberConditionDraft::MIN)
                    .try_get(&form, cx)
                    .unwrap(),
                "10"
            );
            assert_eq!(
                number
                    .then(NumberConditionDraft::MAX)
                    .try_get(&form, cx)
                    .unwrap(),
                "200"
            );
        });
    }

    #[gpui::test]
    fn field_type_change_retires_the_old_payload_and_installs_empty_values(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            let form = cx.new(|_| {
                Form::new(QueryDraft {
                    filters: FilterGroupDraft {
                        relation: GroupRelation::All,
                        negated: false,
                        children: vec![FilterNodeDraft {
                            kind: FilterNodeKind::Condition(FilterConditionDraft {
                                negated: false,
                                field: ConditionField::Number(NumberConditionDraft {
                                    field: FieldKind::ReadCount,
                                    relation: Some(NumberRelation::Eq),
                                    single: "12".to_owned(),
                                    min: "1".to_owned(),
                                    max: "20".to_owned(),
                                }),
                            }),
                        }],
                    },
                    sorts: Vec::new(),
                })
            });
            let item = QueryDraft::ROOT
                .then(QueryDraft::FILTERS)
                .then(FilterGroupDraft::CHILDREN)
                .items(&form, cx)
                .remove(0);
            let condition = item
                .then(FilterNodeDraft::KIND)
                .case(FilterNodeKind::CONDITION)
                .resolve(&form, cx)
                .unwrap()
                .unwrap();
            let field = condition.then(FilterConditionDraft::FIELD);
            let old_single = field
                .clone()
                .case(ConditionField::NUMBER)
                .resolve(&form, cx)
                .unwrap()
                .unwrap()
                .then(NumberConditionDraft::SINGLE);

            field
                .clone()
                .try_set(&form, ConditionField::for_field(FieldKind::Title), cx)
                .unwrap();
            assert!(matches!(
                old_single.try_get(&form, cx),
                Err(ResolveError::Retired { .. })
            ));
            let text = field
                .case(ConditionField::TEXT)
                .resolve(&form, cx)
                .unwrap()
                .unwrap();
            assert_eq!(
                text.clone()
                    .then(TextConditionDraft::RELATION)
                    .try_get(&form, cx)
                    .unwrap(),
                None
            );
            assert_eq!(
                text.then(TextConditionDraft::VALUE)
                    .try_get(&form, cx)
                    .unwrap(),
                ""
            );
        });
    }

    #[gpui::test]
    fn recursive_item_reorder_and_delete_preserve_runtime_identity(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let nested_children = vec![FilterNodeDraft::condition(), FilterNodeDraft::condition()];
            let form = cx.new(|_| {
                Form::new(QueryDraft {
                    filters: FilterGroupDraft {
                        relation: GroupRelation::All,
                        negated: false,
                        children: vec![FilterNodeDraft {
                            kind: FilterNodeKind::Group(FilterGroupDraft {
                                relation: GroupRelation::Any,
                                negated: false,
                                children: nested_children,
                            }),
                        }],
                    },
                    sorts: Vec::new(),
                })
            });
            let group_item = QueryDraft::ROOT
                .then(QueryDraft::FILTERS)
                .then(FilterGroupDraft::CHILDREN)
                .items(&form, cx)
                .remove(0);
            let group = group_item
                .then(FilterNodeDraft::KIND)
                .case(FilterNodeKind::GROUP)
                .resolve(&form, cx)
                .unwrap()
                .unwrap();
            let children = group.then(FilterGroupDraft::CHILDREN);
            let items = children.try_items(&form, cx).unwrap();
            let first = items[0].clone();
            let second = items[1].clone();
            let first_key = first.key();
            let second_key = second.key();

            children.move_before(&form, &second, &first, cx).unwrap();
            let reordered = children.try_items(&form, cx).unwrap();
            assert_eq!(reordered[0].key(), second_key);
            assert_eq!(reordered[1].key(), first_key);

            children.remove(&form, first.clone(), cx).unwrap();
            assert!(matches!(
                first.try_get(&form, cx),
                Err(ResolveError::Retired { .. })
            ));
            assert_eq!(children.try_items(&form, cx).unwrap()[0].key(), second_key);
        });
    }

    #[gpui::test]
    fn missing_relation_only_reports_the_relation_until_submit_can_validate_the_value(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            let form = cx.new(|_| {
                Form::new(QueryDraft {
                    filters: FilterGroupDraft {
                        relation: GroupRelation::All,
                        negated: false,
                        children: vec![FilterNodeDraft {
                            kind: FilterNodeKind::Condition(FilterConditionDraft {
                                negated: false,
                                field: ConditionField::Text(TextConditionDraft {
                                    field: FieldKind::Title,
                                    relation: None,
                                    value: String::new(),
                                }),
                            }),
                        }],
                    },
                    sorts: Vec::new(),
                })
                .with_validator(QueryDraftValidator)
            });
            let item = QueryDraft::ROOT
                .then(QueryDraft::FILTERS)
                .then(FilterGroupDraft::CHILDREN)
                .items(&form, cx)
                .remove(0);
            let text = item
                .then(FilterNodeDraft::KIND)
                .case(FilterNodeKind::CONDITION)
                .resolve(&form, cx)
                .unwrap()
                .unwrap()
                .then(FilterConditionDraft::FIELD)
                .case(ConditionField::TEXT)
                .resolve(&form, cx)
                .unwrap()
                .unwrap();
            let relation = text.clone().then(TextConditionDraft::RELATION);
            let value = text.then(TextConditionDraft::VALUE);
            form.update(cx, |form, cx| form.validate(ValidationTrigger::Submit, cx));

            assert_eq!(relation.try_errors(&form, cx).unwrap().len(), 1);
            assert!(value.try_errors(&form, cx).unwrap().is_empty());
        });
    }
}

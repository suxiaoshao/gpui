use super::{
    options::{
        AuthorRelation, FieldKind, GroupRelation, NumberRelation, TagsRelation, TextRelation,
    },
    state::{
        AdvancedQueryState, AuthorCondition, AuthorValue, ConditionDraft, ConditionRow,
        FilterGroup, FilterNode, NumberValue, SortRow, TagsCondition,
    },
};
use crate::store::query::{
    AuthorPredicate, BoolField, FilterExpr, NumberOp, Predicate, QuerySpec, SortSpec,
    TagsPredicate, TextField, TextOp,
};
use gpui_component::input::InputState;
use std::collections::HashSet;

impl AdvancedQueryState {
    pub(crate) fn query_spec(&mut self, cx: &gpui::App) -> Result<QuerySpec, String> {
        Self::clear_group_errors(&mut self.root);
        for sort in &mut self.sorts {
            sort.error = None;
        }
        let filter = Self::group_expr(&mut self.root, cx)?;
        let sorts = self
            .sorts
            .iter_mut()
            .map(|sort| sort.sort_spec())
            .collect::<Result<Vec<_>, _>>()?;
        Ok(QuerySpec { filter, sorts })
    }

    pub(super) fn clear_group_errors(group: &mut FilterGroup) {
        for item in &mut group.items {
            match item {
                FilterNode::Condition(condition) => condition.error = None,
                FilterNode::Group(group) => Self::clear_group_errors(group),
            }
        }
    }

    fn group_expr(group: &mut FilterGroup, cx: &gpui::App) -> Result<FilterExpr, String> {
        let filters = group
            .items
            .iter_mut()
            .map(|item| match item {
                FilterNode::Condition(condition) => condition.expr(cx),
                FilterNode::Group(group) => Self::group_expr(group, cx),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let expr = match group.relation {
            GroupRelation::All => FilterExpr::All(filters),
            GroupRelation::Any => FilterExpr::Any(filters),
        };
        Ok(if group.negated {
            FilterExpr::Not(Box::new(expr))
        } else {
            expr
        })
    }
}

impl SortRow {
    fn sort_spec(&mut self) -> Result<SortSpec, String> {
        let Some(field) = self.field else {
            let message = "请选择排序字段".to_owned();
            self.error = Some(message.clone());
            return Err(message);
        };
        Ok(SortSpec {
            expr: field.sort_expr(),
            direction: self.direction,
        })
    }
}

impl ConditionRow {
    fn expr(&mut self, cx: &gpui::App) -> Result<FilterExpr, String> {
        let expr = match self.expr_inner(cx) {
            Ok(expr) => expr,
            Err(err) => {
                self.error = Some(err.clone());
                return Err(err);
            }
        };
        Ok(if self.negated {
            FilterExpr::Not(Box::new(expr))
        } else {
            expr
        })
    }

    fn expr_inner(&self, cx: &gpui::App) -> Result<FilterExpr, String> {
        let predicate = match &self.draft {
            ConditionDraft::NoField => return Err("请选择字段".to_owned()),
            ConditionDraft::NoCondition { .. } => return Err("请选择条件".to_owned()),
            ConditionDraft::Text(condition) => {
                let value = condition.input.read(cx).value().trim().to_owned();
                if value.is_empty() {
                    return Err("请输入文本".to_owned());
                }
                let field = condition
                    .field
                    .text_field()
                    .ok_or_else(|| "字段与文本条件不匹配".to_owned())?;
                Predicate::Text {
                    field,
                    op: match condition.relation {
                        TextRelation::Contains => TextOp::Contains,
                        TextRelation::StartsWith => TextOp::StartsWith,
                        TextRelation::EndsWith => TextOp::EndsWith,
                        TextRelation::Equals => TextOp::Equals,
                    },
                    value,
                }
            }
            ConditionDraft::Number(condition) => Predicate::Number {
                field: condition
                    .field
                    .number_field()
                    .ok_or_else(|| "字段与数字条件不匹配".to_owned())?,
                op: number_op(condition.relation, &condition.value, cx)?,
            },
            ConditionDraft::Bool(condition) => {
                let value = condition
                    .value_select
                    .read(cx)
                    .selected_value()
                    .copied()
                    .ok_or_else(|| "请选择有效项".to_owned())?;
                Predicate::Bool {
                    field: BoolField::IsLimit,
                    value,
                }
            }
            ConditionDraft::Tags(condition) => match condition.relation {
                TagsRelation::Intersects => {
                    TagsPredicate::Intersects(selected_tags(condition, cx)?)
                }
                TagsRelation::ContainsAll => {
                    TagsPredicate::ContainsAll(selected_tags(condition, cx)?)
                }
                TagsRelation::ContainedBy => {
                    TagsPredicate::ContainedBy(selected_tags(condition, cx)?)
                }
                TagsRelation::Equals => TagsPredicate::Equals(selected_tags(condition, cx)?),
                TagsRelation::IsEmpty => TagsPredicate::IsEmpty,
                TagsRelation::IsNotEmpty => TagsPredicate::IsNotEmpty,
            }
            .into(),
            ConditionDraft::Author(condition) => return author_expr(condition, cx),
        };
        Ok(FilterExpr::Predicate(predicate))
    }
}

impl ConditionDraft {
    pub(super) fn field(&self) -> Option<FieldKind> {
        match self {
            ConditionDraft::NoField => None,
            ConditionDraft::NoCondition { field, .. }
            | ConditionDraft::Text(super::state::TextCondition { field, .. })
            | ConditionDraft::Number(super::state::NumberCondition { field, .. }) => Some(*field),
            ConditionDraft::Bool(_) => Some(FieldKind::IsLimit),
            ConditionDraft::Tags(_) => Some(FieldKind::Tags),
            ConditionDraft::Author(_) => Some(FieldKind::Author),
        }
    }
}

impl From<TagsPredicate> for Predicate {
    fn from(value: TagsPredicate) -> Self {
        Self::Tags(value)
    }
}

fn number_op(
    relation: NumberRelation,
    value: &NumberValue,
    cx: &gpui::App,
) -> Result<NumberOp, String> {
    Ok(match (relation, value) {
        (NumberRelation::Between, NumberValue::Range(range)) => {
            let (min, max) = parse_range(range, cx)?;
            NumberOp::Between { min, max }
        }
        (relation, NumberValue::Single(input)) => {
            let value = parse_i32(input, cx)?;
            match relation {
                NumberRelation::Eq => NumberOp::Eq(value),
                NumberRelation::Ne => NumberOp::Ne(value),
                NumberRelation::Lt => NumberOp::Lt(value),
                NumberRelation::Lte => NumberOp::Lte(value),
                NumberRelation::Gt => NumberOp::Gt(value),
                NumberRelation::Gte => NumberOp::Gte(value),
                NumberRelation::Between => return Err("请填写有效范围".to_owned()),
            }
        }
        _ => return Err("请输入有效数字".to_owned()),
    })
}

fn parse_i32(input: &gpui::Entity<InputState>, cx: &gpui::App) -> Result<i32, String> {
    let value = input.read(cx).value().trim().to_owned();
    if value.is_empty() {
        return Err("请输入数字".to_owned());
    }
    value.parse().map_err(|_| "请输入有效数字".to_owned())
}

fn parse_range(
    range: &gpui::Entity<super::components::NumericRangeInputState>,
    cx: &gpui::App,
) -> Result<(i32, i32), String> {
    range.read(cx).values(cx).map_err(|err| match err {
        super::components::RangeInputError::Missing => "请填写最小值和最大值".to_owned(),
        super::components::RangeInputError::InvalidNumber => "请输入有效数字".to_owned(),
        super::components::RangeInputError::Reversed => "最大值必须大于或等于最小值".to_owned(),
    })
}

fn selected_tags(condition: &TagsCondition, cx: &gpui::App) -> Result<HashSet<String>, String> {
    let Some(value) = &condition.value else {
        return Ok(HashSet::new());
    };
    let values = value.read(cx).selected_values();
    if values.is_empty() {
        return Err("请选择至少一项".to_owned());
    }
    Ok(values.into_iter().collect())
}

fn author_expr(condition: &AuthorCondition, cx: &gpui::App) -> Result<FilterExpr, String> {
    match &condition.value {
        AuthorValue::Text(input) => {
            let value = input.read(cx).value().trim().to_owned();
            if value.is_empty() {
                return Err("请输入文本".to_owned());
            }
            Ok(FilterExpr::Predicate(author_text_predicate(
                condition.relation,
                value,
            )?))
        }
        AuthorValue::Single(value) => {
            let author = value
                .read(cx)
                .selected_value()
                .cloned()
                .ok_or_else(|| "请选择有效作者".to_owned())?;
            match condition.relation {
                AuthorRelation::Is => Ok(author_is_expr(author)),
                AuthorRelation::IsNot => Ok(author_is_not_expr(author)),
                _ => Err("作者条件和值输入器不匹配".to_owned()),
            }
        }
        AuthorValue::Multi(value) => {
            let authors = value.read(cx).selected_values();
            if authors.is_empty() {
                return Err("请选择至少一项".to_owned());
            }
            let predicate = match condition.relation {
                AuthorRelation::In => AuthorPredicate::In(authors),
                AuthorRelation::NotIn => AuthorPredicate::NotIn(authors),
                _ => return Err("作者条件和值输入器不匹配".to_owned()),
            };
            Ok(FilterExpr::Predicate(Predicate::Author(predicate)))
        }
    }
}

fn author_text_predicate(relation: AuthorRelation, value: String) -> Result<Predicate, String> {
    Ok(Predicate::Text {
        field: TextField::AuthorName,
        op: author_text_op(relation)?,
        value,
    })
}

fn author_text_op(relation: AuthorRelation) -> Result<TextOp, String> {
    match relation {
        AuthorRelation::NameContains => Ok(TextOp::Contains),
        AuthorRelation::NameStartsWith => Ok(TextOp::StartsWith),
        AuthorRelation::NameEndsWith => Ok(TextOp::EndsWith),
        AuthorRelation::NameEquals => Ok(TextOp::Equals),
        _ => Err("作者条件和值输入器不匹配".to_owned()),
    }
}

fn author_is_expr(author: crate::store::query::AuthorRef) -> FilterExpr {
    FilterExpr::Predicate(Predicate::Author(AuthorPredicate::Is(author)))
}

fn author_is_not_expr(author: crate::store::query::AuthorRef) -> FilterExpr {
    FilterExpr::Not(Box::new(author_is_expr(author)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::query::AuthorRef;

    #[test]
    fn author_name_relations_map_to_author_name_text_predicates() {
        assert_eq!(
            author_text_predicate(AuthorRelation::NameContains, "张三".to_owned()),
            Ok(Predicate::Text {
                field: TextField::AuthorName,
                op: TextOp::Contains,
                value: "张三".to_owned(),
            })
        );
        assert_eq!(
            author_text_predicate(AuthorRelation::NameStartsWith, "张".to_owned()),
            Ok(Predicate::Text {
                field: TextField::AuthorName,
                op: TextOp::StartsWith,
                value: "张".to_owned(),
            })
        );
        assert_eq!(
            author_text_predicate(AuthorRelation::NameEndsWith, "三".to_owned()),
            Ok(Predicate::Text {
                field: TextField::AuthorName,
                op: TextOp::EndsWith,
                value: "三".to_owned(),
            })
        );
        assert_eq!(
            author_text_predicate(AuthorRelation::NameEquals, "张三".to_owned()),
            Ok(Predicate::Text {
                field: TextField::AuthorName,
                op: TextOp::Equals,
                value: "张三".to_owned(),
            })
        );
    }

    #[test]
    fn author_is_not_relation_maps_to_not_author_is_expression() {
        assert_eq!(
            author_is_not_expr(AuthorRef::Id(42)),
            FilterExpr::Not(Box::new(FilterExpr::Predicate(Predicate::Author(
                AuthorPredicate::Is(AuthorRef::Id(42)),
            ))))
        );
    }
}

use super::{
    options::{
        AuthorOption, AuthorRelation, BoolRelation, FieldKind, FieldSelectItems, GroupRelation,
        NumberRelation, QueryOptions, SelectChoice, SortDirectionChoice, SortField, TagsRelation,
        TextRelation, author_relation_items, bool_relation_items, bool_value_items, field_items,
        number_relation_items, sort_direction_items, sort_field_items, tags_relation_items,
        text_relation_items,
    },
    sort::move_sort_before,
};
use crate::{
    components::{EntityPickerState, MultiSelectState, NumericRangeInputState, RangeInputError},
    store::query::{
        AuthorPredicate, BoolField, FilterExpr, NumberOp, Predicate, QuerySpec, SortDirection,
        SortSpec, TagsPredicate, TextField, TextOp,
    },
};
use gpui::{AppContext, Context, Entity, Subscription, Window};
use gpui_component::{
    IndexPath,
    input::InputState,
    select::{SelectEvent, SelectItem, SelectState},
};
use std::collections::HashSet;

use super::super::QueryView;

type FieldSelectState = SelectState<FieldSelectItems>;
type TextRelationSelectState = SelectState<Vec<SelectChoice<TextRelation>>>;
type NumberRelationSelectState = SelectState<Vec<SelectChoice<NumberRelation>>>;
type BoolRelationSelectState = SelectState<Vec<SelectChoice<BoolRelation>>>;
type BoolValueSelectState = SelectState<Vec<SelectChoice<bool>>>;
type TagsRelationSelectState = SelectState<Vec<SelectChoice<TagsRelation>>>;
type AuthorRelationSelectState = SelectState<Vec<SelectChoice<AuthorRelation>>>;
type SortFieldSelectState = SelectState<Vec<SelectChoice<SortField>>>;
type SortDirectionSelectState = SelectState<Vec<SelectChoice<SortDirectionChoice>>>;

pub(crate) struct AdvancedQueryState {
    pub(super) root: FilterGroup,
    pub(super) sorts: Vec<SortRow>,
    pub(super) options: QueryOptions,
    pub(super) disabled: bool,
    next_id: u64,
    subscriptions: Vec<Subscription>,
}

pub(super) struct FilterGroup {
    pub(super) id: u64,
    pub(super) relation: GroupRelation,
    pub(super) negated: bool,
    pub(super) items: Vec<FilterNode>,
}

pub(super) enum FilterNode {
    Condition(ConditionRow),
    Group(FilterGroup),
}

pub(super) struct ConditionRow {
    pub(super) id: u64,
    pub(super) negated: bool,
    pub(super) field_select: Entity<FieldSelectState>,
    pub(super) draft: ConditionDraft,
    pub(super) error: Option<String>,
}

pub(super) enum ConditionDraft {
    NoField,
    NoCondition {
        field: FieldKind,
        relation_select: RelationSelect,
    },
    Text(TextCondition),
    Number(NumberCondition),
    Bool(BoolCondition),
    Tags(TagsCondition),
    Author(AuthorCondition),
}

pub(super) enum RelationSelect {
    Text(Entity<TextRelationSelectState>),
    Number(Entity<NumberRelationSelectState>),
    Bool(Entity<BoolRelationSelectState>),
    Tags(Entity<TagsRelationSelectState>),
    Author(Entity<AuthorRelationSelectState>),
}

pub(super) struct TextCondition {
    pub(super) field: FieldKind,
    pub(super) relation: TextRelation,
    pub(super) relation_select: Entity<TextRelationSelectState>,
    pub(super) input: Entity<InputState>,
}

pub(super) struct NumberCondition {
    pub(super) field: FieldKind,
    pub(super) relation: NumberRelation,
    pub(super) relation_select: Entity<NumberRelationSelectState>,
    pub(super) value: NumberValue,
}

pub(super) enum NumberValue {
    Single(Entity<InputState>),
    Range(Entity<NumericRangeInputState>),
}

pub(super) struct BoolCondition {
    pub(super) relation_select: Entity<BoolRelationSelectState>,
    pub(super) value_select: Entity<BoolValueSelectState>,
}

pub(super) struct TagsCondition {
    pub(super) relation: TagsRelation,
    pub(super) relation_select: Entity<TagsRelationSelectState>,
    pub(super) value: Option<Entity<MultiSelectState<super::options::TagOption>>>,
}

pub(super) struct AuthorCondition {
    pub(super) relation: AuthorRelation,
    pub(super) relation_select: Entity<AuthorRelationSelectState>,
    pub(super) value: AuthorValue,
}

pub(super) enum AuthorValue {
    Text(Entity<InputState>),
    Single(Entity<EntityPickerState<AuthorOption>>),
    Multi(Entity<MultiSelectState<AuthorOption>>),
}

pub(super) struct SortRow {
    pub(super) id: u64,
    pub(super) field: Option<SortField>,
    pub(super) direction: SortDirection,
    pub(super) field_select: Entity<SortFieldSelectState>,
    pub(super) direction_select: Entity<SortDirectionSelectState>,
    pub(super) error: Option<String>,
}

impl AdvancedQueryState {
    pub(crate) fn new(
        options: QueryOptions,
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) -> Self {
        let mut this = Self {
            root: FilterGroup {
                id: 0,
                relation: GroupRelation::All,
                negated: false,
                items: Vec::new(),
            },
            sorts: Vec::new(),
            options,
            disabled: false,
            next_id: 1,
            subscriptions: Vec::new(),
        };
        this.add_condition(0, window, cx);
        this
    }

    pub(crate) fn set_options(&mut self, options: QueryOptions, cx: &mut Context<QueryView>) {
        self.options = options;
        Self::refresh_group_options(&self.options, &mut self.root, cx);
    }

    pub(crate) fn set_disabled(&mut self, disabled: bool, cx: &mut Context<QueryView>) {
        self.disabled = disabled;
        Self::set_group_disabled(&mut self.root, disabled, cx);
    }

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

    pub(crate) fn add_condition(
        &mut self,
        group_id: u64,
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) {
        if self.disabled {
            return;
        }
        let id = self.alloc_id();
        let field_select =
            cx.new(|cx| SelectState::new(field_items(), None, window, cx).searchable(true));
        self.subscriptions.push(cx.subscribe_in(
            &field_select,
            window,
            move |this, _, event: &SelectEvent<FieldSelectItems>, window, cx| {
                if let SelectEvent::Confirm(Some(field)) = event {
                    this.advanced.set_condition_field(id, *field, window, cx);
                    cx.notify();
                }
            },
        ));
        let condition = ConditionRow {
            id,
            negated: false,
            field_select,
            draft: ConditionDraft::NoField,
            error: None,
        };
        if let Some(group) = self.find_group_mut(group_id) {
            group.items.push(FilterNode::Condition(condition));
        }
    }

    pub(crate) fn add_group(&mut self, group_id: u64) {
        if self.disabled {
            return;
        }
        let id = self.alloc_id();
        if let Some(group) = self.find_group_mut(group_id) {
            group.items.push(FilterNode::Group(FilterGroup {
                id,
                relation: GroupRelation::All,
                negated: false,
                items: Vec::new(),
            }));
        }
    }

    pub(crate) fn remove_node(&mut self, node_id: u64) {
        if self.disabled {
            return;
        }
        Self::remove_node_from(&mut self.root, node_id);
    }

    pub(super) fn set_group_relation(&mut self, group_id: u64, relation: GroupRelation) {
        if self.disabled {
            return;
        }
        if let Some(group) = self.find_group_mut(group_id) {
            group.relation = relation;
        }
    }

    pub(crate) fn set_group_negated(&mut self, group_id: u64, negated: bool) {
        if self.disabled {
            return;
        }
        if let Some(group) = self.find_group_mut(group_id) {
            group.negated = negated;
        }
    }

    pub(crate) fn set_condition_negated(&mut self, condition_id: u64, negated: bool) {
        if self.disabled {
            return;
        }
        if let Some(condition) = self.find_condition_mut(condition_id) {
            condition.negated = negated;
        }
    }

    pub(crate) fn add_sort(&mut self, window: &mut Window, cx: &mut Context<QueryView>) {
        if self.disabled {
            return;
        }
        let id = self.alloc_id();
        let sort = self.new_sort_row(id, Some(SortField::Title), SortDirection::Asc, window, cx);
        self.sorts.push(sort);
    }

    pub(crate) fn remove_sort(&mut self, sort_id: u64) {
        if self.disabled {
            return;
        }
        self.sorts.retain(|sort| sort.id != sort_id);
    }

    pub(crate) fn move_sort_before(&mut self, source_id: u64, target_id: u64) {
        if self.disabled {
            return;
        }
        move_sort_before(&mut self.sorts, source_id, target_id);
    }

    fn set_condition_field(
        &mut self,
        condition_id: u64,
        field: FieldKind,
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) {
        if self.disabled {
            return;
        }
        let relation_select = match field {
            FieldKind::Title | FieldKind::Description | FieldKind::LatestChapterTitle => {
                RelationSelect::Text(self.new_text_relation_select(condition_id, window, cx))
            }
            FieldKind::WordCount | FieldKind::ReadCount | FieldKind::ReplyCount => {
                RelationSelect::Number(self.new_number_relation_select(condition_id, window, cx))
            }
            FieldKind::IsLimit => {
                RelationSelect::Bool(self.new_bool_relation_select(condition_id, window, cx))
            }
            FieldKind::Tags => {
                RelationSelect::Tags(self.new_tags_relation_select(condition_id, window, cx))
            }
            FieldKind::Author => {
                RelationSelect::Author(self.new_author_relation_select(condition_id, window, cx))
            }
        };
        if let Some(condition) = self.find_condition_mut(condition_id) {
            condition.error = None;
            condition.draft = ConditionDraft::NoCondition {
                field,
                relation_select,
            };
        }
    }

    fn set_text_relation(
        &mut self,
        condition_id: u64,
        relation: TextRelation,
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) {
        if self.disabled {
            return;
        }
        let Some(field) = self.condition_field(condition_id) else {
            return;
        };
        let relation_select = self.new_text_relation_select(condition_id, window, cx);
        relation_select.update(cx, |select, cx| {
            select.set_selected_value(&relation, window, cx);
        });
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("输入文本"));
        if let Some(condition) = self.find_condition_mut(condition_id) {
            condition.error = None;
            condition.draft = ConditionDraft::Text(TextCondition {
                field,
                relation,
                relation_select,
                input,
            });
        }
    }

    fn set_number_relation(
        &mut self,
        condition_id: u64,
        relation: NumberRelation,
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) {
        if self.disabled {
            return;
        }
        let Some(field) = self.condition_field(condition_id) else {
            return;
        };
        let relation_select = self.new_number_relation_select(condition_id, window, cx);
        relation_select.update(cx, |select, cx| {
            select.set_selected_value(&relation, window, cx);
        });
        let value = match relation {
            NumberRelation::Between => NumberValue::Range(
                cx.new(|cx| NumericRangeInputState::new("最小值", "最大值", window, cx)),
            ),
            _ => NumberValue::Single(
                cx.new(|cx| InputState::new(window, cx).placeholder("输入数字")),
            ),
        };
        if let Some(condition) = self.find_condition_mut(condition_id) {
            condition.error = None;
            condition.draft = ConditionDraft::Number(NumberCondition {
                field,
                relation,
                relation_select,
                value,
            });
        }
    }

    fn set_bool_relation(
        &mut self,
        condition_id: u64,
        _relation: BoolRelation,
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) {
        if self.disabled {
            return;
        }
        let relation_select = self.new_bool_relation_select(condition_id, window, cx);
        relation_select.update(cx, |select, cx| {
            select.set_selected_value(&BoolRelation::Is, window, cx);
        });
        let value_select = cx.new(|cx| {
            SelectState::new(
                bool_value_items(),
                Some(IndexPath::default().row(1)),
                window,
                cx,
            )
        });
        if let Some(condition) = self.find_condition_mut(condition_id) {
            condition.error = None;
            condition.draft = ConditionDraft::Bool(BoolCondition {
                relation_select,
                value_select,
            });
        }
    }

    fn set_tags_relation(
        &mut self,
        condition_id: u64,
        relation: TagsRelation,
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) {
        if self.disabled {
            return;
        }
        let relation_select = self.new_tags_relation_select(condition_id, window, cx);
        relation_select.update(cx, |select, cx| {
            select.set_selected_value(&relation, window, cx);
        });
        let value = relation.needs_value().then(|| {
            cx.new(|cx| MultiSelectState::new(self.options.tags.clone(), "选择标签", window, cx))
        });
        if let Some(condition) = self.find_condition_mut(condition_id) {
            condition.error = None;
            condition.draft = ConditionDraft::Tags(TagsCondition {
                relation,
                relation_select,
                value,
            });
        }
    }

    fn set_author_relation(
        &mut self,
        condition_id: u64,
        relation: AuthorRelation,
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) {
        if self.disabled {
            return;
        }
        let relation_select = self.new_author_relation_select(condition_id, window, cx);
        relation_select.update(cx, |select, cx| {
            select.set_selected_value(&relation, window, cx);
        });
        let value = match relation {
            AuthorRelation::NameContains
            | AuthorRelation::NameStartsWith
            | AuthorRelation::NameEndsWith
            | AuthorRelation::NameEquals => {
                AuthorValue::Text(cx.new(|cx| InputState::new(window, cx).placeholder("输入文本")))
            }
            AuthorRelation::Is | AuthorRelation::IsNot => AuthorValue::Single(cx.new(|cx| {
                EntityPickerState::new(self.options.authors.clone(), "选择作者", window, cx)
            })),
            AuthorRelation::In | AuthorRelation::NotIn => AuthorValue::Multi(cx.new(|cx| {
                MultiSelectState::new(self.options.authors.clone(), "选择作者", window, cx)
            })),
        };
        if let Some(condition) = self.find_condition_mut(condition_id) {
            condition.error = None;
            condition.draft = ConditionDraft::Author(AuthorCondition {
                relation,
                relation_select,
                value,
            });
        }
    }

    fn set_sort_field(&mut self, sort_id: u64, field: Option<SortField>) {
        if self.disabled {
            return;
        }
        if let Some(sort) = self.sorts.iter_mut().find(|sort| sort.id == sort_id) {
            sort.field = field;
            sort.error = None;
        }
    }

    fn set_sort_direction(&mut self, sort_id: u64, direction: SortDirection) {
        if self.disabled {
            return;
        }
        if let Some(sort) = self.sorts.iter_mut().find(|sort| sort.id == sort_id) {
            sort.direction = direction;
        }
    }

    fn new_text_relation_select(
        &mut self,
        condition_id: u64,
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) -> Entity<TextRelationSelectState> {
        let select = cx.new(|cx| SelectState::new(text_relation_items(), None, window, cx));
        self.subscriptions.push(cx.subscribe_in(
            &select,
            window,
            move |this, _, event: &SelectEvent<Vec<SelectChoice<TextRelation>>>, window, cx| {
                if let SelectEvent::Confirm(Some(relation)) = event {
                    this.advanced
                        .set_text_relation(condition_id, *relation, window, cx);
                    cx.notify();
                }
            },
        ));
        select
    }

    fn new_number_relation_select(
        &mut self,
        condition_id: u64,
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) -> Entity<NumberRelationSelectState> {
        let select = cx.new(|cx| SelectState::new(number_relation_items(), None, window, cx));
        self.subscriptions.push(cx.subscribe_in(
            &select,
            window,
            move |this, _, event: &SelectEvent<Vec<SelectChoice<NumberRelation>>>, window, cx| {
                if let SelectEvent::Confirm(Some(relation)) = event {
                    this.advanced
                        .set_number_relation(condition_id, *relation, window, cx);
                    cx.notify();
                }
            },
        ));
        select
    }

    fn new_bool_relation_select(
        &mut self,
        condition_id: u64,
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) -> Entity<BoolRelationSelectState> {
        let select = cx.new(|cx| SelectState::new(bool_relation_items(), None, window, cx));
        self.subscriptions.push(cx.subscribe_in(
            &select,
            window,
            move |this, _, event: &SelectEvent<Vec<SelectChoice<BoolRelation>>>, window, cx| {
                if let SelectEvent::Confirm(Some(relation)) = event {
                    this.advanced
                        .set_bool_relation(condition_id, *relation, window, cx);
                    cx.notify();
                }
            },
        ));
        select
    }

    fn new_tags_relation_select(
        &mut self,
        condition_id: u64,
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) -> Entity<TagsRelationSelectState> {
        let select = cx.new(|cx| SelectState::new(tags_relation_items(), None, window, cx));
        self.subscriptions.push(cx.subscribe_in(
            &select,
            window,
            move |this, _, event: &SelectEvent<Vec<SelectChoice<TagsRelation>>>, window, cx| {
                if let SelectEvent::Confirm(Some(relation)) = event {
                    this.advanced
                        .set_tags_relation(condition_id, *relation, window, cx);
                    cx.notify();
                }
            },
        ));
        select
    }

    fn new_author_relation_select(
        &mut self,
        condition_id: u64,
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) -> Entity<AuthorRelationSelectState> {
        let select = cx.new(|cx| SelectState::new(author_relation_items(), None, window, cx));
        self.subscriptions.push(cx.subscribe_in(
            &select,
            window,
            move |this, _, event: &SelectEvent<Vec<SelectChoice<AuthorRelation>>>, window, cx| {
                if let SelectEvent::Confirm(Some(relation)) = event {
                    this.advanced
                        .set_author_relation(condition_id, *relation, window, cx);
                    cx.notify();
                }
            },
        ));
        select
    }

    fn new_sort_row(
        &mut self,
        id: u64,
        field: Option<SortField>,
        direction: SortDirection,
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) -> SortRow {
        let field_index = field.and_then(|field| {
            sort_field_items()
                .iter()
                .position(|item| *item.value() == field)
                .map(|row| IndexPath::default().row(row))
        });
        let direction_choice = match direction {
            SortDirection::Asc => SortDirectionChoice::Asc,
            SortDirection::Desc => SortDirectionChoice::Desc,
        };
        let direction_index = sort_direction_items()
            .iter()
            .position(|item| *item.value() == direction_choice)
            .map(|row| IndexPath::default().row(row));
        let field_select =
            cx.new(|cx| SelectState::new(sort_field_items(), field_index, window, cx));
        let direction_select =
            cx.new(|cx| SelectState::new(sort_direction_items(), direction_index, window, cx));
        self.subscriptions.push(cx.subscribe_in(
            &field_select,
            window,
            move |this, _, event: &SelectEvent<Vec<SelectChoice<SortField>>>, _, cx| {
                let SelectEvent::Confirm(field) = event;
                this.advanced.set_sort_field(id, *field);
                cx.notify();
            },
        ));
        self.subscriptions.push(cx.subscribe_in(
            &direction_select,
            window,
            move |this, _, event: &SelectEvent<Vec<SelectChoice<SortDirectionChoice>>>, _, cx| {
                if let SelectEvent::Confirm(Some(direction)) = event {
                    let direction = match direction {
                        SortDirectionChoice::Asc => SortDirection::Asc,
                        SortDirectionChoice::Desc => SortDirection::Desc,
                    };
                    this.advanced.set_sort_direction(id, direction);
                    cx.notify();
                }
            },
        ));
        SortRow {
            id,
            field,
            direction,
            field_select,
            direction_select,
            error: None,
        }
    }

    fn condition_field(&self, condition_id: u64) -> Option<FieldKind> {
        self.root
            .find_condition(condition_id)
            .and_then(|condition| condition.draft.field())
    }

    fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn find_group_mut(&mut self, group_id: u64) -> Option<&mut FilterGroup> {
        self.root.find_group_mut(group_id)
    }

    fn find_condition_mut(&mut self, condition_id: u64) -> Option<&mut ConditionRow> {
        self.root.find_condition_mut(condition_id)
    }

    fn remove_node_from(group: &mut FilterGroup, node_id: u64) -> bool {
        if let Some(ix) = group.items.iter().position(|item| item.id() == node_id) {
            group.items.remove(ix);
            return true;
        }
        group.items.iter_mut().any(|item| match item {
            FilterNode::Condition(_) => false,
            FilterNode::Group(group) => Self::remove_node_from(group, node_id),
        })
    }

    fn refresh_group_options(
        options: &QueryOptions,
        group: &mut FilterGroup,
        cx: &mut Context<QueryView>,
    ) {
        for item in &mut group.items {
            match item {
                FilterNode::Condition(condition) => {
                    refresh_condition_options(options, condition, cx)
                }
                FilterNode::Group(group) => Self::refresh_group_options(options, group, cx),
            }
        }
    }

    fn set_group_disabled(group: &mut FilterGroup, disabled: bool, cx: &mut Context<QueryView>) {
        for item in &mut group.items {
            match item {
                FilterNode::Condition(condition) => condition.set_disabled(disabled, cx),
                FilterNode::Group(group) => Self::set_group_disabled(group, disabled, cx),
            }
        }
    }

    fn clear_group_errors(group: &mut FilterGroup) {
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

impl FilterGroup {
    fn find_group_mut(&mut self, group_id: u64) -> Option<&mut FilterGroup> {
        if self.id == group_id {
            return Some(self);
        }
        self.items.iter_mut().find_map(|item| match item {
            FilterNode::Condition(_) => None,
            FilterNode::Group(group) => group.find_group_mut(group_id),
        })
    }

    fn find_condition_mut(&mut self, condition_id: u64) -> Option<&mut ConditionRow> {
        self.items.iter_mut().find_map(|item| match item {
            FilterNode::Condition(condition) if condition.id == condition_id => Some(condition),
            FilterNode::Condition(_) => None,
            FilterNode::Group(group) => group.find_condition_mut(condition_id),
        })
    }

    fn find_condition(&self, condition_id: u64) -> Option<&ConditionRow> {
        self.items.iter().find_map(|item| match item {
            FilterNode::Condition(condition) if condition.id == condition_id => Some(condition),
            FilterNode::Condition(_) => None,
            FilterNode::Group(group) => group.find_condition(condition_id),
        })
    }
}

impl FilterNode {
    fn id(&self) -> u64 {
        match self {
            FilterNode::Condition(condition) => condition.id,
            FilterNode::Group(group) => group.id,
        }
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
    fn set_disabled(&mut self, disabled: bool, cx: &mut Context<QueryView>) {
        match &self.draft {
            ConditionDraft::Number(condition) => {
                if let NumberValue::Range(range) = &condition.value {
                    range.update(cx, |range, cx| range.set_disabled(disabled, cx));
                }
            }
            ConditionDraft::Tags(condition) => {
                if let Some(value) = &condition.value {
                    value.update(cx, |value, cx| value.set_disabled(disabled, cx));
                }
            }
            ConditionDraft::Author(condition) => match &condition.value {
                AuthorValue::Single(value) => {
                    value.update(cx, |value, cx| value.set_disabled(disabled, cx));
                }
                AuthorValue::Multi(value) => {
                    value.update(cx, |value, cx| value.set_disabled(disabled, cx));
                }
                AuthorValue::Text(_) => {}
            },
            ConditionDraft::NoField
            | ConditionDraft::NoCondition { .. }
            | ConditionDraft::Text(_)
            | ConditionDraft::Bool(_) => {}
        }
    }

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
    fn field(&self) -> Option<FieldKind> {
        match self {
            ConditionDraft::NoField => None,
            ConditionDraft::NoCondition { field, .. }
            | ConditionDraft::Text(TextCondition { field, .. })
            | ConditionDraft::Number(NumberCondition { field, .. }) => Some(*field),
            ConditionDraft::Bool(_) => Some(FieldKind::IsLimit),
            ConditionDraft::Tags(_) => Some(FieldKind::Tags),
            ConditionDraft::Author(_) => Some(FieldKind::Author),
        }
    }
}

impl From<TagsPredicate> for Predicate {
    fn from(value: TagsPredicate) -> Self {
        Predicate::Tags(value)
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

fn parse_i32(input: &Entity<InputState>, cx: &gpui::App) -> Result<i32, String> {
    let value = input.read(cx).value().trim().to_owned();
    if value.is_empty() {
        return Err("请输入数字".to_owned());
    }
    value
        .parse::<i32>()
        .map_err(|_| "请输入有效数字".to_owned())
}

fn parse_range(
    range: &Entity<NumericRangeInputState>,
    cx: &gpui::App,
) -> Result<(i32, i32), String> {
    range.read(cx).values(cx).map_err(|err| match err {
        RangeInputError::Missing => "请填写最小值和最大值".to_owned(),
        RangeInputError::InvalidNumber => "请输入有效数字".to_owned(),
        RangeInputError::Reversed => "最大值必须大于或等于最小值".to_owned(),
    })
}

fn selected_tags(condition: &TagsCondition, cx: &gpui::App) -> Result<HashSet<String>, String> {
    let Some(value) = &condition.value else {
        return Ok(HashSet::new());
    };
    let values = value.read(cx).selected_keys();
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
                .selected_key()
                .ok_or_else(|| "请选择有效作者".to_owned())?;
            match condition.relation {
                AuthorRelation::Is => Ok(author_is_expr(author)),
                AuthorRelation::IsNot => Ok(author_is_not_expr(author)),
                _ => Err("作者条件和值输入器不匹配".to_owned()),
            }
        }
        AuthorValue::Multi(value) => {
            let authors = value.read(cx).selected_keys();
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

fn refresh_condition_options(
    options: &QueryOptions,
    condition: &mut ConditionRow,
    cx: &mut Context<QueryView>,
) {
    match &condition.draft {
        ConditionDraft::Tags(TagsCondition {
            value: Some(value), ..
        }) => {
            value.update(cx, |value, cx| value.set_options(options.tags.clone(), cx));
        }
        ConditionDraft::Author(AuthorCondition {
            value: AuthorValue::Single(value),
            ..
        }) => {
            value.update(cx, |value, cx| {
                value.set_options(options.authors.clone(), cx)
            });
        }
        ConditionDraft::Author(AuthorCondition {
            value: AuthorValue::Multi(value),
            ..
        }) => {
            value.update(cx, |value, cx| {
                value.set_options(options.authors.clone(), cx)
            });
        }
        _ => {}
    }
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

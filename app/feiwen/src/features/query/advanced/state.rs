use super::{
    components::NumericRangeInputState,
    options::{
        AuthorOption, AuthorRelation, BoolRelation, FieldKind, FieldSelectItems, GroupRelation,
        NumberRelation, QueryOptions, SelectChoice, SortDirectionChoice, SortField, TagsRelation,
        TextRelation, author_relation_items, bool_relation_items, bool_value_items, field_items,
        number_relation_items, sort_direction_items, sort_field_items, tags_relation_items,
        text_relation_items,
    },
    sort::move_sort_before,
};
use crate::store::query::SortDirection;
use gpui::{AppContext, Context, Entity, Subscription, Window};
use gpui_component::{
    IndexPath,
    combobox::ComboboxState,
    input::InputState,
    select::{SearchableVec, SelectEvent, SelectItem, SelectState},
};

use super::super::QueryView;

type FieldSelectState = SelectState<FieldSelectItems>;
type TextRelationSelectState = SelectState<Vec<SelectChoice<TextRelation>>>;
type NumberRelationSelectState = SelectState<Vec<SelectChoice<NumberRelation>>>;
type BoolRelationSelectState = SelectState<Vec<SelectChoice<BoolRelation>>>;
type BoolValueSelectState = SelectState<Vec<SelectChoice<bool>>>;
type TagsRelationSelectState = SelectState<Vec<SelectChoice<TagsRelation>>>;
type AuthorRelationSelectState = SelectState<Vec<SelectChoice<AuthorRelation>>>;
type AuthorSelectState = SelectState<SearchableVec<AuthorOption>>;
pub(super) type TagComboboxState = ComboboxState<SearchableVec<super::options::TagOption>>;
pub(super) type AuthorComboboxState = ComboboxState<SearchableVec<AuthorOption>>;
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
    pub(super) value: Option<Entity<TagComboboxState>>,
}

pub(super) struct AuthorCondition {
    pub(super) relation: AuthorRelation,
    pub(super) relation_select: Entity<AuthorRelationSelectState>,
    pub(super) value: AuthorValue,
}

pub(super) enum AuthorValue {
    Text(Entity<InputState>),
    Single(Entity<AuthorSelectState>),
    Multi(Entity<AuthorComboboxState>),
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

    pub(crate) fn set_disabled(&mut self, disabled: bool, cx: &mut Context<QueryView>) {
        self.disabled = disabled;
        Self::set_group_disabled(&mut self.root, disabled, cx);
    }

    pub(crate) fn condition_count(&self) -> usize {
        self.root.condition_count()
    }

    pub(crate) fn sort_count(&self) -> usize {
        self.sorts.len()
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
            cx.new(|cx| {
                ComboboxState::new(
                    SearchableVec::new(self.options.tags.clone()),
                    Vec::new(),
                    window,
                    cx,
                )
                .multiple(true)
                .searchable(true)
            })
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
                SelectState::new(
                    SearchableVec::new(self.options.authors.clone()),
                    None,
                    window,
                    cx,
                )
                .searchable(true)
            })),
            AuthorRelation::In | AuthorRelation::NotIn => AuthorValue::Multi(cx.new(|cx| {
                ComboboxState::new(
                    SearchableVec::new(self.options.authors.clone()),
                    Vec::new(),
                    window,
                    cx,
                )
                .multiple(true)
                .searchable(true)
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

    fn set_group_disabled(group: &mut FilterGroup, disabled: bool, cx: &mut Context<QueryView>) {
        for item in &mut group.items {
            match item {
                FilterNode::Condition(condition) => condition.set_disabled(disabled, cx),
                FilterNode::Group(group) => Self::set_group_disabled(group, disabled, cx),
            }
        }
    }
}

impl FilterGroup {
    fn condition_count(&self) -> usize {
        self.items
            .iter()
            .map(|node| match node {
                FilterNode::Condition(_) => 1,
                FilterNode::Group(group) => group.condition_count(),
            })
            .sum()
    }

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

impl ConditionRow {
    fn set_disabled(&mut self, disabled: bool, cx: &mut Context<QueryView>) {
        match &self.draft {
            ConditionDraft::Number(condition) => {
                if let NumberValue::Range(range) = &condition.value {
                    range.update(cx, |range, cx| range.set_disabled(disabled, cx));
                }
            }
            ConditionDraft::Author(condition) => match &condition.value {
                AuthorValue::Single(_) => {}
                AuthorValue::Multi(_) => {}
                AuthorValue::Text(_) => {}
            },
            ConditionDraft::NoField
            | ConditionDraft::NoCondition { .. }
            | ConditionDraft::Text(_)
            | ConditionDraft::Tags(_)
            | ConditionDraft::Bool(_) => {}
        }
    }
}

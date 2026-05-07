use super::QueryView;
use crate::store::query::{
    AuthorPredicate, AuthorRef, BoolField, FilterExpr, NumberField, NumberOp, Predicate, QuerySpec,
    SortDirection, SortExpr, SortSpec, TagsPredicate, TextField, TextOp,
};
use gpui::{
    AnyElement, AppContext, Context, Entity, IntoElement, ParentElement, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, Sizable, StyledExt,
    button::{Button, ButtonVariants, Toggle, ToggleGroup, ToggleVariants},
    h_flex,
    input::{Input, InputState},
    label::Label,
    scroll::ScrollableElement,
    tag::Tag,
    v_flex,
};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GroupKind {
    All,
    Any,
    Not,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum QueryField {
    Title,
    Description,
    LatestChapter,
    AuthorName,
    NovelId,
    LatestChapterId,
    WordCount,
    ReadCount,
    ReplyCount,
    AuthorId,
    IsLimit,
    Tags,
    Author,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum QueryOperator {
    Contains,
    StartsWith,
    EndsWith,
    Equals,
    NotEquals,
    LessThan,
    LessThanOrEquals,
    GreaterThan,
    GreaterThanOrEquals,
    Between,
    Intersects,
    ContainsAll,
    ContainedBy,
    SetEquals,
    IsEmpty,
    IsNotEmpty,
    In,
    NotIn,
    NameContains,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SortUiExpr {
    Title,
    Author,
    IsLimit,
    WordCount,
    ReadCount,
    ReplyCount,
    LatestChapter,
    ReadPerWord,
    ReplyPlusRead,
    WordMinusReply,
    ReplyTimesTwo,
}

struct ConditionRow {
    id: u64,
    field: QueryField,
    operator: QueryOperator,
    value_input: Entity<InputState>,
}

struct FilterGroup {
    id: u64,
    kind: GroupKind,
    items: Vec<FilterNode>,
}

enum FilterNode {
    Condition(ConditionRow),
    Group(FilterGroup),
}

struct SortRow {
    id: u64,
    expr: SortUiExpr,
    direction: SortDirection,
}

pub(crate) struct AdvancedQueryState {
    root: FilterGroup,
    sorts: Vec<SortRow>,
    next_id: u64,
}

impl AdvancedQueryState {
    pub(crate) fn new() -> Self {
        Self {
            root: FilterGroup {
                id: 0,
                kind: GroupKind::All,
                items: Vec::new(),
            },
            sorts: Vec::new(),
            next_id: 1,
        }
    }

    pub(crate) fn query_spec(
        &self,
        quick_query: &str,
        selected_tags: &HashSet<String>,
        cx: &gpui::App,
    ) -> Result<QuerySpec, String> {
        let mut filters = Vec::new();
        if !quick_query.is_empty() {
            filters.push(FilterExpr::Any(vec![
                FilterExpr::Predicate(Predicate::Text {
                    field: TextField::Title,
                    op: TextOp::Contains,
                    value: quick_query.to_owned(),
                }),
                FilterExpr::Predicate(Predicate::Text {
                    field: TextField::AuthorName,
                    op: TextOp::Contains,
                    value: quick_query.to_owned(),
                }),
            ]));
        }
        if !selected_tags.is_empty() {
            filters.push(FilterExpr::Predicate(Predicate::Tags(
                TagsPredicate::ContainsAll(selected_tags.clone()),
            )));
        }
        let advanced = self.group_expr(&self.root, cx)?;
        if !matches!(&advanced, FilterExpr::All(items) if items.is_empty()) {
            filters.push(advanced);
        }

        Ok(QuerySpec {
            filter: FilterExpr::All(filters),
            sorts: self.sorts.iter().map(SortRow::sort_spec).collect(),
        })
    }

    pub(crate) fn add_condition(
        &mut self,
        group_id: u64,
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) {
        let id = self.alloc_id();
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("值"));
        let condition = ConditionRow {
            id,
            field: QueryField::Title,
            operator: QueryOperator::Contains,
            value_input: input,
        };
        if let Some(group) = self.find_group_mut(group_id) {
            group.items.push(FilterNode::Condition(condition));
        }
    }

    pub(crate) fn add_group(&mut self, group_id: u64) {
        let id = self.alloc_id();
        if let Some(group) = self.find_group_mut(group_id) {
            group.items.push(FilterNode::Group(FilterGroup {
                id,
                kind: GroupKind::All,
                items: Vec::new(),
            }));
        }
    }

    pub(crate) fn remove_node(&mut self, node_id: u64) {
        Self::remove_node_from(&mut self.root, node_id);
    }

    pub(crate) fn set_group_kind(&mut self, group_id: u64, kind: GroupKind) {
        if let Some(group) = self.find_group_mut(group_id) {
            group.kind = kind;
        }
    }

    pub(crate) fn cycle_field(
        &mut self,
        condition_id: u64,
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) {
        if let Some(condition) = self.find_condition_mut(condition_id) {
            condition.field = condition.field.next();
            condition.operator = condition.field.default_operator();
            let default_value = condition.field.default_value();
            condition.value_input.update(cx, |input, cx| {
                input.set_value(default_value, window, cx);
            });
        }
    }

    pub(crate) fn cycle_operator(&mut self, condition_id: u64) {
        if let Some(condition) = self.find_condition_mut(condition_id) {
            condition.operator = condition.field.next_operator(condition.operator);
        }
    }

    pub(crate) fn add_sort(&mut self) {
        let id = self.alloc_id();
        self.sorts.push(SortRow {
            id,
            expr: SortUiExpr::ReadPerWord,
            direction: SortDirection::Desc,
        });
    }

    pub(crate) fn remove_sort(&mut self, sort_id: u64) {
        self.sorts.retain(|sort| sort.id != sort_id);
    }

    pub(crate) fn move_sort_up(&mut self, sort_id: u64) {
        if let Some(ix) = self.sorts.iter().position(|sort| sort.id == sort_id)
            && ix > 0
        {
            self.sorts.swap(ix - 1, ix);
        }
    }

    pub(crate) fn move_sort_down(&mut self, sort_id: u64) {
        if let Some(ix) = self.sorts.iter().position(|sort| sort.id == sort_id)
            && ix + 1 < self.sorts.len()
        {
            self.sorts.swap(ix, ix + 1);
        }
    }

    pub(crate) fn cycle_sort_expr(&mut self, sort_id: u64) {
        if let Some(sort) = self.sorts.iter_mut().find(|sort| sort.id == sort_id) {
            sort.expr = sort.expr.next();
        }
    }

    pub(crate) fn toggle_sort_direction(&mut self, sort_id: u64) {
        if let Some(sort) = self.sorts.iter_mut().find(|sort| sort.id == sort_id) {
            sort.direction = match sort.direction {
                SortDirection::Asc => SortDirection::Desc,
                SortDirection::Desc => SortDirection::Asc,
            };
        }
    }

    pub(crate) fn render_filters(&self, cx: &mut Context<QueryView>) -> impl IntoElement {
        v_flex()
            .size_full()
            .gap_3()
            .p_3()
            .child(section_header(
                "查询构建器",
                "组合字段条件、标签集合和作者规则",
            ))
            .child(self.render_group(&self.root, 0, cx))
            .overflow_y_scrollbar()
    }

    pub(crate) fn render_sorts(&self, cx: &mut Context<QueryView>) -> impl IntoElement {
        v_flex()
            .size_full()
            .gap_3()
            .p_3()
            .child(
                h_flex()
                    .justify_between()
                    .child(section_header("排序规则", "列表顺序即优先级"))
                    .child(
                        Button::new("query-add-sort")
                            .small()
                            .outline()
                            .label("添加排序")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.advanced.add_sort();
                                cx.notify();
                            })),
                    ),
            )
            .children(
                self.sorts
                    .iter()
                    .enumerate()
                    .map(|(ix, sort)| self.render_sort_row(ix, sort, cx)),
            )
            .when(self.sorts.is_empty(), |this| {
                this.child(
                    div()
                        .p_3()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("未设置排序规则，结果按入库顺序展示。"),
                )
            })
            .overflow_y_scrollbar()
    }

    fn render_group(
        &self,
        group: &FilterGroup,
        depth: usize,
        cx: &mut Context<QueryView>,
    ) -> impl IntoElement {
        let group_id = group.id;
        v_flex()
            .gap_2()
            .pl(px((depth as f32) * 16.))
            .border_l_1()
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .justify_between()
                    .gap_2()
                    .child(self.render_group_kind(group, cx))
                    .child(
                        h_flex()
                            .gap_1()
                            .child(
                                Button::new(("query-add-condition", group_id))
                                    .small()
                                    .ghost()
                                    .label("添加条件")
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.advanced.add_condition(group_id, window, cx);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new(("query-add-group", group_id))
                                    .small()
                                    .ghost()
                                    .label("添加组")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.advanced.add_group(group_id);
                                        cx.notify();
                                    })),
                            )
                            .when(group.id != 0, |this| {
                                this.child(
                                    Button::new(("query-remove-group", group_id))
                                        .small()
                                        .ghost()
                                        .label("删除")
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.advanced.remove_node(group_id);
                                            cx.notify();
                                        })),
                                )
                            }),
                    ),
            )
            .children(group.items.iter().map(|item| match item {
                FilterNode::Condition(condition) => {
                    self.render_condition(condition, cx).into_any_element()
                }
                FilterNode::Group(group) => {
                    self.render_group(group, depth + 1, cx).into_any_element()
                }
            }))
            .when(group.items.is_empty(), |this| {
                this.child(
                    div()
                        .p_3()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("添加条件或子组开始构建高级检索。"),
                )
            })
    }

    fn render_group_kind(
        &self,
        group: &FilterGroup,
        cx: &mut Context<QueryView>,
    ) -> impl IntoElement {
        let group_id = group.id;
        let view = cx.entity().downgrade();
        ToggleGroup::new(("query-group-kind", group_id))
            .segmented()
            .outline()
            .small()
            .child(
                Toggle::new(("group-all", group_id))
                    .label("全部满足")
                    .checked(group.kind == GroupKind::All),
            )
            .child(
                Toggle::new(("group-any", group_id))
                    .label("任一满足")
                    .checked(group.kind == GroupKind::Any),
            )
            .child(
                Toggle::new(("group-not", group_id))
                    .label("排除")
                    .checked(group.kind == GroupKind::Not),
            )
            .on_click(move |checked: &Vec<bool>, _, cx| {
                let kind = match checked.iter().position(|checked| *checked) {
                    Some(0) => GroupKind::All,
                    Some(1) => GroupKind::Any,
                    Some(2) => GroupKind::Not,
                    _ => return,
                };
                let _ = view.update(cx, |this, cx| {
                    this.advanced.set_group_kind(group_id, kind);
                    cx.notify();
                });
            })
    }

    fn render_condition(
        &self,
        condition: &ConditionRow,
        cx: &mut Context<QueryView>,
    ) -> impl IntoElement {
        let id = condition.id;
        v_flex()
            .gap_1()
            .border_1()
            .border_color(cx.theme().border)
            .rounded_md()
            .p_2()
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new(("query-field", id))
                            .small()
                            .outline()
                            .label(condition.field.label())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.advanced.cycle_field(id, window, cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new(("query-op", id))
                            .small()
                            .outline()
                            .label(condition.operator.label())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.advanced.cycle_operator(id);
                                cx.notify();
                            })),
                    )
                    .when(condition.operator.needs_value(), |this| {
                        this.child(Input::new(&condition.value_input).small().min_w(px(180.)))
                    })
                    .child(
                        Button::new(("query-remove-condition", id))
                            .small()
                            .ghost()
                            .label("删除")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.advanced.remove_node(id);
                                cx.notify();
                            })),
                    ),
            )
            .when(condition.field.uses_tokens(), |this| {
                let value = condition.value_input.read(cx).value();
                let tokens = parse_list(value.as_ref());
                this.child(
                    h_flex().gap_1().flex_wrap().children(
                        tokens
                            .into_iter()
                            .map(|token| Tag::secondary().outline().child(token)),
                    ),
                )
            })
    }

    fn render_sort_row(
        &self,
        index: usize,
        sort: &SortRow,
        cx: &mut Context<QueryView>,
    ) -> AnyElement {
        let id = sort.id;
        h_flex()
            .gap_2()
            .items_center()
            .border_1()
            .border_color(cx.theme().border)
            .rounded_md()
            .p_2()
            .child(Label::new(format!("{}", index + 1)).text_sm())
            .child(
                Button::new(("query-sort-expr", id))
                    .small()
                    .outline()
                    .label(sort.expr.label())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.advanced.cycle_sort_expr(id);
                        cx.notify();
                    })),
            )
            .child(
                Button::new(("query-sort-direction", id))
                    .small()
                    .outline()
                    .label(match sort.direction {
                        SortDirection::Asc => "升序",
                        SortDirection::Desc => "降序",
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.advanced.toggle_sort_direction(id);
                        cx.notify();
                    })),
            )
            .child(
                Button::new(("query-sort-up", id))
                    .small()
                    .ghost()
                    .label("上移")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.advanced.move_sort_up(id);
                        cx.notify();
                    })),
            )
            .child(
                Button::new(("query-sort-down", id))
                    .small()
                    .ghost()
                    .label("下移")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.advanced.move_sort_down(id);
                        cx.notify();
                    })),
            )
            .child(
                Button::new(("query-sort-remove", id))
                    .small()
                    .ghost()
                    .label("删除")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.advanced.remove_sort(id);
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    fn group_expr(&self, group: &FilterGroup, cx: &gpui::App) -> Result<FilterExpr, String> {
        let filters = group
            .items
            .iter()
            .map(|item| match item {
                FilterNode::Condition(condition) => self.condition_expr(condition, cx),
                FilterNode::Group(group) => self.group_expr(group, cx),
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(match group.kind {
            GroupKind::All => FilterExpr::All(filters),
            GroupKind::Any => FilterExpr::Any(filters),
            GroupKind::Not => FilterExpr::Not(Box::new(FilterExpr::All(filters))),
        })
    }

    fn condition_expr(
        &self,
        condition: &ConditionRow,
        cx: &gpui::App,
    ) -> Result<FilterExpr, String> {
        let value = condition.value_input.read(cx).value().trim().to_string();
        let predicate = match condition.field {
            QueryField::Title
            | QueryField::Description
            | QueryField::LatestChapter
            | QueryField::AuthorName => {
                require_value(&value, condition.field.label())?;
                Predicate::Text {
                    field: condition.field.text_field().unwrap(),
                    op: condition.operator.text_op().unwrap(),
                    value,
                }
            }
            QueryField::NovelId
            | QueryField::LatestChapterId
            | QueryField::WordCount
            | QueryField::ReadCount
            | QueryField::ReplyCount
            | QueryField::AuthorId => Predicate::Number {
                field: condition.field.number_field().unwrap(),
                op: condition.operator.number_op(&value)?,
            },
            QueryField::IsLimit => Predicate::Bool {
                field: BoolField::IsLimit,
                value: parse_bool(&value)?,
            },
            QueryField::Tags => Predicate::Tags(condition.operator.tags_predicate(&value)?),
            QueryField::Author => Predicate::Author(condition.operator.author_predicate(&value)?),
        };
        Ok(FilterExpr::Predicate(predicate))
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
}

impl FilterNode {
    fn id(&self) -> u64 {
        match self {
            FilterNode::Condition(condition) => condition.id,
            FilterNode::Group(group) => group.id,
        }
    }
}

impl QueryField {
    const ALL: [Self; 13] = [
        Self::Title,
        Self::Description,
        Self::LatestChapter,
        Self::AuthorName,
        Self::NovelId,
        Self::LatestChapterId,
        Self::WordCount,
        Self::ReadCount,
        Self::ReplyCount,
        Self::AuthorId,
        Self::IsLimit,
        Self::Tags,
        Self::Author,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Title => "标题",
            Self::Description => "简介",
            Self::LatestChapter => "最新章节",
            Self::AuthorName => "作者名",
            Self::NovelId => "作品 ID",
            Self::LatestChapterId => "章节 ID",
            Self::WordCount => "字数",
            Self::ReadCount => "阅读数",
            Self::ReplyCount => "回复数",
            Self::AuthorId => "作者 ID",
            Self::IsLimit => "是否受限",
            Self::Tags => "标签",
            Self::Author => "作者",
        }
    }

    fn next(self) -> Self {
        let ix = Self::ALL
            .iter()
            .position(|field| *field == self)
            .unwrap_or(0);
        Self::ALL[(ix + 1) % Self::ALL.len()]
    }

    fn default_operator(self) -> QueryOperator {
        self.operators()[0]
    }

    fn next_operator(self, current: QueryOperator) -> QueryOperator {
        let operators = self.operators();
        let ix = operators.iter().position(|op| *op == current).unwrap_or(0);
        operators[(ix + 1) % operators.len()]
    }

    fn operators(self) -> &'static [QueryOperator] {
        match self {
            Self::Title | Self::Description | Self::LatestChapter | Self::AuthorName => &[
                QueryOperator::Contains,
                QueryOperator::StartsWith,
                QueryOperator::EndsWith,
                QueryOperator::Equals,
            ],
            Self::NovelId
            | Self::LatestChapterId
            | Self::WordCount
            | Self::ReadCount
            | Self::ReplyCount
            | Self::AuthorId => &[
                QueryOperator::Equals,
                QueryOperator::NotEquals,
                QueryOperator::LessThan,
                QueryOperator::LessThanOrEquals,
                QueryOperator::GreaterThan,
                QueryOperator::GreaterThanOrEquals,
                QueryOperator::Between,
            ],
            Self::IsLimit => &[QueryOperator::Equals],
            Self::Tags => &[
                QueryOperator::Intersects,
                QueryOperator::ContainsAll,
                QueryOperator::ContainedBy,
                QueryOperator::SetEquals,
                QueryOperator::IsEmpty,
                QueryOperator::IsNotEmpty,
            ],
            Self::Author => &[
                QueryOperator::Equals,
                QueryOperator::In,
                QueryOperator::NotIn,
                QueryOperator::NameContains,
            ],
        }
    }

    fn default_value(self) -> &'static str {
        match self {
            Self::IsLimit => "否",
            _ => "",
        }
    }

    fn uses_tokens(self) -> bool {
        matches!(self, Self::Tags | Self::Author)
    }

    fn text_field(self) -> Option<TextField> {
        match self {
            Self::Title => Some(TextField::Title),
            Self::Description => Some(TextField::Description),
            Self::LatestChapter => Some(TextField::LatestChapter),
            Self::AuthorName => Some(TextField::AuthorName),
            _ => None,
        }
    }

    fn number_field(self) -> Option<NumberField> {
        match self {
            Self::NovelId => Some(NumberField::NovelId),
            Self::LatestChapterId => Some(NumberField::LatestChapterId),
            Self::WordCount => Some(NumberField::WordCount),
            Self::ReadCount => Some(NumberField::ReadCount),
            Self::ReplyCount => Some(NumberField::ReplyCount),
            Self::AuthorId => Some(NumberField::AuthorId),
            _ => None,
        }
    }
}

impl QueryOperator {
    fn label(self) -> &'static str {
        match self {
            Self::Contains => "包含",
            Self::StartsWith => "开头是",
            Self::EndsWith => "结尾是",
            Self::Equals => "等于",
            Self::NotEquals => "不等于",
            Self::LessThan => "小于",
            Self::LessThanOrEquals => "小于等于",
            Self::GreaterThan => "大于",
            Self::GreaterThanOrEquals => "大于等于",
            Self::Between => "介于",
            Self::Intersects => "有交集",
            Self::ContainsAll => "包含全部",
            Self::ContainedBy => "被包含",
            Self::SetEquals => "集合相等",
            Self::IsEmpty => "为空",
            Self::IsNotEmpty => "不为空",
            Self::In => "在集合中",
            Self::NotIn => "不在集合中",
            Self::NameContains => "名称包含",
        }
    }

    fn needs_value(self) -> bool {
        !matches!(self, Self::IsEmpty | Self::IsNotEmpty)
    }

    fn text_op(self) -> Option<TextOp> {
        match self {
            Self::Contains => Some(TextOp::Contains),
            Self::StartsWith => Some(TextOp::StartsWith),
            Self::EndsWith => Some(TextOp::EndsWith),
            Self::Equals => Some(TextOp::Equals),
            _ => None,
        }
    }

    fn number_op(self, value: &str) -> Result<NumberOp, String> {
        Ok(match self {
            Self::Equals => NumberOp::Eq(parse_i32(value)?),
            Self::NotEquals => NumberOp::Ne(parse_i32(value)?),
            Self::LessThan => NumberOp::Lt(parse_i32(value)?),
            Self::LessThanOrEquals => NumberOp::Lte(parse_i32(value)?),
            Self::GreaterThan => NumberOp::Gt(parse_i32(value)?),
            Self::GreaterThanOrEquals => NumberOp::Gte(parse_i32(value)?),
            Self::Between => {
                let (min, max) = parse_range(value)?;
                NumberOp::Between { min, max }
            }
            _ => return Err("数字字段不支持该操作符".to_owned()),
        })
    }

    fn tags_predicate(self, value: &str) -> Result<TagsPredicate, String> {
        let values = parse_set(value);
        Ok(match self {
            Self::Intersects => TagsPredicate::Intersects(require_set(values)?),
            Self::ContainsAll => TagsPredicate::ContainsAll(require_set(values)?),
            Self::ContainedBy => TagsPredicate::ContainedBy(require_set(values)?),
            Self::SetEquals => TagsPredicate::Equals(require_set(values)?),
            Self::IsEmpty => TagsPredicate::IsEmpty,
            Self::IsNotEmpty => TagsPredicate::IsNotEmpty,
            _ => return Err("标签字段不支持该操作符".to_owned()),
        })
    }

    fn author_predicate(self, value: &str) -> Result<AuthorPredicate, String> {
        Ok(match self {
            Self::Equals => AuthorPredicate::Is(parse_author_ref(value)?),
            Self::In => AuthorPredicate::In(parse_author_refs(value)?),
            Self::NotIn => AuthorPredicate::NotIn(parse_author_refs(value)?),
            Self::NameContains => {
                require_value(value, "作者")?;
                AuthorPredicate::NameContains(value.to_owned())
            }
            _ => return Err("作者字段不支持该操作符".to_owned()),
        })
    }
}

impl SortRow {
    fn sort_spec(&self) -> SortSpec {
        SortSpec {
            expr: self.expr.sort_expr(),
            direction: self.direction,
        }
    }
}

impl SortUiExpr {
    const ALL: [Self; 11] = [
        Self::Title,
        Self::Author,
        Self::IsLimit,
        Self::WordCount,
        Self::ReadCount,
        Self::ReplyCount,
        Self::LatestChapter,
        Self::ReadPerWord,
        Self::ReplyPlusRead,
        Self::WordMinusReply,
        Self::ReplyTimesTwo,
    ];

    fn next(self) -> Self {
        let ix = Self::ALL.iter().position(|expr| *expr == self).unwrap_or(0);
        Self::ALL[(ix + 1) % Self::ALL.len()]
    }

    fn label(self) -> &'static str {
        match self {
            Self::Title => "标题",
            Self::Author => "作者",
            Self::IsLimit => "是否受限",
            Self::WordCount => "字数",
            Self::ReadCount => "阅读数",
            Self::ReplyCount => "回复数",
            Self::LatestChapter => "最新章节 ID",
            Self::ReadPerWord => "阅读数 / 字数",
            Self::ReplyPlusRead => "回复数 + 阅读数",
            Self::WordMinusReply => "字数 - 回复数",
            Self::ReplyTimesTwo => "回复数 * 2",
        }
    }

    fn sort_expr(self) -> SortExpr {
        match self {
            Self::Title => SortExpr::Text(TextField::Title),
            Self::Author => SortExpr::Text(TextField::AuthorName),
            Self::IsLimit => SortExpr::Bool(BoolField::IsLimit),
            Self::WordCount => SortExpr::Number(NumberField::WordCount),
            Self::ReadCount => SortExpr::Number(NumberField::ReadCount),
            Self::ReplyCount => SortExpr::Number(NumberField::ReplyCount),
            Self::LatestChapter => SortExpr::Number(NumberField::LatestChapterId),
            Self::ReadPerWord => SortExpr::Div(
                Box::new(SortExpr::Number(NumberField::ReadCount)),
                Box::new(SortExpr::Number(NumberField::WordCount)),
            ),
            Self::ReplyPlusRead => SortExpr::Add(
                Box::new(SortExpr::Number(NumberField::ReplyCount)),
                Box::new(SortExpr::Number(NumberField::ReadCount)),
            ),
            Self::WordMinusReply => SortExpr::Sub(
                Box::new(SortExpr::Number(NumberField::WordCount)),
                Box::new(SortExpr::Number(NumberField::ReplyCount)),
            ),
            Self::ReplyTimesTwo => SortExpr::Mul(
                Box::new(SortExpr::Number(NumberField::ReplyCount)),
                Box::new(SortExpr::Constant(2.)),
            ),
        }
    }
}

fn section_header(title: &str, description: &str) -> impl IntoElement {
    v_flex()
        .gap_1()
        .child(Label::new(title).font_semibold())
        .child(Label::new(description).text_xs())
}

fn require_value(value: &str, field: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{field} 需要输入值"))
    } else {
        Ok(())
    }
}

fn parse_i32(value: &str) -> Result<i32, String> {
    require_value(value, "数字字段")?;
    value
        .trim()
        .parse::<i32>()
        .map_err(|_| format!("无法解析数字: {value}"))
}

fn parse_range(value: &str) -> Result<(i32, i32), String> {
    let parts = value
        .split([',', '，'])
        .flat_map(|part| part.split(".."))
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.len() != 2 {
        return Err("范围需要输入两个数字，例如 1000..5000".to_owned());
    }
    let min = parse_i32(parts[0])?;
    let max = parse_i32(parts[1])?;
    if min > max {
        return Err("范围下限不能大于上限".to_owned());
    }
    Ok((min, max))
}

fn parse_bool(value: &str) -> Result<bool, String> {
    match value.trim().to_lowercase().as_str() {
        "是" | "true" | "1" | "yes" => Ok(true),
        "否" | "false" | "0" | "no" => Ok(false),
        _ => Err("布尔字段请输入 是/否 或 true/false".to_owned()),
    }
}

fn parse_set(value: &str) -> HashSet<String> {
    parse_list(value).into_iter().collect()
}

fn parse_list(value: &str) -> Vec<String> {
    value
        .split([',', '，', '\n'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn require_set(values: HashSet<String>) -> Result<HashSet<String>, String> {
    if values.is_empty() {
        Err("集合条件至少需要一个值".to_owned())
    } else {
        Ok(values)
    }
}

fn parse_author_refs(value: &str) -> Result<Vec<AuthorRef>, String> {
    let refs = parse_list(value)
        .into_iter()
        .map(|value| parse_author_ref(&value))
        .collect::<Result<Vec<_>, _>>()?;
    if refs.is_empty() {
        Err("作者集合至少需要一个值".to_owned())
    } else {
        Ok(refs)
    }
}

fn parse_author_ref(value: &str) -> Result<AuthorRef, String> {
    require_value(value, "作者")?;
    Ok(value
        .trim()
        .parse::<i32>()
        .map(AuthorRef::Id)
        .unwrap_or_else(|_| AuthorRef::Name(value.trim().to_owned())))
}

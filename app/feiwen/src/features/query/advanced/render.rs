use super::{
    options::GroupRelation,
    sort::DragSortRow,
    state::{
        AdvancedQueryState, AuthorValue, BoolCondition, ConditionDraft, ConditionRow, FilterGroup,
        FilterNode, NumberValue, RelationSelect, SortRow, TagsCondition,
    },
};
use crate::store::query::SortDirection;
use crate::{features::query::QueryView, foundation::assets::IconName as FeiwenIconName};
use gpui::{
    AnyElement, AppContext, Context, ElementId, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px, rems,
};
use gpui_component::{
    ActiveTheme, Disableable, Icon, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants, Toggle, ToggleGroup, ToggleVariants},
    h_flex,
    input::{Input, NumberInput},
    label::Label,
    scroll::ScrollableElement,
    select::Select,
    switch::Switch,
    table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow},
    v_flex,
};

const CONDITION_FIELD_COLUMN_WIDTH: f32 = 140.;
const CONDITION_RELATION_COLUMN_WIDTH: f32 = 120.;
const CONDITION_NEGATED_COLUMN_WIDTH: f32 = 56.;
const CONDITION_ACTION_COLUMN_WIDTH: f32 = 56.;
const CONDITION_COLUMN_COUNT: usize = 5;
const SORT_ORDER_COLUMN_WIDTH: f32 = 72.;
const SORT_DIRECTION_COLUMN_WIDTH: f32 = 112.;
const SORT_ACTION_COLUMN_WIDTH: f32 = 56.;

impl AdvancedQueryState {
    pub(crate) fn render_filters(
        &self,
        disabled: bool,
        cx: &mut Context<QueryView>,
    ) -> impl IntoElement {
        v_flex()
            .size_full()
            .gap_3()
            .p_3()
            .child(
                h_flex()
                    .justify_between()
                    .items_start()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(Label::new("查询构建器").font_semibold())
                            .child(
                                Label::new("通过字段、条件、值和排除开关组合高级检索")
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground),
                            ),
                    )
                    .child(
                        Button::new("query-add-root-condition")
                            .icon(IconName::Plus)
                            .label("添加条件")
                            .disabled(disabled)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.advanced.add_condition(0, window, cx);
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(render_group(&self.root, 0, disabled, cx)),
            )
    }

    pub(crate) fn render_sorts(
        &self,
        disabled: bool,
        cx: &mut Context<QueryView>,
    ) -> impl IntoElement {
        v_flex()
            .size_full()
            .gap_3()
            .p_3()
            .child(
                h_flex()
                    .justify_between()
                    .items_start()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(Label::new("排序规则").font_semibold())
                            .child(
                                Label::new("拖拽排序项调整优先级")
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground),
                            ),
                    )
                    .child(
                        Button::new("query-add-sort")
                            .icon(IconName::Plus)
                            .label("添加排序")
                            .disabled(disabled)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.advanced.add_sort(window, cx);
                                cx.notify();
                            })),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .when(self.sorts.is_empty(), |this| {
                        this.child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .min_h(px(96.))
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child("添加排序规则后，列表顺序就是排序优先级"),
                        )
                    })
                    .when(!self.sorts.is_empty(), |this| {
                        this.child(render_sorts_table(&self.sorts, disabled, cx))
                    }),
            )
    }
}

fn render_group(
    group: &FilterGroup,
    depth: usize,
    disabled: bool,
    cx: &mut Context<QueryView>,
) -> AnyElement {
    let group_id = group.id;
    let relation = group.relation;
    let can_remove = group.id != 0;
    let indent = px((depth as f32) * 16.);

    v_flex()
        .ml(indent)
        .pl_3()
        .gap_2()
        .border_l_1()
        .border_color(cx.theme().border)
        .child(
            h_flex()
                .items_center()
                .justify_between()
                .gap_2()
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(
                            Label::new(format!("第 {} 层", depth + 1))
                                .text_xs()
                                .text_color(cx.theme().muted_foreground),
                        )
                        .child(group_relation_toggle(group_id, relation, disabled, cx))
                        .child(
                            Switch::new(("group-negated", group_id))
                                .checked(group.negated)
                                .label("排除")
                                .disabled(disabled)
                                .on_click(cx.listener(move |this, checked, _, cx| {
                                    this.advanced.set_group_negated(group_id, *checked);
                                    cx.notify();
                                })),
                        ),
                )
                .child(
                    h_flex()
                        .gap_1()
                        .child(
                            Button::new(("group-add-condition", group_id))
                                .ghost()
                                .icon(IconName::Plus)
                                .label("添加条件")
                                .disabled(disabled)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.advanced.add_condition(group_id, window, cx);
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new(("group-add-subgroup", group_id))
                                .ghost()
                                .icon(IconName::Plus)
                                .label("添加子组")
                                .disabled(disabled)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.advanced.add_group(group_id);
                                    cx.notify();
                                })),
                        )
                        .when(can_remove, |this| {
                            this.child(
                                icon_button(
                                    ("group-remove", group_id),
                                    FeiwenIconName::Trash,
                                    "删除条件组",
                                )
                                .disabled(disabled)
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.advanced.remove_node(group_id);
                                        cx.notify();
                                    },
                                )),
                            )
                        }),
                ),
        )
        .when(group.items.is_empty(), |this| {
            this.child(
                div()
                    .py_4()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("添加条件或子组开始构建高级检索。"),
            )
        })
        .child(render_conditions_table(group, depth, disabled, cx))
        .into_any_element()
}

fn render_conditions_table(
    group: &FilterGroup,
    depth: usize,
    disabled: bool,
    cx: &mut Context<QueryView>,
) -> impl IntoElement {
    Table::new()
        .small()
        .w_full()
        .child(
            TableHeader::new().child(
                TableRow::new()
                    .child(condition_table_head("字段", CONDITION_FIELD_COLUMN_WIDTH))
                    .child(condition_table_head(
                        "条件",
                        CONDITION_RELATION_COLUMN_WIDTH,
                    ))
                    .child(condition_value_table_head("值"))
                    .child(condition_table_head("排除", CONDITION_NEGATED_COLUMN_WIDTH))
                    .child(condition_table_head("操作", CONDITION_ACTION_COLUMN_WIDTH)),
            ),
        )
        .child(
            TableBody::new().children(group.items.iter().flat_map(|item| match item {
                FilterNode::Condition(condition) => render_condition_rows(condition, disabled, cx),
                FilterNode::Group(group) => vec![render_group_row(group, depth, disabled, cx)],
            })),
        )
}

fn condition_table_head(label: &'static str, width: f32) -> TableHead {
    TableHead::new()
        .w(px(width))
        .min_w(px(width))
        .flex_none()
        .child(Label::new(label).text_xs().truncate())
}

fn condition_value_table_head(label: &'static str) -> TableHead {
    TableHead::new()
        .min_w(px(0.))
        .flex_grow()
        .child(Label::new(label).text_xs().truncate())
}

fn condition_table_cell(width: f32, child: impl IntoElement) -> TableCell {
    TableCell::new()
        .w(px(width))
        .min_w(px(width))
        .flex_none()
        .child(child)
}

fn condition_value_table_cell(child: impl IntoElement) -> TableCell {
    TableCell::new()
        .min_w(px(0.))
        .flex_grow()
        .child(div().w_full().min_w_0().child(child))
}

fn condition_span_cell(child: impl IntoElement) -> TableCell {
    TableCell::new()
        .col_span(CONDITION_COLUMN_COUNT)
        .min_w(px(0.))
        .w_full()
        .child(child)
}

fn render_group_row(
    group: &FilterGroup,
    depth: usize,
    disabled: bool,
    cx: &mut Context<QueryView>,
) -> TableRow {
    TableRow::new().child(condition_span_cell(render_group(
        group,
        depth + 1,
        disabled,
        cx,
    )))
}

fn group_relation_toggle(
    group_id: u64,
    relation: GroupRelation,
    disabled: bool,
    cx: &mut Context<QueryView>,
) -> impl IntoElement {
    ToggleGroup::new(("group-relation", group_id))
        .segmented()
        .outline()
        .child(
            Toggle::new(("group-relation-all", group_id))
                .label("全部满足")
                .checked(matches!(relation, GroupRelation::All)),
        )
        .child(
            Toggle::new(("group-relation-any", group_id))
                .label("任一满足")
                .checked(matches!(relation, GroupRelation::Any)),
        )
        .disabled(disabled)
        .on_click(cx.listener(move |this, checkeds: &Vec<bool>, _, cx| {
            let next = match relation {
                GroupRelation::All if checkeds.get(1).copied().unwrap_or(false) => {
                    GroupRelation::Any
                }
                GroupRelation::Any if checkeds.first().copied().unwrap_or(false) => {
                    GroupRelation::All
                }
                current => current,
            };
            this.advanced.set_group_relation(group_id, next);
            cx.notify();
        }))
}

fn render_condition_rows(
    condition: &ConditionRow,
    disabled: bool,
    cx: &mut Context<QueryView>,
) -> Vec<TableRow> {
    let mut rows = vec![render_condition_row(condition, disabled, cx)];
    if let Some(error) = condition.error.as_ref() {
        rows.push(render_condition_error_row(error.clone(), cx));
    }
    rows
}

fn render_condition_row(
    condition: &ConditionRow,
    disabled: bool,
    cx: &mut Context<QueryView>,
) -> TableRow {
    let condition_id = condition.id;
    TableRow::new()
        .child(condition_table_cell(
            CONDITION_FIELD_COLUMN_WIDTH,
            Select::new(&condition.field_select)
                .placeholder("请选择字段")
                .disabled(disabled)
                .w_full(),
        ))
        .child(condition_table_cell(
            CONDITION_RELATION_COLUMN_WIDTH,
            render_relation_select(&condition.draft, disabled, cx),
        ))
        .child(condition_value_table_cell(render_value_editor(
            &condition.draft,
            disabled,
            cx,
        )))
        .child(
            condition_table_cell(
                CONDITION_NEGATED_COLUMN_WIDTH,
                h_flex().w_full().justify_center().child(
                    Switch::new(("condition-negated", condition_id))
                        .checked(condition.negated)
                        .disabled(disabled)
                        .on_click(cx.listener(move |this, checked, _, cx| {
                            this.advanced.set_condition_negated(condition_id, *checked);
                            cx.notify();
                        })),
                ),
            )
            .min_w(px(0.)),
        )
        .child(
            condition_table_cell(
                CONDITION_ACTION_COLUMN_WIDTH,
                h_flex().w_full().justify_center().child(
                    icon_button(
                        ("condition-remove", condition_id),
                        FeiwenIconName::Trash,
                        "删除条件",
                    )
                    .disabled(disabled)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.advanced.remove_node(condition_id);
                        cx.notify();
                    })),
                ),
            )
            .min_w(px(0.)),
        )
}

fn render_condition_error_row(error: String, cx: &mut Context<QueryView>) -> TableRow {
    TableRow::new().child(condition_span_cell(
        h_flex()
            .gap_1()
            .items_center()
            .text_color(cx.theme().danger)
            .child(Icon::new(IconName::TriangleAlert))
            .child(Label::new(error).text_xs()),
    ))
}

fn render_relation_select(
    draft: &ConditionDraft,
    disabled: bool,
    cx: &mut Context<QueryView>,
) -> AnyElement {
    match draft {
        ConditionDraft::NoField => placeholder_control("请选择字段", true, cx),
        ConditionDraft::NoCondition {
            relation_select, ..
        } => render_relation_entity(relation_select, disabled),
        ConditionDraft::Text(condition) => Select::new(&condition.relation_select)
            .disabled(disabled)
            .w_full()
            .into_any_element(),
        ConditionDraft::Number(condition) => Select::new(&condition.relation_select)
            .disabled(disabled)
            .w_full()
            .into_any_element(),
        ConditionDraft::Bool(condition) => Select::new(&condition.relation_select)
            .disabled(disabled)
            .w_full()
            .into_any_element(),
        ConditionDraft::Tags(condition) => Select::new(&condition.relation_select)
            .disabled(disabled)
            .w_full()
            .into_any_element(),
        ConditionDraft::Author(condition) => Select::new(&condition.relation_select)
            .disabled(disabled)
            .w_full()
            .into_any_element(),
    }
}

fn render_relation_entity(relation_select: &RelationSelect, disabled: bool) -> AnyElement {
    match relation_select {
        RelationSelect::Text(select) => Select::new(select)
            .disabled(disabled)
            .w_full()
            .into_any_element(),
        RelationSelect::Number(select) => Select::new(select)
            .disabled(disabled)
            .w_full()
            .into_any_element(),
        RelationSelect::Bool(select) => Select::new(select)
            .disabled(disabled)
            .w_full()
            .into_any_element(),
        RelationSelect::Tags(select) => Select::new(select)
            .disabled(disabled)
            .w_full()
            .into_any_element(),
        RelationSelect::Author(select) => Select::new(select)
            .disabled(disabled)
            .w_full()
            .into_any_element(),
    }
}

fn render_value_editor(
    draft: &ConditionDraft,
    disabled: bool,
    cx: &mut Context<QueryView>,
) -> AnyElement {
    match draft {
        ConditionDraft::NoField => placeholder_control("请选择字段", true, cx),
        ConditionDraft::NoCondition { .. } => placeholder_control("请选择条件", true, cx),
        ConditionDraft::Text(condition) => Input::new(&condition.input)
            .disabled(disabled)
            .w_full()
            .into_any_element(),
        ConditionDraft::Number(condition) => render_number_value(&condition.value, disabled),
        ConditionDraft::Bool(BoolCondition { value_select, .. }) => Select::new(value_select)
            .disabled(disabled)
            .w_full()
            .into_any_element(),
        ConditionDraft::Tags(condition) => render_tags_value(condition, disabled, cx),
        ConditionDraft::Author(condition) => match &condition.value {
            AuthorValue::Text(value) => Input::new(value)
                .disabled(disabled)
                .w_full()
                .into_any_element(),
            AuthorValue::Single(value) => value.clone().into_any_element(),
            AuthorValue::Multi(value) => value.clone().into_any_element(),
        },
    }
}

fn render_number_value(value: &NumberValue, disabled: bool) -> AnyElement {
    match value {
        NumberValue::Single(input) => NumberInput::new(input)
            .disabled(disabled)
            .w_full()
            .into_any_element(),
        NumberValue::Range(range) => range.clone().into_any_element(),
    }
}

fn render_tags_value(
    condition: &TagsCondition,
    _disabled: bool,
    cx: &mut Context<QueryView>,
) -> AnyElement {
    match &condition.value {
        Some(value) => value.clone().into_any_element(),
        None => placeholder_control("无需填写", false, cx),
    }
}

fn placeholder_control(text: &'static str, muted: bool, cx: &mut Context<QueryView>) -> AnyElement {
    div()
        .h(rems(2.))
        .flex()
        .items_center()
        .px_2()
        .border_1()
        .border_color(cx.theme().input)
        .rounded(cx.theme().radius)
        .bg(if muted {
            cx.theme().muted
        } else {
            cx.theme().background
        })
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(text)
        .into_any_element()
}

fn render_sorts_table(
    sorts: &[SortRow],
    disabled: bool,
    cx: &mut Context<QueryView>,
) -> impl IntoElement {
    let rows = sorts
        .iter()
        .enumerate()
        .map(|(ix, sort)| render_sort_item(ix, sort, disabled, cx).into_any_element())
        .collect::<Vec<_>>();

    v_flex()
        .w_full()
        .child(
            h_flex()
                .w_full()
                .bg(cx.theme().table_head)
                .text_color(cx.theme().table_head_foreground)
                .border_b_1()
                .border_color(cx.theme().table_row_border)
                .child(sort_header_cell("顺序", SORT_ORDER_COLUMN_WIDTH))
                .child(sort_field_header_cell("排序字段"))
                .child(sort_header_cell("方向", SORT_DIRECTION_COLUMN_WIDTH))
                .child(sort_header_cell("操作", SORT_ACTION_COLUMN_WIDTH)),
        )
        .child(v_flex().w_full().children(rows))
}

fn sort_header_cell(label: &'static str, width: f32) -> impl IntoElement {
    div()
        .w(px(width))
        .min_w(px(width))
        .flex_none()
        .px(px(8.))
        .py(px(6.))
        .flex()
        .items_center()
        .child(Label::new(label).text_xs().truncate())
}

fn sort_field_header_cell(label: &'static str) -> impl IntoElement {
    div()
        .min_w(px(0.))
        .flex_grow()
        .px(px(8.))
        .py(px(6.))
        .flex()
        .items_center()
        .child(Label::new(label).text_xs().truncate())
}

fn sort_fixed_cell(width: f32, child: impl IntoElement) -> impl IntoElement {
    div()
        .w(px(width))
        .min_w(px(width))
        .flex_none()
        .px(px(8.))
        .py(px(6.))
        .flex()
        .items_center()
        .child(child)
}

fn sort_field_cell(child: impl IntoElement) -> impl IntoElement {
    div()
        .min_w(px(0.))
        .flex_grow()
        .px(px(8.))
        .py(px(6.))
        .flex()
        .items_center()
        .child(div().w_full().min_w_0().child(child))
}

fn render_sort_item(
    ix: usize,
    sort: &SortRow,
    disabled: bool,
    cx: &mut Context<QueryView>,
) -> impl IntoElement {
    let sort_id = sort.id;
    let field_label = sort
        .field
        .map(|field| field.label())
        .unwrap_or("未选择排序字段");
    let direction_label = sort_direction_label(sort.direction);

    v_flex()
        .w_full()
        .when(ix > 0, |this| {
            this.border_t_1().border_color(cx.theme().table_row_border)
        })
        .child(
            h_flex()
                .id(("sort-row", sort_id))
                .w_full()
                .items_center()
                .hover(|style| style.bg(cx.theme().accent.opacity(0.18)))
                .when(!disabled, |this| {
                    this.drag_over::<DragSortRow>(move |this, drag, _window, cx| {
                        if drag.row_id == sort_id {
                            this
                        } else {
                            this.border_l_2()
                                .border_color(cx.theme().drag_border)
                                .bg(cx.theme().accent.opacity(0.25))
                        }
                    })
                })
                .when(!disabled, |this| {
                    this.on_drop(cx.listener(move |this, drag: &DragSortRow, _window, cx| {
                        this.advanced.move_sort_before(drag.row_id, sort_id);
                        cx.notify();
                    }))
                })
                .child(sort_fixed_cell(
                    SORT_ORDER_COLUMN_WIDTH,
                    h_flex()
                        .w_full()
                        .items_center()
                        .gap_1()
                        .child(
                            div()
                                .id(("sort-drag-handle", sort_id))
                                .p_1()
                                .rounded_sm()
                                .when(!disabled, |this| {
                                    this.cursor_grab()
                                        .hover(|style| style.bg(cx.theme().accent))
                                        .on_drag(
                                            DragSortRow::new(
                                                sort_id,
                                                ix + 1,
                                                field_label,
                                                direction_label,
                                                sort.error.is_some(),
                                            ),
                                            |drag, _position, _window, cx| {
                                                cx.stop_propagation();
                                                cx.new(|_| drag.clone())
                                            },
                                        )
                                })
                                .child(Icon::new(IconName::EllipsisVertical)),
                        )
                        .child(
                            Label::new(format!("{}", ix + 1))
                                .text_sm()
                                .text_color(cx.theme().muted_foreground),
                        ),
                ))
                .child(sort_field_cell(
                    Select::new(&sort.field_select)
                        .placeholder("请选择排序字段")
                        .disabled(disabled)
                        .w_full(),
                ))
                .child(sort_fixed_cell(
                    SORT_DIRECTION_COLUMN_WIDTH,
                    Select::new(&sort.direction_select)
                        .disabled(disabled)
                        .w_full(),
                ))
                .child(sort_fixed_cell(
                    SORT_ACTION_COLUMN_WIDTH,
                    h_flex().w_full().justify_center().child(
                        icon_button(
                            ("sort-remove", sort_id),
                            FeiwenIconName::Trash,
                            "删除排序规则",
                        )
                        .disabled(disabled)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.advanced.remove_sort(sort_id);
                            cx.notify();
                        })),
                    ),
                )),
        )
        .when_some(sort.error.as_ref(), |this, error| {
            this.child(
                h_flex()
                    .w_full()
                    .gap_1()
                    .items_center()
                    .px(px(8.))
                    .py(px(6.))
                    .text_color(cx.theme().danger)
                    .child(Icon::new(IconName::TriangleAlert))
                    .child(Label::new(error.clone()).text_xs()),
            )
        })
}

fn icon_button(id: impl Into<ElementId>, icon: impl Into<Icon>, tooltip: &'static str) -> Button {
    Button::new(id).ghost().icon(icon).tooltip(tooltip)
}

fn sort_direction_label(direction: SortDirection) -> &'static str {
    match direction {
        SortDirection::Asc => "升序",
        SortDirection::Desc => "降序",
    }
}

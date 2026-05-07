use super::{
    options::GroupRelation,
    sort::DragSortRow,
    state::{
        AdvancedQueryState, AuthorValue, BoolCondition, ConditionDraft, ConditionRow, FilterGroup,
        FilterNode, IdValue, NumberValue, RelationSelect, SortRow, TagsCondition,
    },
};
use crate::store::query::SortDirection;
use crate::{features::query::QueryView, foundation::assets::IconName as FeiwenIconName};
use gpui::{
    AnyElement, AppContext, Context, ElementId, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px, rems,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, StyledExt,
    button::{Button, ButtonVariants, Toggle, ToggleGroup, ToggleVariants},
    divider::Divider,
    h_flex,
    input::{Input, NumberInput},
    label::Label,
    scroll::ScrollableElement,
    select::Select,
    switch::Switch,
    v_flex,
};

impl AdvancedQueryState {
    pub(crate) fn render_filters(&self, cx: &mut Context<QueryView>) -> impl IntoElement {
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
                    .child(render_group(&self.root, 0, cx)),
            )
    }

    pub(crate) fn render_sorts(&self, cx: &mut Context<QueryView>) -> impl IntoElement {
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
                    .gap_1()
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
                    .children(self.sorts.iter().enumerate().map(|(ix, sort)| {
                        v_flex()
                            .gap_1()
                            .when(ix > 0, |this| this.child(row_separator()))
                            .child(render_sort_row(ix, sort, cx))
                    })),
            )
    }
}

fn render_group(group: &FilterGroup, depth: usize, cx: &mut Context<QueryView>) -> AnyElement {
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
                        .child(group_relation_toggle(group_id, relation, cx))
                        .child(
                            Switch::new(("group-negated", group_id))
                                .checked(group.negated)
                                .label("排除")
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
        .children(group.items.iter().enumerate().map(|(ix, item)| {
            v_flex()
                .gap_2()
                .when(ix > 0, |this| this.child(row_separator()))
                .child(match item {
                    FilterNode::Condition(condition) => render_condition(condition, cx),
                    FilterNode::Group(group) => render_group(group, depth + 1, cx),
                })
        }))
        .into_any_element()
}

fn group_relation_toggle(
    group_id: u64,
    relation: GroupRelation,
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

fn render_condition(condition: &ConditionRow, cx: &mut Context<QueryView>) -> AnyElement {
    let condition_id = condition.id;
    v_flex()
        .gap_1()
        .px_2()
        .py_2()
        .hover(|style| style.bg(cx.theme().accent.opacity(0.18)))
        .child(
            div()
                .grid()
                .gap_2()
                .grid_cols(4)
                .child(labeled_control(
                    "字段",
                    Select::new(&condition.field_select)
                        .placeholder("请选择字段")
                        .w_full()
                        .into_any_element(),
                    cx,
                ))
                .child(labeled_control(
                    "条件",
                    render_relation_select(&condition.draft, cx),
                    cx,
                ))
                .child(labeled_control(
                    "值",
                    render_value_editor(&condition.draft, cx),
                    cx,
                ))
                .child(
                    v_flex()
                        .gap_1()
                        .child(
                            Label::new("排除")
                                .text_xs()
                                .text_color(cx.theme().muted_foreground),
                        )
                        .child(
                            h_flex()
                                .h(rems(2.))
                                .items_center()
                                .justify_between()
                                .child(
                                    Switch::new(("condition-negated", condition_id))
                                        .checked(condition.negated)
                                        .on_click(cx.listener(move |this, checked, _, cx| {
                                            this.advanced
                                                .set_condition_negated(condition_id, *checked);
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    icon_button(
                                        ("condition-remove", condition_id),
                                        FeiwenIconName::Trash,
                                        "删除条件",
                                    )
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            this.advanced.remove_node(condition_id);
                                            cx.notify();
                                        },
                                    )),
                                ),
                        ),
                ),
        )
        .when_some(condition.error.as_ref(), |this, error| {
            this.child(
                h_flex()
                    .gap_1()
                    .items_center()
                    .text_color(cx.theme().danger)
                    .child(Icon::new(IconName::TriangleAlert))
                    .child(Label::new(error.clone()).text_xs()),
            )
        })
        .into_any_element()
}

fn labeled_control(
    label: &'static str,
    control: AnyElement,
    cx: &mut Context<QueryView>,
) -> impl IntoElement {
    v_flex()
        .min_w_0()
        .gap_1()
        .child(
            Label::new(label)
                .text_xs()
                .text_color(cx.theme().muted_foreground),
        )
        .child(control)
}

fn render_relation_select(draft: &ConditionDraft, cx: &mut Context<QueryView>) -> AnyElement {
    match draft {
        ConditionDraft::NoField => placeholder_control("请选择字段", true, cx),
        ConditionDraft::NoCondition {
            relation_select, ..
        } => render_relation_entity(relation_select),
        ConditionDraft::Text(condition) => Select::new(&condition.relation_select)
            .w_full()
            .into_any_element(),
        ConditionDraft::Number(condition) => Select::new(&condition.relation_select)
            .w_full()
            .into_any_element(),
        ConditionDraft::Id(condition) => Select::new(&condition.relation_select)
            .w_full()
            .into_any_element(),
        ConditionDraft::Bool(condition) => Select::new(&condition.relation_select)
            .w_full()
            .into_any_element(),
        ConditionDraft::Tags(condition) => Select::new(&condition.relation_select)
            .w_full()
            .into_any_element(),
        ConditionDraft::Author(condition) => Select::new(&condition.relation_select)
            .w_full()
            .into_any_element(),
    }
}

fn render_relation_entity(relation_select: &RelationSelect) -> AnyElement {
    match relation_select {
        RelationSelect::Text(select) => Select::new(select).w_full().into_any_element(),
        RelationSelect::Number(select) => Select::new(select).w_full().into_any_element(),
        RelationSelect::Id(select) => Select::new(select).w_full().into_any_element(),
        RelationSelect::Bool(select) => Select::new(select).w_full().into_any_element(),
        RelationSelect::Tags(select) => Select::new(select).w_full().into_any_element(),
        RelationSelect::Author(select) => Select::new(select).w_full().into_any_element(),
    }
}

fn render_value_editor(draft: &ConditionDraft, cx: &mut Context<QueryView>) -> AnyElement {
    match draft {
        ConditionDraft::NoField => placeholder_control("请选择字段", true, cx),
        ConditionDraft::NoCondition { .. } => placeholder_control("请选择条件", true, cx),
        ConditionDraft::Text(condition) => Input::new(&condition.input).w_full().into_any_element(),
        ConditionDraft::Number(condition) => render_number_value(&condition.value),
        ConditionDraft::Id(condition) => render_id_value(&condition.value),
        ConditionDraft::Bool(BoolCondition { value_select, .. }) => {
            Select::new(value_select).w_full().into_any_element()
        }
        ConditionDraft::Tags(condition) => render_tags_value(condition, cx),
        ConditionDraft::Author(condition) => match &condition.value {
            AuthorValue::Single(value) => value.clone().into_any_element(),
            AuthorValue::Multi(value) => value.clone().into_any_element(),
        },
    }
}

fn render_number_value(value: &NumberValue) -> AnyElement {
    match value {
        NumberValue::Single(input) => NumberInput::new(input).w_full().into_any_element(),
        NumberValue::Range(range) => range.clone().into_any_element(),
    }
}

fn render_id_value(value: &IdValue) -> AnyElement {
    match value {
        IdValue::Picker(value) => value.clone().into_any_element(),
        IdValue::Number(input) => NumberInput::new(input).w_full().into_any_element(),
        IdValue::Range(range) => range.clone().into_any_element(),
    }
}

fn render_tags_value(condition: &TagsCondition, cx: &mut Context<QueryView>) -> AnyElement {
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

fn render_sort_row(ix: usize, sort: &SortRow, cx: &mut Context<QueryView>) -> AnyElement {
    let sort_id = sort.id;
    let field_label = sort
        .field
        .map(|field| field.label())
        .unwrap_or("未选择排序字段");
    let direction_label = sort_direction_label(sort.direction);

    v_flex()
        .gap_1()
        .child(
            h_flex()
                .id(("sort-row", sort_id))
                .gap_2()
                .items_center()
                .px_2()
                .py_2()
                .hover(|style| style.bg(cx.theme().accent.opacity(0.18)))
                .drag_over::<DragSortRow>(move |this, drag, _window, cx| {
                    if drag.row_id == sort_id {
                        this
                    } else {
                        this.border_l_2()
                            .border_color(cx.theme().drag_border)
                            .bg(cx.theme().accent.opacity(0.25))
                    }
                })
                .on_drop(cx.listener(move |this, drag: &DragSortRow, _window, cx| {
                    this.advanced.move_sort_before(drag.row_id, sort_id);
                    cx.notify();
                }))
                .child(
                    div()
                        .id(("sort-drag-handle", sort_id))
                        .cursor_grab()
                        .p_1()
                        .rounded_sm()
                        .hover(|style| style.bg(cx.theme().accent))
                        .child(Icon::new(IconName::EllipsisVertical))
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
                        ),
                )
                .child(
                    Label::new(format!("{}", ix + 1))
                        .text_sm()
                        .text_color(cx.theme().muted_foreground),
                )
                .child(
                    v_flex()
                        .gap_1()
                        .flex_1()
                        .min_w(px(140.))
                        .child(
                            Label::new("排序字段")
                                .text_xs()
                                .text_color(cx.theme().muted_foreground),
                        )
                        .child(
                            Select::new(&sort.field_select)
                                .placeholder("请选择排序字段")
                                .w_full(),
                        ),
                )
                .child(
                    v_flex()
                        .gap_1()
                        .w(px(112.))
                        .child(
                            Label::new("方向")
                                .text_xs()
                                .text_color(cx.theme().muted_foreground),
                        )
                        .child(Select::new(&sort.direction_select).w_full()),
                )
                .child(
                    icon_button(
                        ("sort-remove", sort_id),
                        FeiwenIconName::Trash,
                        "删除排序规则",
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.advanced.remove_sort(sort_id);
                        cx.notify();
                    })),
                ),
        )
        .when_some(sort.error.as_ref(), |this, error| {
            this.child(
                h_flex()
                    .gap_1()
                    .items_center()
                    .text_color(cx.theme().danger)
                    .child(Icon::new(IconName::TriangleAlert))
                    .child(Label::new(error.clone()).text_xs()),
            )
        })
        .into_any_element()
}

fn icon_button(id: impl Into<ElementId>, icon: impl Into<Icon>, tooltip: &'static str) -> Button {
    Button::new(id).ghost().icon(icon).tooltip(tooltip)
}

fn row_separator() -> impl IntoElement {
    Divider::horizontal()
}

fn sort_direction_label(direction: SortDirection) -> &'static str {
    match direction {
        SortDirection::Asc => "升序",
        SortDirection::Desc => "降序",
    }
}

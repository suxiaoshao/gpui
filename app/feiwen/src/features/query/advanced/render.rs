use fluent_bundle::FluentArgs;
use gpui::{
    AnyElement, AppContext as _, Context, ElementId, Entity, InteractiveElement, IntoElement,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, div, prelude::FluentBuilder,
    px, rems,
};
use gpui_component::{
    ActiveTheme, Disableable, Icon, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants, Toggle, ToggleGroup, ToggleVariants},
    combobox::Combobox,
    h_flex,
    input::{Input, NumberInput},
    label::Label,
    scroll::ScrollableElement,
    searchable_list::{SearchableListDelegate, SearchableListItem},
    select::Select,
    switch::Switch,
    table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow},
    v_flex,
};
use gpui_form::{ErrorParamValue, PathKey, ValidationIssue, ValidationMessage};

use super::{
    controller::{
        AdvancedQueryController, AuthorConditionControls, ConditionEditor, ConditionRow,
        FilterGroup, FilterNode, NumberConditionControls, QueryPath, SortRow,
        TagsConditionControls,
    },
    options::{AuthorRelation, GroupRelation, NumberRelation},
    sort::DragSortRow,
};
use crate::{
    features::query::QueryView,
    foundation::{I18n, assets::IconName as FeiwenIconName},
    store::query::SortDirection,
};

const CONDITION_FIELD_COLUMN_WIDTH: f32 = 140.;
const CONDITION_RELATION_COLUMN_WIDTH: f32 = 120.;
const CONDITION_NEGATED_COLUMN_WIDTH: f32 = 56.;
const CONDITION_ACTION_COLUMN_WIDTH: f32 = 56.;
const CONDITION_COLUMN_COUNT: usize = 5;
const SORT_ORDER_COLUMN_WIDTH: f32 = 72.;
const SORT_DIRECTION_COLUMN_WIDTH: f32 = 112.;
const SORT_ACTION_COLUMN_WIDTH: f32 = 56.;

impl AdvancedQueryController {
    pub(crate) fn render_filters(
        &self,
        catalog_disabled: bool,
        cx: &mut Context<QueryView>,
    ) -> impl IntoElement {
        let root_id = self.root.id.clone();
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
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.advanced.add_condition(root_id.clone(), window, cx);
                            })),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(render_group(
                        &self.root,
                        &self.form,
                        &self.options,
                        0,
                        catalog_disabled,
                        cx,
                    )),
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
                        this.child(render_sorts_table(&self.sorts, &self.form, cx))
                    }),
            )
    }
}

fn render_group(
    group: &FilterGroup,
    form: &Entity<gpui_form::Form<super::super::form::QueryDraft>>,
    options: &super::options::QueryOptions,
    depth: usize,
    catalog_disabled: bool,
    cx: &mut Context<QueryView>,
) -> AnyElement {
    let group_id = group.id.clone();
    let relation = group.relation.value(form, cx).unwrap_or(GroupRelation::All);
    let negated = group.negated.value(form, cx).unwrap_or(false);
    let can_remove = depth > 0;
    let indent = px((depth as f32) * 16.);
    let negated_id = group_id.clone();
    let add_condition_id = group_id.clone();
    let add_group_id = group_id.clone();
    let remove_id = group_id.clone();

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
                        .child(group_relation_toggle(group_id.clone(), relation, cx))
                        .child(
                            Switch::new(path_element_id("group-negated", &group_id))
                                .checked(negated)
                                .label("排除")
                                .on_click(cx.listener(move |this, checked, _, cx| {
                                    this.advanced.set_group_negated(
                                        negated_id.clone(),
                                        *checked,
                                        cx,
                                    );
                                })),
                        ),
                )
                .child(
                    h_flex()
                        .gap_1()
                        .child(
                            Button::new(path_element_id("group-add-condition", &group_id))
                                .ghost()
                                .icon(IconName::Plus)
                                .label("添加条件")
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.advanced.add_condition(
                                        add_condition_id.clone(),
                                        window,
                                        cx,
                                    );
                                })),
                        )
                        .child(
                            Button::new(path_element_id("group-add-subgroup", &group_id))
                                .ghost()
                                .icon(IconName::Plus)
                                .label("添加子组")
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.advanced.add_group(add_group_id.clone(), window, cx);
                                })),
                        )
                        .when(can_remove, |this| {
                            this.child(
                                icon_button(
                                    path_element_id("group-remove", &group_id),
                                    FeiwenIconName::Trash,
                                    "删除条件组",
                                )
                                .on_click(cx.listener(
                                    move |this, _, window, cx| {
                                        this.advanced.remove_node(remove_id.clone(), window, cx);
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
        .child(render_conditions_table(
            group,
            form,
            options,
            depth,
            catalog_disabled,
            cx,
        ))
        .into_any_element()
}

fn render_conditions_table(
    group: &FilterGroup,
    form: &Entity<gpui_form::Form<super::super::form::QueryDraft>>,
    options: &super::options::QueryOptions,
    depth: usize,
    catalog_disabled: bool,
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
            TableBody::new().children(group.items.iter().map(|item| match item {
                FilterNode::Condition(condition) => {
                    render_condition_row(condition, form, options, catalog_disabled, cx)
                }
                FilterNode::Group(group) => TableRow::new().child(condition_span_cell(
                    render_group(group, form, options, depth + 1, catalog_disabled, cx),
                )),
            })),
        )
}

fn group_relation_toggle(
    group_id: PathKey,
    relation: GroupRelation,
    cx: &mut Context<QueryView>,
) -> impl IntoElement {
    ToggleGroup::new(path_element_id("group-relation", &group_id))
        .segmented()
        .outline()
        .child(
            Toggle::new(path_element_id("group-relation-all", &group_id))
                .label("全部满足")
                .checked(matches!(relation, GroupRelation::All)),
        )
        .child(
            Toggle::new(path_element_id("group-relation-any", &group_id))
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
            this.advanced.set_group_relation(group_id.clone(), next, cx);
        }))
}

fn render_condition_row(
    condition: &ConditionRow,
    form: &Entity<gpui_form::Form<super::super::form::QueryDraft>>,
    options: &super::options::QueryOptions,
    catalog_disabled: bool,
    cx: &mut Context<QueryView>,
) -> TableRow {
    let condition_id = condition.id.clone();
    let negated_id = condition_id.clone();
    let remove_id = condition_id.clone();
    let negated = condition.negated.value(form, cx).unwrap_or(false);
    let field_errors = path_error_messages(&condition.field, form, cx);
    let editor_disabled = catalog_disabled
        && matches!(
            condition.editor,
            ConditionEditor::Tags(_) | ConditionEditor::Author(_)
        );

    TableRow::new()
        .child(condition_table_cell(
            CONDITION_FIELD_COLUMN_WIDTH,
            control_with_errors(
                Select::new(&condition.field_select)
                    .placeholder("请选择字段")
                    .w_full(),
                field_errors,
                cx,
            ),
        ))
        .child(condition_table_cell(
            CONDITION_RELATION_COLUMN_WIDTH,
            render_relation_editor(&condition.editor, form, editor_disabled, cx),
        ))
        .child(condition_value_table_cell(render_value_editor(
            &condition.editor,
            form,
            options,
            editor_disabled,
            cx,
        )))
        .child(
            condition_table_cell(
                CONDITION_NEGATED_COLUMN_WIDTH,
                h_flex().w_full().justify_center().child(
                    Switch::new(path_element_id("condition-negated", &condition_id))
                        .checked(negated)
                        .on_click(cx.listener(move |this, checked, _, cx| {
                            this.advanced
                                .set_condition_negated(negated_id.clone(), *checked, cx);
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
                        path_element_id("condition-remove", &condition_id),
                        FeiwenIconName::Trash,
                        "删除条件",
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.advanced.remove_node(remove_id.clone(), window, cx);
                    })),
                ),
            )
            .min_w(px(0.)),
        )
}

fn render_relation_editor(
    editor: &ConditionEditor,
    form: &Entity<gpui_form::Form<super::super::form::QueryDraft>>,
    disabled: bool,
    cx: &mut Context<QueryView>,
) -> AnyElement {
    match editor {
        ConditionEditor::Unselected => placeholder_control("请选择字段", true, cx),
        ConditionEditor::Text(controls) => control_with_errors(
            Select::new(&*controls.relation).disabled(disabled).w_full(),
            path_error_messages(&controls.relation_path, form, cx),
            cx,
        ),
        ConditionEditor::Number(controls) => control_with_errors(
            Select::new(&*controls.relation).disabled(disabled).w_full(),
            path_error_messages(&controls.relation_path, form, cx),
            cx,
        ),
        ConditionEditor::Bool(controls) => control_with_errors(
            Select::new(&*controls.relation).disabled(disabled).w_full(),
            path_error_messages(&controls.relation_path, form, cx),
            cx,
        ),
        ConditionEditor::Tags(controls) => control_with_errors(
            Select::new(&*controls.relation).disabled(disabled).w_full(),
            path_error_messages(&controls.relation_path, form, cx),
            cx,
        ),
        ConditionEditor::Author(controls) => control_with_errors(
            Select::new(&*controls.relation).disabled(disabled).w_full(),
            path_error_messages(&controls.relation_path, form, cx),
            cx,
        ),
    }
}

fn render_value_editor(
    editor: &ConditionEditor,
    form: &Entity<gpui_form::Form<super::super::form::QueryDraft>>,
    options: &super::options::QueryOptions,
    disabled: bool,
    cx: &mut Context<QueryView>,
) -> AnyElement {
    match editor {
        ConditionEditor::Unselected => placeholder_control("请选择字段", true, cx),
        ConditionEditor::Text(controls) => control_with_errors(
            Input::new(&controls.value).disabled(disabled).w_full(),
            path_error_messages(&controls.value_path, form, cx),
            cx,
        ),
        ConditionEditor::Number(controls) => render_number_value(controls, form, disabled, cx),
        ConditionEditor::Bool(controls) => control_with_errors(
            Select::new(&*controls.value).disabled(disabled).w_full(),
            path_error_messages(&controls.value_path, form, cx),
            cx,
        ),
        ConditionEditor::Tags(controls) => render_tags_value(controls, form, options, disabled, cx),
        ConditionEditor::Author(controls) => {
            render_author_value(controls, form, options, disabled, cx)
        }
    }
}

fn render_number_value(
    controls: &NumberConditionControls,
    form: &Entity<gpui_form::Form<super::super::form::QueryDraft>>,
    disabled: bool,
    cx: &mut Context<QueryView>,
) -> AnyElement {
    let relation = controls.relation_path.value(form, cx).ok().flatten();
    if relation == Some(NumberRelation::Between) {
        return h_flex()
            .w_full()
            .gap_2()
            .child(control_with_errors(
                NumberInput::new(&controls.min).disabled(disabled).w_full(),
                path_error_messages(&controls.min_path, form, cx),
                cx,
            ))
            .child(control_with_errors(
                NumberInput::new(&controls.max).disabled(disabled).w_full(),
                path_error_messages(&controls.max_path, form, cx),
                cx,
            ))
            .into_any_element();
    }
    control_with_errors(
        NumberInput::new(&controls.single)
            .disabled(disabled)
            .w_full(),
        path_error_messages(&controls.single_path, form, cx),
        cx,
    )
}

fn render_tags_value(
    controls: &TagsConditionControls,
    form: &Entity<gpui_form::Form<super::super::form::QueryDraft>>,
    options: &super::options::QueryOptions,
    disabled: bool,
    cx: &mut Context<QueryView>,
) -> AnyElement {
    let relation = controls.relation_path.value(form, cx).ok().flatten();
    if relation.is_some_and(|relation| !relation.needs_value()) {
        return placeholder_control("无需填写", false, cx);
    }
    let errors = path_error_messages(&controls.values_path, form, cx);
    let mut hints = Vec::new();
    if let Ok(values) = controls.values_path.value(form, cx) {
        let missing = values
            .iter()
            .filter(|value| !options.tags.iter().any(|option| &option.name == *value))
            .count();
        if missing > 0 {
            hints.push(format!("{missing} 个已选标签当前不在目录中"));
        }
    }
    control_with_feedback(
        render_multi_combobox(&*controls.values, "选择标签", disabled),
        errors,
        hints,
        cx,
    )
}

fn render_author_value(
    controls: &AuthorConditionControls,
    form: &Entity<gpui_form::Form<super::super::form::QueryDraft>>,
    options: &super::options::QueryOptions,
    disabled: bool,
    cx: &mut Context<QueryView>,
) -> AnyElement {
    match controls.relation_path.value(form, cx).ok().flatten() {
        Some(
            AuthorRelation::NameContains
            | AuthorRelation::NameStartsWith
            | AuthorRelation::NameEndsWith
            | AuthorRelation::NameEquals,
        ) => control_with_errors(
            Input::new(&controls.text).disabled(disabled).w_full(),
            path_error_messages(&controls.text_path, form, cx),
            cx,
        ),
        Some(AuthorRelation::Is | AuthorRelation::IsNot) => {
            let errors = path_error_messages(&controls.single_path, form, cx);
            let mut hints = Vec::new();
            if let Ok(Some(value)) = controls.single_path.value(form, cx)
                && !options.authors.iter().any(|option| option.author == value)
            {
                hints.push("已选作者当前不在目录中".to_owned());
            }
            control_with_feedback(
                Select::new(&*controls.single)
                    .placeholder("选择作者")
                    .search_placeholder("搜索")
                    .menu_width(px(320.))
                    .disabled(disabled)
                    .w_full(),
                errors,
                hints,
                cx,
            )
        }
        Some(AuthorRelation::In | AuthorRelation::NotIn) => {
            let errors = path_error_messages(&controls.multiple_path, form, cx);
            let mut hints = Vec::new();
            if let Ok(values) = controls.multiple_path.value(form, cx) {
                let missing = values
                    .iter()
                    .filter(|value| {
                        !options
                            .authors
                            .iter()
                            .any(|option| &option.author == *value)
                    })
                    .count();
                if missing > 0 {
                    hints.push(format!("{missing} 个已选作者当前不在目录中"));
                }
            }
            control_with_feedback(
                render_multi_combobox(&*controls.multiple, "选择作者", disabled),
                errors,
                hints,
                cx,
            )
        }
        None => placeholder_control("请选择条件", true, cx),
    }
}

fn render_multi_combobox<D>(
    state: &Entity<gpui_component::combobox::ComboboxState<D>>,
    placeholder: &'static str,
    disabled: bool,
) -> AnyElement
where
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem + Clone + 'static,
    <D::Item as SearchableListItem>::Value: PartialEq + Clone,
{
    let trigger_state = state.clone();
    Combobox::new(state)
        .placeholder(placeholder)
        .search_placeholder("搜索")
        .menu_width(px(360.))
        .menu_max_h(rems(12.))
        .disabled(disabled)
        .render_trigger(move |ctx, _, cx| {
            let items = ctx.selection;
            if items.is_empty() {
                return div()
                    .text_color(cx.theme().muted_foreground)
                    .child(placeholder)
                    .into_any_element();
            }
            h_flex()
                .w_full()
                .flex_wrap()
                .gap_1()
                .children(
                    items
                        .iter()
                        .cloned()
                        .enumerate()
                        .map(|(ix, (index, item))| {
                            let state = trigger_state.clone();
                            h_flex()
                                .gap_0p5()
                                .items_center()
                                .rounded_sm()
                                .border_1()
                                .border_color(cx.theme().border)
                                .px_1()
                                .text_xs()
                                .child(item.title())
                                .when(!disabled, |this| {
                                    this.child(
                                        Button::new(SharedString::from(format!(
                                            "multi-select-remove-{placeholder}-{ix}"
                                        )))
                                        .ghost()
                                        .xsmall()
                                        .icon(Icon::new(IconName::Close).xsmall())
                                        .tab_stop(false)
                                        .on_click(
                                            move |_, _, cx| {
                                                state.update(cx, |state, cx| {
                                                    state.remove_selected_index(index, cx);
                                                });
                                            },
                                        ),
                                    )
                                })
                        }),
                )
                .into_any_element()
        })
        .w_full()
        .into_any_element()
}

fn render_sorts_table(
    sorts: &[SortRow],
    form: &Entity<gpui_form::Form<super::super::form::QueryDraft>>,
    cx: &mut Context<QueryView>,
) -> impl IntoElement {
    let rows = sorts
        .iter()
        .enumerate()
        .map(|(ix, sort)| render_sort_item(ix, sort, form, cx).into_any_element())
        .collect::<Vec<_>>();
    v_flex()
        .w_full()
        .child(
            h_flex()
                .w_full()
                .bg(cx.theme().tokens.table_head.background)
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

fn render_sort_item(
    ix: usize,
    sort: &SortRow,
    form: &Entity<gpui_form::Form<super::super::form::QueryDraft>>,
    cx: &mut Context<QueryView>,
) -> impl IntoElement {
    let sort_id = sort.id.clone();
    let drag_over_id = sort_id.clone();
    let drop_id = sort_id.clone();
    let remove_id = sort_id.clone();
    let field = sort.field_path.value(form, cx).ok().flatten();
    let direction = sort
        .direction_path
        .value(form, cx)
        .unwrap_or(SortDirection::Asc);
    let errors = path_error_messages(&sort.field_path, form, cx);
    let has_error = !errors.is_empty();
    let field_label = field.map(|field| field.label()).unwrap_or("未选择排序字段");
    let direction_label = sort_direction_label(direction);

    v_flex()
        .w_full()
        .when(ix > 0, |this| {
            this.border_t_1().border_color(cx.theme().table_row_border)
        })
        .child(
            h_flex()
                .id(path_element_id("sort-row", &sort_id))
                .w_full()
                .items_center()
                .hover(|style| style.bg(cx.theme().tokens.accent.background.opacity(0.18)))
                .drag_over::<DragSortRow>(move |this, drag, _window, cx| {
                    if drag.row_id == drag_over_id {
                        this
                    } else {
                        this.border_l_2().border_color(cx.theme().drag_border).bg(cx
                            .theme()
                            .tokens
                            .accent
                            .background
                            .opacity(0.25))
                    }
                })
                .on_drop(cx.listener(move |this, drag: &DragSortRow, window, cx| {
                    this.advanced.move_sort_before(
                        drag.row_id.clone(),
                        drop_id.clone(),
                        window,
                        cx,
                    );
                }))
                .child(sort_fixed_cell(
                    SORT_ORDER_COLUMN_WIDTH,
                    h_flex()
                        .w_full()
                        .items_center()
                        .gap_1()
                        .child(
                            div()
                                .id(path_element_id("sort-drag-handle", &sort_id))
                                .p_1()
                                .rounded_sm()
                                .cursor_grab()
                                .hover(|style| style.bg(cx.theme().tokens.accent.background))
                                .on_drag(
                                    DragSortRow::new(
                                        sort_id.clone(),
                                        ix + 1,
                                        field_label,
                                        direction_label,
                                        has_error,
                                    ),
                                    |drag, _position, _window, cx| {
                                        cx.stop_propagation();
                                        cx.new(|_| drag.clone())
                                    },
                                )
                                .child(Icon::new(IconName::EllipsisVertical)),
                        )
                        .child(
                            Label::new(format!("{}", ix + 1))
                                .text_sm()
                                .text_color(cx.theme().muted_foreground),
                        ),
                ))
                .child(sort_field_cell(control_with_errors(
                    Select::new(&*sort.field_select)
                        .placeholder("请选择排序字段")
                        .w_full(),
                    errors,
                    cx,
                )))
                .child(sort_fixed_cell(
                    SORT_DIRECTION_COLUMN_WIDTH,
                    Select::new(&sort.direction_select).w_full(),
                ))
                .child(sort_fixed_cell(
                    SORT_ACTION_COLUMN_WIDTH,
                    h_flex().w_full().justify_center().child(
                        icon_button(
                            path_element_id("sort-remove", &sort_id),
                            FeiwenIconName::Trash,
                            "删除排序规则",
                        )
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.advanced.remove_sort(remove_id.clone(), window, cx);
                            },
                        )),
                    ),
                )),
        )
}

fn control_with_errors(
    control: impl IntoElement,
    messages: Vec<String>,
    cx: &mut Context<QueryView>,
) -> AnyElement {
    control_with_feedback(control, messages, Vec::new(), cx)
}

fn control_with_feedback(
    control: impl IntoElement,
    errors: Vec<String>,
    hints: Vec<String>,
    cx: &mut Context<QueryView>,
) -> AnyElement {
    v_flex()
        .w_full()
        .gap_1()
        .child(control)
        .children(errors.into_iter().map(|message| {
            h_flex()
                .gap_1()
                .items_center()
                .text_color(cx.theme().danger)
                .child(Icon::new(IconName::TriangleAlert))
                .child(Label::new(message).text_xs())
        }))
        .children(hints.into_iter().map(|message| {
            h_flex()
                .gap_1()
                .items_center()
                .text_color(cx.theme().muted_foreground)
                .child(Icon::new(IconName::Info))
                .child(Label::new(message).text_xs())
        }))
        .into_any_element()
}

fn path_error_messages<T: Clone + PartialEq + 'static>(
    path: &QueryPath<T>,
    form: &Entity<gpui_form::Form<super::super::form::QueryDraft>>,
    cx: &Context<QueryView>,
) -> Vec<String> {
    path.errors(form, cx)
        .unwrap_or_default()
        .iter()
        .map(|issue| validation_issue_message(issue, cx))
        .collect()
}

fn validation_issue_message(issue: &ValidationIssue, cx: &Context<QueryView>) -> String {
    match issue.message() {
        ValidationMessage::Literal(message) => message.to_string(),
        ValidationMessage::Key { key, params } => {
            let mut args = FluentArgs::new();
            for (name, value) in params {
                let value = match value {
                    ErrorParamValue::String(value) => value.to_string(),
                    ErrorParamValue::Integer(value) => value.to_string(),
                    ErrorParamValue::Unsigned(value) => value.to_string(),
                    ErrorParamValue::Float(value) => value.to_string(),
                    ErrorParamValue::Bool(value) => value.to_string(),
                };
                args.set(name.as_ref(), value);
            }
            cx.global::<I18n>().t_with_args(key.as_ref(), &args)
        }
    }
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
        .flex_grow(1.0)
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
        .flex_grow(1.0)
        .child(div().w_full().min_w_0().child(child))
}

fn condition_span_cell(child: impl IntoElement) -> TableCell {
    TableCell::new()
        .col_span(CONDITION_COLUMN_COUNT)
        .min_w(px(0.))
        .w_full()
        .child(child)
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
        .flex_grow(1.0)
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
        .flex_grow(1.0)
        .px(px(8.))
        .py(px(6.))
        .flex()
        .items_center()
        .child(div().w_full().min_w_0().child(child))
}

fn icon_button(id: impl Into<ElementId>, icon: impl Into<Icon>, tooltip: &'static str) -> Button {
    Button::new(id).ghost().icon(icon).tooltip(tooltip)
}

fn path_element_id(label: &'static str, key: &PathKey) -> ElementId {
    ElementId::NamedChild(std::sync::Arc::new(key.into()), label.into())
}

fn sort_direction_label(direction: SortDirection) -> &'static str {
    match direction {
        SortDirection::Asc => "升序",
        SortDirection::Desc => "降序",
    }
}

use gpui::{App, AppContext as _, Context, Entity, Subscription, Window};
use gpui_component::{
    combobox::ComboboxState,
    input::InputState,
    searchable_list::SearchableListDelegate,
    select::{SearchableVec, SelectEvent, SelectState},
};
use gpui_form::{
    ControlBinding, ControlProjection, DynamicItemsPath, DynamicPath, Form, ItemPath,
    MutationError, PathKey, PrepareError, Prepared, ResolveError, TotalItemsPath, TotalPath,
};
use gpui_form_gpui_component::{FormCombobox, FormInput, FormSelect};

use super::options::{
    AuthorOption, AuthorRelation, BoolRelation, FieldSelectItems, GroupRelation, NumberRelation,
    QueryOptions, SelectChoice, SortField, TagsRelation, TextRelation, author_relation_items,
    bool_relation_items, bool_value_items, field_items, number_relation_items,
    sort_direction_items, sort_field_items, tags_relation_items, text_relation_items,
};
use crate::{
    features::query::{
        QueryView,
        form::{
            AuthorConditionDraft, BoolConditionDraft, ConditionField, FilterConditionDraft,
            FilterGroupDraft, FilterNodeDraft, FilterNodeKind, NumberConditionDraft, QueryDraft,
            QueryDraftValidator, SortDraft, TagsConditionDraft, TextConditionDraft,
        },
    },
    store::query::{AuthorRef, SortDirection},
};

type FieldSelectState = SelectState<FieldSelectItems>;

pub(crate) struct AdvancedQueryController {
    pub(crate) form: Entity<Form<QueryDraft>>,
    pub(super) root: RootFilterGroup,
    pub(super) sorts: Vec<SortRow>,
    pub(super) options: QueryOptions,
    _subscriptions: Vec<Subscription>,
}

pub(super) struct RootFilterGroup {
    pub(super) id: PathKey,
    pub(super) relation: TotalPath<QueryDraft, GroupRelation>,
    pub(super) negated: TotalPath<QueryDraft, bool>,
    children: TotalItemsPath<QueryDraft, FilterNodeDraft>,
    pub(super) items: Vec<FilterNode>,
}

pub(super) struct DynamicFilterGroup {
    pub(super) id: PathKey,
    pub(super) item: ItemPath<QueryDraft, FilterNodeDraft>,
    pub(super) relation: DynamicPath<QueryDraft, GroupRelation>,
    pub(super) negated: DynamicPath<QueryDraft, bool>,
    children: DynamicItemsPath<QueryDraft, FilterNodeDraft>,
    pub(super) items: Vec<FilterNode>,
}

pub(super) enum FilterNode {
    Condition(Box<ConditionRow>),
    Group(Box<DynamicFilterGroup>),
}

pub(super) struct ConditionRow {
    pub(super) id: PathKey,
    pub(super) item: ItemPath<QueryDraft, FilterNodeDraft>,
    pub(super) negated: DynamicPath<QueryDraft, bool>,
    pub(super) field: DynamicPath<QueryDraft, ConditionField>,
    pub(super) field_select: Entity<FieldSelectState>,
    pub(super) editor: ConditionEditor,
    _field_binding: ControlBinding,
    _subscriptions: Vec<Subscription>,
}

pub(super) enum ConditionEditor {
    Unselected,
    Text(TextConditionControls),
    Number(NumberConditionControls),
    Bool(BoolConditionControls),
    Tags(TagsConditionControls),
    Author(AuthorConditionControls),
}

pub(super) struct TextConditionControls {
    pub(super) relation_path: DynamicPath<QueryDraft, Option<TextRelation>>,
    pub(super) relation: FormSelect<Vec<SelectChoice<TextRelation>>>,
    pub(super) value_path: DynamicPath<QueryDraft, String>,
    pub(super) value: FormInput,
}

pub(super) struct NumberConditionControls {
    pub(super) relation_path: DynamicPath<QueryDraft, Option<NumberRelation>>,
    pub(super) relation: FormSelect<Vec<SelectChoice<NumberRelation>>>,
    pub(super) single_path: DynamicPath<QueryDraft, String>,
    pub(super) single: FormInput,
    pub(super) min_path: DynamicPath<QueryDraft, String>,
    pub(super) min: FormInput,
    pub(super) max_path: DynamicPath<QueryDraft, String>,
    pub(super) max: FormInput,
}

pub(super) struct BoolConditionControls {
    pub(super) relation_path: DynamicPath<QueryDraft, Option<BoolRelation>>,
    pub(super) relation: FormSelect<Vec<SelectChoice<BoolRelation>>>,
    pub(super) value_path: DynamicPath<QueryDraft, Option<bool>>,
    pub(super) value: FormSelect<Vec<SelectChoice<bool>>>,
}

pub(super) struct TagsConditionControls {
    pub(super) relation_path: DynamicPath<QueryDraft, Option<TagsRelation>>,
    pub(super) relation: FormSelect<Vec<SelectChoice<TagsRelation>>>,
    pub(super) values_path: DynamicPath<QueryDraft, Vec<String>>,
    pub(super) values: FormCombobox<SearchableVec<super::options::TagOption>>,
}

pub(super) struct AuthorConditionControls {
    pub(super) relation_path: DynamicPath<QueryDraft, Option<AuthorRelation>>,
    pub(super) relation: FormSelect<Vec<SelectChoice<AuthorRelation>>>,
    pub(super) text_path: DynamicPath<QueryDraft, String>,
    pub(super) text: FormInput,
    pub(super) single_path: DynamicPath<QueryDraft, Option<AuthorRef>>,
    pub(super) single: FormSelect<SearchableVec<AuthorOption>>,
    pub(super) multiple_path: DynamicPath<QueryDraft, Vec<AuthorRef>>,
    pub(super) multiple: FormCombobox<SearchableVec<AuthorOption>>,
}

pub(super) struct SortRow {
    pub(super) id: PathKey,
    pub(super) item: ItemPath<QueryDraft, SortDraft>,
    pub(super) field_path: DynamicPath<QueryDraft, Option<SortField>>,
    pub(super) direction_path: DynamicPath<QueryDraft, Option<SortDirection>>,
    pub(super) field_select: FormSelect<Vec<SelectChoice<SortField>>>,
    pub(super) direction_select: FormSelect<Vec<SelectChoice<SortDirection>>>,
}

struct GroupReconcileCandidate {
    entries: Vec<NodeReconcileCandidate>,
}

enum NodeReconcileCandidate {
    ReuseCondition {
        old_index: usize,
        item: ItemPath<QueryDraft, FilterNodeDraft>,
        negated: DynamicPath<QueryDraft, bool>,
        field: DynamicPath<QueryDraft, ConditionField>,
        editor: Option<Box<ConditionEditor>>,
    },
    ReuseGroup {
        old_index: usize,
        id: PathKey,
        item: ItemPath<QueryDraft, FilterNodeDraft>,
        relation: DynamicPath<QueryDraft, GroupRelation>,
        negated: DynamicPath<QueryDraft, bool>,
        children: DynamicItemsPath<QueryDraft, FilterNodeDraft>,
        items: Box<GroupReconcileCandidate>,
    },
    New(FilterNode),
}

enum SortReconcileCandidate {
    Reuse {
        old_index: usize,
        item: ItemPath<QueryDraft, SortDraft>,
    },
    New(Box<SortRow>),
}

impl FilterNode {
    fn id(&self) -> &PathKey {
        match self {
            Self::Condition(condition) => &condition.id,
            Self::Group(group) => &group.id,
        }
    }
}

impl ConditionRow {
    fn is_current(&self, form: &Entity<Form<QueryDraft>>, cx: &App) -> bool {
        let Ok(field) = self.field.try_get(form, cx) else {
            return false;
        };
        match (&self.editor, field) {
            (ConditionEditor::Unselected, ConditionField::Unselected) => true,
            (ConditionEditor::Text(controls), ConditionField::Text(_)) => {
                controls.relation_path.try_get(form, cx).is_ok()
            }
            (ConditionEditor::Number(controls), ConditionField::Number(_)) => {
                controls.relation_path.try_get(form, cx).is_ok()
            }
            (ConditionEditor::Bool(controls), ConditionField::Bool(_)) => {
                controls.relation_path.try_get(form, cx).is_ok()
            }
            (ConditionEditor::Tags(controls), ConditionField::Tags(_)) => {
                controls.relation_path.try_get(form, cx).is_ok()
            }
            (ConditionEditor::Author(controls), ConditionField::Author(_)) => {
                controls.relation_path.try_get(form, cx).is_ok()
            }
            _ => false,
        }
    }
}

impl AdvancedQueryController {
    pub(crate) fn new(
        options: QueryOptions,
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) -> Self {
        let form = cx.new(|_| Form::new(QueryDraft::default()).with_validator(QueryDraftValidator));
        let (root, sorts) = Self::build_controls(&form, &options, window, cx)
            .expect("default query form paths must bind");
        let _subscriptions = vec![cx.subscribe_in(
            &form,
            window,
            |view, _, event: &gpui_form::FormEvent<QueryDraft>, window, cx| {
                if needs_structural_reconcile(event) {
                    view.advanced.reconcile_controls(window, cx);
                }
                cx.notify();
            },
        )];
        Self {
            form,
            root,
            sorts,
            options,
            _subscriptions,
        }
    }

    pub(crate) fn prepare(
        &mut self,
        cx: &mut Context<QueryView>,
    ) -> Result<Prepared<QueryDraft>, PrepareError> {
        self.form.update(cx, |form, cx| form.prepare(cx))
    }

    pub(crate) fn load_draft(
        &mut self,
        draft: QueryDraft,
        _window: &mut Window,
        cx: &mut Context<QueryView>,
    ) {
        self.form.update(cx, |form, cx| form.replace(draft, cx));
    }

    pub(crate) fn reset(&mut self, _window: &mut Window, cx: &mut Context<QueryView>) {
        self.form.update(cx, |form, cx| form.reset(cx));
    }

    pub(crate) fn update_options(
        &mut self,
        options: QueryOptions,
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) {
        self.options = options;
        self.root
            .update_options(&self.form, &self.options, window, cx);
        cx.notify();
    }

    pub(crate) fn condition_count(&self) -> usize {
        self.root.condition_count()
    }

    pub(crate) fn sort_count(&self) -> usize {
        self.sorts.len()
    }

    pub(crate) fn add_condition(
        &mut self,
        group_id: PathKey,
        _window: &mut Window,
        cx: &mut Context<QueryView>,
    ) {
        let result = if self.root.id == group_id {
            self.root
                .children
                .append(&self.form, FilterNodeDraft::condition(), cx)
        } else if let Some(group) = self.root.find_dynamic_group(&group_id) {
            group
                .children
                .append(&self.form, FilterNodeDraft::condition(), cx)
        } else {
            return;
        };
        match result {
            Ok(_) => {}
            Err(error) => log_mutation_error("add query condition", error),
        }
    }

    pub(crate) fn add_group(
        &mut self,
        group_id: PathKey,
        _window: &mut Window,
        cx: &mut Context<QueryView>,
    ) {
        let value = FilterNodeDraft {
            kind: FilterNodeKind::Group(FilterGroupDraft {
                relation: GroupRelation::All,
                negated: false,
                children: Vec::new(),
            }),
        };
        let result = if self.root.id == group_id {
            self.root.children.append(&self.form, value, cx)
        } else if let Some(group) = self.root.find_dynamic_group(&group_id) {
            group.children.append(&self.form, value, cx)
        } else {
            return;
        };
        match result {
            Ok(_) => {}
            Err(error) => log_mutation_error("add query group", error),
        }
    }

    pub(crate) fn remove_node(
        &mut self,
        node_id: PathKey,
        _window: &mut Window,
        cx: &mut Context<QueryView>,
    ) {
        match self.root.remove_descendant(&node_id, &self.form, cx) {
            Ok(None) => {}
            Ok(_) => {}
            Err(error) => log_mutation_error("remove query node", error),
        }
    }

    pub(super) fn set_group_relation(
        &mut self,
        group_id: PathKey,
        relation: GroupRelation,
        cx: &mut Context<QueryView>,
    ) {
        if self.root.id == group_id {
            self.root.relation.set(&self.form, relation, cx);
        } else if let Some(group) = self.root.find_dynamic_group(&group_id)
            && let Err(error) = group.relation.try_set(&self.form, relation, cx)
        {
            log_resolve_error("set query group relation", error);
        }
    }

    pub(crate) fn set_group_negated(
        &mut self,
        group_id: PathKey,
        negated: bool,
        cx: &mut Context<QueryView>,
    ) {
        if self.root.id == group_id {
            self.root.negated.set(&self.form, negated, cx);
        } else if let Some(group) = self.root.find_dynamic_group(&group_id)
            && let Err(error) = group.negated.try_set(&self.form, negated, cx)
        {
            log_resolve_error("set query group negation", error);
        }
    }

    pub(crate) fn set_condition_negated(
        &mut self,
        condition_id: PathKey,
        negated: bool,
        cx: &mut Context<QueryView>,
    ) {
        let Some(path) = self
            .root
            .find_condition(&condition_id)
            .map(|condition| condition.negated.clone())
        else {
            return;
        };
        if let Err(error) = path.try_set(&self.form, negated, cx) {
            log_resolve_error("set query condition negation", error);
        }
    }

    pub(crate) fn add_sort(&mut self, _window: &mut Window, cx: &mut Context<QueryView>) {
        let sorts = QueryDraft::ROOT.then(QueryDraft::SORTS);
        match sorts.append(
            &self.form,
            SortDraft {
                field: None,
                direction: Some(SortDirection::Asc),
            },
            cx,
        ) {
            Ok(_) => {}
            Err(error) => log_mutation_error("add query sort", error),
        }
    }

    pub(crate) fn remove_sort(
        &mut self,
        sort_id: PathKey,
        _window: &mut Window,
        cx: &mut Context<QueryView>,
    ) {
        let Some(item) = self
            .sorts
            .iter()
            .find(|row| row.id == sort_id)
            .map(|row| row.item.clone())
        else {
            return;
        };
        let sorts = QueryDraft::ROOT.then(QueryDraft::SORTS);
        match sorts.remove(&self.form, item, cx) {
            Ok(_) => {}
            Err(error) => log_mutation_error("remove query sort", error),
        }
    }

    pub(crate) fn move_sort_before(
        &mut self,
        source_id: PathKey,
        target_id: PathKey,
        _window: &mut Window,
        cx: &mut Context<QueryView>,
    ) {
        if source_id == target_id {
            return;
        }
        let source = self
            .sorts
            .iter()
            .find(|row| row.id == source_id)
            .map(|row| row.item.clone());
        let target = self
            .sorts
            .iter()
            .find(|row| row.id == target_id)
            .map(|row| row.item.clone());
        let (Some(source), Some(target)) = (source, target) else {
            return;
        };
        let sorts = QueryDraft::ROOT.then(QueryDraft::SORTS);
        match sorts.move_before(&self.form, &source, &target, cx) {
            Ok(()) => {}
            Err(error) => log_mutation_error("move query sort", error),
        }
    }

    fn reconcile_controls<Owner: 'static>(&mut self, window: &mut Window, cx: &mut Context<Owner>) {
        let root_items = self.root.children.items(&self.form, cx);
        let staged = Self::stage_group_items(
            &self.form,
            root_items,
            &self.root.items,
            &self.options,
            window,
            cx,
        )
        .and_then(|root_items| {
            Self::stage_sorts(&self.form, &self.sorts, window, cx).map(|sorts| (root_items, sorts))
        });

        match staged {
            Ok((root_candidate, sort_candidate)) => {
                let old_root_items = std::mem::take(&mut self.root.items);
                let old_sorts = std::mem::take(&mut self.sorts);
                self.root.items = Self::commit_group_items(old_root_items, root_candidate);
                self.sorts = Self::commit_sorts(old_sorts, sort_candidate);
            }
            Err(error) => {
                tracing::error!(error = %error, "failed to reconcile typed query controls");
                if let Ok((root, sorts)) =
                    Self::build_controls(&self.form, &self.options, window, cx)
                {
                    self.root = root;
                    self.sorts = sorts;
                }
            }
        }
        cx.notify();
    }

    fn stage_group_items<Owner: 'static>(
        form: &Entity<Form<QueryDraft>>,
        current_items: Vec<ItemPath<QueryDraft, FilterNodeDraft>>,
        old_items: &[FilterNode],
        options: &QueryOptions,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Result<GroupReconcileCandidate, MutationError> {
        let mut entries = Vec::new();
        for item in current_items {
            let item_id = item.key();
            let old_index = old_items.iter().position(|node| node.id() == &item_id);
            let kind = item.clone().then(FilterNodeDraft::KIND);
            match kind.try_get(form, cx)? {
                FilterNodeKind::Condition(_) => {
                    let condition = kind
                        .clone()
                        .case(FilterNodeKind::CONDITION)
                        .resolve(form, cx)?
                        .expect("condition payload must remain active while its case is selected");
                    let negated = condition.clone().then(FilterConditionDraft::NEGATED);
                    let field = condition.then(FilterConditionDraft::FIELD);
                    if let Some(old_index) = old_index
                        && let FilterNode::Condition(old_condition) = &old_items[old_index]
                    {
                        let editor = if old_condition.is_current(form, cx) {
                            None
                        } else {
                            Some(Box::new(Self::build_editor(
                                form,
                                field.clone(),
                                options,
                                window,
                                cx,
                            )?))
                        };
                        entries.push(NodeReconcileCandidate::ReuseCondition {
                            old_index,
                            item,
                            negated,
                            field,
                            editor,
                        });
                        continue;
                    }
                    entries.push(NodeReconcileCandidate::New(FilterNode::Condition(
                        Box::new(Self::build_condition(
                            form, item_id, item, negated, field, options, window, cx,
                        )?),
                    )));
                }
                FilterNodeKind::Group(_) => {
                    let group_path = kind
                        .clone()
                        .case(FilterNodeKind::GROUP)
                        .resolve(form, cx)?
                        .expect("group payload must remain active while its case is selected");
                    let relation = group_path.clone().then(FilterGroupDraft::RELATION);
                    let negated = group_path.clone().then(FilterGroupDraft::NEGATED);
                    let group_children = group_path.then(FilterGroupDraft::CHILDREN);
                    if let Some(old_index) = old_index
                        && let FilterNode::Group(group) = &old_items[old_index]
                        && group.relation.try_get(form, cx).is_ok()
                    {
                        let items = Self::stage_group_items(
                            form,
                            group_children.try_items(form, cx)?,
                            &group.items,
                            options,
                            window,
                            cx,
                        )?;
                        entries.push(NodeReconcileCandidate::ReuseGroup {
                            old_index,
                            id: item_id,
                            item,
                            relation,
                            negated,
                            children: group_children,
                            items: Box::new(items),
                        });
                        continue;
                    }
                    entries.push(NodeReconcileCandidate::New(FilterNode::Group(Box::new(
                        Self::build_group(
                            form,
                            item_id,
                            item,
                            relation,
                            negated,
                            group_children,
                            options,
                            window,
                            cx,
                        )?,
                    ))));
                }
            }
        }
        Ok(GroupReconcileCandidate { entries })
    }

    fn stage_sorts<Owner: 'static>(
        form: &Entity<Form<QueryDraft>>,
        old_sorts: &[SortRow],
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Result<Vec<SortReconcileCandidate>, MutationError> {
        let sorts_path = QueryDraft::ROOT.then(QueryDraft::SORTS);
        let mut candidates = Vec::new();
        for item in sorts_path.items(form, cx) {
            let id = item.key();
            if let Some(index) = old_sorts.iter().position(|row| row.id == id) {
                candidates.push(SortReconcileCandidate::Reuse {
                    old_index: index,
                    item,
                });
            } else {
                candidates.push(SortReconcileCandidate::New(Box::new(Self::build_sort(
                    form, item, window, cx,
                )?)));
            }
        }
        Ok(candidates)
    }

    fn commit_group_items(
        old_items: Vec<FilterNode>,
        candidate: GroupReconcileCandidate,
    ) -> Vec<FilterNode> {
        let mut old_items = old_items.into_iter().map(Some).collect::<Vec<_>>();
        candidate
            .entries
            .into_iter()
            .map(|candidate| match candidate {
                NodeReconcileCandidate::ReuseCondition {
                    old_index,
                    item,
                    negated,
                    field,
                    editor,
                } => {
                    let Some(FilterNode::Condition(mut condition)) = old_items[old_index].take()
                    else {
                        unreachable!("staged condition row must keep its original variant")
                    };
                    condition.item = item;
                    condition.negated = negated;
                    condition.field = field;
                    if let Some(editor) = editor {
                        condition.editor = *editor;
                    }
                    FilterNode::Condition(condition)
                }
                NodeReconcileCandidate::ReuseGroup {
                    old_index,
                    id,
                    item,
                    relation,
                    negated,
                    children,
                    items,
                } => {
                    let Some(FilterNode::Group(mut group)) = old_items[old_index].take() else {
                        unreachable!("staged group row must keep its original variant")
                    };
                    let old_children = std::mem::take(&mut group.items);
                    group.id = id;
                    group.item = item;
                    group.relation = relation;
                    group.negated = negated;
                    group.children = children;
                    group.items = Self::commit_group_items(old_children, *items);
                    FilterNode::Group(group)
                }
                NodeReconcileCandidate::New(node) => node,
            })
            .collect()
    }

    fn commit_sorts(
        old_sorts: Vec<SortRow>,
        candidates: Vec<SortReconcileCandidate>,
    ) -> Vec<SortRow> {
        let mut old_sorts = old_sorts.into_iter().map(Some).collect::<Vec<_>>();
        candidates
            .into_iter()
            .map(|candidate| match candidate {
                SortReconcileCandidate::Reuse { old_index, item } => {
                    let mut row = old_sorts[old_index]
                        .take()
                        .expect("staged sort row must exist");
                    row.item = item;
                    row
                }
                SortReconcileCandidate::New(row) => *row,
            })
            .collect()
    }

    fn build_controls<Owner: 'static>(
        form: &Entity<Form<QueryDraft>>,
        options: &QueryOptions,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Result<(RootFilterGroup, Vec<SortRow>), MutationError> {
        let filters = QueryDraft::ROOT.then(QueryDraft::FILTERS);
        let root_id = filters.key(form.read(cx));
        let relation = filters.clone().then(FilterGroupDraft::RELATION);
        let negated = filters.clone().then(FilterGroupDraft::NEGATED);
        let children = filters.then(FilterGroupDraft::CHILDREN);
        let items = Self::build_group_items(form, children.items(form, cx), options, window, cx)?;
        let root = RootFilterGroup {
            id: root_id,
            relation,
            negated,
            children,
            items,
        };

        let sorts_path = QueryDraft::ROOT.then(QueryDraft::SORTS);
        let mut sorts = Vec::new();
        for item in sorts_path.items(form, cx) {
            sorts.push(Self::build_sort(form, item, window, cx)?);
        }
        Ok((root, sorts))
    }

    #[allow(clippy::too_many_arguments)]
    fn build_group<Owner: 'static>(
        form: &Entity<Form<QueryDraft>>,
        id: PathKey,
        item: ItemPath<QueryDraft, FilterNodeDraft>,
        relation: DynamicPath<QueryDraft, GroupRelation>,
        negated: DynamicPath<QueryDraft, bool>,
        children: DynamicItemsPath<QueryDraft, FilterNodeDraft>,
        options: &QueryOptions,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Result<DynamicFilterGroup, MutationError> {
        let items =
            Self::build_group_items(form, children.try_items(form, cx)?, options, window, cx)?;
        Ok(DynamicFilterGroup {
            id,
            item,
            relation,
            negated,
            children,
            items,
        })
    }

    fn build_group_items<Owner: 'static>(
        form: &Entity<Form<QueryDraft>>,
        current_items: Vec<ItemPath<QueryDraft, FilterNodeDraft>>,
        options: &QueryOptions,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Result<Vec<FilterNode>, MutationError> {
        let mut items = Vec::new();
        for item in current_items {
            let item_id = item.key();
            let kind = item.clone().then(FilterNodeDraft::KIND);
            let value = kind.try_get(form, cx)?;
            match value {
                FilterNodeKind::Condition(_) => {
                    let condition = kind
                        .clone()
                        .case(FilterNodeKind::CONDITION)
                        .resolve(form, cx)?
                        .expect("condition payload must remain active while its case is selected");
                    let negated = condition.clone().then(FilterConditionDraft::NEGATED);
                    let field = condition.then(FilterConditionDraft::FIELD);
                    items.push(FilterNode::Condition(Box::new(Self::build_condition(
                        form, item_id, item, negated, field, options, window, cx,
                    )?)));
                }
                FilterNodeKind::Group(_) => {
                    let group = kind
                        .clone()
                        .case(FilterNodeKind::GROUP)
                        .resolve(form, cx)?
                        .expect("group payload must remain active while its case is selected");
                    items.push(FilterNode::Group(Box::new(Self::build_group(
                        form,
                        item_id,
                        item,
                        group.clone().then(FilterGroupDraft::RELATION),
                        group.clone().then(FilterGroupDraft::NEGATED),
                        group.then(FilterGroupDraft::CHILDREN),
                        options,
                        window,
                        cx,
                    )?)));
                }
            }
        }
        Ok(items)
    }

    #[allow(clippy::too_many_arguments)]
    fn build_condition<Owner: 'static>(
        form: &Entity<Form<QueryDraft>>,
        id: PathKey,
        item: ItemPath<QueryDraft, FilterNodeDraft>,
        negated: DynamicPath<QueryDraft, bool>,
        field_path: DynamicPath<QueryDraft, ConditionField>,
        options: &QueryOptions,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Result<ConditionRow, MutationError> {
        let selected_field = field_path.try_get(form, cx)?.field();
        let selected_index = selected_field.and_then(|selected| field_items().position(&selected));
        let field_select = cx
            .new(|cx| SelectState::new(field_items(), selected_index, window, cx).searchable(true));
        let (field_binding, field_writer) = field_path.try_bind_control_in(
            form,
            &field_select,
            |state, projection, window, cx| match projection {
                ControlProjection::Value(value) => {
                    let selected = value
                        .field()
                        .and_then(|field| field_items().position(&field));
                    state.set_selected_index(selected, window, cx);
                }
                ControlProjection::Retired => {}
            },
            window,
            cx,
        )?;
        let field_subscription = cx.subscribe_in(
            &field_select,
            window,
            move |_, _, event: &SelectEvent<FieldSelectItems>, window, cx| {
                if let SelectEvent::Confirm(Some(field)) = event {
                    field_writer.defer_set(ConditionField::for_field(*field), window, cx);
                }
            },
        );
        let editor = Self::build_editor(form, field_path.clone(), options, window, cx)?;
        Ok(ConditionRow {
            id,
            item,
            negated,
            field: field_path,
            field_select,
            editor,
            _field_binding: field_binding,
            _subscriptions: vec![field_subscription],
        })
    }

    fn build_editor<Owner: 'static>(
        form: &Entity<Form<QueryDraft>>,
        field: DynamicPath<QueryDraft, ConditionField>,
        options: &QueryOptions,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Result<ConditionEditor, ResolveError> {
        match field.try_get(form, cx)? {
            ConditionField::Unselected => Ok(ConditionEditor::Unselected),
            ConditionField::Text(_) => {
                let path = field
                    .clone()
                    .case(ConditionField::TEXT)
                    .resolve(form, cx)?
                    .expect("text payload must remain active while its case is selected");
                let relation_path = path.clone().then(TextConditionDraft::RELATION);
                let value_path = path.then(TextConditionDraft::VALUE);
                Ok(ConditionEditor::Text(TextConditionControls {
                    relation_path: relation_path.clone(),
                    relation: FormSelect::try_new(
                        form,
                        relation_path,
                        |window, cx| SelectState::new(text_relation_items(), None, window, cx),
                        window,
                        cx,
                    )?,
                    value_path: value_path.clone(),
                    value: FormInput::try_new(
                        form,
                        value_path,
                        |window, cx| InputState::new(window, cx).placeholder("输入文本"),
                        window,
                        cx,
                    )?,
                }))
            }
            ConditionField::Number(_) => {
                let path = field
                    .clone()
                    .case(ConditionField::NUMBER)
                    .resolve(form, cx)?
                    .expect("number payload must remain active while its case is selected");
                let relation_path = path.clone().then(NumberConditionDraft::RELATION);
                let single_path = path.clone().then(NumberConditionDraft::SINGLE);
                let min_path = path.clone().then(NumberConditionDraft::MIN);
                let max_path = path.then(NumberConditionDraft::MAX);
                Ok(ConditionEditor::Number(NumberConditionControls {
                    relation_path: relation_path.clone(),
                    relation: FormSelect::try_new(
                        form,
                        relation_path,
                        |window, cx| SelectState::new(number_relation_items(), None, window, cx),
                        window,
                        cx,
                    )?,
                    single_path: single_path.clone(),
                    single: FormInput::try_new(
                        form,
                        single_path,
                        |window, cx| InputState::new(window, cx).placeholder("输入数字"),
                        window,
                        cx,
                    )?,
                    min_path: min_path.clone(),
                    min: FormInput::try_new(
                        form,
                        min_path,
                        |window, cx| InputState::new(window, cx).placeholder("最小值"),
                        window,
                        cx,
                    )?,
                    max_path: max_path.clone(),
                    max: FormInput::try_new(
                        form,
                        max_path,
                        |window, cx| InputState::new(window, cx).placeholder("最大值"),
                        window,
                        cx,
                    )?,
                }))
            }
            ConditionField::Bool(_) => {
                let path = field
                    .clone()
                    .case(ConditionField::BOOL)
                    .resolve(form, cx)?
                    .expect("bool payload must remain active while its case is selected");
                let relation_path = path.clone().then(BoolConditionDraft::RELATION);
                let value_path = path.then(BoolConditionDraft::VALUE);
                Ok(ConditionEditor::Bool(BoolConditionControls {
                    relation_path: relation_path.clone(),
                    relation: FormSelect::try_new(
                        form,
                        relation_path,
                        |window, cx| SelectState::new(bool_relation_items(), None, window, cx),
                        window,
                        cx,
                    )?,
                    value_path: value_path.clone(),
                    value: FormSelect::try_new(
                        form,
                        value_path,
                        |window, cx| SelectState::new(bool_value_items(), None, window, cx),
                        window,
                        cx,
                    )?,
                }))
            }
            ConditionField::Tags(_) => {
                let path = field
                    .clone()
                    .case(ConditionField::TAGS)
                    .resolve(form, cx)?
                    .expect("tags payload must remain active while its case is selected");
                let relation_path = path.clone().then(TagsConditionDraft::RELATION);
                let values_path = path.then(TagsConditionDraft::VALUES);
                let current_values = values_path.try_get(form, cx)?;
                let tag_options = options.tag_items_with_current(&current_values);
                Ok(ConditionEditor::Tags(TagsConditionControls {
                    relation_path: relation_path.clone(),
                    relation: FormSelect::try_new(
                        form,
                        relation_path,
                        |window, cx| SelectState::new(tags_relation_items(), None, window, cx),
                        window,
                        cx,
                    )?,
                    values_path: values_path.clone(),
                    values: FormCombobox::try_new(
                        form,
                        values_path,
                        move |window, cx| {
                            ComboboxState::new(
                                SearchableVec::new(tag_options),
                                Vec::new(),
                                window,
                                cx,
                            )
                            .multiple(true)
                            .searchable(true)
                        },
                        window,
                        cx,
                    )?,
                }))
            }
            ConditionField::Author(_) => {
                let path = field
                    .clone()
                    .case(ConditionField::AUTHOR)
                    .resolve(form, cx)?
                    .expect("author payload must remain active while its case is selected");
                let relation_path = path.clone().then(AuthorConditionDraft::RELATION);
                let text_path = path.clone().then(AuthorConditionDraft::TEXT);
                let single_path = path.clone().then(AuthorConditionDraft::SINGLE);
                let multiple_path = path.then(AuthorConditionDraft::MULTIPLE);
                let single_value = single_path.try_get(form, cx)?;
                let multiple_values = multiple_path.try_get(form, cx)?;
                let mut current_values = multiple_values.clone();
                if let Some(value) = &single_value
                    && !current_values.contains(value)
                {
                    current_values.push(value.clone());
                }
                let author_options = options.author_items_with_current(&current_values);
                let single_options = author_options.clone();
                let multiple_options = author_options;
                Ok(ConditionEditor::Author(AuthorConditionControls {
                    relation_path: relation_path.clone(),
                    relation: FormSelect::try_new(
                        form,
                        relation_path,
                        |window, cx| SelectState::new(author_relation_items(), None, window, cx),
                        window,
                        cx,
                    )?,
                    text_path: text_path.clone(),
                    text: FormInput::try_new(
                        form,
                        text_path,
                        |window, cx| InputState::new(window, cx).placeholder("输入文本"),
                        window,
                        cx,
                    )?,
                    single_path: single_path.clone(),
                    single: FormSelect::try_new(
                        form,
                        single_path,
                        move |window, cx| {
                            SelectState::new(SearchableVec::new(single_options), None, window, cx)
                                .searchable(true)
                        },
                        window,
                        cx,
                    )?,
                    multiple_path: multiple_path.clone(),
                    multiple: FormCombobox::try_new(
                        form,
                        multiple_path,
                        move |window, cx| {
                            ComboboxState::new(
                                SearchableVec::new(multiple_options),
                                Vec::new(),
                                window,
                                cx,
                            )
                            .multiple(true)
                            .searchable(true)
                        },
                        window,
                        cx,
                    )?,
                }))
            }
        }
    }

    fn build_sort<Owner: 'static>(
        form: &Entity<Form<QueryDraft>>,
        item: ItemPath<QueryDraft, SortDraft>,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Result<SortRow, ResolveError> {
        let id = item.key();
        let field_path = item.clone().then(SortDraft::FIELD);
        let direction_path = item.clone().then(SortDraft::DIRECTION);
        Ok(SortRow {
            id,
            item,
            field_path: field_path.clone(),
            direction_path: direction_path.clone(),
            field_select: FormSelect::try_new(
                form,
                field_path,
                |window, cx| SelectState::new(sort_field_items(), None, window, cx),
                window,
                cx,
            )?,
            direction_select: FormSelect::try_new(
                form,
                direction_path,
                |window, cx| SelectState::new(sort_direction_items(), None, window, cx),
                window,
                cx,
            )?,
        })
    }
}

fn needs_structural_reconcile(event: &gpui_form::FormEvent<QueryDraft>) -> bool {
    let gpui_form::FormEvent::ModelChanged(change) = event else {
        return false;
    };
    let filters = QueryDraft::ROOT
        .then(QueryDraft::FILTERS)
        .then(FilterGroupDraft::CHILDREN);
    let sorts = QueryDraft::ROOT.then(QueryDraft::SORTS);
    let filters = change.impact(&filters);
    let sorts = change.impact(&sorts);
    filters.structure_changed() || filters.retired() || sorts.structure_changed() || sorts.retired()
}

impl RootFilterGroup {
    fn condition_count(&self) -> usize {
        condition_count(&self.items)
    }

    fn find_dynamic_group(&self, id: &PathKey) -> Option<&DynamicFilterGroup> {
        find_dynamic_group(&self.items, id)
    }

    fn find_condition(&self, id: &PathKey) -> Option<&ConditionRow> {
        find_condition(&self.items, id)
    }

    fn remove_descendant(
        &self,
        id: &PathKey,
        form: &Entity<Form<QueryDraft>>,
        cx: &mut App,
    ) -> Result<Option<FilterNodeDraft>, MutationError> {
        if let Some(item) = direct_item(&self.items, id) {
            return self.children.remove(form, item, cx).map(Some);
        }
        for node in &self.items {
            if let FilterNode::Group(group) = node
                && let Some(value) = group.remove_descendant(id, form, cx)?
            {
                return Ok(Some(value));
            }
        }
        Ok(None)
    }

    fn update_options(
        &mut self,
        form: &Entity<Form<QueryDraft>>,
        options: &QueryOptions,
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) {
        update_group_options(&mut self.items, form, options, window, cx);
    }
}

impl DynamicFilterGroup {
    fn condition_count(&self) -> usize {
        condition_count(&self.items)
    }

    fn find_dynamic_group(&self, id: &PathKey) -> Option<&Self> {
        if &self.id == id {
            Some(self)
        } else {
            find_dynamic_group(&self.items, id)
        }
    }

    fn find_condition(&self, id: &PathKey) -> Option<&ConditionRow> {
        find_condition(&self.items, id)
    }

    fn remove_descendant(
        &self,
        id: &PathKey,
        form: &Entity<Form<QueryDraft>>,
        cx: &mut App,
    ) -> Result<Option<FilterNodeDraft>, MutationError> {
        if let Some(item) = direct_item(&self.items, id) {
            return self.children.remove(form, item, cx).map(Some);
        }
        for node in &self.items {
            if let FilterNode::Group(group) = node
                && let Some(value) = group.remove_descendant(id, form, cx)?
            {
                return Ok(Some(value));
            }
        }
        Ok(None)
    }

    fn update_options(
        &mut self,
        form: &Entity<Form<QueryDraft>>,
        options: &QueryOptions,
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) {
        update_group_options(&mut self.items, form, options, window, cx);
    }
}

fn condition_count(items: &[FilterNode]) -> usize {
    items
        .iter()
        .map(|node| match node {
            FilterNode::Condition(_) => 1,
            FilterNode::Group(group) => group.condition_count(),
        })
        .sum()
}

fn find_dynamic_group<'a>(items: &'a [FilterNode], id: &PathKey) -> Option<&'a DynamicFilterGroup> {
    items.iter().find_map(|node| match node {
        FilterNode::Condition(_) => None,
        FilterNode::Group(group) => group.find_dynamic_group(id),
    })
}

fn find_condition<'a>(items: &'a [FilterNode], id: &PathKey) -> Option<&'a ConditionRow> {
    items.iter().find_map(|node| match node {
        FilterNode::Condition(condition) if &condition.id == id => Some(condition.as_ref()),
        FilterNode::Condition(_) => None,
        FilterNode::Group(group) => group.find_condition(id),
    })
}

fn direct_item(
    items: &[FilterNode],
    id: &PathKey,
) -> Option<ItemPath<QueryDraft, FilterNodeDraft>> {
    items.iter().find_map(|node| match node {
        FilterNode::Condition(condition) if &condition.id == id => Some(condition.item.clone()),
        FilterNode::Group(group) if &group.id == id => Some(group.item.clone()),
        FilterNode::Condition(_) | FilterNode::Group(_) => None,
    })
}

fn update_group_options(
    items: &mut [FilterNode],
    form: &Entity<Form<QueryDraft>>,
    options: &QueryOptions,
    window: &mut Window,
    cx: &mut Context<QueryView>,
) {
    for node in items {
        match node {
            FilterNode::Condition(condition) => {
                condition.editor.update_options(form, options, window, cx)
            }
            FilterNode::Group(group) => group.update_options(form, options, window, cx),
        }
    }
}

impl ConditionEditor {
    fn update_options(
        &mut self,
        form: &Entity<Form<QueryDraft>>,
        options: &QueryOptions,
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) {
        match self {
            Self::Tags(controls) => {
                let values = match controls.values_path.try_get(form, cx) {
                    Ok(values) => values,
                    Err(error) => {
                        log_resolve_error("update query tag options", error);
                        return;
                    }
                };
                let items = options.tag_items_with_current(&values);
                controls.values.update(cx, |state, cx| {
                    state.set_items(SearchableVec::new(items), window, cx);
                    state.set_selected_values(&values, window, cx);
                });
            }
            Self::Author(controls) => {
                let single = match controls.single_path.try_get(form, cx) {
                    Ok(value) => value,
                    Err(error) => {
                        log_resolve_error("update query author options", error);
                        return;
                    }
                };
                let multiple = match controls.multiple_path.try_get(form, cx) {
                    Ok(values) => values,
                    Err(error) => {
                        log_resolve_error("update query author options", error);
                        return;
                    }
                };
                let mut current_values = multiple.clone();
                if let Some(value) = &single
                    && !current_values.contains(value)
                {
                    current_values.push(value.clone());
                }
                let items = options.author_items_with_current(&current_values);
                controls.single.update(cx, |state, cx| {
                    state.set_items(SearchableVec::new(items.clone()), window, cx);
                    match &single {
                        Some(value) => state.set_selected_value(value, window, cx),
                        None => state.set_selected_index(None, window, cx),
                    }
                });
                controls.multiple.update(cx, |state, cx| {
                    state.set_items(SearchableVec::new(items), window, cx);
                    state.set_selected_values(&multiple, window, cx);
                });
            }
            Self::Unselected | Self::Text(_) | Self::Number(_) | Self::Bool(_) => {}
        }
    }
}

fn log_mutation_error(action: &'static str, error: MutationError) {
    match &error {
        MutationError::Resolve(ResolveError::Retired { .. } | ResolveError::MissingItem { .. }) => {
            tracing::debug!(action, error = %error, "ignored stale query form callback");
        }
        _ => tracing::error!(action, error = %error, "query form mutation failed"),
    }
}

fn log_resolve_error(action: &'static str, error: ResolveError) {
    match &error {
        ResolveError::Retired { .. } | ResolveError::MissingItem { .. } => {
            tracing::debug!(action, error = %error, "ignored stale query form projection");
        }
        _ => tracing::error!(action, error = %error, "query form projection failed"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{IntoElement, Render, TestAppContext, VisualTestContext, WindowHandle, div};

    use super::super::options::FieldKind;

    struct ControllerHarness {
        advanced: AdvancedQueryController,
    }

    impl ControllerHarness {
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let options = QueryOptions::default();
            let form =
                cx.new(|_| Form::new(QueryDraft::default()).with_validator(QueryDraftValidator));
            let (root, sorts) =
                AdvancedQueryController::build_controls(&form, &options, window, cx)
                    .expect("default query controls must bind");
            let subscription = cx.subscribe_in(
                &form,
                window,
                |harness, _, event: &gpui_form::FormEvent<QueryDraft>, window, cx| {
                    if needs_structural_reconcile(event) {
                        harness.advanced.reconcile_controls(window, cx);
                    }
                    cx.notify();
                },
            );
            Self {
                advanced: AdvancedQueryController {
                    form,
                    root,
                    sorts,
                    options,
                    _subscriptions: vec![subscription],
                },
            }
        }
    }

    impl Render for ControllerHarness {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    fn open_harness(cx: &mut TestAppContext) -> WindowHandle<ControllerHarness> {
        cx.update(|cx| {
            cx.open_window(Default::default(), |window, cx| {
                cx.new(|cx| ControllerHarness::new(window, cx))
            })
            .expect("open query controller test window")
        })
    }

    fn only_condition(controller: &AdvancedQueryController) -> &ConditionRow {
        let [FilterNode::Condition(condition)] = controller.root.items.as_slice() else {
            panic!("expected exactly one root condition")
        };
        condition
    }

    #[gpui::test]
    fn unrelated_leaf_and_validation_changes_keep_condition_row(cx: &mut TestAppContext) {
        let window = open_harness(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let root = window.root(&mut cx).expect("query controller root");
        let (form, negated, selector) = cx.update(|_, cx| {
            root.read_with(cx, |harness, _| {
                let condition = only_condition(&harness.advanced);
                (
                    harness.advanced.form.clone(),
                    condition.negated.clone(),
                    condition.field_select.clone(),
                )
            })
        });

        cx.update(|_, cx| {
            negated
                .try_set(&form, true, cx)
                .expect("set unrelated condition leaf");
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            root.read_with(cx, |harness, _| {
                assert_eq!(
                    only_condition(&harness.advanced).field_select.entity_id(),
                    selector.entity_id()
                );
            });
            assert!(form.update(cx, |form, cx| form.prepare(cx)).is_err());
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            root.read_with(cx, |harness, _| {
                assert_eq!(
                    only_condition(&harness.advanced).field_select.entity_id(),
                    selector.entity_id(),
                    "validation-only changes must not rebuild the row"
                );
            });
        });
    }

    #[gpui::test]
    fn sort_reorder_preserves_row_identity_by_path_key(cx: &mut TestAppContext) {
        let window = open_harness(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let root = window.root(&mut cx).expect("query controller root");
        let form =
            cx.update(|_, cx| root.read_with(cx, |harness, _| harness.advanced.form.clone()));
        let (first, second) = cx.update(|_, cx| {
            let sorts = QueryDraft::ROOT.then(QueryDraft::SORTS);
            let first = sorts
                .append(
                    &form,
                    SortDraft {
                        field: Some(SortField::Title),
                        direction: Some(SortDirection::Asc),
                    },
                    cx,
                )
                .expect("append first sort");
            let second = sorts
                .append(
                    &form,
                    SortDraft {
                        field: Some(SortField::AuthorName),
                        direction: Some(SortDirection::Desc),
                    },
                    cx,
                )
                .expect("append second sort");
            (first, second)
        });
        cx.run_until_parked();

        let first_id = first.key();
        let second_id = second.key();
        let (first_control, second_control) = cx.update(|_, cx| {
            root.read_with(cx, |harness, _| {
                let first = harness
                    .advanced
                    .sorts
                    .iter()
                    .find(|row| row.id == first_id)
                    .expect("first sort row");
                let second = harness
                    .advanced
                    .sorts
                    .iter()
                    .find(|row| row.id == second_id)
                    .expect("second sort row");
                (
                    std::ops::Deref::deref(&first.field_select).clone(),
                    std::ops::Deref::deref(&second.field_select).clone(),
                )
            })
        });
        cx.update(|_, cx| {
            QueryDraft::ROOT
                .then(QueryDraft::SORTS)
                .move_before(&form, &second, &first, cx)
                .expect("reorder sorts");
        });
        cx.run_until_parked();

        cx.update(|_, cx| {
            root.read_with(cx, |harness, _| {
                assert_eq!(harness.advanced.sorts[0].id, second_id);
                assert_eq!(harness.advanced.sorts[1].id, first_id);
                let first = harness
                    .advanced
                    .sorts
                    .iter()
                    .find(|row| row.id == first_id)
                    .expect("reordered first row");
                let second = harness
                    .advanced
                    .sorts
                    .iter()
                    .find(|row| row.id == second_id)
                    .expect("reordered second row");
                assert_eq!(
                    std::ops::Deref::deref(&first.field_select).entity_id(),
                    first_control.entity_id()
                );
                assert_eq!(
                    std::ops::Deref::deref(&second.field_select).entity_id(),
                    second_control.entity_id()
                );
            });
        });
    }

    #[gpui::test]
    fn removed_condition_writer_cannot_mutate_reinserted_row(cx: &mut TestAppContext) {
        let window = open_harness(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let root = window.root(&mut cx).expect("query controller root");
        let (form, old_item, old_id, old_selector) = cx.update(|_, cx| {
            root.read_with(cx, |harness, _| {
                let condition = only_condition(&harness.advanced);
                (
                    harness.advanced.form.clone(),
                    condition.item.clone(),
                    condition.id.clone(),
                    condition.field_select.clone(),
                )
            })
        });

        let replacement_id = cx.update(|_, cx| {
            old_selector.update(cx, |_, cx| {
                cx.emit(SelectEvent::Confirm(Some(FieldKind::Title)));
            });
            let children = QueryDraft::ROOT
                .then(QueryDraft::FILTERS)
                .then(FilterGroupDraft::CHILDREN);
            children
                .remove(&form, old_item, cx)
                .expect("remove old condition");
            children
                .append(&form, FilterNodeDraft::condition(), cx)
                .expect("reinsert condition")
                .key()
        });
        cx.run_until_parked();

        cx.update(|_, cx| {
            root.read_with(cx, |harness, cx| {
                let condition = only_condition(&harness.advanced);
                assert_ne!(condition.id, old_id);
                assert_eq!(condition.id, replacement_id);
                assert_eq!(
                    condition
                        .field
                        .try_get(&harness.advanced.form, cx)
                        .expect("replacement field remains active"),
                    ConditionField::Unselected,
                    "the queued writer belongs to the retired occurrence"
                );
            });
        });
    }

    #[gpui::test]
    fn external_values_project_to_condition_and_sort_selectors(cx: &mut TestAppContext) {
        let window = open_harness(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let root = window.root(&mut cx).expect("query controller root");
        let (form, condition_id, field, field_selector) = cx.update(|_, cx| {
            root.read_with(cx, |harness, _| {
                let condition = only_condition(&harness.advanced);
                (
                    harness.advanced.form.clone(),
                    condition.id.clone(),
                    condition.field.clone(),
                    condition.field_select.clone(),
                )
            })
        });

        cx.update(|_, cx| {
            field
                .try_set(&form, ConditionField::for_field(FieldKind::Title), cx)
                .expect("set condition field externally");
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            root.read_with(cx, |harness, cx| {
                let condition = only_condition(&harness.advanced);
                assert_eq!(condition.id, condition_id);
                assert_eq!(
                    condition.field_select.entity_id(),
                    field_selector.entity_id()
                );
                assert_eq!(
                    field_selector.read(cx).selected_value().copied(),
                    Some(FieldKind::Title)
                );
                assert!(matches!(condition.editor, ConditionEditor::Text(_)));
            });
        });

        let sort_item = cx.update(|_, cx| {
            QueryDraft::ROOT
                .then(QueryDraft::SORTS)
                .append(
                    &form,
                    SortDraft {
                        field: Some(SortField::Title),
                        direction: Some(SortDirection::Asc),
                    },
                    cx,
                )
                .expect("append sort")
        });
        cx.run_until_parked();
        let (direction, direction_selector) = cx.update(|_, cx| {
            root.read_with(cx, |harness, _| {
                let row = harness
                    .advanced
                    .sorts
                    .iter()
                    .find(|row| row.id == sort_item.key())
                    .expect("sort row");
                (
                    row.direction_path.clone(),
                    std::ops::Deref::deref(&row.direction_select).clone(),
                )
            })
        });
        cx.update(|_, cx| {
            direction
                .try_set(&form, Some(SortDirection::Desc), cx)
                .expect("set sort direction externally");
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            assert_eq!(
                direction_selector.read(cx).selected_value().copied(),
                Some(SortDirection::Desc)
            );
        });
    }
}

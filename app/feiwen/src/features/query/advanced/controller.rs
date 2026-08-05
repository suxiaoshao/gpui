use gpui::{App, AppContext as _, Context, Entity, Subscription, Window};
use gpui_component::{
    IndexPath,
    combobox::ComboboxState,
    input::InputState,
    searchable_list::SearchableListDelegate,
    select::{SearchableVec, SelectEvent, SelectItem, SelectState},
};
use gpui_form::{
    DynamicItemsPath, DynamicPath, Form, FormSchema, ItemPath, MutationError, PathKey,
    PrepareError, Prepared, ResolveError, TotalItemsPath, TotalPath, ValidationIssue,
};
use gpui_form_gpui_component::{FormCombobox, FormInput, FormSelect};

use super::options::{
    AuthorOption, AuthorRelation, BoolRelation, FieldKind, FieldSelectItems, GroupRelation,
    NumberRelation, QueryOptions, SelectChoice, SortDirectionChoice, SortField, TagsRelation,
    TextRelation, author_relation_items, bool_relation_items, bool_value_items, field_items,
    number_relation_items, sort_direction_items, sort_field_items, tags_relation_items,
    text_relation_items,
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
type SortDirectionSelectState = SelectState<Vec<SelectChoice<SortDirectionChoice>>>;

#[derive(Clone)]
pub(super) enum QueryPath<T: 'static> {
    Total(TotalPath<QueryDraft, T>),
    Dynamic(DynamicPath<QueryDraft, T>),
}

impl<T: Clone + PartialEq + 'static> QueryPath<T> {
    pub(super) fn value(
        &self,
        form: &Entity<Form<QueryDraft>>,
        cx: &App,
    ) -> Result<T, ResolveError> {
        match self {
            Self::Total(path) => Ok(path.value(form, cx)),
            Self::Dynamic(path) => path.try_value(form, cx),
        }
    }

    fn set(
        &self,
        form: &Entity<Form<QueryDraft>>,
        value: T,
        cx: &mut App,
    ) -> Result<(), MutationError> {
        match self {
            Self::Total(path) => {
                path.set(form, value, cx);
                Ok(())
            }
            Self::Dynamic(path) => path.try_set(form, value, cx),
        }
    }

    pub(super) fn errors(
        &self,
        form: &Entity<Form<QueryDraft>>,
        cx: &App,
    ) -> Result<Vec<ValidationIssue>, ResolveError> {
        match self {
            Self::Total(path) => Ok(path.errors(form, cx)),
            Self::Dynamic(path) => path.try_errors(form, cx),
        }
    }
}

#[derive(Clone)]
enum QueryItemsPath<Item: FormSchema> {
    Total(TotalItemsPath<QueryDraft, Item>),
    Dynamic(DynamicItemsPath<QueryDraft, Item>),
}

impl<Item: FormSchema> QueryItemsPath<Item> {
    fn items(
        &self,
        form: &Entity<Form<QueryDraft>>,
        cx: &mut App,
    ) -> Result<Vec<ItemPath<QueryDraft, Item>>, MutationError> {
        match self {
            Self::Total(path) => path.items(form, cx),
            Self::Dynamic(path) => path.try_items(form, cx),
        }
    }

    fn append(
        &self,
        form: &Entity<Form<QueryDraft>>,
        value: Item,
        cx: &mut App,
    ) -> Result<ItemPath<QueryDraft, Item>, MutationError> {
        match self {
            Self::Total(path) => path.append(form, value, cx),
            Self::Dynamic(path) => path.append(form, value, cx),
        }
    }

    fn remove(
        &self,
        form: &Entity<Form<QueryDraft>>,
        item: ItemPath<QueryDraft, Item>,
        cx: &mut App,
    ) -> Result<Item, MutationError> {
        match self {
            Self::Total(path) => path.remove(form, item, cx),
            Self::Dynamic(path) => path.remove(form, item, cx),
        }
    }

    fn move_before(
        &self,
        form: &Entity<Form<QueryDraft>>,
        item: &ItemPath<QueryDraft, Item>,
        anchor: &ItemPath<QueryDraft, Item>,
        cx: &mut App,
    ) -> Result<(), MutationError> {
        match self {
            Self::Total(path) => path.move_before(form, item, anchor, cx),
            Self::Dynamic(path) => path.move_before(form, item, anchor, cx),
        }
    }
}

pub(crate) struct AdvancedQueryController {
    pub(crate) form: Entity<Form<QueryDraft>>,
    pub(super) root: FilterGroup,
    pub(super) sorts: Vec<SortRow>,
    pub(super) options: QueryOptions,
    _subscriptions: Vec<Subscription>,
}

pub(super) struct FilterGroup {
    pub(super) id: PathKey,
    pub(super) relation: QueryPath<GroupRelation>,
    pub(super) negated: QueryPath<bool>,
    children: QueryItemsPath<FilterNodeDraft>,
    source: Option<(
        QueryItemsPath<FilterNodeDraft>,
        ItemPath<QueryDraft, FilterNodeDraft>,
    )>,
    pub(super) items: Vec<FilterNode>,
}

pub(super) enum FilterNode {
    Condition(Box<ConditionRow>),
    Group(Box<FilterGroup>),
}

pub(super) struct ConditionRow {
    pub(super) id: PathKey,
    source: (
        QueryItemsPath<FilterNodeDraft>,
        ItemPath<QueryDraft, FilterNodeDraft>,
    ),
    pub(super) negated: QueryPath<bool>,
    pub(super) field: QueryPath<ConditionField>,
    pub(super) field_select: Entity<FieldSelectState>,
    pub(super) editor: ConditionEditor,
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
    pub(super) relation_path: QueryPath<Option<TextRelation>>,
    pub(super) relation: FormSelect<Vec<SelectChoice<TextRelation>>>,
    pub(super) value_path: QueryPath<String>,
    pub(super) value: FormInput,
}

pub(super) struct NumberConditionControls {
    pub(super) relation_path: QueryPath<Option<NumberRelation>>,
    pub(super) relation: FormSelect<Vec<SelectChoice<NumberRelation>>>,
    pub(super) single_path: QueryPath<String>,
    pub(super) single: FormInput,
    pub(super) min_path: QueryPath<String>,
    pub(super) min: FormInput,
    pub(super) max_path: QueryPath<String>,
    pub(super) max: FormInput,
}

pub(super) struct BoolConditionControls {
    pub(super) relation_path: QueryPath<Option<BoolRelation>>,
    pub(super) relation: FormSelect<Vec<SelectChoice<BoolRelation>>>,
    pub(super) value_path: QueryPath<Option<bool>>,
    pub(super) value: FormSelect<Vec<SelectChoice<bool>>>,
}

pub(super) struct TagsConditionControls {
    pub(super) relation_path: QueryPath<Option<TagsRelation>>,
    pub(super) relation: FormSelect<Vec<SelectChoice<TagsRelation>>>,
    pub(super) values_path: QueryPath<Vec<String>>,
    pub(super) values: FormCombobox<SearchableVec<super::options::TagOption>>,
}

pub(super) struct AuthorConditionControls {
    pub(super) relation_path: QueryPath<Option<AuthorRelation>>,
    pub(super) relation: FormSelect<Vec<SelectChoice<AuthorRelation>>>,
    pub(super) text_path: QueryPath<String>,
    pub(super) text: FormInput,
    pub(super) single_path: QueryPath<Option<AuthorRef>>,
    pub(super) single: FormSelect<SearchableVec<AuthorOption>>,
    pub(super) multiple_path: QueryPath<Vec<AuthorRef>>,
    pub(super) multiple: FormCombobox<SearchableVec<AuthorOption>>,
}

pub(super) struct SortRow {
    pub(super) id: PathKey,
    pub(super) item: ItemPath<QueryDraft, SortDraft>,
    pub(super) field_path: QueryPath<Option<SortField>>,
    pub(super) direction_path: QueryPath<SortDirection>,
    pub(super) field_select: FormSelect<Vec<SelectChoice<SortField>>>,
    pub(super) direction_select: Entity<SortDirectionSelectState>,
    _subscriptions: Vec<Subscription>,
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
        let Ok(field) = self.field.value(form, cx) else {
            return false;
        };
        match (&self.editor, field) {
            (ConditionEditor::Unselected, ConditionField::Unselected) => true,
            (ConditionEditor::Text(controls), ConditionField::Text(_)) => {
                controls.relation_path.value(form, cx).is_ok()
            }
            (ConditionEditor::Number(controls), ConditionField::Number(_)) => {
                controls.relation_path.value(form, cx).is_ok()
            }
            (ConditionEditor::Bool(controls), ConditionField::Bool(_)) => {
                controls.relation_path.value(form, cx).is_ok()
            }
            (ConditionEditor::Tags(controls), ConditionField::Tags(_)) => {
                controls.relation_path.value(form, cx).is_ok()
            }
            (ConditionEditor::Author(controls), ConditionField::Author(_)) => {
                controls.relation_path.value(form, cx).is_ok()
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
        let form = cx.new(|_| {
            Form::try_new_with_validator(QueryDraft::default(), QueryDraftValidator)
                .expect("default query form schema must be valid")
        });
        let (root, sorts) = Self::build_controls(&form, &options, window, cx)
            .expect("default query form paths must bind");
        let _subscriptions = vec![cx.subscribe(&form, |_, _, _: &gpui_form::FormEvent, cx| {
            cx.notify();
        })];
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
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) {
        self.form.update(cx, |form, cx| form.replace(draft, cx));
        self.replace_controls(window, cx);
    }

    pub(crate) fn reset(&mut self, window: &mut Window, cx: &mut Context<QueryView>) {
        self.form.update(cx, |form, cx| form.reset(cx));
        self.replace_controls(window, cx);
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
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) {
        let Some(children) = self
            .root
            .find_group(&group_id)
            .map(|group| group.children.clone())
        else {
            return;
        };
        match children.append(&self.form, FilterNodeDraft::condition(), cx) {
            Ok(_) => self.reconcile_controls(window, cx),
            Err(error) => log_mutation_error("add query condition", error),
        }
    }

    pub(crate) fn add_group(
        &mut self,
        group_id: PathKey,
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) {
        let Some(children) = self
            .root
            .find_group(&group_id)
            .map(|group| group.children.clone())
        else {
            return;
        };
        let value = FilterNodeDraft {
            kind: FilterNodeKind::Group(FilterGroupDraft {
                relation: GroupRelation::All,
                negated: false,
                children: Vec::new(),
            }),
        };
        match children.append(&self.form, value, cx) {
            Ok(_) => self.reconcile_controls(window, cx),
            Err(error) => log_mutation_error("add query group", error),
        }
    }

    pub(crate) fn remove_node(
        &mut self,
        node_id: PathKey,
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) {
        let Some((parent, item)) = self.root.find_source(&node_id) else {
            return;
        };
        match parent.remove(&self.form, item, cx) {
            Ok(_) => self.reconcile_controls(window, cx),
            Err(error) => log_mutation_error("remove query node", error),
        }
    }

    pub(super) fn set_group_relation(
        &mut self,
        group_id: PathKey,
        relation: GroupRelation,
        cx: &mut Context<QueryView>,
    ) {
        let Some(path) = self
            .root
            .find_group(&group_id)
            .map(|group| group.relation.clone())
        else {
            return;
        };
        if let Err(error) = path.set(&self.form, relation, cx) {
            log_mutation_error("set query group relation", error);
        }
    }

    pub(crate) fn set_group_negated(
        &mut self,
        group_id: PathKey,
        negated: bool,
        cx: &mut Context<QueryView>,
    ) {
        let Some(path) = self
            .root
            .find_group(&group_id)
            .map(|group| group.negated.clone())
        else {
            return;
        };
        if let Err(error) = path.set(&self.form, negated, cx) {
            log_mutation_error("set query group negation", error);
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
        if let Err(error) = path.set(&self.form, negated, cx) {
            log_mutation_error("set query condition negation", error);
        }
    }

    pub(crate) fn add_sort(&mut self, window: &mut Window, cx: &mut Context<QueryView>) {
        let sorts = QueryItemsPath::Total(QueryDraft::ROOT.then(QueryDraft::SORTS));
        match sorts.append(
            &self.form,
            SortDraft {
                field: None,
                direction: SortDirection::Asc,
            },
            cx,
        ) {
            Ok(_) => self.reconcile_controls(window, cx),
            Err(error) => log_mutation_error("add query sort", error),
        }
    }

    pub(crate) fn remove_sort(
        &mut self,
        sort_id: PathKey,
        window: &mut Window,
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
        let sorts = QueryItemsPath::Total(QueryDraft::ROOT.then(QueryDraft::SORTS));
        match sorts.remove(&self.form, item, cx) {
            Ok(_) => self.reconcile_controls(window, cx),
            Err(error) => log_mutation_error("remove query sort", error),
        }
    }

    pub(crate) fn move_sort_before(
        &mut self,
        source_id: PathKey,
        target_id: PathKey,
        window: &mut Window,
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
        let sorts = QueryItemsPath::Total(QueryDraft::ROOT.then(QueryDraft::SORTS));
        match sorts.move_before(&self.form, &source, &target, cx) {
            Ok(()) => self.reconcile_controls(window, cx),
            Err(error) => log_mutation_error("move query sort", error),
        }
    }

    fn set_condition_field(
        &mut self,
        condition_id: PathKey,
        field: FieldKind,
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) {
        let Some(path) = self
            .root
            .find_condition(&condition_id)
            .map(|condition| condition.field.clone())
        else {
            return;
        };
        if path
            .value(&self.form, cx)
            .is_ok_and(|current| current.field() == Some(field))
        {
            return;
        }
        match path.set(&self.form, ConditionField::for_field(field), cx) {
            Ok(()) => self.reconcile_controls(window, cx),
            Err(error) => log_mutation_error("change query condition field", error),
        }
    }

    fn set_sort_direction(
        &mut self,
        sort_id: PathKey,
        direction: SortDirection,
        cx: &mut Context<QueryView>,
    ) {
        let Some(path) = self
            .sorts
            .iter()
            .find(|sort| sort.id == sort_id)
            .map(|sort| sort.direction_path.clone())
        else {
            return;
        };
        if let Err(error) = path.set(&self.form, direction, cx) {
            log_mutation_error("set query sort direction", error);
        }
    }

    fn replace_controls(&mut self, window: &mut Window, cx: &mut Context<QueryView>) {
        match Self::build_controls(&self.form, &self.options, window, cx) {
            Ok((root, sorts)) => {
                self.root = root;
                self.sorts = sorts;
            }
            Err(error) => {
                tracing::error!(error = %error, "failed to rebuild typed query controls");
            }
        }
        cx.notify();
    }

    fn reconcile_controls(&mut self, window: &mut Window, cx: &mut Context<QueryView>) {
        let children = self.root.children.clone();
        let old_root_items = std::mem::take(&mut self.root.items);
        let old_sorts = std::mem::take(&mut self.sorts);
        let reconciled = Self::reconcile_group_items(
            &self.form,
            children,
            old_root_items,
            &self.options,
            window,
            cx,
        )
        .and_then(|root_items| {
            Self::reconcile_sorts(&self.form, old_sorts, window, cx)
                .map(|sorts| (root_items, sorts))
        });

        match reconciled {
            Ok((root_items, sorts)) => {
                self.root.items = root_items;
                self.sorts = sorts;
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

    fn reconcile_group_items(
        form: &Entity<Form<QueryDraft>>,
        children: QueryItemsPath<FilterNodeDraft>,
        mut old_items: Vec<FilterNode>,
        options: &QueryOptions,
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) -> Result<Vec<FilterNode>, MutationError> {
        let mut next_items = Vec::new();
        for item in children.items(form, cx)? {
            let item_id = item.key();
            let old = old_items
                .iter()
                .position(|node| node.id() == &item_id)
                .map(|index| old_items.remove(index));
            let kind = item.clone().then(FilterNodeDraft::KIND);
            match kind.try_value(form, cx)? {
                FilterNodeKind::Condition(_) => {
                    if let Some(FilterNode::Condition(mut condition)) = old
                        && condition.is_current(form, cx)
                    {
                        condition.source = (children.clone(), item);
                        next_items.push(FilterNode::Condition(condition));
                        continue;
                    }
                    let condition = kind.try_case(form.read(cx), FilterNodeKind::CONDITION)?;
                    next_items.push(FilterNode::Condition(Box::new(Self::build_condition(
                        form,
                        item_id,
                        children.clone(),
                        item,
                        condition,
                        options,
                        window,
                        cx,
                    )?)));
                }
                FilterNodeKind::Group(_) => {
                    let group_path = kind.try_case(form.read(cx), FilterNodeKind::GROUP)?;
                    let relation =
                        QueryPath::Dynamic(group_path.clone().then(FilterGroupDraft::RELATION));
                    let negated =
                        QueryPath::Dynamic(group_path.clone().then(FilterGroupDraft::NEGATED));
                    let group_children =
                        QueryItemsPath::Dynamic(group_path.then(FilterGroupDraft::CHILDREN));
                    if let Some(FilterNode::Group(mut group)) = old
                        && group.relation.value(form, cx).is_ok()
                    {
                        let previous = std::mem::take(&mut group.items);
                        group.id = item_id;
                        group.relation = relation;
                        group.negated = negated;
                        group.children = group_children.clone();
                        group.source = Some((children.clone(), item));
                        group.items = Self::reconcile_group_items(
                            form,
                            group_children,
                            previous,
                            options,
                            window,
                            cx,
                        )?;
                        next_items.push(FilterNode::Group(group));
                        continue;
                    }
                    next_items.push(FilterNode::Group(Box::new(Self::build_group(
                        form,
                        item_id,
                        relation,
                        negated,
                        group_children,
                        Some((children.clone(), item)),
                        options,
                        window,
                        cx,
                    )?)));
                }
            }
        }
        Ok(next_items)
    }

    fn reconcile_sorts(
        form: &Entity<Form<QueryDraft>>,
        mut old_sorts: Vec<SortRow>,
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) -> Result<Vec<SortRow>, MutationError> {
        let sorts_path = QueryDraft::ROOT.then(QueryDraft::SORTS);
        let mut next_sorts = Vec::new();
        for item in sorts_path.items(form, cx)? {
            let id = item.key();
            if let Some(index) = old_sorts.iter().position(|row| row.id == id) {
                let mut row = old_sorts.remove(index);
                row.item = item;
                next_sorts.push(row);
            } else {
                next_sorts.push(Self::build_sort(form, item, window, cx)?);
            }
        }
        Ok(next_sorts)
    }

    fn build_controls(
        form: &Entity<Form<QueryDraft>>,
        options: &QueryOptions,
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) -> Result<(FilterGroup, Vec<SortRow>), MutationError> {
        let filters = QueryDraft::ROOT.then(QueryDraft::FILTERS);
        let root_id = filters.key(form.read(cx));
        let root = Self::build_group(
            form,
            root_id,
            QueryPath::Total(filters.clone().then(FilterGroupDraft::RELATION)),
            QueryPath::Total(filters.clone().then(FilterGroupDraft::NEGATED)),
            QueryItemsPath::Total(filters.then(FilterGroupDraft::CHILDREN)),
            None,
            options,
            window,
            cx,
        )?;

        let sorts_path = QueryDraft::ROOT.then(QueryDraft::SORTS);
        let mut sorts = Vec::new();
        for item in sorts_path.items(form, cx)? {
            sorts.push(Self::build_sort(form, item, window, cx)?);
        }
        Ok((root, sorts))
    }

    #[allow(clippy::too_many_arguments)]
    fn build_group(
        form: &Entity<Form<QueryDraft>>,
        id: PathKey,
        relation: QueryPath<GroupRelation>,
        negated: QueryPath<bool>,
        children: QueryItemsPath<FilterNodeDraft>,
        source: Option<(
            QueryItemsPath<FilterNodeDraft>,
            ItemPath<QueryDraft, FilterNodeDraft>,
        )>,
        options: &QueryOptions,
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) -> Result<FilterGroup, MutationError> {
        let mut items = Vec::new();
        for item in children.items(form, cx)? {
            let item_id = item.key();
            let kind = item.clone().then(FilterNodeDraft::KIND);
            let value = kind.try_value(form, cx)?;
            match value {
                FilterNodeKind::Condition(_) => {
                    let condition = kind.try_case(form.read(cx), FilterNodeKind::CONDITION)?;
                    items.push(FilterNode::Condition(Box::new(Self::build_condition(
                        form,
                        item_id,
                        children.clone(),
                        item,
                        condition,
                        options,
                        window,
                        cx,
                    )?)));
                }
                FilterNodeKind::Group(_) => {
                    let group = kind.try_case(form.read(cx), FilterNodeKind::GROUP)?;
                    items.push(FilterNode::Group(Box::new(Self::build_group(
                        form,
                        item_id,
                        QueryPath::Dynamic(group.clone().then(FilterGroupDraft::RELATION)),
                        QueryPath::Dynamic(group.clone().then(FilterGroupDraft::NEGATED)),
                        QueryItemsPath::Dynamic(group.then(FilterGroupDraft::CHILDREN)),
                        Some((children.clone(), item)),
                        options,
                        window,
                        cx,
                    )?)));
                }
            }
        }
        Ok(FilterGroup {
            id,
            relation,
            negated,
            children,
            source,
            items,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn build_condition(
        form: &Entity<Form<QueryDraft>>,
        id: PathKey,
        parent: QueryItemsPath<FilterNodeDraft>,
        item: ItemPath<QueryDraft, FilterNodeDraft>,
        condition: DynamicPath<QueryDraft, FilterConditionDraft>,
        options: &QueryOptions,
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) -> Result<ConditionRow, MutationError> {
        let negated = QueryPath::Dynamic(condition.clone().then(FilterConditionDraft::NEGATED));
        let field_path = condition.then(FilterConditionDraft::FIELD);
        let selected_field = field_path.try_value(form, cx)?.field();
        let selected_index = selected_field.and_then(|selected| field_items().position(&selected));
        let field_select = cx
            .new(|cx| SelectState::new(field_items(), selected_index, window, cx).searchable(true));
        let callback_id = id.clone();
        let field_subscription = cx.subscribe_in(
            &field_select,
            window,
            move |this, _, event: &SelectEvent<FieldSelectItems>, window, cx| {
                if let SelectEvent::Confirm(Some(field)) = event {
                    this.advanced
                        .set_condition_field(callback_id.clone(), *field, window, cx);
                }
            },
        );
        let editor = Self::build_editor(form, field_path.clone(), options, window, cx)?;
        Ok(ConditionRow {
            id,
            source: (parent, item),
            negated,
            field: QueryPath::Dynamic(field_path),
            field_select,
            editor,
            _subscriptions: vec![field_subscription],
        })
    }

    fn build_editor(
        form: &Entity<Form<QueryDraft>>,
        field: DynamicPath<QueryDraft, ConditionField>,
        options: &QueryOptions,
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) -> Result<ConditionEditor, ResolveError> {
        match field.try_value(form, cx)? {
            ConditionField::Unselected => Ok(ConditionEditor::Unselected),
            ConditionField::Text(_) => {
                let path = field.try_case(form.read(cx), ConditionField::TEXT)?;
                let relation_path = path.clone().then(TextConditionDraft::RELATION);
                let value_path = path.then(TextConditionDraft::VALUE);
                Ok(ConditionEditor::Text(TextConditionControls {
                    relation_path: QueryPath::Dynamic(relation_path.clone()),
                    relation: FormSelect::try_new(
                        form,
                        relation_path,
                        |window, cx| SelectState::new(text_relation_items(), None, window, cx),
                        window,
                        cx,
                    )?,
                    value_path: QueryPath::Dynamic(value_path.clone()),
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
                let path = field.try_case(form.read(cx), ConditionField::NUMBER)?;
                let relation_path = path.clone().then(NumberConditionDraft::RELATION);
                let single_path = path.clone().then(NumberConditionDraft::SINGLE);
                let min_path = path.clone().then(NumberConditionDraft::MIN);
                let max_path = path.then(NumberConditionDraft::MAX);
                Ok(ConditionEditor::Number(NumberConditionControls {
                    relation_path: QueryPath::Dynamic(relation_path.clone()),
                    relation: FormSelect::try_new(
                        form,
                        relation_path,
                        |window, cx| SelectState::new(number_relation_items(), None, window, cx),
                        window,
                        cx,
                    )?,
                    single_path: QueryPath::Dynamic(single_path.clone()),
                    single: FormInput::try_new(
                        form,
                        single_path,
                        |window, cx| InputState::new(window, cx).placeholder("输入数字"),
                        window,
                        cx,
                    )?,
                    min_path: QueryPath::Dynamic(min_path.clone()),
                    min: FormInput::try_new(
                        form,
                        min_path,
                        |window, cx| InputState::new(window, cx).placeholder("最小值"),
                        window,
                        cx,
                    )?,
                    max_path: QueryPath::Dynamic(max_path.clone()),
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
                let path = field.try_case(form.read(cx), ConditionField::BOOL)?;
                let relation_path = path.clone().then(BoolConditionDraft::RELATION);
                let value_path = path.then(BoolConditionDraft::VALUE);
                Ok(ConditionEditor::Bool(BoolConditionControls {
                    relation_path: QueryPath::Dynamic(relation_path.clone()),
                    relation: FormSelect::try_new(
                        form,
                        relation_path,
                        |window, cx| SelectState::new(bool_relation_items(), None, window, cx),
                        window,
                        cx,
                    )?,
                    value_path: QueryPath::Dynamic(value_path.clone()),
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
                let path = field.try_case(form.read(cx), ConditionField::TAGS)?;
                let relation_path = path.clone().then(TagsConditionDraft::RELATION);
                let values_path = path.then(TagsConditionDraft::VALUES);
                let current_values = values_path.try_value(form, cx)?;
                let tag_options = options.tag_items_with_current(&current_values);
                Ok(ConditionEditor::Tags(TagsConditionControls {
                    relation_path: QueryPath::Dynamic(relation_path.clone()),
                    relation: FormSelect::try_new(
                        form,
                        relation_path,
                        |window, cx| SelectState::new(tags_relation_items(), None, window, cx),
                        window,
                        cx,
                    )?,
                    values_path: QueryPath::Dynamic(values_path.clone()),
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
                let path = field.try_case(form.read(cx), ConditionField::AUTHOR)?;
                let relation_path = path.clone().then(AuthorConditionDraft::RELATION);
                let text_path = path.clone().then(AuthorConditionDraft::TEXT);
                let single_path = path.clone().then(AuthorConditionDraft::SINGLE);
                let multiple_path = path.then(AuthorConditionDraft::MULTIPLE);
                let single_value = single_path.try_value(form, cx)?;
                let multiple_values = multiple_path.try_value(form, cx)?;
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
                    relation_path: QueryPath::Dynamic(relation_path.clone()),
                    relation: FormSelect::try_new(
                        form,
                        relation_path,
                        |window, cx| SelectState::new(author_relation_items(), None, window, cx),
                        window,
                        cx,
                    )?,
                    text_path: QueryPath::Dynamic(text_path.clone()),
                    text: FormInput::try_new(
                        form,
                        text_path,
                        |window, cx| InputState::new(window, cx).placeholder("输入文本"),
                        window,
                        cx,
                    )?,
                    single_path: QueryPath::Dynamic(single_path.clone()),
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
                    multiple_path: QueryPath::Dynamic(multiple_path.clone()),
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

    fn build_sort(
        form: &Entity<Form<QueryDraft>>,
        item: ItemPath<QueryDraft, SortDraft>,
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) -> Result<SortRow, ResolveError> {
        let id = item.key();
        let field_path = item.clone().then(SortDraft::FIELD);
        let direction_path = item.clone().then(SortDraft::DIRECTION);
        let direction = direction_path.try_value(form, cx)?;
        let direction_choice = match direction {
            SortDirection::Asc => SortDirectionChoice::Asc,
            SortDirection::Desc => SortDirectionChoice::Desc,
        };
        let direction_index = sort_direction_items()
            .iter()
            .position(|item| item.value() == &direction_choice)
            .map(|row| IndexPath::default().row(row));
        let direction_select =
            cx.new(|cx| SelectState::new(sort_direction_items(), direction_index, window, cx));
        let callback_id = id.clone();
        let direction_subscription = cx.subscribe_in(
            &direction_select,
            window,
            move |this, _, event: &SelectEvent<Vec<SelectChoice<SortDirectionChoice>>>, _, cx| {
                if let SelectEvent::Confirm(Some(direction)) = event {
                    let direction = match direction {
                        SortDirectionChoice::Asc => SortDirection::Asc,
                        SortDirectionChoice::Desc => SortDirection::Desc,
                    };
                    this.advanced
                        .set_sort_direction(callback_id.clone(), direction, cx);
                }
            },
        );
        Ok(SortRow {
            id,
            item,
            field_path: QueryPath::Dynamic(field_path.clone()),
            direction_path: QueryPath::Dynamic(direction_path),
            field_select: FormSelect::try_new(
                form,
                field_path,
                |window, cx| SelectState::new(sort_field_items(), None, window, cx),
                window,
                cx,
            )?,
            direction_select,
            _subscriptions: vec![direction_subscription],
        })
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

    fn find_group(&self, id: &PathKey) -> Option<&Self> {
        if &self.id == id {
            return Some(self);
        }
        self.items.iter().find_map(|node| match node {
            FilterNode::Condition(_) => None,
            FilterNode::Group(group) => group.find_group(id),
        })
    }

    fn find_condition(&self, id: &PathKey) -> Option<&ConditionRow> {
        self.items.iter().find_map(|node| match node {
            FilterNode::Condition(condition) if &condition.id == id => Some(condition.as_ref()),
            FilterNode::Condition(_) => None,
            FilterNode::Group(group) => group.find_condition(id),
        })
    }

    fn find_source(
        &self,
        id: &PathKey,
    ) -> Option<(
        QueryItemsPath<FilterNodeDraft>,
        ItemPath<QueryDraft, FilterNodeDraft>,
    )> {
        if &self.id == id {
            return self.source.clone();
        }
        self.items.iter().find_map(|node| match node {
            FilterNode::Condition(condition) if &condition.id == id => {
                Some(condition.source.clone())
            }
            FilterNode::Condition(_) => None,
            FilterNode::Group(group) => group.find_source(id),
        })
    }

    fn update_options(
        &mut self,
        form: &Entity<Form<QueryDraft>>,
        options: &QueryOptions,
        window: &mut Window,
        cx: &mut Context<QueryView>,
    ) {
        for node in &mut self.items {
            match node {
                FilterNode::Condition(condition) => {
                    condition.editor.update_options(form, options, window, cx)
                }
                FilterNode::Group(group) => group.update_options(form, options, window, cx),
            }
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
                let values = match controls.values_path.value(form, cx) {
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
                let single = match controls.single_path.value(form, cx) {
                    Ok(value) => value,
                    Err(error) => {
                        log_resolve_error("update query author options", error);
                        return;
                    }
                };
                let multiple = match controls.multiple_path.value(form, cx) {
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

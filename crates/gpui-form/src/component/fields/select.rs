use std::marker::PhantomData;

use gpui::{App, AppContext as _, Entity, Focusable, Window};
use gpui_component::{
    searchable_list::{SearchableListDelegate, SearchableListItem},
    select::SelectState,
};

use crate::{
    AnyFormField, ComponentStateOptions, FieldChangeCause, FieldCore, FieldError, FieldMeta,
    FieldValidationReport, FormComponentBinding, FormField, FormMeta, ValidationTrigger,
};

pub trait SelectFieldValue: Clone + PartialEq + 'static {
    type Selected: Clone + PartialEq + 'static;

    fn to_selected_value(&self) -> Option<Self::Selected>;
    fn from_selected_value(selected: Option<Self::Selected>, previous: &Self) -> Self;
}

impl<T> SelectFieldValue for Option<T>
where
    T: Clone + PartialEq + 'static,
{
    type Selected = T;

    fn to_selected_value(&self) -> Option<Self::Selected> {
        self.clone()
    }

    fn from_selected_value(selected: Option<Self::Selected>, _previous: &Self) -> Self {
        selected
    }
}

pub struct SelectBinding<T, D>(PhantomData<fn() -> (T, D)>);

impl<T, D> SelectBinding<T, D>
where
    T: SelectFieldValue,
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem<Value = T::Selected>,
{
    pub fn new_state_with_delegate(
        initial: &T,
        delegate: D,
        searchable: bool,
        _options: ComponentStateOptions,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<SelectState<D>> {
        let selected_index = initial
            .to_selected_value()
            .and_then(|value| delegate.position(&value));
        cx.new(|cx| SelectState::new(delegate, selected_index, window, cx).searchable(searchable))
    }

    pub fn read_value_with_previous(state: &Entity<SelectState<D>>, previous: &T, cx: &App) -> T {
        T::from_selected_value(state.read(cx).selected_value().cloned(), previous)
    }

    pub fn write_value(
        state: &Entity<SelectState<D>>,
        value: &T,
        _cause: FieldChangeCause,
        window: &mut Window,
        cx: &mut App,
    ) {
        let selected = value.to_selected_value();
        state.update(cx, |select, cx| {
            if let Some(value) = selected.as_ref() {
                select.set_selected_value(value, window, cx);
            } else {
                select.set_selected_index(None, window, cx);
            }
        });
    }

    pub fn set_items(
        state: &Entity<SelectState<D>>,
        delegate: D,
        window: &mut Window,
        cx: &mut App,
    ) {
        state.update(cx, |select, cx| {
            select.set_items(delegate, window, cx);
        });
    }

    pub fn focus(state: &Entity<SelectState<D>>, window: &mut Window, cx: &mut App) -> bool {
        let focus_handle = state.read(cx).focus_handle(cx);
        focus_handle.focus(window, cx);
        true
    }
}

impl<T, D> FormComponentBinding<T> for SelectBinding<T, D>
where
    T: SelectFieldValue + Default,
    D: SearchableListDelegate + Default + 'static,
    D::Item: SearchableListItem<Value = T::Selected>,
{
    type State = SelectState<D>;
    type Event = gpui_component::select::SelectEvent<D>;

    fn new_state(
        initial: &T,
        options: ComponentStateOptions,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self::State> {
        Self::new_state_with_delegate(initial, D::default(), false, options, window, cx)
    }

    fn read_value(state: &Entity<Self::State>, cx: &App) -> T {
        Self::read_value_with_previous(state, &T::default(), cx)
    }

    fn write_value(
        state: &Entity<Self::State>,
        value: &T,
        cause: FieldChangeCause,
        window: &mut Window,
        cx: &mut App,
    ) {
        Self::write_value(state, value, cause, window, cx);
    }

    fn focus(state: &Entity<Self::State>, window: &mut Window, cx: &mut App) -> bool {
        Self::focus(state, window, cx)
    }
}

pub struct SelectFieldStore<T, D>
where
    T: SelectFieldValue,
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem<Value = T::Selected>,
{
    core: FieldCore<T>,
    select_state: Entity<SelectState<D>>,
}

impl<T, D> SelectFieldStore<T, D>
where
    T: SelectFieldValue,
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem<Value = T::Selected>,
{
    pub fn new(value: T, select_state: Entity<SelectState<D>>) -> Self {
        Self {
            core: FieldCore::new(value),
            select_state,
        }
    }

    pub fn select_state(&self) -> Entity<SelectState<D>> {
        self.select_state.clone()
    }

    pub fn core(&self) -> &FieldCore<T> {
        &self.core
    }

    pub fn core_mut(&mut self) -> &mut FieldCore<T> {
        &mut self.core
    }

    pub fn read_selected_value(&self, cx: &App) -> Option<T::Selected> {
        self.select_state.read(cx).selected_value().cloned()
    }

    pub fn read_component_value(&self, cx: &App) -> T {
        SelectBinding::<T, D>::read_value_with_previous(&self.select_state, self.core.value(), cx)
    }

    pub fn set_items(&self, delegate: D, window: &mut Window, cx: &mut App) {
        SelectBinding::<T, D>::set_items(&self.select_state, delegate, window, cx);
    }

    pub fn write_component_value(
        &self,
        value: &T,
        cause: FieldChangeCause,
        window: &mut Window,
        cx: &mut App,
    ) {
        SelectBinding::<T, D>::write_value(&self.select_state, value, cause, window, cx);
    }
}

impl<T, D> FormField for SelectFieldStore<T, D>
where
    T: SelectFieldValue,
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem<Value = T::Selected>,
{
    type Value = T;
    type ComponentState = SelectState<D>;

    fn value(&self) -> &Self::Value {
        self.core.value()
    }

    fn set_value(&mut self, value: Self::Value, cause: FieldChangeCause) {
        self.core.set_value(value, cause);
    }

    fn reset(&mut self, window: &mut Window, cx: &mut App) {
        self.core.reset();
        self.write_component_value(self.core.value(), FieldChangeCause::Reset, window, cx);
    }

    fn component_state(&self) -> Option<Entity<Self::ComponentState>> {
        Some(self.select_state.clone())
    }

    fn meta(&self) -> &FieldMeta {
        self.core.meta()
    }

    fn is_required(&self) -> bool {
        self.core.is_required()
    }

    fn errors(&self) -> &[FieldError] {
        self.core.errors()
    }

    fn visible_errors(&self, form_meta: &FormMeta) -> Vec<&FieldError> {
        self.core.visible_errors(form_meta)
    }

    fn set_errors(&mut self, errors: Vec<FieldError>) {
        self.core.set_errors(errors);
    }

    fn clear_errors(&mut self) {
        self.core.clear_errors();
    }

    fn mark_touched(&mut self) {
        self.core.meta_mut().mark_touched();
    }

    fn mark_blurred(&mut self) {
        self.core.meta_mut().mark_blurred();
    }

    fn validate(&mut self, _trigger: ValidationTrigger) -> FieldValidationReport {
        FieldValidationReport::new(self.core.errors().to_vec())
    }

    fn focus(&mut self, window: &mut Window, cx: &mut App) -> bool {
        SelectBinding::<T, D>::focus(&self.select_state, window, cx)
    }
}

impl<T, D> AnyFormField for SelectFieldStore<T, D>
where
    T: SelectFieldValue,
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem<Value = T::Selected>,
{
    fn meta(&self) -> &FieldMeta {
        self.core.meta()
    }

    fn is_required(&self) -> bool {
        self.core.is_required()
    }

    fn errors(&self) -> &[FieldError] {
        self.core.errors()
    }

    fn visible_errors(&self, form_meta: &FormMeta) -> Vec<&FieldError> {
        self.core.visible_errors(form_meta)
    }

    fn set_errors(&mut self, errors: Vec<FieldError>) {
        self.core.set_errors(errors);
    }

    fn clear_errors(&mut self) {
        self.core.clear_errors();
    }

    fn focus_any(&mut self, window: &mut Window, cx: &mut App) -> bool {
        self.focus(window, cx)
    }
}

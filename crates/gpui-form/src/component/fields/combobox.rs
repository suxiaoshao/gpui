use std::marker::PhantomData;

use gpui::{App, AppContext as _, Entity, Focusable, Window};
use gpui_component::{
    IndexPath,
    combobox::ComboboxState,
    searchable_list::{SearchableListDelegate, SearchableListItem},
};

use crate::{
    AnyFormField, ComponentStateOptions, FieldChangeCause, FieldCore, FieldError, FieldMeta,
    FieldValidationReport, FormComponentBinding, FormField, FormMeta, ValidationTrigger,
};

pub trait ComboboxFieldValue: Clone + PartialEq + 'static {
    type Selected: Clone + PartialEq + 'static;

    fn to_selected_values(&self) -> Vec<Self::Selected>;
    fn from_selected_values(selected: Vec<Self::Selected>, previous: &Self) -> Self;
}

impl<T> ComboboxFieldValue for Vec<T>
where
    T: Clone + PartialEq + 'static,
{
    type Selected = T;

    fn to_selected_values(&self) -> Vec<Self::Selected> {
        self.clone()
    }

    fn from_selected_values(selected: Vec<Self::Selected>, _previous: &Self) -> Self {
        selected
    }
}

impl<T> ComboboxFieldValue for Option<T>
where
    T: Clone + PartialEq + 'static,
{
    type Selected = T;

    fn to_selected_values(&self) -> Vec<Self::Selected> {
        self.clone().into_iter().collect()
    }

    fn from_selected_values(mut selected: Vec<Self::Selected>, _previous: &Self) -> Self {
        selected.drain(..).next()
    }
}

pub struct ComboboxBinding<T, D>(PhantomData<fn() -> (T, D)>);

impl<T, D> ComboboxBinding<T, D>
where
    T: ComboboxFieldValue,
    D: SearchableListDelegate + Clone + 'static,
    D::Item: SearchableListItem<Value = T::Selected>,
{
    pub fn new_state_with_delegate(
        initial: &T,
        delegate: D,
        multiple: bool,
        searchable: bool,
        _options: ComponentStateOptions,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<ComboboxState<D>> {
        let selected_indices = Self::selected_indices_for(&delegate, initial);
        cx.new(|cx| {
            ComboboxState::new(delegate, selected_indices, window, cx)
                .multiple(multiple)
                .searchable(searchable)
        })
    }

    pub fn read_value_with_previous(state: &Entity<ComboboxState<D>>, previous: &T, cx: &App) -> T {
        T::from_selected_values(state.read(cx).selected_values(), previous)
    }

    pub fn write_value_with_delegate(
        state: &Entity<ComboboxState<D>>,
        delegate: &D,
        value: &T,
        _cause: FieldChangeCause,
        window: &mut Window,
        cx: &mut App,
    ) {
        let selected_indices = Self::selected_indices_for(delegate, value);
        state.update(cx, |combobox, cx| {
            combobox.set_selected_indices(selected_indices, window, cx);
        });
    }

    pub fn set_items(
        state: &Entity<ComboboxState<D>>,
        delegate: D,
        window: &mut Window,
        cx: &mut App,
    ) {
        state.update(cx, |combobox, cx| {
            combobox.set_items(delegate, window, cx);
        });
    }

    pub fn focus(state: &Entity<ComboboxState<D>>, window: &mut Window, cx: &mut App) -> bool {
        let focus_handle = state.read(cx).focus_handle(cx);
        focus_handle.focus(window, cx);
        true
    }

    fn selected_indices_for(delegate: &D, value: &T) -> Vec<IndexPath> {
        value
            .to_selected_values()
            .iter()
            .filter_map(|selected| delegate.position(selected))
            .collect()
    }
}

impl<T, D> FormComponentBinding<T> for ComboboxBinding<T, D>
where
    T: ComboboxFieldValue + Default,
    D: SearchableListDelegate + Clone + Default + 'static,
    D::Item: SearchableListItem<Value = T::Selected>,
{
    type State = ComboboxState<D>;
    type Event = gpui_component::combobox::ComboboxEvent<D>;

    fn new_state(
        initial: &T,
        options: ComponentStateOptions,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self::State> {
        Self::new_state_with_delegate(initial, D::default(), false, false, options, window, cx)
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
        Self::write_value_with_delegate(state, &D::default(), value, cause, window, cx);
    }

    fn focus(state: &Entity<Self::State>, window: &mut Window, cx: &mut App) -> bool {
        Self::focus(state, window, cx)
    }
}

pub struct ComboboxFieldStore<T, D>
where
    T: ComboboxFieldValue,
    D: SearchableListDelegate + Clone + 'static,
    D::Item: SearchableListItem<Value = T::Selected>,
{
    core: FieldCore<T>,
    combobox_state: Entity<ComboboxState<D>>,
    delegate: D,
}

impl<T, D> ComboboxFieldStore<T, D>
where
    T: ComboboxFieldValue,
    D: SearchableListDelegate + Clone + 'static,
    D::Item: SearchableListItem<Value = T::Selected>,
{
    pub fn new(value: T, combobox_state: Entity<ComboboxState<D>>, delegate: D) -> Self {
        Self {
            core: FieldCore::new(value),
            combobox_state,
            delegate,
        }
    }

    pub fn combobox_state(&self) -> Entity<ComboboxState<D>> {
        self.combobox_state.clone()
    }

    pub fn core(&self) -> &FieldCore<T> {
        &self.core
    }

    pub fn core_mut(&mut self) -> &mut FieldCore<T> {
        &mut self.core
    }

    pub fn delegate(&self) -> &D {
        &self.delegate
    }

    pub fn set_items(&mut self, delegate: D, window: &mut Window, cx: &mut App) {
        self.delegate = delegate.clone();
        ComboboxBinding::<T, D>::set_items(&self.combobox_state, delegate, window, cx);
    }

    pub fn read_selected_values(&self, cx: &App) -> Vec<T::Selected> {
        self.combobox_state.read(cx).selected_values()
    }

    pub fn read_component_value(&self, cx: &App) -> T {
        ComboboxBinding::<T, D>::read_value_with_previous(
            &self.combobox_state,
            self.core.value(),
            cx,
        )
    }

    pub fn write_component_value(
        &self,
        value: &T,
        cause: FieldChangeCause,
        window: &mut Window,
        cx: &mut App,
    ) {
        ComboboxBinding::<T, D>::write_value_with_delegate(
            &self.combobox_state,
            &self.delegate,
            value,
            cause,
            window,
            cx,
        );
    }
}

impl<T, D> FormField for ComboboxFieldStore<T, D>
where
    T: ComboboxFieldValue,
    D: SearchableListDelegate + Clone + 'static,
    D::Item: SearchableListItem<Value = T::Selected>,
{
    type Value = T;
    type ComponentState = ComboboxState<D>;

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
        Some(self.combobox_state.clone())
    }

    fn meta(&self) -> &FieldMeta {
        self.core.meta()
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
        ComboboxBinding::<T, D>::focus(&self.combobox_state, window, cx)
    }
}

impl<T, D> AnyFormField for ComboboxFieldStore<T, D>
where
    T: ComboboxFieldValue,
    D: SearchableListDelegate + Clone + 'static,
    D::Item: SearchableListItem<Value = T::Selected>,
{
    fn meta(&self) -> &FieldMeta {
        self.core.meta()
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

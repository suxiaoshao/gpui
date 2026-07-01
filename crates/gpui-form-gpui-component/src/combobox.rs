use std::marker::PhantomData;

use gpui::{App, AppContext as _, Entity, Focusable, Window};
use gpui_component::{
    IndexPath,
    combobox::{ComboboxEvent, ComboboxState},
    searchable_list::{SearchableListDelegate, SearchableListItem},
};
use gpui_form::{
    ComponentStateOptions, FieldChangeCause, FieldError, FieldPath, FormComponentBinding,
    FormComponentEvent, ValidationTrigger,
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
    type Event = ComboboxEvent<D>;
    type Draft = T;

    fn new_state(
        initial: &T,
        options: ComponentStateOptions,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self::State> {
        Self::new_state_with_delegate(initial, D::default(), false, false, options, window, cx)
    }

    fn draft_from_value(value: &T) -> Self::Draft {
        value.clone()
    }

    fn read_draft(state: &Entity<Self::State>, cx: &App) -> Self::Draft {
        T::from_selected_values(state.read(cx).selected_values(), &T::default())
    }

    fn parse_draft(
        draft: &Self::Draft,
        _path: FieldPath,
        _trigger: ValidationTrigger,
        _cx: &App,
    ) -> Result<T, Box<FieldError>> {
        Ok(draft.clone())
    }

    fn write_value(
        state: &Entity<Self::State>,
        value: &T,
        _cause: FieldChangeCause,
        window: &mut Window,
        cx: &mut App,
    ) {
        let delegate = D::default();
        let selected_indices = Self::selected_indices_for(&delegate, value);
        state.update(cx, |combobox, cx| {
            combobox.set_selected_indices(selected_indices, window, cx);
        });
    }

    fn event_kind(event: &Self::Event) -> Option<FormComponentEvent> {
        match event {
            ComboboxEvent::Change(_) => {
                Some(FormComponentEvent::Change(FieldChangeCause::UserInput))
            }
            ComboboxEvent::Confirm(_) => Some(FormComponentEvent::Blur),
        }
    }

    fn focus(state: &Entity<Self::State>, window: &mut Window, cx: &mut App) -> bool {
        Self::focus(state, window, cx)
    }
}

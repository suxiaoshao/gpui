use gpui_kit::component::{
    combobox::{Combobox, ComboboxEvent, ComboboxState},
    searchable_list::{SearchableGroup, SearchableListItem, SearchableVec},
    select::SelectItem,
};
use gpui_kit::*;
use std::{ops::Deref, rc::Rc};

#[derive(Clone, Debug)]
pub(crate) struct PickerSection<T> {
    pub(crate) title: Option<SharedString>,
    pub(crate) items: Vec<Rc<T>>,
}

impl<T> PickerSection<T> {
    pub(crate) fn untitled(items: impl IntoIterator<Item = T>) -> Self {
        Self {
            title: None,
            items: items.into_iter().map(Rc::new).collect(),
        }
    }

    pub(crate) fn section(
        title: impl Into<SharedString>,
        items: impl IntoIterator<Item = T>,
    ) -> Self {
        Self {
            title: Some(title.into()),
            items: items.into_iter().map(Rc::new).collect(),
        }
    }
}

/// Domain options carry availability; the component owns filtering and selection.
#[derive(Clone)]
pub(crate) struct PickerItem<T: SelectItem + Clone> {
    option: Rc<T>,
    selectable: bool,
}

impl<T: SelectItem + Clone> SearchableListItem for PickerItem<T> {
    type Value = T::Value;

    fn title(&self) -> SharedString {
        self.option.title()
    }
    fn display_title(&self) -> Option<AnyElement> {
        self.option.display_title()
    }
    fn value(&self) -> &Self::Value {
        self.option.value()
    }
    fn matches(&self, query: &str) -> bool {
        self.option.matches(query)
    }
    fn disabled(&self) -> bool {
        !self.selectable || self.option.disabled()
    }
    fn render(&self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.option.render(window, cx)
    }
}

type PickerItems<T> = SearchableVec<SearchableGroup<PickerItem<T>>>;

fn picker_items<T: SelectItem + Clone>(
    sections: Vec<PickerSection<T>>,
    selectable: bool,
) -> PickerItems<T> {
    SearchableVec::new(
        sections
            .into_iter()
            .map(|section| {
                SearchableGroup::new(section.title.unwrap_or_default()).items(
                    section
                        .items
                        .into_iter()
                        .map(|option| PickerItem { option, selectable }),
                )
            })
            .collect::<Vec<_>>(),
    )
}

/// Connects component value changes to the domain callback and retains its subscription.
#[derive(Clone)]
pub(crate) struct PickerControl<T: SelectItem + Clone + 'static> {
    state: Entity<ComboboxState<PickerItems<T>>>,
    _subscription: Rc<Subscription>,
}

impl<T: SelectItem + Clone + 'static> PickerControl<T> {
    pub(crate) fn new<Owner: 'static>(
        sections: Vec<PickerSection<T>>,
        selected: Option<T::Value>,
        selectable: bool,
        searchable: bool,
        on_change: impl Fn(T, &mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Self {
        let state = cx.new(|cx| {
            let mut state =
                ComboboxState::new(picker_items(sections, selectable), vec![], window, cx)
                    .searchable(searchable);
            state.set_selected_values(&selected.into_iter().collect::<Vec<_>>(), window, cx);
            state
        });
        let on_change = Rc::new(on_change);
        let subscription = cx.subscribe_in(&state, window, move |_, state, event, window, cx| {
            // Confirm is also emitted when cancelling/closing without changing a value.
            let ComboboxEvent::Change(values) = event else {
                return;
            };
            let item = state
                .read(cx)
                .selection()
                .iter()
                .find(|(_, item)| values.first() == Some(item.value()))
                .map(|(_, item)| item.clone());
            if let Some(item) = item.filter(|item| !item.disabled()) {
                let on_change = on_change.clone();
                window.defer(cx, move |window, cx| {
                    on_change((*item.option).clone(), window, cx)
                });
            }
        });
        Self {
            state,
            _subscription: Rc::new(subscription),
        }
    }

    pub(crate) fn replace_projection(
        &self,
        sections: Vec<PickerSection<T>>,
        selected: Option<T::Value>,
        selectable: bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.state.update(cx, |state, cx| {
            let query = state.query(cx);
            state.set_items(picker_items(sections, selectable), window, cx);
            // Resolve the domain value in the full catalog, then restore the search view.
            state.set_selected_values(&selected.into_iter().collect::<Vec<_>>(), window, cx);
            state.set_query(query, window, cx);
            cx.notify();
        });
    }

    pub(crate) fn selected_item(&self, cx: &App) -> Option<Rc<T>> {
        self.state
            .read(cx)
            .selection()
            .first()
            .map(|(_, item)| item.option.clone())
    }

    pub(crate) fn element(&self) -> Combobox<PickerItems<T>> {
        Combobox::new(&self.state)
    }
}

impl<T: SelectItem + Clone + 'static> Deref for PickerControl<T> {
    type Target = Entity<ComboboxState<PickerItems<T>>>;
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

#[cfg(test)]
mod tests {
    use super::{PickerControl, PickerSection};
    use gpui_kit::component::{combobox::ComboboxEvent, searchable_list::SearchableListItem};
    use gpui_kit::{
        AppContext as _, Context, IntoElement, Render, TestAppContext, VisualTestContext, Window,
        div,
    };
    use std::{cell::Cell, rc::Rc};

    struct Root {
        picker: PickerControl<String>,
    }
    impl Render for Root {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    #[gpui_kit::test]
    fn projection_preserves_value_and_query_without_writing_and_readonly_cannot_write(
        cx: &mut TestAppContext,
    ) {
        cx.update(gpui_kit::init);
        let writes = Rc::new(Cell::new(0));
        let counter = writes.clone();
        let window = cx
            .update(|cx| {
                cx.open_window(Default::default(), |window, cx| {
                    cx.new(|cx| Root {
                        picker: PickerControl::new(
                            vec![PickerSection::untitled([
                                "alpha".to_string(),
                                "beta".to_string(),
                            ])],
                            Some("beta".into()),
                            true,
                            true,
                            move |_, _, _| counter.set(counter.get() + 1),
                            window,
                            cx,
                        ),
                    })
                })
            })
            .unwrap();
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let root = window.root(&mut cx).unwrap();
        let picker = root.read_with(&cx, |root, _| root.picker.clone());
        let identity = picker.entity_id();
        cx.update(|window, cx| {
            picker.update(cx, |state, cx| state.set_query("alpha", window, cx));
            picker.replace_projection(
                vec![PickerSection::untitled(["beta".into(), "alpha".into()])],
                Some("beta".into()),
                false,
                window,
                cx,
            );
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            assert_eq!(picker.entity_id(), identity);
            assert_eq!(picker.read(cx).query(cx).as_ref(), "alpha");
            assert_eq!(
                picker.selected_item(cx).as_deref().map(String::as_str),
                Some("beta")
            );
            assert!(picker.read(cx).selection()[0].1.disabled());
            picker.update(cx, |_, cx| {
                cx.emit(ComboboxEvent::Confirm(vec!["beta".into()]));
                cx.emit(ComboboxEvent::Change(vec!["beta".into()]));
            });
        });
        cx.run_until_parked();
        assert_eq!(writes.get(), 0);
        cx.update(|window, cx| {
            picker.replace_projection(
                vec![PickerSection::untitled(["alpha".into()])],
                Some("alpha".into()),
                true,
                window,
                cx,
            );
            picker.update(cx, |_, cx| {
                cx.emit(ComboboxEvent::Change(vec!["alpha".into()]))
            });
        });
        cx.run_until_parked();
        assert_eq!(writes.get(), 1);
    }
}

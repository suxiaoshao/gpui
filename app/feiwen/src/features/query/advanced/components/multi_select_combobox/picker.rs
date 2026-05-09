use gpui::{
    App, Bounds, Context, Edges, InteractiveElement, IntoElement, Length, MouseDownEvent,
    ParentElement, Pixels, RenderOnce, SharedString, Styled, Task, Window, anchored, deferred, div,
    point, prelude::FluentBuilder as _, px, rems,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, IndexPath, Selectable, StyledExt, h_flex,
    label::Label,
    list::{List, ListDelegate, ListState},
    select::SelectItem,
    v_flex,
};
use std::rc::Rc;

type OnCancel = Rc<dyn Fn(&mut Window, &mut App) + 'static>;
type OnConfirm<T> = Rc<dyn Fn(T, &mut Window, &mut App) + 'static>;

#[derive(Clone, Debug)]
pub(crate) struct PickerSection<T> {
    title: Option<SharedString>,
    items: Vec<Rc<T>>,
}

impl<T> PickerSection<T> {
    pub(crate) fn flat(items: impl IntoIterator<Item = T>) -> Vec<Self> {
        vec![Self {
            title: None,
            items: items.into_iter().map(Rc::new).collect(),
        }]
    }
}

#[derive(IntoElement, Clone)]
pub(crate) struct PickerListItem<T>
where
    T: SelectItem + Clone + 'static,
    T::Value: Clone + PartialEq + 'static,
{
    id: SharedString,
    item: Rc<T>,
    is_preselected: bool,
    is_checked: bool,
}

impl<T> PickerListItem<T>
where
    T: SelectItem + Clone + 'static,
    T::Value: Clone + PartialEq + 'static,
{
    fn new(id: SharedString, item: Rc<T>) -> Self {
        Self {
            id,
            item,
            is_preselected: false,
            is_checked: false,
        }
    }

    fn checked(mut self, checked: bool) -> Self {
        self.is_checked = checked;
        self
    }
}

impl<T> Selectable for PickerListItem<T>
where
    T: SelectItem + Clone + 'static,
    T::Value: Clone + PartialEq + 'static,
{
    fn selected(mut self, selected: bool) -> Self {
        self.is_preselected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.is_preselected
    }
}

impl<T> RenderOnce for PickerListItem<T>
where
    T: SelectItem + Clone + 'static,
    T::Value: Clone + PartialEq + 'static,
{
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        h_flex()
            .id(self.id)
            .w_full()
            .gap_2()
            .min_h(px(28.))
            .px_3()
            .py_1()
            .rounded(cx.theme().radius)
            .items_center()
            .when(self.is_preselected, |this| {
                this.bg(cx.theme().secondary_active)
            })
            .when(!self.is_preselected, |this| {
                this.hover(|this| this.bg(cx.theme().secondary_hover))
            })
            .child(Icon::new(IconName::Check).when(!self.is_checked, |this| this.invisible()))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(Label::new(self.item.title()).text_sm()),
            )
    }
}

pub(crate) struct PickerListDelegate<T>
where
    T: SelectItem + Clone + 'static,
    T::Value: Clone + PartialEq + 'static,
{
    ix: Option<IndexPath>,
    all_sections: Vec<PickerSection<T>>,
    sections: Vec<PickerSection<T>>,
    selected_values: Vec<T::Value>,
    last_query: String,
    loading: bool,
    empty_label: SharedString,
    on_confirm: OnConfirm<T>,
    on_cancel: OnCancel,
}

impl<T> PickerListDelegate<T>
where
    T: SelectItem + Clone + 'static,
    T::Value: Clone + PartialEq + 'static,
{
    pub(crate) fn new(
        sections: Vec<PickerSection<T>>,
        loading: bool,
        empty_label: SharedString,
        selected_values: Vec<T::Value>,
        on_confirm: OnConfirm<T>,
        on_cancel: OnCancel,
    ) -> Self {
        let mut this = Self {
            ix: None,
            all_sections: sections.clone(),
            sections,
            selected_values,
            last_query: String::new(),
            loading,
            empty_label,
            on_confirm,
            on_cancel,
        };
        this.apply_query();
        this
    }

    pub(crate) fn set_sections(&mut self, sections: Vec<PickerSection<T>>) {
        self.all_sections = sections;
        self.apply_query();
    }

    pub(crate) fn set_selected_values(&mut self, selected_values: Vec<T::Value>) {
        self.selected_values = selected_values;
        self.apply_query();
    }

    pub(crate) fn set_query(&mut self, query: impl Into<String>) {
        self.last_query = query.into();
        self.apply_query();
    }

    #[cfg(test)]
    pub(crate) fn selected_index_for<V>(
        sections: &[PickerSection<T>],
        selected_value: Option<&V>,
    ) -> Option<IndexPath>
    where
        T::Value: PartialEq<V>,
        V: ?Sized,
    {
        let selected_value = selected_value?;
        sections
            .iter()
            .enumerate()
            .find_map(|(section_ix, section)| {
                section
                    .items
                    .iter()
                    .position(|item| item.value() == selected_value)
                    .map(|row_ix| IndexPath::default().section(section_ix).row(row_ix))
            })
    }

    #[cfg(test)]
    pub(crate) fn selected_index_for_current_sections<V>(
        &self,
        selected_value: Option<&V>,
    ) -> Option<IndexPath>
    where
        T::Value: PartialEq<V>,
        V: ?Sized,
    {
        Self::selected_index_for(&self.sections, selected_value)
    }

    fn apply_query(&mut self) {
        let query = self.last_query.trim().to_lowercase();
        self.sections = self
            .all_sections
            .iter()
            .filter_map(|section| {
                let mut items = section
                    .items
                    .iter()
                    .enumerate()
                    .filter(|(_, item)| query.is_empty() || item.matches(&query))
                    .collect::<Vec<_>>();
                items.sort_by_key(|(original_ix, item)| {
                    self.selected_position(item.value())
                        .map_or((1, usize::MAX, *original_ix), |selected_ix| {
                            (0, selected_ix, *original_ix)
                        })
                });
                let items = items
                    .into_iter()
                    .map(|(_, item)| item.clone())
                    .collect::<Vec<_>>();
                (!items.is_empty()).then(|| PickerSection {
                    title: section.title.clone(),
                    items,
                })
            })
            .collect();
    }

    fn selected_position(&self, value: &T::Value) -> Option<usize> {
        self.selected_values
            .iter()
            .position(|selected| selected == value)
    }

    #[cfg(test)]
    fn checked_titles(&self) -> Vec<SharedString> {
        self.sections
            .iter()
            .flat_map(|section| {
                section
                    .items
                    .iter()
                    .filter(|item| {
                        self.selected_values
                            .iter()
                            .any(|selected| selected == item.value())
                    })
                    .map(|item| item.title())
            })
            .collect()
    }

    #[cfg(test)]
    fn visible_titles(&self) -> Vec<SharedString> {
        self.sections
            .iter()
            .flat_map(|section| section.items.iter().map(|item| item.title()))
            .collect()
    }
}

impl<T> ListDelegate for PickerListDelegate<T>
where
    T: SelectItem + Clone + 'static,
    T::Value: Clone + PartialEq + 'static,
{
    type Item = PickerListItem<T>;

    fn perform_search(
        &mut self,
        query: &str,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        self.last_query = query.to_string();
        self.apply_query();
        Task::ready(())
    }

    fn sections_count(&self, _cx: &App) -> usize {
        self.sections.len()
    }

    fn items_count(&self, section: usize, _cx: &App) -> usize {
        self.sections
            .get(section)
            .map_or(0, |section| section.items.len())
    }

    fn render_section_header(
        &mut self,
        section: usize,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<impl IntoElement> {
        let title = self.sections.get(section)?.title.clone()?;
        Some(
            div()
                .py_0p5()
                .px_2()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(title),
        )
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        self.sections
            .get(ix.section)
            .and_then(|section| section.items.get(ix.row))
            .cloned()
            .map(|item| {
                let is_checked = self
                    .selected_values
                    .iter()
                    .any(|selected| selected == item.value());
                PickerListItem::new(
                    format!("picker-item-{}-{}", ix.section, ix.row).into(),
                    item,
                )
                .checked(is_checked)
            })
    }

    fn render_empty(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        h_flex()
            .justify_center()
            .py_6()
            .text_color(cx.theme().muted_foreground)
            .child(Label::new(self.empty_label.clone()).text_sm())
            .into_any_element()
    }

    fn loading(&self, _cx: &App) -> bool {
        self.loading
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) {
        self.ix = ix;
    }

    fn confirm(
        &mut self,
        _secondary: bool,
        window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        let Some(ix) = self.ix else {
            return;
        };
        let Some(item) = self
            .sections
            .get(ix.section)
            .and_then(|section| section.items.get(ix.row))
        else {
            return;
        };
        (self.on_confirm)(item.as_ref().clone(), window, cx);
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<ListState<Self>>) {
        (self.on_cancel)(window, cx);
    }
}

pub(crate) struct PickerPopoverOptions {
    pub(crate) min_width: Option<Pixels>,
    pub(crate) max_width: Option<Pixels>,
    pub(crate) max_height: Option<Length>,
    pub(crate) search_placeholder: Option<SharedString>,
}

impl PickerPopoverOptions {
    pub(crate) fn fixed_width(width: Pixels) -> Self {
        Self {
            min_width: Some(width),
            max_width: Some(width),
            max_height: Some(rems(12.).into()),
            search_placeholder: None,
        }
    }

    pub(crate) fn search_placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.search_placeholder = Some(placeholder.into());
        self
    }
}

fn picker_popover_width(bounds: Bounds<Pixels>, options: &PickerPopoverOptions) -> Pixels {
    let mut width = bounds.size.width + px(2.);

    if let Some(min_width) = options.min_width {
        width = width.max(min_width);
    }

    if let Some(max_width) = options.max_width {
        width = width.min(max_width);
    }

    width
}

pub(crate) fn render_picker_popover<D, F>(
    bounds: Bounds<Pixels>,
    list: gpui::Entity<ListState<D>>,
    options: PickerPopoverOptions,
    on_mouse_down_out: F,
    cx: &App,
) -> impl IntoElement
where
    D: ListDelegate + 'static,
    F: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
{
    let popup_radius = cx.theme().radius.min(px(8.));
    let width = picker_popover_width(bounds, &options);

    deferred(
        anchored()
            .anchor(gpui::Anchor::TopLeft)
            .snap_to_window_with_margin(px(8.))
            .position(point(bounds.left(), bounds.bottom()))
            .offset(point(px(0.), px(6.)))
            .child(
                div().w(width).on_mouse_down_out(on_mouse_down_out).child(
                    v_flex()
                        .occlude()
                        .bg(cx.theme().background)
                        .border_1()
                        .border_color(cx.theme().border)
                        .rounded(popup_radius)
                        .shadow_md()
                        .child(
                            List::new(&list)
                                .when_some(options.search_placeholder, |this, placeholder| {
                                    this.search_placeholder(placeholder)
                                })
                                .max_h(options.max_height.unwrap_or(rems(20.).into()))
                                .paddings(Edges::all(px(4.))),
                        ),
                ),
            ),
    )
    .with_priority(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    #[derive(Clone, Debug)]
    struct TestItem {
        title: &'static str,
        value: i32,
        description: &'static str,
    }

    impl SelectItem for TestItem {
        type Value = i32;

        fn title(&self) -> SharedString {
            self.title.into()
        }

        fn render(&self, _: &mut Window, _: &mut App) -> impl IntoElement {
            self.title.into_any_element()
        }

        fn value(&self) -> &Self::Value {
            &self.value
        }

        fn matches(&self, query: &str) -> bool {
            self.title.contains(query) || self.description.contains(query)
        }
    }

    fn noop_confirm<T>() -> OnConfirm<T>
    where
        T: 'static,
    {
        Rc::new(|_, _, _| {})
    }

    fn noop_cancel() -> OnCancel {
        Rc::new(|_, _| {})
    }

    #[test]
    fn selected_index_for_returns_none_when_missing() {
        let sections = PickerSection::flat([TestItem {
            title: "one",
            value: 1,
            description: "first",
        }]);

        assert_eq!(
            PickerListDelegate::selected_index_for(&sections, Some(&2)),
            None
        );
    }

    #[test]
    fn selected_index_for_resolves_items() {
        let sections = PickerSection::flat([
            TestItem {
                title: "one",
                value: 1,
                description: "first",
            },
            TestItem {
                title: "two",
                value: 2,
                description: "second",
            },
        ]);

        assert_eq!(
            PickerListDelegate::selected_index_for(&sections, Some(&2)),
            Some(IndexPath::default().section(0).row(1))
        );
    }

    #[test]
    fn search_filters_with_select_item_matches() {
        let sections = PickerSection::flat([
            TestItem {
                title: "仙侠",
                value: 1,
                description: "长篇",
            },
            TestItem {
                title: "历史",
                value: 2,
                description: "架空",
            },
        ]);
        let mut delegate = PickerListDelegate::new(
            sections,
            false,
            "无匹配结果".into(),
            Vec::new(),
            noop_confirm(),
            noop_cancel(),
        );

        delegate.set_query("架空");

        assert_eq!(delegate.visible_titles(), vec![SharedString::from("历史")]);
    }

    #[test]
    fn clearing_query_restores_visible_items() {
        let sections = PickerSection::flat([
            TestItem {
                title: "仙侠",
                value: 1,
                description: "长篇",
            },
            TestItem {
                title: "历史",
                value: 2,
                description: "架空",
            },
        ]);
        let mut delegate = PickerListDelegate::new(
            sections,
            false,
            "无匹配结果".into(),
            Vec::new(),
            noop_confirm(),
            noop_cancel(),
        );

        delegate.set_query("架空");
        delegate.set_query("");

        assert_eq!(
            delegate.visible_titles(),
            vec![SharedString::from("仙侠"), SharedString::from("历史")]
        );
    }

    #[test]
    fn selected_values_are_pinned_without_query() {
        let sections = PickerSection::flat([
            TestItem {
                title: "one",
                value: 1,
                description: "match",
            },
            TestItem {
                title: "two",
                value: 2,
                description: "match",
            },
            TestItem {
                title: "three",
                value: 3,
                description: "match",
            },
        ]);
        let delegate = PickerListDelegate::new(
            sections,
            false,
            "无匹配结果".into(),
            vec![3, 1],
            noop_confirm(),
            noop_cancel(),
        );

        assert_eq!(
            delegate.visible_titles(),
            vec![
                SharedString::from("three"),
                SharedString::from("one"),
                SharedString::from("two")
            ]
        );
        assert_eq!(
            delegate.checked_titles(),
            vec![SharedString::from("three"), SharedString::from("one")]
        );
    }

    #[test]
    fn current_selected_index_uses_reordered_visible_sections() {
        let sections = PickerSection::flat([
            TestItem {
                title: "one",
                value: 1,
                description: "match",
            },
            TestItem {
                title: "two",
                value: 2,
                description: "match",
            },
            TestItem {
                title: "three",
                value: 3,
                description: "match",
            },
        ]);
        let delegate = PickerListDelegate::new(
            sections,
            false,
            "无匹配结果".into(),
            vec![3],
            noop_confirm(),
            noop_cancel(),
        );

        assert_eq!(
            delegate.selected_index_for_current_sections(Some(&3)),
            Some(IndexPath::default().section(0).row(0))
        );
    }

    #[test]
    fn selected_values_are_pinned_after_filtering() {
        let sections = PickerSection::flat([
            TestItem {
                title: "one",
                value: 1,
                description: "match",
            },
            TestItem {
                title: "two",
                value: 2,
                description: "other",
            },
            TestItem {
                title: "three",
                value: 3,
                description: "match",
            },
            TestItem {
                title: "four",
                value: 4,
                description: "match",
            },
        ]);
        let mut delegate = PickerListDelegate::new(
            sections,
            false,
            "无匹配结果".into(),
            vec![3],
            noop_confirm(),
            noop_cancel(),
        );

        delegate.set_query("match");

        assert_eq!(
            delegate.visible_titles(),
            vec![
                SharedString::from("three"),
                SharedString::from("one"),
                SharedString::from("four")
            ]
        );
    }

    #[test]
    fn picker_list_item_keeps_checked_and_preselected_separate() {
        let item = Rc::new(TestItem {
            title: "one",
            value: 1,
            description: "first",
        });
        let checked = PickerListItem::new("item".into(), item.clone()).checked(true);
        assert!(checked.is_checked);
        assert!(!checked.is_preselected);

        let preselected = PickerListItem::new("item".into(), item).selected(true);
        assert!(!preselected.is_checked);
        assert!(preselected.is_preselected);
    }
}

use crate::assets::IconName;
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Disableable, Icon, IndexPath, Selectable, Sizable, Size, StyledExt as _,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    list::{List, ListDelegate, ListState},
    v_flex,
};
use std::rc::Rc;

type OnCancel = Rc<dyn Fn(&mut Window, &mut App) + 'static>;
type OnConfirm<T> = Rc<dyn Fn(T, &mut Window, &mut App) + 'static>;
type TriggerClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;
type BoundsChangeHandler = Rc<dyn Fn(Bounds<Pixels>, &mut App) + 'static>;

#[derive(Clone, Debug)]
pub(crate) struct PickerSection<T> {
    pub(crate) title: Option<SharedString>,
    pub(crate) items: Vec<Rc<T>>,
}

impl<T> PickerSection<T> {
    pub(crate) fn flat(items: impl IntoIterator<Item = T>) -> Vec<Self> {
        vec![Self {
            title: None,
            items: items.into_iter().map(Rc::new).collect(),
        }]
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

#[derive(IntoElement, Clone)]
pub(crate) struct PickerListItem<T: gpui_component::select::SelectItem + Clone + 'static> {
    id: SharedString,
    item: Rc<T>,
    is_selected: bool,
}

impl<T> PickerListItem<T>
where
    T: gpui_component::select::SelectItem + Clone + 'static,
{
    fn new(id: SharedString, item: Rc<T>) -> Self {
        Self {
            id,
            item,
            is_selected: false,
        }
    }
}

impl<T> Selectable for PickerListItem<T>
where
    T: gpui_component::select::SelectItem + Clone + 'static,
{
    fn selected(mut self, selected: bool) -> Self {
        self.is_selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.is_selected
    }
}

impl<T> RenderOnce for PickerListItem<T>
where
    T: gpui_component::select::SelectItem + Clone + 'static,
{
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        h_flex()
            .id(self.id)
            .w_full()
            .relative()
            .gap_x_1()
            .min_h(px(28.))
            .px_3()
            .py_1()
            .rounded(cx.theme().radius)
            .text_base()
            .text_color(cx.theme().foreground)
            .items_center()
            .justify_between()
            .when(self.is_selected, |this| {
                this.bg(cx.theme().secondary_active)
            })
            .when(!self.is_selected, |this| {
                this.hover(|this| this.bg(cx.theme().secondary_hover))
            })
            .child(
                h_flex()
                    .relative()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .gap_x_1()
                    .child(div().w_full().child(self.item.render(window, cx))),
            )
    }
}

pub(crate) struct PickerListDelegate<T> {
    ix: Option<IndexPath>,
    all_sections: Vec<PickerSection<T>>,
    sections: Vec<PickerSection<T>>,
    last_query: String,
    loading: bool,
    empty_label: SharedString,
    on_confirm: OnConfirm<T>,
    on_cancel: OnCancel,
}

impl<T> PickerListDelegate<T>
where
    T: gpui_component::select::SelectItem + Clone + 'static,
{
    pub(crate) fn new(
        sections: Vec<PickerSection<T>>,
        loading: bool,
        empty_label: SharedString,
        on_confirm: OnConfirm<T>,
        on_cancel: OnCancel,
    ) -> Self {
        Self {
            ix: None,
            all_sections: sections.clone(),
            sections,
            last_query: String::new(),
            loading,
            empty_label,
            on_confirm,
            on_cancel,
        }
    }

    pub(crate) fn set_sections(&mut self, sections: Vec<PickerSection<T>>) {
        self.all_sections = sections;
        self.apply_query();
    }

    pub(crate) fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
    }

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

    fn apply_query(&mut self) {
        let query = self.last_query.trim().to_lowercase();
        if query.is_empty() {
            self.sections = self.all_sections.clone();
            return;
        }

        self.sections = self
            .all_sections
            .iter()
            .filter_map(|section| {
                let items = section
                    .items
                    .iter()
                    .filter(|item| item.matches(&query))
                    .cloned()
                    .collect::<Vec<_>>();
                if items.is_empty() {
                    None
                } else {
                    Some(PickerSection {
                        title: section.title.clone(),
                        items,
                    })
                }
            })
            .collect();
    }
}

impl<T> ListDelegate for PickerListDelegate<T>
where
    T: gpui_component::select::SelectItem + Clone + 'static,
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
                PickerListItem::new(
                    format!("picker-item-{}-{}", ix.section, ix.row).into(),
                    item,
                )
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

#[derive(Default)]
pub(crate) struct PickerPopoverOptions {
    pub(crate) min_width: Option<Pixels>,
    pub(crate) max_width: Option<Pixels>,
    pub(crate) max_height: Option<Length>,
    pub(crate) search_placeholder: Option<SharedString>,
    pub(crate) footer: Option<AnyElement>,
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
    list: Entity<ListState<D>>,
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
            .anchor(Corner::BottomLeft)
            .snap_to_window_with_margin(px(8.))
            .position(point(bounds.left(), bounds.top()))
            .child(
                div().w(width).on_mouse_down_out(on_mouse_down_out).child(
                    v_flex()
                        .occlude()
                        .mb_1p5()
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
                                .with_size(Size::Medium)
                                .max_h(options.max_height.unwrap_or(rems(20.).into()))
                                .paddings(Edges::all(px(4.))),
                        )
                        .when_some(options.footer, |this, footer| this.child(footer)),
                ),
            ),
    )
    .with_priority(1)
}

#[derive(IntoElement)]
pub(crate) struct PickerTrigger {
    id: ElementId,
    title: AnyElement,
    selected: bool,
    open: bool,
    disabled: bool,
    on_toggle: TriggerClickHandler,
    on_bounds_change: BoundsChangeHandler,
}

impl PickerTrigger {
    pub(crate) fn new(
        id: impl Into<ElementId>,
        title: impl IntoElement,
        on_toggle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
        on_bounds_change: impl Fn(Bounds<Pixels>, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into_any_element(),
            selected: false,
            open: false,
            disabled: false,
            on_toggle: Rc::new(on_toggle),
            on_bounds_change: Rc::new(on_bounds_change),
        }
    }

    pub(crate) fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub(crate) fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }
}

impl RenderOnce for PickerTrigger {
    fn render(self, _: &mut Window, _cx: &mut App) -> impl IntoElement {
        let is_active = self.selected || self.open;
        let title = self.title;
        let on_bounds_change = self.on_bounds_change;

        Button::new(self.id)
            .ghost()
            .selected(is_active)
            .rounded(px(8.))
            .small()
            .disabled(self.disabled)
            .on_click(move |event, window, cx| (self.on_toggle)(event, window, cx))
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .gap_1p5()
                    .child(
                        div()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .truncate()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child(title),
                    )
                    .child(
                        Icon::new(if self.open {
                            IconName::ChevronUp
                        } else {
                            IconName::ChevronDown
                        })
                        .xsmall(),
                    ),
            )
            .child(
                canvas(
                    move |bounds, _, cx| (on_bounds_change)(bounds, cx),
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{PickerListDelegate, PickerPopoverOptions, PickerSection, picker_popover_width};
    use gpui::{App, Bounds, IntoElement, SharedString, Window, point, px};
    use gpui_component::{IndexPath, select::SelectItem};
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
    fn selected_index_for_resolves_grouped_items() {
        let sections = vec![
            PickerSection::section(
                "A",
                [TestItem {
                    title: "one",
                    value: 1,
                    description: "first",
                }],
            ),
            PickerSection::section(
                "B",
                [TestItem {
                    title: "two",
                    value: 2,
                    description: "second",
                }],
            ),
        ];

        assert_eq!(
            PickerListDelegate::selected_index_for(&sections, Some(&2)),
            Some(IndexPath::default().section(1).row(0))
        );
    }

    #[test]
    fn apply_query_filters_on_custom_matches() {
        let mut delegate = PickerListDelegate::new(
            PickerSection::flat([
                TestItem {
                    title: "one",
                    value: 1,
                    description: "alpha",
                },
                TestItem {
                    title: "two",
                    value: 2,
                    description: "beta",
                },
            ]),
            false,
            "Empty".into(),
            Rc::new(|_, _, _| {}),
            Rc::new(|_, _| {}),
        );

        delegate.last_query = "beta".to_string();
        delegate.apply_query();

        assert_eq!(delegate.sections[0].items.len(), 1);
        assert_eq!(delegate.sections[0].items[0].value(), &2);
    }

    #[test]
    fn picker_popover_width_respects_max_width() {
        let bounds = Bounds::from_corners(point(px(0.), px(0.)), point(px(520.), px(40.)));
        let options = PickerPopoverOptions {
            max_width: Some(px(320.)),
            ..Default::default()
        };

        assert_eq!(picker_popover_width(bounds, &options), px(320.));
    }

    #[test]
    fn picker_popover_width_respects_min_width() {
        let bounds = Bounds::from_corners(point(px(0.), px(0.)), point(px(120.), px(40.)));
        let options = PickerPopoverOptions {
            min_width: Some(px(240.)),
            ..Default::default()
        };

        assert_eq!(picker_popover_width(bounds, &options), px(240.));
    }
}

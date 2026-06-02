use super::{COMPOSER_BUTTON_RADIUS, COMPOSER_BUTTON_SIZE};
use crate::foundation::assets::IconName;
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Icon, IndexPath, Selectable, Sizable, Size, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    list::{List, ListDelegate, ListState},
    popover::Popover,
    select::SelectItem,
    v_flex,
};
use std::rc::Rc;

type OnCancel = Rc<dyn Fn(&mut Window, &mut App) + 'static>;
type OnConfirm<T> = Rc<dyn Fn(T, &mut Window, &mut App) + 'static>;

#[derive(Clone, Debug)]
pub(in crate::features::home) struct PickerSection<T> {
    pub(in crate::features::home) title: Option<SharedString>,
    pub(in crate::features::home) items: Vec<Rc<T>>,
}

impl<T> PickerSection<T> {
    #[cfg(test)]
    pub(super) fn flat(items: impl IntoIterator<Item = T>) -> Vec<Self> {
        vec![Self {
            title: None,
            items: items.into_iter().map(Rc::new).collect(),
        }]
    }

    pub(in crate::features::home) fn untitled(items: impl IntoIterator<Item = T>) -> Self {
        Self {
            title: None,
            items: items.into_iter().map(Rc::new).collect(),
        }
    }

    pub(in crate::features::home) fn section(
        title: impl Into<SharedString>,
        items: impl IntoIterator<Item = T>,
    ) -> Self {
        Self {
            title: Some(title.into()),
            items: items.into_iter().map(Rc::new).collect(),
        }
    }
}

pub(in crate::features::home) struct PickerListDelegate<T>
where
    T: SelectItem + Clone + 'static,
{
    ix: Option<IndexPath>,
    all_sections: Vec<PickerSection<T>>,
    sections: Vec<PickerSection<T>>,
    last_query: String,
    selected_value: Option<T::Value>,
    empty_label: SharedString,
    on_confirm: OnConfirm<T>,
    on_cancel: OnCancel,
}

#[derive(IntoElement, Clone)]
pub(in crate::features::home) struct PickerListItem<T>
where
    T: SelectItem + Clone + 'static,
{
    id: SharedString,
    item: Rc<T>,
    is_selected: bool,
}

impl<T> PickerListItem<T>
where
    T: SelectItem + Clone + 'static,
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
    T: SelectItem + Clone + 'static,
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
    T: SelectItem + Clone + 'static,
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
                    .child(div().w_full().min_w_0().child(self.item.render(window, cx))),
            )
    }
}

impl<T> PickerListDelegate<T>
where
    T: SelectItem + Clone + 'static,
{
    pub(in crate::features::home) fn new(
        sections: Vec<PickerSection<T>>,
        selected_value: Option<T::Value>,
        empty_label: SharedString,
        on_confirm: OnConfirm<T>,
        on_cancel: OnCancel,
    ) -> Self {
        Self {
            ix: None,
            all_sections: sections.clone(),
            sections,
            last_query: String::new(),
            selected_value,
            empty_label,
            on_confirm,
            on_cancel,
        }
    }

    pub(in crate::features::home) fn set_sections(&mut self, sections: Vec<PickerSection<T>>) {
        self.all_sections = sections;
        self.apply_query();
    }

    pub(in crate::features::home) fn set_selected_value(
        &mut self,
        selected_value: Option<T::Value>,
    ) {
        self.selected_value = selected_value;
    }

    pub(in crate::features::home) fn set_empty_label(
        &mut self,
        empty_label: impl Into<SharedString>,
    ) {
        self.empty_label = empty_label.into();
    }

    pub(in crate::features::home) fn selected_index(&self) -> Option<IndexPath> {
        Self::selected_index_for(&self.sections, self.selected_value.as_ref())
    }

    pub(in crate::features::home) fn selected_index_for(
        sections: &[PickerSection<T>],
        selected_value: Option<&T::Value>,
    ) -> Option<IndexPath> {
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
                (!items.is_empty()).then(|| PickerSection {
                    title: section.title.clone(),
                    items,
                })
            })
            .collect();
    }
}

impl<T> ListDelegate for PickerListDelegate<T>
where
    T: SelectItem + Clone + 'static,
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
            Label::new(title)
                .text_xs()
                .px_2()
                .pt_2()
                .pb_1()
                .text_color(cx.theme().muted_foreground),
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
            .cloned()
        else {
            return;
        };

        self.selected_value = Some(item.value().clone());
        (self.on_confirm)(item.as_ref().clone(), window, cx);
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<ListState<Self>>) {
        (self.on_cancel)(window, cx);
    }
}

pub(in crate::features::home) fn picker_trigger(
    id: &'static str,
    icon: IconName,
    label: impl Into<SharedString>,
    open: bool,
) -> Button {
    Button::new(id)
        .ghost()
        .selected(open)
        .with_size(px(COMPOSER_BUTTON_SIZE))
        .h(px(COMPOSER_BUTTON_SIZE))
        .px(px(8.))
        .py(px(0.))
        .rounded(px(COMPOSER_BUTTON_RADIUS))
        .child(
            h_flex()
                .items_center()
                .min_w_0()
                .gap_1p5()
                .child(Icon::new(icon).size_4())
                .child(
                    Label::new(label.into())
                        .text_sm()
                        .font_medium()
                        .whitespace_nowrap()
                        .truncate(),
                )
                .child(
                    Icon::new(if open {
                        IconName::ChevronUp
                    } else {
                        IconName::ChevronDown
                    })
                    .size_3(),
                ),
        )
}

pub(in crate::features::home) struct PickerPopoverConfig<D, F>
where
    D: ListDelegate + 'static,
    F: Fn(&bool, &mut Window, &mut App) + 'static,
{
    pub(in crate::features::home) id: &'static str,
    pub(in crate::features::home) open: bool,
    pub(in crate::features::home) trigger: Button,
    pub(in crate::features::home) list: Entity<ListState<D>>,
    pub(in crate::features::home) width: Pixels,
    pub(in crate::features::home) max_height: Length,
    pub(in crate::features::home) search_placeholder: Option<SharedString>,
    pub(in crate::features::home) footer: Option<AnyElement>,
    pub(in crate::features::home) on_open_change: F,
}

pub(in crate::features::home) fn picker_popover<D, F>(
    cx: &App,
    config: PickerPopoverConfig<D, F>,
) -> impl IntoElement
where
    D: ListDelegate + 'static,
    F: Fn(&bool, &mut Window, &mut App) + 'static,
{
    Popover::new(config.id)
        .anchor(Anchor::BottomLeft)
        .appearance(false)
        .open(config.open)
        .on_open_change(config.on_open_change)
        .trigger(config.trigger)
        .child(
            v_flex()
                .w(config.width)
                .occlude()
                .mb_1p5()
                .rounded(px(12.))
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().popover)
                .shadow_lg()
                .child(
                    List::new(&config.list)
                        .when_some(config.search_placeholder, |this, placeholder| {
                            this.search_placeholder(placeholder)
                        })
                        .with_size(Size::Small)
                        .scrollbar_visible(false)
                        .max_h(config.max_height)
                        .paddings(Edges::all(px(4.))),
                )
                .when_some(config.footer, |this, footer| this.child(footer)),
        )
}

#[cfg(test)]
mod tests {
    use super::{PickerListDelegate, PickerSection};
    use gpui::{App, IntoElement, SharedString, Window};
    use gpui_component::IndexPath;
    use gpui_component::select::SelectItem;
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
            None,
            "Empty".into(),
            Rc::new(|_, _, _| {}),
            Rc::new(|_, _| {}),
        );

        delegate.last_query = "beta".to_string();
        delegate.apply_query();

        assert_eq!(delegate.sections[0].items.len(), 1);
        assert_eq!(delegate.sections[0].items[0].value(), &2);
    }
}

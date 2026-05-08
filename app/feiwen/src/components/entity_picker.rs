use gpui::{
    App, AppContext, Bounds, Context, Entity, InteractiveElement, IntoElement, MouseDownEvent,
    ParentElement, Pixels, Render, SharedString, StatefulInteractiveElement, Styled, Window,
    canvas, div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable, h_flex, label::Label, list::ListState,
    select::SelectItem, v_flex,
};
use std::rc::Rc;

use super::picker::{
    PickerListDelegate, PickerPopoverOptions, PickerSection, render_picker_popover,
};

pub(crate) struct EntityPickerState<T>
where
    T: SelectItem + Clone + 'static,
    T::Value: Clone + PartialEq + 'static,
{
    options: Vec<T>,
    selected: Option<T::Value>,
    list: Entity<ListState<PickerListDelegate<T>>>,
    picker_bounds: Bounds<Pixels>,
    open: bool,
    placeholder: SharedString,
    disabled: bool,
}

impl<T> EntityPickerState<T>
where
    T: SelectItem + Clone + 'static,
    T::Value: Clone + PartialEq + 'static,
{
    pub(crate) fn new(
        options: Vec<T>,
        placeholder: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let state = cx.entity().downgrade();
        let on_confirm = Rc::new(move |item: T, window: &mut Window, cx: &mut App| {
            let state = state.clone();
            window.defer(cx, move |window, cx| {
                let _ = state.update(cx, |picker, cx| {
                    picker.select_value(item.value().clone(), window, cx);
                });
            });
        });

        let state = cx.entity().downgrade();
        let on_cancel = Rc::new(move |window: &mut Window, cx: &mut App| {
            let _ = state.update(cx, |picker, cx| picker.close(window, cx));
        });

        let sections = PickerSection::flat(options.clone());
        let list = cx.new(|cx| {
            ListState::new(
                PickerListDelegate::new(
                    sections,
                    false,
                    "无匹配结果".into(),
                    Vec::new(),
                    on_confirm,
                    on_cancel,
                ),
                window,
                cx,
            )
            .searchable(true)
        });

        Self {
            options,
            selected: None,
            list,
            picker_bounds: Bounds::default(),
            open: false,
            placeholder: placeholder.into(),
            disabled: false,
        }
    }

    pub(crate) fn selected_key(&self) -> Option<T::Value> {
        self.selected.clone()
    }

    pub(crate) fn set_options(&mut self, options: Vec<T>, cx: &mut Context<Self>) {
        self.options = options;
        if let Some(selected) = &self.selected
            && !self.options.iter().any(|option| option.value() == selected)
        {
            self.selected = None;
        }
        self.sync_list(cx);
        cx.notify();
    }

    pub(crate) fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        if disabled {
            self.open = false;
        }
        cx.notify();
    }

    fn selected_option(&self) -> Option<T> {
        let selected = self.selected.as_ref()?;
        self.options
            .iter()
            .find(|option| option.value() == selected)
            .cloned()
    }

    fn selected_values(&self) -> Vec<T::Value> {
        self.selected.iter().cloned().collect()
    }

    fn sync_list(&mut self, cx: &mut Context<Self>) {
        let sections = PickerSection::flat(self.options.clone());
        let selected_values = self.selected_values();
        self.list.update(cx, |list, _| {
            list.delegate_mut().set_sections(sections);
            list.delegate_mut().set_selected_values(selected_values);
        });
    }

    fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.open = true;
        self.sync_list(cx);
        let sections = PickerSection::flat(self.options.clone());
        let selected_ix = PickerListDelegate::selected_index_for(&sections, self.selected.as_ref());
        self.list.update(cx, |list, cx| {
            list.set_selected_index(selected_ix, window, cx);
            list.focus(window, cx);
        });
        cx.notify();
    }

    fn close(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.open = false;
        cx.notify();
    }

    fn toggle_open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        if self.open {
            self.close(window, cx);
        } else {
            self.open(window, cx);
        }
    }

    fn select_value(&mut self, value: T::Value, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.selected = Some(value);
        self.sync_list(cx);
        let sections = PickerSection::flat(self.options.clone());
        let selected_ix = PickerListDelegate::selected_index_for(&sections, self.selected.as_ref());
        self.list.update(cx, |list, cx| {
            list.set_selected_index(selected_ix, window, cx)
        });
        self.close(window, cx);
    }

    fn trigger_title(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        self.selected_option().map_or_else(
            || {
                Label::new(self.placeholder.clone())
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .into_any_element()
            },
            |option| {
                v_flex()
                    .min_w_0()
                    .child(Label::new(option.title()).text_sm())
                    .into_any_element()
            },
        )
    }
}

impl<T> Render for EntityPickerState<T>
where
    T: SelectItem + Clone + 'static,
    T::Value: Clone + PartialEq + 'static,
{
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let list = self.list.clone();
        let bounds = self.picker_bounds;
        let trigger_title = self.trigger_title(cx);
        let on_mouse_down_out = cx.listener(|picker, event: &MouseDownEvent, window, cx| {
            if picker.picker_bounds.contains(&event.position) {
                return;
            }
            picker.close(window, cx);
        });

        div()
            .child(
                h_flex()
                    .id("entity-picker-trigger")
                    .relative()
                    .min_h(px(32.))
                    .w_full()
                    .gap_1()
                    .px_2()
                    .border_1()
                    .border_color(cx.theme().input)
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().background)
                    .items_center()
                    .when(self.disabled, |this| {
                        this.bg(cx.theme().muted.opacity(0.55))
                    })
                    .when(!self.disabled, |this| {
                        this.cursor_pointer()
                            .on_click(cx.listener(|picker, _, window, cx| {
                                picker.toggle_open(window, cx);
                            }))
                    })
                    .child(
                        canvas(
                            {
                                let state = cx.entity();
                                move |bounds, _, cx| {
                                    state.update(cx, |picker, _| {
                                        picker.picker_bounds = bounds;
                                    });
                                }
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .size_full(),
                    )
                    .child(
                        div()
                            .id("entity-picker-value")
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .truncate()
                            .child(trigger_title),
                    )
                    .child(
                        Icon::new(IconName::ChevronDown)
                            .xsmall()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
            .when(self.open && !self.disabled, |this| {
                this.child(render_picker_popover(
                    bounds,
                    list,
                    PickerPopoverOptions::fixed_width(px(320.)).search_placeholder("搜索"),
                    on_mouse_down_out,
                    cx,
                ))
            })
    }
}

#[cfg(test)]
mod tests {
    use gpui::{IntoElement, SharedString, Window};

    use super::*;

    #[derive(Clone)]
    struct OptionItem {
        label: &'static str,
        value: i32,
        description: &'static str,
    }

    impl SelectItem for OptionItem {
        type Value = i32;

        fn title(&self) -> SharedString {
            self.label.into()
        }

        fn render(&self, _: &mut Window, _: &mut App) -> impl IntoElement {
            self.label.into_any_element()
        }

        fn value(&self) -> &Self::Value {
            &self.value
        }

        fn matches(&self, query: &str) -> bool {
            self.label.contains(query) || self.description.contains(query)
        }
    }

    #[test]
    fn select_item_matches_label_and_description() {
        let item = OptionItem {
            label: "作者",
            value: 1,
            description: "匿名作者",
        };

        assert!(item.matches("作者"));
        assert!(item.matches("匿名"));
        assert!(!item.matches("标签"));
    }
}

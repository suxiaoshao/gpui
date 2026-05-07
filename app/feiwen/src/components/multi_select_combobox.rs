use gpui::{
    App, AppContext, Bounds, Context, Entity, Focusable, InteractiveElement, IntoElement,
    MouseDownEvent, ParentElement, Pixels, Render, SharedString, Styled, Subscription, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, ElementExt, Sizable, h_flex,
    input::{Input, InputEvent, InputState},
    list::ListState,
    select::SelectItem,
    v_flex,
};
use std::rc::Rc;

use super::{
    ComboboxChip,
    picker::{PickerListDelegate, PickerPopoverOptions, PickerSection, render_picker_popover},
};

pub(crate) struct MultiSelectState<T>
where
    T: SelectItem + Clone + 'static,
    T::Value: Clone + PartialEq + 'static,
{
    options: Vec<T>,
    selected: Vec<T::Value>,
    list: Entity<ListState<PickerListDelegate<T>>>,
    query_input: Entity<InputState>,
    picker_bounds: Bounds<Pixels>,
    picker_bounds_captured: bool,
    open: bool,
    placeholder: SharedString,
    _subscriptions: Vec<Subscription>,
}

impl<T> MultiSelectState<T>
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
                    picker.toggle_value(item.value().clone(), window, cx);
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
        });
        let query_input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        let _subscriptions =
            vec![cx.subscribe_in(&query_input, window, Self::on_query_input_event)];

        Self {
            options,
            selected: Vec::new(),
            list,
            query_input,
            picker_bounds: Bounds::default(),
            picker_bounds_captured: false,
            open: false,
            placeholder: placeholder.into(),
            _subscriptions,
        }
    }

    pub(crate) fn selected_keys(&self) -> Vec<T::Value> {
        self.selected.clone()
    }

    pub(crate) fn set_options(&mut self, options: Vec<T>, cx: &mut Context<Self>) {
        self.options = options;
        self.selected
            .retain(|value| self.options.iter().any(|option| option.value() == value));
        self.sync_list(cx);
        cx.notify();
    }

    fn on_query_input_event(
        &mut self,
        state: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::Focus => self.open(window, cx),
            InputEvent::Change => {
                self.open = true;
                let query = state.read(cx).value().to_string();
                self.filter_list(query, cx);
                cx.notify();
            }
            InputEvent::PressEnter { .. } | InputEvent::Blur => {}
        }
    }

    fn sync_list(&mut self, cx: &mut Context<Self>) {
        let sections = PickerSection::flat(self.options.clone());
        let selected_values = self.selected.clone();
        self.list.update(cx, |list, _| {
            list.delegate_mut().set_sections(sections);
            list.delegate_mut().set_selected_values(selected_values);
        });
    }

    fn filter_list(&mut self, query: impl Into<String>, cx: &mut Context<Self>) {
        let query = query.into();
        self.list.update(cx, |list, cx| {
            list.delegate_mut().set_query(query);
            cx.notify();
        });
    }

    fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = true;
        self.sync_list(cx);
        let query = self.query_input.read(cx).value().to_string();
        self.filter_list(query, cx);
        self.query_input
            .update(cx, |input, cx| input.focus_handle(cx).focus(window, cx));
        cx.notify();
    }

    fn close(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.open = false;
        cx.notify();
    }

    fn toggle_value(&mut self, value: T::Value, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ix) = self.selected.iter().position(|selected| *selected == value) {
            self.selected.remove(ix);
        } else {
            self.selected.push(value);
        }
        self.sync_list(cx);
        self.clear_query(window, cx);
        self.open = true;
        cx.notify();
    }

    fn remove_value(&mut self, value: T::Value, cx: &mut Context<Self>) {
        self.selected.retain(|selected| *selected != value);
        self.sync_list(cx);
        cx.notify();
    }

    fn clear_query(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.query_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.filter_list("", cx);
    }

    fn selected_options(&self) -> Vec<T> {
        self.selected
            .iter()
            .filter_map(|value| {
                self.options
                    .iter()
                    .find(|option| option.value() == value)
                    .cloned()
            })
            .collect()
    }
}

impl<T> Render for MultiSelectState<T>
where
    T: SelectItem + Clone + 'static,
    T::Value: Clone + PartialEq + 'static,
{
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let bounds_state = cx.entity();
        let list = self.list.clone();
        let bounds = self.picker_bounds;
        let selected = self.selected_options();
        let has_selection = !selected.is_empty();
        let input_placeholder = if self.selected.is_empty() {
            self.placeholder.clone()
        } else {
            SharedString::default()
        };
        self.query_input.update(cx, |input, cx| {
            input.set_placeholder(input_placeholder, window, cx);
        });
        let on_mouse_down_out = cx.listener(|picker, event: &MouseDownEvent, window, cx| {
            if picker.picker_bounds.contains(&event.position) {
                return;
            }
            picker.close(window, cx);
        });

        v_flex()
            .w_full()
            .min_w_0()
            .gap_1()
            .child(
                h_flex()
                    .relative()
                    .min_h(px(32.))
                    .w_full()
                    .min_w_0()
                    .gap_1()
                    .items_center()
                    .when(has_selection, |this| this.px_1())
                    .when(!has_selection, |this| this.px_2p5())
                    .py_1()
                    .border_1()
                    .border_color(cx.theme().input)
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().background)
                    .on_prepaint(move |bounds, window, cx| {
                        let needs_frame = bounds_state.update(cx, |picker, _| {
                            let first_capture = !picker.picker_bounds_captured;
                            picker.picker_bounds = bounds;
                            picker.picker_bounds_captured = true;
                            first_capture
                        });
                        if needs_frame {
                            window.request_animation_frame();
                        }
                    })
                    .child(
                        h_flex()
                            .gap_1()
                            .flex_wrap()
                            .flex_1()
                            .min_w_0()
                            .items_center()
                            .children(selected.into_iter().enumerate().map(|(ix, option)| {
                                let value = option.value().clone();
                                let title = option.title();
                                div()
                                    .id(("multi-select-chip-wrapper", ix))
                                    .flex_none()
                                    .child(
                                        ComboboxChip::new(
                                            ("multi-select-chip", ix),
                                            ("multi-select-remove", ix),
                                            title,
                                        )
                                        .on_remove({
                                            let entity = entity.clone();
                                            move |_, _, cx| {
                                                entity.update(cx, |picker, cx| {
                                                    picker.remove_value(value.clone(), cx);
                                                });
                                            }
                                        }),
                                    )
                                    .into_any_element()
                            }))
                            .child(
                                div()
                                    .id("multi-select-input")
                                    .flex_1()
                                    .min_w(px(64.))
                                    .child(
                                        Input::new(&self.query_input)
                                            .appearance(false)
                                            .bordered(false)
                                            .small()
                                            .p_0()
                                            .min_w(px(64.))
                                            .w_full(),
                                    ),
                            ),
                    ),
            )
            .when(self.open, |this| {
                this.child(render_picker_popover(
                    bounds,
                    list,
                    PickerPopoverOptions::fixed_width(px(360.)),
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
    struct OptionItem(&'static str);

    impl SelectItem for OptionItem {
        type Value = String;

        fn title(&self) -> SharedString {
            self.0.into()
        }

        fn render(&self, _: &mut Window, _: &mut App) -> impl IntoElement {
            self.0.into_any_element()
        }

        fn value(&self) -> &Self::Value {
            unreachable!("test only checks title/matches")
        }

        fn matches(&self, query: &str) -> bool {
            self.0.contains(query)
        }
    }

    #[test]
    fn select_item_matches_label() {
        assert!(OptionItem("仙侠").matches("仙"));
        assert!(!OptionItem("仙侠").matches("历史"));
    }
}

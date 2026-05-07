use gpui::{
    AnyElement, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement,
    Render, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    scroll::ScrollableElement,
    tag::Tag,
    v_flex,
};

use super::picker::PickerOption;

pub(crate) struct MultiSelectState<T: PickerOption> {
    options: Vec<T>,
    selected: Vec<T::Key>,
    input: Entity<InputState>,
    open: bool,
    _subscriptions: Vec<gpui::Subscription>,
}

impl<T: PickerOption> MultiSelectState<T> {
    pub(crate) fn new(
        options: Vec<T>,
        placeholder: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        let _subscriptions = vec![cx.subscribe_in(&input, window, Self::on_input_event)];
        Self {
            options,
            selected: Vec::new(),
            input,
            open: false,
            _subscriptions,
        }
    }

    pub(crate) fn selected_keys(&self) -> Vec<T::Key> {
        self.selected.clone()
    }

    pub(crate) fn set_options(&mut self, options: Vec<T>, cx: &mut Context<Self>) {
        self.options = options;
        self.selected
            .retain(|key| self.options.iter().any(|option| option.key() == *key));
        cx.notify();
    }

    fn on_input_event(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(event, InputEvent::Change | InputEvent::Focus) {
            self.open = true;
            cx.notify();
        }
    }

    fn toggle_open(&mut self, cx: &mut Context<Self>) {
        self.open = !self.open;
        cx.notify();
    }

    fn toggle_key(&mut self, key: T::Key, cx: &mut Context<Self>) {
        if let Some(ix) = self.selected.iter().position(|selected| *selected == key) {
            self.selected.remove(ix);
        } else {
            self.selected.push(key);
        }
        cx.notify();
    }

    fn remove_key(&mut self, key: T::Key, cx: &mut Context<Self>) {
        self.selected.retain(|selected| *selected != key);
        cx.notify();
    }

    fn selected_options(&self) -> Vec<T> {
        self.selected
            .iter()
            .filter_map(|key| self.options.iter().find(|option| option.key() == *key))
            .cloned()
            .collect()
    }

    fn filtered_options(&self, cx: &gpui::App) -> Vec<T> {
        let query = self.input.read(cx).value().to_string();
        self.options
            .iter()
            .filter(|option| option.matches(&query))
            .cloned()
            .collect()
    }
}

impl<T: PickerOption> Render for MultiSelectState<T> {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let selected = self.selected_options();
        let filtered = self.filtered_options(cx);
        let selected_keys = self.selected.clone();

        v_flex()
            .gap_1()
            .child(
                h_flex()
                    .min_h(px(32.))
                    .gap_1()
                    .p_1()
                    .border_1()
                    .border_color(cx.theme().input)
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().background)
                    .child(
                        h_flex()
                            .gap_1()
                            .flex_wrap()
                            .flex_1()
                            .children(selected.into_iter().enumerate().map(|(ix, option)| {
                                let key = option.key();
                                h_flex()
                                    .gap_1()
                                    .child(Tag::secondary().outline().child(option.label()))
                                    .child(
                                        Button::new(("multi-select-remove", ix))
                                            .xsmall()
                                            .ghost()
                                            .icon(IconName::Close)
                                            .on_click({
                                                let entity = entity.clone();
                                                move |_, _, cx| {
                                                    entity.update(cx, |this, cx| {
                                                        this.remove_key(key.clone(), cx);
                                                    });
                                                }
                                            }),
                                    )
                                    .into_any_element()
                            }))
                            .child(
                                Input::new(&self.input)
                                    .appearance(false)
                                    .small()
                                    .min_w(px(120.)),
                            ),
                    )
                    .child(
                        Button::new("multi-select-toggle")
                            .xsmall()
                            .ghost()
                            .icon(IconName::ChevronDown)
                            .on_click({
                                let entity = entity.clone();
                                move |_, _, cx| {
                                    entity.update(cx, |this, cx| this.toggle_open(cx));
                                }
                            }),
                    ),
            )
            .when(self.open, |this| {
                this.child(
                    v_flex()
                        .max_h(px(180.))
                        .overflow_y_scrollbar()
                        .border_1()
                        .border_color(cx.theme().border)
                        .rounded(cx.theme().radius)
                        .bg(cx.theme().background)
                        .shadow_sm()
                        .when(filtered.is_empty(), |this| {
                            this.child(
                                div()
                                    .p_2()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("无匹配结果"),
                            )
                        })
                        .children(filtered.into_iter().enumerate().map(|(ix, option)| {
                            render_option(ix, option, &selected_keys, &entity)
                        })),
                )
            })
    }
}

fn render_option<T: PickerOption>(
    ix: usize,
    option: T,
    selected_keys: &[T::Key],
    entity: &Entity<MultiSelectState<T>>,
) -> AnyElement {
    let key = option.key();
    let selected = selected_keys.contains(&key);
    h_flex()
        .id(("multi-select-option", ix))
        .gap_2()
        .px_2()
        .py_1()
        .items_center()
        .cursor_pointer()
        .child(
            Icon::new(IconName::Check)
                .xsmall()
                .when(!selected, |this| this.invisible()),
        )
        .child(
            v_flex()
                .gap_0p5()
                .child(Label::new(option.label()).text_sm())
                .when_some(option.description(), |this, description| {
                    this.child(Label::new(description).text_xs())
                }),
        )
        .on_click({
            let entity = entity.clone();
            move |_, _, cx| {
                entity.update(cx, |this, cx| {
                    this.toggle_key(key.clone(), cx);
                });
            }
        })
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::SharedString;

    #[derive(Clone)]
    struct OptionItem(&'static str);

    impl PickerOption for OptionItem {
        type Key = String;

        fn key(&self) -> Self::Key {
            self.0.to_owned()
        }

        fn label(&self) -> SharedString {
            self.0.into()
        }
    }

    #[test]
    fn picker_option_matches_label() {
        assert!(OptionItem("仙侠").matches("仙"));
        assert!(!OptionItem("仙侠").matches("历史"));
    }
}

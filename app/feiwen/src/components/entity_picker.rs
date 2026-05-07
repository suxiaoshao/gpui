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
    v_flex,
};

use super::picker::PickerOption;

pub(crate) struct EntityPickerState<T: PickerOption> {
    options: Vec<T>,
    selected: Option<T::Key>,
    input: Entity<InputState>,
    open: bool,
    _subscriptions: Vec<gpui::Subscription>,
}

impl<T: PickerOption> EntityPickerState<T> {
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
            selected: None,
            input,
            open: false,
            _subscriptions,
        }
    }

    pub(crate) fn selected_key(&self) -> Option<T::Key> {
        self.selected.clone()
    }

    pub(crate) fn set_options(&mut self, options: Vec<T>, cx: &mut Context<Self>) {
        self.options = options;
        if let Some(selected) = &self.selected
            && !self.options.iter().any(|option| option.key() == *selected)
        {
            self.selected = None;
        }
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

    fn select_key(&mut self, key: T::Key, cx: &mut Context<Self>) {
        self.selected = Some(key);
        self.open = false;
        cx.notify();
    }

    fn selected_option(&self) -> Option<T> {
        let selected = self.selected.as_ref()?;
        self.options
            .iter()
            .find(|option| option.key() == *selected)
            .cloned()
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

impl<T: PickerOption> Render for EntityPickerState<T> {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let selected = self.selected_option();
        let filtered = self.filtered_options(cx);

        v_flex()
            .gap_1()
            .child(
                h_flex()
                    .min_h(px(32.))
                    .gap_1()
                    .px_2()
                    .border_1()
                    .border_color(cx.theme().input)
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().background)
                    .child(
                        v_flex()
                            .flex_1()
                            .when_some(selected, |this, option| {
                                this.child(Label::new(option.label()).text_sm()).when_some(
                                    option.description(),
                                    |this, description| {
                                        this.child(Label::new(description).text_xs())
                                    },
                                )
                            })
                            .when(self.selected.is_none(), |this| {
                                this.child(Input::new(&self.input).appearance(false).small())
                            }),
                    )
                    .child(
                        Button::new("entity-picker-toggle")
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
                            render_option(ix, option, &entity, &self.selected)
                        })),
                )
            })
    }
}

fn render_option<T: PickerOption>(
    ix: usize,
    option: T,
    entity: &Entity<EntityPickerState<T>>,
    selected: &Option<T::Key>,
) -> AnyElement {
    let key = option.key();
    let is_selected = selected.as_ref().is_some_and(|selected| *selected == key);
    h_flex()
        .id(("entity-picker-option", ix))
        .gap_2()
        .px_2()
        .py_1()
        .items_center()
        .cursor_pointer()
        .child(if is_selected {
            Icon::new(IconName::Check).xsmall().into_any_element()
        } else {
            Icon::new(IconName::BookOpen).xsmall().into_any_element()
        })
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
                    this.select_key(key.clone(), cx);
                });
            }
        })
        .into_any_element()
}

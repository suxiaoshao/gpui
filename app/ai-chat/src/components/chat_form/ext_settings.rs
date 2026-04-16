use super::picker::{
    PickerListDelegate, PickerPopoverOptions, PickerSection, PickerTrigger, render_picker_popover,
};
use crate::{
    i18n::I18n,
    llm::{ExtSettingControl, ExtSettingItem, ExtSettingOption},
};
use gpui::{StatefulInteractiveElement, prelude::FluentBuilder as _, *};
use gpui_component::{
    Selectable, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    list::ListState,
    scroll::ScrollableElement,
    select::SelectItem,
    text::TextView,
    tooltip::Tooltip,
    v_flex,
};
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExtSettingsEvent {
    Change(ExtSettingItem),
}

impl EventEmitter<ExtSettingsEvent> for ExtSettings {}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SelectSettingOption {
    value: String,
    label_key: &'static str,
}

impl SelectSettingOption {
    fn new(option: ExtSettingOption) -> Self {
        Self {
            value: option.value.to_string(),
            label_key: option.label_key,
        }
    }

    fn label(&self, cx: &App) -> SharedString {
        cx.global::<I18n>().t(self.label_key).into()
    }
}

impl SelectItem for SelectSettingOption {
    type Value = String;

    fn title(&self) -> SharedString {
        self.value.clone().into()
    }

    fn render(&self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div().text_sm().child(self.label(cx))
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

struct SelectSettingState {
    item: ExtSettingItem,
    raw_options: Vec<ExtSettingOption>,
    options: Vec<SelectSettingOption>,
    picker: Entity<ListState<PickerListDelegate<SelectSettingOption>>>,
    picker_bounds: Bounds<Pixels>,
    picker_open: bool,
}

enum ExtSettingState {
    Select(SelectSettingState),
    Boolean(ExtSettingItem),
}

pub(crate) struct ExtSettings {
    settings: Vec<ExtSettingState>,
}

impl ExtSettings {
    pub(crate) fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            settings: Vec::new(),
        }
    }

    pub(crate) fn clear(&mut self, cx: &mut Context<Self>) {
        self.settings.clear();
        cx.notify();
    }

    pub(crate) fn set_items(
        &mut self,
        items: Vec<ExtSettingItem>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings = items
            .into_iter()
            .map(|item| match item.control.clone() {
                ExtSettingControl::Select { value, options } => {
                    let raw_options = options.clone();
                    let options = options
                        .into_iter()
                        .map(SelectSettingOption::new)
                        .collect::<Vec<_>>();
                    let sections = PickerSection::flat(options.iter().cloned());
                    let selected_ix =
                        PickerListDelegate::selected_index_for(&sections, Some(&value));
                    let state = cx.entity().downgrade();
                    let key = item.key;
                    let on_confirm = Rc::new(
                        move |option: SelectSettingOption, window: &mut Window, cx: &mut App| {
                            let state = state.clone();
                            window.defer(cx, move |window, cx| {
                                let _ = state.update(cx, |settings, cx| {
                                    settings.select_value(key, option.value.clone(), window, cx);
                                });
                            });
                        },
                    );
                    let state = cx.entity().downgrade();
                    let on_cancel = Rc::new(move |window: &mut Window, cx: &mut App| {
                        let _ = state.update(cx, |settings, cx| {
                            settings.close_picker(key, window, cx);
                        });
                    });
                    let empty_label = cx.global::<I18n>().t(item.label_key);
                    let picker = cx.new(|cx| {
                        let mut list = ListState::new(
                            PickerListDelegate::new(
                                sections.clone(),
                                false,
                                empty_label.into(),
                                on_confirm.clone(),
                                on_cancel.clone(),
                            ),
                            window,
                            cx,
                        );
                        list.set_selected_index(selected_ix, window, cx);
                        list
                    });
                    ExtSettingState::Select(SelectSettingState {
                        item,
                        raw_options,
                        options,
                        picker,
                        picker_bounds: Bounds::default(),
                        picker_open: false,
                    })
                }
                ExtSettingControl::Boolean(_) => ExtSettingState::Boolean(item),
            })
            .collect();
        cx.notify();
    }

    fn select_value(
        &mut self,
        key: &'static str,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(ExtSettingState::Select(setting)) = self
            .settings
            .iter_mut()
            .find(|setting| matches!(setting, ExtSettingState::Select(setting) if setting.item.key == key))
        else {
            return;
        };
        let sections = PickerSection::flat(setting.options.iter().cloned());
        let selected_ix = PickerListDelegate::selected_index_for(&sections, Some(&value));
        setting.item.control = ExtSettingControl::Select {
            value: value.clone(),
            options: setting.raw_options.clone(),
        };
        setting.picker.update(cx, |picker, cx| {
            picker.delegate_mut().set_sections(sections);
            picker.set_selected_index(selected_ix, window, cx);
        });
        setting.picker_open = false;
        cx.emit(ExtSettingsEvent::Change(setting.item.clone()));
        cx.notify();
    }

    fn close_picker(&mut self, key: &'static str, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(ExtSettingState::Select(setting)) = self
            .settings
            .iter_mut()
            .find(|setting| matches!(setting, ExtSettingState::Select(setting) if setting.item.key == key))
        else {
            return;
        };
        setting.picker_open = false;
        cx.notify();
    }

    fn toggle_picker(&mut self, key: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ExtSettingState::Select(setting)) = self
            .settings
            .iter_mut()
            .find(|setting| matches!(setting, ExtSettingState::Select(setting) if setting.item.key == key))
        else {
            return;
        };
        setting.picker_open = !setting.picker_open;
        if setting.picker_open {
            setting
                .picker
                .update(cx, |picker, cx| picker.focus(window, cx));
        }
        cx.notify();
    }

    fn toggle_boolean(&mut self, key: &'static str, cx: &mut Context<Self>) {
        let Some(ExtSettingState::Boolean(setting)) = self.settings.iter_mut().find(
            |setting| matches!(setting, ExtSettingState::Boolean(setting) if setting.key == key),
        ) else {
            return;
        };
        let ExtSettingControl::Boolean(value) = &mut setting.control else {
            return;
        };
        *value = !(*value);
        cx.emit(ExtSettingsEvent::Change(setting.clone()));
        cx.notify();
    }

    fn setting_tooltip(
        item: &ExtSettingItem,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyView> {
        let tooltip_key = item.tooltip?;
        let markdown = cx.global::<I18n>().t(tooltip_key);
        Some(
            Tooltip::element(move |_window, _cx| {
                div().w(px(520.)).child(
                    v_flex()
                        .p_2()
                        .h(px(360.))
                        .overflow_hidden()
                        .overflow_y_scrollbar()
                        .child(
                            div().child(
                                TextView::markdown(
                                    SharedString::from(format!("ext-setting-tooltip-{index}")),
                                    markdown.clone(),
                                )
                                .selectable(true),
                            ),
                        ),
                )
            })
            .build(window, cx),
        )
    }
}

impl Render for ExtSettings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let controls = self
            .settings
            .iter()
            .enumerate()
            .map(|(index, setting)| match setting {
                ExtSettingState::Boolean(setting) => {
                    let ExtSettingControl::Boolean(value) = setting.control else {
                        unreachable!("boolean setting state always carries boolean control");
                    };
                    let button =
                        Button::new(SharedString::from(format!("ext-setting-boolean-{index}")))
                            .ghost()
                            .selected(value)
                            .rounded(px(8.))
                            .small()
                            .label(cx.global::<I18n>().t(setting.label_key))
                            .on_click({
                                let key = setting.key;
                                cx.listener(move |settings, _event, _window, cx| {
                                    settings.toggle_boolean(key, cx);
                                })
                            });
                    let mut container = div()
                        .id(SharedString::from(format!(
                            "ext-setting-boolean-wrapper-{index}"
                        )))
                        .child(button);
                    if let Some(tooltip) = Self::setting_tooltip(setting, index, _window, cx) {
                        container = container.hoverable_tooltip(move |_, _| tooltip.clone());
                    }
                    container.into_any_element()
                }
                ExtSettingState::Select(setting) => {
                    let selected_value = match &setting.item.control {
                        ExtSettingControl::Select { value, .. } => value.as_str(),
                        ExtSettingControl::Boolean(_) => {
                            unreachable!("select setting state always carries select control")
                        }
                    };
                    let selected_label = setting
                        .options
                        .iter()
                        .find(|option| option.value == selected_value)
                        .map(|option| option.label(cx))
                        .unwrap_or_else(|| cx.global::<I18n>().t(setting.item.label_key).into());
                    let picker = setting.picker.clone();
                    let bounds = setting.picker_bounds;
                    let key = setting.item.key;
                    let trigger = PickerTrigger::new(
                        SharedString::from(format!("ext-setting-select-trigger-{index}")),
                        selected_label,
                        {
                            let key = setting.item.key;
                            cx.listener(move |settings, _event, window, cx| {
                                settings.toggle_picker(key, window, cx);
                            })
                        },
                        {
                            let state = cx.entity();
                            let key = setting.item.key;
                            move |next_bounds, cx| {
                                state.update(cx, |settings, _| {
                                    if let Some(ExtSettingState::Select(setting)) =
                                        settings.settings.iter_mut().find(|setting| {
                                            matches!(
                                                setting,
                                                ExtSettingState::Select(setting)
                                                    if setting.item.key == key
                                            )
                                        })
                                    {
                                        setting.picker_bounds = next_bounds;
                                    }
                                })
                            }
                        },
                    )
                    .selected(false)
                    .open(setting.picker_open);
                    let on_mouse_down_out =
                        cx.listener(move |settings, event: &MouseDownEvent, window, cx| {
                            let Some(ExtSettingState::Select(setting)) =
                                settings.settings.iter().find(|setting| {
                                    matches!(
                                        setting,
                                        ExtSettingState::Select(setting) if setting.item.key == key
                                    )
                                })
                            else {
                                return;
                            };
                            if setting.picker_bounds.contains(&event.position) {
                                return;
                            }
                            settings.close_picker(key, window, cx);
                        });

                    let mut container = div()
                        .id(SharedString::from(format!(
                            "ext-setting-select-wrapper-{index}"
                        )))
                        .child(trigger);
                    if let Some(tooltip) = Self::setting_tooltip(&setting.item, index, _window, cx)
                    {
                        container = container.hoverable_tooltip(move |_, _| tooltip.clone());
                    }
                    container
                        .when(setting.picker_open, |this| {
                            this.child(render_picker_popover(
                                bounds,
                                picker,
                                PickerPopoverOptions {
                                    min_width: Some(px(150.)),
                                    ..Default::default()
                                },
                                on_mouse_down_out,
                                cx,
                            ))
                        })
                        .into_any_element()
                }
            })
            .collect::<Vec<_>>();

        if controls.is_empty() {
            return div();
        }

        h_flex().items_center().gap_1().children(controls)
    }
}

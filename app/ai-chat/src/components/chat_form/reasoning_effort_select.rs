use super::picker::{
    PickerListDelegate, PickerPopoverOptions, PickerSection, PickerTrigger, render_picker_popover,
};
use crate::{
    i18n::I18n,
    llm::ExtSettingOption,
};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{list::ListState, select::SelectItem};
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ReasoningEffortSelectEvent {
    Change(String),
}

impl EventEmitter<ReasoningEffortSelectEvent> for ReasoningEffortSelect {}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ReasoningEffortOption {
    value: String,
    label_key: &'static str,
}

impl ReasoningEffortOption {
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

impl SelectItem for ReasoningEffortOption {
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

pub(crate) struct ReasoningEffortSelect {
    selected_value: Option<String>,
    options: Vec<ReasoningEffortOption>,
    picker: Entity<ListState<PickerListDelegate<ReasoningEffortOption>>>,
    picker_bounds: Bounds<Pixels>,
    picker_open: bool,
}

impl ReasoningEffortSelect {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.entity().downgrade();
        let on_confirm = Rc::new(
            move |option: ReasoningEffortOption, window: &mut Window, cx: &mut App| {
                let state = state.clone();
                window.defer(cx, move |window, cx| {
                    let _ = state.update(cx, |select, cx| {
                        select.select(option.value.clone(), window, cx);
                    });
                });
            },
        );
        let state = cx.entity().downgrade();
        let on_cancel = Rc::new(move |window: &mut Window, cx: &mut App| {
            let _ = state.update(cx, |select, cx| {
                select.close(window, cx);
            });
        });
        let empty_label = cx.global::<I18n>().t("field-reasoning-effort");
        let picker = cx.new(|cx| {
            ListState::new(
                PickerListDelegate::new(
                    Vec::new(),
                    false,
                    empty_label.into(),
                    on_confirm.clone(),
                    on_cancel.clone(),
                ),
                window,
                cx,
            )
        });

        Self {
            selected_value: None,
            options: Vec::new(),
            picker,
            picker_bounds: Bounds::default(),
            picker_open: false,
        }
    }

    pub(crate) fn configure(
        &mut self,
        options: Vec<ExtSettingOption>,
        selected_value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.options = options.into_iter().map(ReasoningEffortOption::new).collect();
        self.selected_value = Some(selected_value.clone());
        let sections = PickerSection::flat(self.options.iter().cloned());
        let selected_ix = PickerListDelegate::selected_index_for(&sections, Some(&selected_value));
        self.picker.update(cx, |picker, cx| {
            picker.delegate_mut().set_sections(sections);
            picker.set_selected_index(selected_ix, window, cx);
        });
        cx.notify();
    }

    pub(crate) fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_value = None;
        self.options.clear();
        self.picker_open = false;
        self.picker.update(cx, |picker, cx| {
            picker.delegate_mut().set_sections(Vec::new());
            picker.set_selected_index(None, window, cx);
        });
        cx.notify();
    }

    fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.options.is_empty() {
            return;
        }
        self.picker_open = true;
        self.picker
            .update(cx, |picker, cx| picker.focus(window, cx));
        cx.notify();
    }

    fn close(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.picker_open = false;
        cx.notify();
    }

    fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.picker_open {
            self.close(window, cx);
        } else {
            self.open(window, cx);
        }
    }

    fn select(&mut self, value: String, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_value = Some(value.clone());
        let sections = PickerSection::flat(self.options.iter().cloned());
        let selected_ix = PickerListDelegate::selected_index_for(&sections, Some(&value));
        self.picker.update(cx, |picker, cx| {
            picker.delegate_mut().set_sections(sections);
            picker.set_selected_index(selected_ix, window, cx);
        });
        cx.emit(ReasoningEffortSelectEvent::Change(value));
        self.close(window, cx);
    }
}

impl Render for ReasoningEffortSelect {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(selected_value) = self.selected_value.as_ref() else {
            return div();
        };
        let Some(selected_option) = self
            .options
            .iter()
            .find(|option| option.value == *selected_value)
            .cloned()
        else {
            return div();
        };
        let picker = self.picker.clone();
        let bounds = self.picker_bounds;
        let on_mouse_down_out = cx.listener(|select, event: &MouseDownEvent, window, cx| {
            if select.picker_bounds.contains(&event.position) {
                return;
            }
            select.close(window, cx);
        });

        div()
            .child(
                PickerTrigger::new(
                    "reasoning-effort-picker-trigger",
                    selected_option.label(cx),
                    cx.listener(|select, _event, window, cx| {
                        select.toggle(window, cx);
                    }),
                    {
                        let state = cx.entity();
                        move |next_bounds, cx| {
                            state.update(cx, |select, _| {
                                select.picker_bounds = next_bounds;
                            })
                        }
                    },
                )
                .selected(false)
                .open(self.picker_open),
            )
            .when(self.picker_open, |this| {
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
    }
}

use super::picker::{
    PickerListDelegate, PickerPopoverOptions, PickerSection, PickerTrigger, render_picker_popover,
};
use crate::{extensions::ExtensionConfig, i18n::I18n};
use gpui::{ParentElement as _, Styled as _, prelude::FluentBuilder as _, *};
use gpui_component::{h_flex, label::Label, list::ListState, select::SelectItem};
use std::rc::Rc;

#[derive(Clone, Debug)]
struct ExtensionOption {
    config: ExtensionConfig,
}

impl ExtensionOption {
    fn new(config: ExtensionConfig) -> Self {
        Self { config }
    }
}

impl SelectItem for ExtensionOption {
    type Value = String;

    fn title(&self) -> SharedString {
        self.config.name.clone().into()
    }

    fn render(&self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let label = if let Some(description) = self.config.description.clone() {
            Label::new(self.config.name.clone())
                .text_sm()
                .secondary(description)
        } else {
            Label::new(self.config.name.clone()).text_sm()
        };

        h_flex()
            .w_full()
            .items_center()
            .gap_3()
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .truncate()
                    .child(label),
            )
    }

    fn value(&self) -> &Self::Value {
        &self.config.name
    }

    fn matches(&self, query: &str) -> bool {
        let query = query.to_lowercase();
        self.config.name.to_lowercase().contains(&query)
            || self
                .config
                .description
                .as_ref()
                .is_some_and(|description| description.to_lowercase().contains(&query))
    }
}

pub(crate) struct ExtensionSelect {
    items: Vec<ExtensionOption>,
    selected_name: Option<String>,
    picker: Entity<ListState<PickerListDelegate<ExtensionOption>>>,
    picker_bounds: Bounds<Pixels>,
    picker_open: bool,
}

impl ExtensionSelect {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.entity().downgrade();
        let on_confirm = Rc::new(move |item: ExtensionOption, window: &mut Window, cx: &mut App| {
            let _ = state.update(cx, |select, cx| {
                select.select(item.value().clone(), window, cx);
            });
        });
        let state = cx.entity().downgrade();
        let on_cancel = Rc::new(move |window: &mut Window, cx: &mut App| {
            let _ = state.update(cx, |select, cx| {
                select.close(window, cx);
            });
        });
        let empty_label = cx.global::<I18n>().t("empty-extension-picker");
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
            .searchable(true)
        });

        let mut this = Self {
            items: Vec::new(),
            selected_name: None,
            picker,
            picker_bounds: Bounds::default(),
            picker_open: false,
        };
        this.sync_items(window, cx);
        this
    }

    pub(crate) fn selected_value(&self) -> Option<&String> {
        self.selected_name.as_ref()
    }

    fn sync_items(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let extension_container = cx.global::<crate::extensions::ExtensionContainer>();
        let mut items = extension_container
            .get_all_config()
            .into_iter()
            .map(ExtensionOption::new)
            .collect::<Vec<_>>();
        items.sort_by(|left, right| left.config.name.cmp(&right.config.name));
        self.items = items;

        let sections = PickerSection::flat(self.items.clone());
        let selected_ix = PickerListDelegate::selected_index_for(&sections, self.selected_name.as_ref());
        self.picker.update(cx, |picker, cx| {
            picker.delegate_mut().set_sections(sections);
            picker.set_selected_index(selected_ix, window, cx);
        });
        cx.notify();
    }

    fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.picker_open = true;
        self.picker.update(cx, |picker, cx| picker.focus(window, cx));
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

    fn select(&mut self, selected_name: String, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_name = Some(selected_name);
        self.close(window, cx);
    }

    fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_name = None;
        let sections = PickerSection::flat(self.items.clone());
        self.picker.update(cx, |picker, cx| {
            picker.delegate_mut().set_sections(sections);
            picker.set_selected_index(None, window, cx);
        });
        cx.notify();
    }
}

impl Render for ExtensionSelect {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title: AnyElement = self
            .selected_name
            .as_ref()
            .map(|name| div().child(name.clone()).into_any_element())
            .unwrap_or_else(|| div().child(cx.global::<I18n>().t("field-extension")).into_any_element());
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
                    "extension-picker-trigger",
                    title,
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
                .selected(self.selected_name.is_some())
                .open(self.picker_open)
                .cleanable(cx.listener(|select, _event, window, cx| {
                    select.clear(window, cx);
                })),
            )
            .when(self.picker_open, |this| {
                this.child(render_picker_popover(
                    bounds,
                    picker,
                    PickerPopoverOptions {
                        min_width: Some(px(260.)),
                        search_placeholder: Some(cx.global::<I18n>().t("field-search-extension").into()),
                        ..Default::default()
                    },
                    on_mouse_down_out,
                    cx,
                ))
            })
    }
}

mod effort_select;
mod model_select;
mod picker;
mod preview_models;
mod thinking_effort;

use crate::foundation::{self, assets::IconName};
use effort_select::{EffortOption, effort_sections};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Icon, Sizable, box_shadow,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    list::ListState,
    v_flex,
};
use model_select::{ModelOption, model_sections};
use picker::PickerListDelegate;
use preview_models::{PreviewModel, preview_model, preview_models};
use std::rc::Rc;
use thinking_effort::ThinkingEffort;

pub(super) const COMPOSER_BUTTON_SIZE: f32 = 28.;
pub(super) const COMPOSER_BUTTON_ICON_SIZE: f32 = 18.;
pub(super) const COMPOSER_BUTTON_RADIUS: f32 = 999.;

#[derive(Clone)]
pub(crate) enum ChatFormEvent {
    AddRequested,
    SendRequested,
}

impl EventEmitter<ChatFormEvent> for ChatForm {}

pub(crate) struct ChatForm {
    draft_text: SharedString,
    selected_model_index: usize,
    selected_effort: Option<ThinkingEffort>,
    effort_picker_open: bool,
    effort_picker: Entity<ListState<PickerListDelegate<EffortOption>>>,
    model_picker_open: bool,
    model_picker: Entity<ListState<PickerListDelegate<ModelOption>>>,
}

impl ChatForm {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let selected_model_index = 0;
        let selected_effort = preview_model(selected_model_index).computed_default_effort();
        let state = cx.entity().downgrade();
        let effort_sections = {
            let i18n = cx.global::<foundation::I18n>();
            effort_sections(preview_model(selected_model_index), i18n)
        };
        let effort_selected_ix =
            PickerListDelegate::selected_index_for(&effort_sections, selected_effort.as_ref());
        let effort_empty_label = cx
            .global::<foundation::I18n>()
            .t("chat-form-effort-empty")
            .into();
        let effort_confirm = Rc::new({
            let state = state.clone();
            move |option: EffortOption, window: &mut Window, cx: &mut App| {
                let _ = state.update(cx, |form, cx| {
                    form.select_effort(option.effort, window, cx);
                });
            }
        });
        let effort_cancel = Rc::new({
            let state = state.clone();
            move |window: &mut Window, cx: &mut App| {
                let _ = state.update(cx, |form, cx| {
                    form.set_effort_picker_open(false, window, cx);
                });
            }
        });
        let effort_picker = cx.new(|cx| {
            let mut picker = ListState::new(
                PickerListDelegate::new(
                    effort_sections,
                    selected_effort,
                    effort_empty_label,
                    effort_confirm,
                    effort_cancel,
                ),
                window,
                cx,
            );
            picker.set_selected_index(effort_selected_ix, window, cx);
            picker
        });

        let model_sections = model_sections();
        let model_selected_ix =
            PickerListDelegate::selected_index_for(&model_sections, Some(&selected_model_index));
        let model_empty_label = cx
            .global::<foundation::I18n>()
            .t("chat-form-model-empty")
            .into();
        let model_confirm = Rc::new({
            let state = state.clone();
            move |option: ModelOption, window: &mut Window, cx: &mut App| {
                let _ = state.update(cx, |form, cx| {
                    form.select_model(option.index, window, cx);
                });
            }
        });
        let model_cancel = Rc::new({
            let state = state.clone();
            move |window: &mut Window, cx: &mut App| {
                let _ = state.update(cx, |form, cx| {
                    form.set_model_picker_open(false, window, cx);
                });
            }
        });
        let model_picker = cx.new(|cx| {
            let mut picker = ListState::new(
                PickerListDelegate::new(
                    model_sections,
                    Some(selected_model_index),
                    model_empty_label,
                    model_confirm,
                    model_cancel,
                ),
                window,
                cx,
            )
            .searchable(true);
            picker.set_selected_index(model_selected_ix, window, cx);
            picker
        });

        Self {
            draft_text: SharedString::default(),
            selected_model_index,
            selected_effort,
            effort_picker_open: false,
            effort_picker,
            model_picker_open: false,
            model_picker,
        }
    }

    fn selected_model(&self) -> &'static PreviewModel {
        preview_model(self.selected_model_index)
    }

    fn set_effort_picker_open(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.effort_picker_open = open;
        if open {
            self.model_picker_open = false;
            self.effort_picker
                .update(cx, |picker, cx| picker.focus(window, cx));
        }
        cx.notify();
    }

    fn set_model_picker_open(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.model_picker_open = open;
        if open {
            self.effort_picker_open = false;
            self.model_picker
                .update(cx, |picker, cx| picker.focus(window, cx));
        }
        cx.notify();
    }

    fn select_effort(
        &mut self,
        effort: ThinkingEffort,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_effort = Some(effort);
        self.sync_effort_picker(window, cx);
        self.set_effort_picker_open(false, window, cx);
    }

    fn select_model(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_model_index = index.min(preview_models().len().saturating_sub(1));
        let model = self.selected_model();
        let selected_is_valid = self
            .selected_effort
            .is_some_and(|effort| model.selectable_efforts().contains(&effort));
        if !selected_is_valid {
            self.selected_effort = model.computed_default_effort();
        }
        self.sync_model_picker(window, cx);
        self.sync_effort_picker(window, cx);
        self.set_model_picker_open(false, window, cx);
    }

    fn sync_effort_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let sections = {
            let i18n = cx.global::<foundation::I18n>();
            effort_sections(self.selected_model(), i18n)
        };
        let selected_value = self.selected_effort;

        self.effort_picker.update(cx, |picker, cx| {
            picker.delegate_mut().set_sections(sections);
            picker.delegate_mut().set_selected_value(selected_value);
            let selected_ix = picker.delegate().selected_index();
            picker.set_selected_index(selected_ix, window, cx);
        });
    }

    fn sync_model_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let sections = model_sections();
        let selected_value = self.selected_model_index;

        self.model_picker.update(cx, |picker, cx| {
            picker.delegate_mut().set_sections(sections);
            picker
                .delegate_mut()
                .set_selected_value(Some(selected_value));
            let selected_ix = picker.delegate().selected_index();
            picker.set_selected_index(selected_ix, window, cx);
        });
    }
}

impl Render for ChatForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let placeholder = cx.global::<foundation::I18n>().t("chat-form-placeholder");
        let add_tooltip = cx.global::<foundation::I18n>().t("chat-form-add-tooltip");
        let send_tooltip = cx.global::<foundation::I18n>().t("chat-form-send-tooltip");
        let is_draft_empty = self.draft_text.as_str().trim().is_empty();
        let draft_text = self.draft_text.clone();

        v_flex()
            .id("ai-chat2-chat-form-preview")
            .w_full()
            .relative()
            .gap(px(2.))
            .p(px(8.))
            .rounded(px(25.))
            .border_1()
            .border_color(cx.theme().border.opacity(0.10))
            .bg(cx.theme().popover.opacity(0.90))
            .text_color(cx.theme().popover_foreground)
            .when(cx.theme().shadow, |this| {
                this.shadow(vec![box_shadow(
                    0.,
                    4.,
                    16.,
                    0.,
                    cx.theme().foreground.opacity(0.05),
                )])
            })
            .child(
                div()
                    .w_full()
                    .min_h(px(56.))
                    .px_3()
                    .pt(px(6.))
                    .when(is_draft_empty, |this| {
                        this.child(
                            Label::new(placeholder.clone())
                                .text_base()
                                .text_color(cx.theme().muted_foreground.opacity(0.72)),
                        )
                    })
                    .when(!is_draft_empty, |this| {
                        this.child(Label::new(draft_text).text_base())
                    }),
            )
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .min_h(px(COMPOSER_BUTTON_SIZE))
                    .child(
                        h_flex()
                            .items_center()
                            .gap(px(5.))
                            .min_w_0()
                            .child(
                                Button::new("chat-form-add")
                                    .ghost()
                                    .with_size(px(COMPOSER_BUTTON_SIZE))
                                    .size(px(COMPOSER_BUTTON_SIZE))
                                    .p(px(0.))
                                    .rounded(px(COMPOSER_BUTTON_RADIUS))
                                    .child(
                                        Icon::new(IconName::Plus)
                                            .with_size(px(COMPOSER_BUTTON_ICON_SIZE)),
                                    )
                                    .tooltip(add_tooltip)
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        cx.emit(ChatFormEvent::AddRequested);
                                    })),
                            )
                            .child(self.render_effort_selector(cx)),
                    )
                    .child(div().flex_1().min_w_0())
                    .child(
                        h_flex()
                            .items_center()
                            .gap(px(5.))
                            .min_w_0()
                            .child(self.render_model_selector(cx))
                            .child(
                                Button::new("chat-form-send")
                                    .primary()
                                    .with_size(px(COMPOSER_BUTTON_SIZE))
                                    .size(px(COMPOSER_BUTTON_SIZE))
                                    .p(px(0.))
                                    .rounded(px(COMPOSER_BUTTON_RADIUS))
                                    .child(
                                        Icon::new(IconName::Send)
                                            .with_size(px(COMPOSER_BUTTON_ICON_SIZE)),
                                    )
                                    .tooltip(send_tooltip)
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        cx.emit(ChatFormEvent::SendRequested);
                                    })),
                            ),
                    ),
            )
    }
}

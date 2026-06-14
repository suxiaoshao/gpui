mod attachment_flow;
mod attachment_views;
mod attachments;
mod composer_editor;
mod effort_select;
mod image_preview;
mod model_select;
mod thinking_effort;

use crate::{
    components::{
        model_picker::{ModelOption, model_sections},
        picker::PickerListDelegate,
    },
    foundation::{self, assets::IconName},
    state,
    state::attachments::ComposerAttachment,
    state::providers::{ProviderModelChoice, ProviderModelKey},
};
use ai_chat_core::{ReasoningSelectionSnapshot, TokenBudgetSelectionMode};
use composer_editor::{ComposerEditor, ComposerEditorEvent, ComposerSnapshot};
use effort_select::{EffortOption, effort_sections};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable, StyledExt, box_shadow,
    button::{Button, ButtonVariants},
    h_flex,
    input::{InputEvent, InputState, NumberInputEvent, StepAction},
    label::Label,
    list::ListState,
    v_flex,
};
use std::{path::Path, rc::Rc};
use thinking_effort::{
    computed_default_reasoning_selection, custom_token_budget_value, reasoning_selection_is_valid,
    reasoning_selections, token_budget_bounds,
};

pub(super) const COMPOSER_BUTTON_SIZE: f32 = 28.;
pub(super) const COMPOSER_BUTTON_ICON_SIZE: f32 = 18.;
pub(super) const COMPOSER_BUTTON_RADIUS: f32 = 999.;
pub(crate) const COMPOSER_EDITOR_KEY_CONTEXT: &str = composer_editor::KEY_CONTEXT;
const COMPOSER_INPUT_HORIZONTAL_PADDING: f32 = 12.;
const COMPOSER_INPUT_TOP_PADDING: f32 = 12.;
const COMPOSER_INPUT_BOTTOM_MARGIN: f32 = 4.;
const COMPOSER_FOOTER_HORIZONTAL_PADDING: f32 = 8.;
const COMPOSER_FOOTER_BOTTOM_MARGIN: f32 = 8.;

#[allow(dead_code, clippy::enum_variant_names)]
#[derive(Clone)]
pub(crate) enum ChatFormEvent {
    AddRequested,
    SendRequested(Box<ChatFormSubmit>),
    StopRequested,
}

impl EventEmitter<ChatFormEvent> for ChatForm {}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ChatFormSubmit {
    pub(crate) composer: ComposerSnapshot,
    pub(crate) provider_model: ProviderModelChoice,
    pub(crate) reasoning_selection: Option<ReasoningSelectionSnapshot>,
}

#[derive(Debug, PartialEq)]
enum ChatFormPrimaryButtonAction {
    Send(Box<ChatFormSubmit>),
    Stop,
}

pub(crate) struct ChatForm {
    composer: Entity<ComposerEditor>,
    model_choices: Result<Vec<ProviderModelChoice>, SharedString>,
    selected_model_key: Option<ProviderModelKey>,
    selected_reasoning_selection: Option<ReasoningSelectionSnapshot>,
    token_budget_input: Entity<InputState>,
    effort_picker_open: bool,
    effort_picker: Entity<ListState<PickerListDelegate<EffortOption>>>,
    model_picker_open: bool,
    model_picker: Entity<ListState<PickerListDelegate<ModelOption>>>,
    attachment_menu_open: bool,
    attachments: Vec<ComposerAttachment>,
    next_attachment_id: u64,
    preview_attachment: Option<ComposerAttachment>,
    agent_running: bool,
    _subscriptions: Vec<Subscription>,
}

pub(crate) fn init(cx: &mut App) {
    composer_editor::init(cx);
}

impl ChatForm {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let placeholder = cx.global::<foundation::I18n>().t("chat-form-placeholder");
        let composer = cx.new(|cx| ComposerEditor::new(placeholder.clone(), window, cx));
        composer.update(cx, |composer, cx| composer.focus(window, cx));
        let model_choices = load_model_choices(cx);
        let selected_model_key = model_choices
            .as_ref()
            .ok()
            .and_then(|choices| choices.first().map(ProviderModelChoice::key));
        let selected_reasoning_selection =
            selected_model_choice_in(&model_choices, selected_model_key.as_ref()).and_then(
                |choice| {
                    computed_default_reasoning_selection(choice.capabilities.reasoning.as_ref())
                },
            );
        let initial_token_budget =
            initial_token_budget_value(&model_choices, selected_model_key.as_ref());
        let token_budget_input = cx
            .new(|cx| InputState::new(window, cx).default_value(initial_token_budget.to_string()));
        let state = cx.entity().downgrade();
        let effort_sections = {
            let i18n = cx.global::<foundation::I18n>();
            effort_sections(
                selected_model_choice_in(&model_choices, selected_model_key.as_ref())
                    .map(|choice| &choice.capabilities),
                i18n,
            )
        };
        let effort_selected_ix = PickerListDelegate::selected_index_for(
            &effort_sections,
            selected_reasoning_selection.as_ref(),
        );
        let effort_empty_label = cx
            .global::<foundation::I18n>()
            .t("chat-form-effort-empty")
            .into();
        let effort_confirm = Rc::new({
            let state = state.clone();
            move |option: EffortOption, window: &mut Window, cx: &mut App| {
                let _ = state.update(cx, |form, cx| {
                    form.select_effort(option.selection().clone(), window, cx);
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
                    selected_reasoning_selection.clone(),
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

        let model_sections =
            model_sections(model_choices.as_ref().map(Vec::as_slice).unwrap_or(&[]));
        let model_selected_ix =
            PickerListDelegate::selected_index_for(&model_sections, selected_model_key.as_ref());
        let model_empty_label = model_empty_label(&model_choices, cx.global::<foundation::I18n>());
        let model_confirm = Rc::new({
            let state = state.clone();
            move |option: ModelOption, window: &mut Window, cx: &mut App| {
                let _ = state.update(cx, |form, cx| {
                    form.select_model(option.key(), window, cx);
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
                    selected_model_key.clone(),
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

        let composer_subscription = cx.subscribe_in(
            &composer,
            window,
            |form, _composer, event: &ComposerEditorEvent, window, cx| match event {
                ComposerEditorEvent::Changed => {
                    cx.notify();
                }
                ComposerEditorEvent::PasteAttachmentRequested(item) => {
                    form.add_attachments_from_clipboard(item.clone(), window, cx);
                }
                ComposerEditorEvent::SubmitRequested(snapshot) => {
                    if let Some(submit) = form.submit_snapshot(snapshot.clone(), window, cx) {
                        cx.emit(ChatFormEvent::SendRequested(Box::new(submit)));
                    }
                }
            },
        );
        let token_budget_change_subscription = cx.subscribe_in(
            &token_budget_input,
            window,
            |form, input, event: &InputEvent, window, cx| {
                if !matches!(event, InputEvent::Change) {
                    return;
                }
                let Ok(value) = input.read(cx).value().as_ref().parse::<u32>() else {
                    return;
                };
                form.apply_custom_token_budget(value, input, window, cx);
            },
        );
        let token_budget_step_subscription = cx.subscribe_in(
            &token_budget_input,
            window,
            |form, input, event: &NumberInputEvent, window, cx| {
                let NumberInputEvent::Step(action) = event;
                let bounds = form.current_token_budget_bounds();
                let step = bounds.map(|bounds| bounds.step()).unwrap_or(1024);
                let current = input
                    .read(cx)
                    .value()
                    .as_ref()
                    .parse::<u32>()
                    .ok()
                    .or_else(|| bounds.map(|bounds| bounds.default_value))
                    .unwrap_or(step);
                let next = match *action {
                    StepAction::Increment => current.saturating_add(step),
                    StepAction::Decrement => current.saturating_sub(step),
                };
                form.apply_custom_token_budget(next, input, window, cx);
            },
        );
        let provider_catalog = state::providers::catalog(cx);
        let provider_catalog_subscription = cx.subscribe_in(
            &provider_catalog,
            window,
            |form, _catalog, event: &state::providers::ProviderCatalogEvent, window, cx| match event
            {
                state::providers::ProviderCatalogEvent::Changed(_) => {
                    form.reload_model_choices(window, cx);
                }
            },
        );

        Self {
            composer,
            model_choices,
            selected_model_key,
            selected_reasoning_selection,
            token_budget_input,
            effort_picker_open: false,
            effort_picker,
            model_picker_open: false,
            model_picker,
            attachment_menu_open: false,
            attachments: Vec::new(),
            next_attachment_id: 1,
            preview_attachment: None,
            agent_running: false,
            _subscriptions: vec![
                composer_subscription,
                token_budget_change_subscription,
                token_budget_step_subscription,
                provider_catalog_subscription,
            ],
        }
    }

    fn selected_model_choice(&self) -> Option<&ProviderModelChoice> {
        selected_model_choice_in(&self.model_choices, self.selected_model_key.as_ref())
    }

    pub(crate) fn focus_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.effort_picker_open {
            self.effort_picker
                .update(cx, |picker, cx| picker.focus(window, cx));
            return;
        }

        if self.model_picker_open {
            self.model_picker
                .update(cx, |picker, cx| picker.focus(window, cx));
            return;
        }

        self.composer
            .update(cx, |composer, cx| composer.focus(window, cx));
    }

    pub(crate) fn refresh_skill_catalog(
        &mut self,
        project_root: Option<&Path>,
        cx: &mut Context<Self>,
    ) {
        self.composer.update(cx, |composer, cx| {
            composer.refresh_skill_catalog(project_root, cx)
        });
    }

    pub(crate) fn set_agent_running(&mut self, running: bool, cx: &mut Context<Self>) {
        if self.agent_running == running {
            return;
        }
        self.agent_running = running;
        cx.notify();
    }

    pub(crate) fn clear_after_submit(&mut self, cx: &mut Context<Self>) {
        self.composer.update(cx, |composer, cx| composer.clear(cx));
        self.attachments.clear();
        self.preview_attachment = None;
        cx.notify();
    }

    fn set_effort_picker_open(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        if open && !self.has_effort_options() {
            self.effort_picker_open = false;
            cx.notify();
            return;
        }
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
            self.reload_model_choices(window, cx);
            self.effort_picker_open = false;
            self.model_picker
                .update(cx, |picker, cx| picker.focus(window, cx));
        }
        cx.notify();
    }

    fn select_effort(
        &mut self,
        selection: ReasoningSelectionSnapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_reasoning_selection = Some(selection);
        self.sync_token_budget_input(window, cx);
        self.set_effort_picker_open(false, window, cx);
    }

    fn select_model(&mut self, key: ProviderModelKey, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_model_key = Some(key);
        self.selected_reasoning_selection = self.selected_model_choice().and_then(|choice| {
            computed_default_reasoning_selection(choice.capabilities.reasoning.as_ref())
        });
        self.sync_token_budget_input(window, cx);
        self.sync_effort_picker(window, cx);
        self.set_model_picker_open(false, window, cx);
    }

    fn sync_effort_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let sections = {
            let i18n = cx.global::<foundation::I18n>();
            effort_sections(
                self.selected_model_choice()
                    .map(|choice| &choice.capabilities),
                i18n,
            )
        };
        let selected_value = self.selected_reasoning_selection.clone();

        self.effort_picker.update(cx, |picker, cx| {
            picker.delegate_mut().set_sections(sections);
            picker.delegate_mut().set_selected_value(selected_value);
            let selected_ix = picker.delegate().selected_index();
            picker.set_selected_index(selected_ix, window, cx);
        });
    }

    fn sync_model_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let sections = model_sections(
            self.model_choices
                .as_ref()
                .map(Vec::as_slice)
                .unwrap_or(&[]),
        );
        let selected_value = self.selected_model_key.clone();
        let empty_label = model_empty_label(&self.model_choices, cx.global::<foundation::I18n>());

        self.model_picker.update(cx, |picker, cx| {
            picker.delegate_mut().set_sections(sections);
            picker.delegate_mut().set_selected_value(selected_value);
            picker.delegate_mut().set_empty_label(empty_label);
            let selected_ix = picker.delegate().selected_index();
            picker.set_selected_index(selected_ix, window, cx);
        });
    }

    fn reload_model_choices(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_loaded_model_choices(load_model_choices(cx), window, cx);
    }

    fn apply_loaded_model_choices(
        &mut self,
        model_choices: Result<Vec<ProviderModelChoice>, SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.model_choices = model_choices;
        match self.model_choices.as_ref() {
            Ok(choices) => {
                let selected_still_exists = self
                    .selected_model_key
                    .as_ref()
                    .is_some_and(|key| choices.iter().any(|choice| &choice.key() == key));
                if !selected_still_exists {
                    self.selected_model_key = choices.first().map(ProviderModelChoice::key);
                }
            }
            Err(_) => {
                self.selected_model_key = None;
            }
        }
        let selected_is_valid = self.selected_model_choice().is_some_and(|choice| {
            self.selected_reasoning_selection
                .as_ref()
                .is_some_and(|selection| {
                    reasoning_selection_is_valid(choice.capabilities.reasoning.as_ref(), selection)
                })
        });
        if !selected_is_valid {
            self.selected_reasoning_selection = self.selected_model_choice().and_then(|choice| {
                computed_default_reasoning_selection(choice.capabilities.reasoning.as_ref())
            });
        }
        self.sync_model_picker(window, cx);
        self.sync_token_budget_input(window, cx);
        self.sync_effort_picker(window, cx);
    }

    fn revalidate_selected_model_for_submit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<ProviderModelChoice> {
        let selected_key = self.selected_model_key.clone()?;
        let model_choices = load_model_choices(cx);
        let selected = selected_model_choice_in(&model_choices, Some(&selected_key)).cloned();
        self.apply_loaded_model_choices(model_choices, window, cx);
        selected
    }

    fn current_token_budget_bounds(&self) -> Option<thinking_effort::TokenBudgetBounds> {
        token_budget_bounds(
            self.selected_model_choice()
                .and_then(|choice| choice.capabilities.reasoning.as_ref()),
        )
    }

    fn sync_token_budget_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bounds) = self.current_token_budget_bounds() else {
            return;
        };
        let value = custom_token_budget_value(self.selected_reasoning_selection.as_ref())
            .map(|value| bounds.clamp(value))
            .unwrap_or(bounds.default_value);
        self.token_budget_input.update(cx, |input, cx| {
            if input.value().as_ref() != value.to_string() {
                input.set_value(value.to_string(), window, cx);
            }
        });
    }

    fn apply_custom_token_budget(
        &mut self,
        value: u32,
        input: &Entity<InputState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(bounds) = self.current_token_budget_bounds() else {
            return;
        };
        let value = bounds.clamp(value);
        input.update(cx, |input, cx| {
            if input.value().as_ref() != value.to_string() {
                input.set_value(value.to_string(), window, cx);
            }
        });
        self.selected_reasoning_selection = Some(ReasoningSelectionSnapshot::TokenBudget {
            mode: TokenBudgetSelectionMode::Custom,
            value: Some(value),
        });
        self.sync_effort_picker(window, cx);
        cx.notify();
    }

    fn can_send(&self, cx: &Context<Self>) -> bool {
        let composer_has_content = self.composer.read(cx).can_submit();
        !self.agent_running
            && (composer_has_content || !self.attachments.is_empty())
            && self.selected_model_choice().is_some()
            && self.attachment_support_issue().is_none()
    }

    fn submit_snapshot(
        &mut self,
        mut snapshot: ComposerSnapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<ChatFormSubmit> {
        snapshot.attachments = self.attachments.clone();
        if snapshot.is_empty() {
            return None;
        }
        if self.attachment_support_issue().is_some() {
            return None;
        }
        if self.agent_running {
            return None;
        }
        let provider_model = self.revalidate_selected_model_for_submit(window, cx)?;
        Some(ChatFormSubmit {
            composer: snapshot,
            provider_model,
            reasoning_selection: self.selected_reasoning_selection.clone(),
        })
    }

    fn primary_button_action(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<ChatFormPrimaryButtonAction> {
        if self.agent_running {
            return Some(ChatFormPrimaryButtonAction::Stop);
        }

        let mut snapshot = self.composer.read(cx).snapshot();
        snapshot.attachments = self.attachments.clone();
        self.submit_snapshot(snapshot, window, cx)
            .map(|submit| ChatFormPrimaryButtonAction::Send(Box::new(submit)))
    }

    fn emit_primary_button_action(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.primary_button_action(window, cx) {
            Some(ChatFormPrimaryButtonAction::Send(submit)) => {
                cx.emit(ChatFormEvent::SendRequested(submit));
            }
            Some(ChatFormPrimaryButtonAction::Stop) => {
                cx.emit(ChatFormEvent::StopRequested);
            }
            None => {}
        }
    }

    pub(super) fn selected_model_label(&self, i18n: &foundation::I18n) -> SharedString {
        match &self.model_choices {
            Err(_) => i18n.t("chat-form-model-load-failed").into(),
            Ok(_) => self
                .selected_model_choice()
                .map(|choice| choice.display_label().into())
                .unwrap_or_else(|| i18n.t("chat-form-model-empty").into()),
        }
    }

    pub(super) fn selected_model_capabilities(
        &self,
    ) -> Option<&ai_chat_core::ModelCapabilitiesSnapshot> {
        self.selected_model_choice()
            .map(|choice| &choice.capabilities)
    }

    pub(super) fn has_effort_options(&self) -> bool {
        self.selected_model_capabilities()
            .map(|capabilities| !reasoning_selections(capabilities.reasoning.as_ref()).is_empty())
            .unwrap_or(false)
    }

    pub(super) fn has_token_budget_options(&self) -> bool {
        self.selected_model_capabilities()
            .and_then(|capabilities| token_budget_bounds(capabilities.reasoning.as_ref()))
            .is_some()
    }

    pub(super) fn has_model_choices(&self) -> bool {
        self.model_choices
            .as_ref()
            .is_ok_and(|choices| !choices.is_empty())
    }
}

impl Render for ChatForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let send_tooltip = cx.global::<foundation::I18n>().t("chat-form-send-tooltip");
        let stop_tooltip = cx.global::<foundation::I18n>().t("chat-form-stop-tooltip");
        let drop_label = cx
            .global::<foundation::I18n>()
            .t("chat-form-attachment-drop");
        let agent_running = self.agent_running;
        let can_submit = self.can_send(cx);

        v_flex()
            .id("ai-chat2-chat-form-preview")
            .w_full()
            .relative()
            .rounded(px(25.))
            .border_1()
            .border_color(cx.theme().input)
            .bg(cx.theme().input_background())
            .text_color(cx.theme().foreground)
            .on_drop(cx.listener(|form, paths: &ExternalPaths, window, cx| {
                form.add_attachment_paths(paths.paths().to_vec(), window, cx);
            }))
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
                v_flex()
                    .w_full()
                    .min_h(px(56.))
                    .px(px(COMPOSER_INPUT_HORIZONTAL_PADDING))
                    .pt(px(COMPOSER_INPUT_TOP_PADDING))
                    .gap(px(attachments::STRIP_BOTTOM_MARGIN))
                    .mb(px(COMPOSER_INPUT_BOTTOM_MARGIN))
                    .when_some(
                        self.render_attachment_support_message(cx),
                        |this, message| this.child(message),
                    )
                    .when(!self.attachments.is_empty(), |this| {
                        this.child(self.render_attachments_strip(cx))
                    })
                    .child(self.composer.clone()),
            )
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .min_h(px(COMPOSER_BUTTON_SIZE))
                    .px(px(COMPOSER_FOOTER_HORIZONTAL_PADDING))
                    .mb(px(COMPOSER_FOOTER_BOTTOM_MARGIN))
                    .child(
                        h_flex()
                            .items_center()
                            .gap(px(5.))
                            .min_w_0()
                            .child(self.render_add_attachment_menu(cx))
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
                                Button::new(if agent_running {
                                    "chat-form-stop"
                                } else {
                                    "chat-form-send"
                                })
                                .primary()
                                .with_size(px(COMPOSER_BUTTON_SIZE))
                                .size(px(COMPOSER_BUTTON_SIZE))
                                .p(px(0.))
                                .rounded(px(COMPOSER_BUTTON_RADIUS))
                                .disabled(!agent_running && !can_submit)
                                .child(
                                    Icon::new(if agent_running {
                                        IconName::Square
                                    } else {
                                        IconName::Send
                                    })
                                    .with_size(px(COMPOSER_BUTTON_ICON_SIZE)),
                                )
                                .tooltip(if agent_running {
                                    stop_tooltip
                                } else {
                                    send_tooltip
                                })
                                .on_click(cx.listener(
                                    |form, _, window, cx| {
                                        form.emit_primary_button_action(window, cx);
                                    },
                                )),
                            ),
                    ),
            )
            .child(
                div()
                    .invisible()
                    .absolute()
                    .top_0()
                    .right_0()
                    .bottom_0()
                    .left_0()
                    .rounded(px(25.))
                    .border_1()
                    .border_color(cx.theme().primary.opacity(0.55))
                    .bg(cx.theme().primary.opacity(0.08))
                    .flex()
                    .items_center()
                    .justify_center()
                    .drag_over::<ExternalPaths>(|this, _, _, _| this.visible())
                    .child(
                        h_flex()
                            .gap_2()
                            .px_3()
                            .py_2()
                            .rounded(px(attachments::CARD_RADIUS))
                            .bg(cx.theme().background.opacity(0.92))
                            .border_1()
                            .border_color(cx.theme().border)
                            .child(
                                Icon::new(IconName::Paperclip)
                                    .size_4()
                                    .text_color(cx.theme().primary),
                            )
                            .child(Label::new(drop_label).text_sm().font_medium()),
                    ),
            )
    }
}

fn load_model_choices(cx: &App) -> Result<Vec<ProviderModelChoice>, SharedString> {
    state::providers::enabled_provider_models(cx).map_err(|err| err.to_string().into())
}

fn selected_model_choice_in<'a>(
    choices: &'a Result<Vec<ProviderModelChoice>, SharedString>,
    key: Option<&ProviderModelKey>,
) -> Option<&'a ProviderModelChoice> {
    let key = key?;
    choices
        .as_ref()
        .ok()?
        .iter()
        .find(|choice| &choice.key() == key)
}

fn initial_token_budget_value(
    choices: &Result<Vec<ProviderModelChoice>, SharedString>,
    key: Option<&ProviderModelKey>,
) -> u32 {
    selected_model_choice_in(choices, key)
        .and_then(|choice| token_budget_bounds(choice.capabilities.reasoning.as_ref()))
        .map(|bounds| bounds.default_value)
        .unwrap_or(1024)
}

fn model_empty_label(
    choices: &Result<Vec<ProviderModelChoice>, SharedString>,
    i18n: &foundation::I18n,
) -> SharedString {
    match choices {
        Ok(_) => i18n.t("chat-form-model-none-configured").into(),
        Err(err) => format!("{}: {}", i18n.t("chat-form-model-load-failed"), err).into(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ChatForm, ChatFormPrimaryButtonAction,
        composer_editor::{ComposerSendPolicy, ComposerSnapshot},
        model_empty_label, selected_model_choice_in,
    };
    use crate::{
        database::{self, FreshStoreGlobal},
        foundation::I18n,
        state,
        state::providers::{ProviderModelChoice, ProviderModelKey},
    };
    use ai_chat_core::{
        CapabilitySourceSnapshot, ContentPart, ModelCapabilitiesSnapshot, ProviderModelMetadata,
        ProviderSecretRefs, ProviderSettingFieldValue, ProviderSettingValue,
        ProviderSettingsPayload, ReasoningCapabilitySnapshot, ReasoningControlSnapshot,
        ReasoningSelectionSnapshot, TokenBudgetSelectionMode, conservative_model_capabilities,
    };
    use ai_chat_db::{NewProvider, NewProviderModel};
    use gpui::{
        App, AppContext as _, SharedString, TestAppContext, VisualTestContext, WindowHandle,
    };
    use tempfile::{TempDir, tempdir};

    #[test]
    fn selected_model_choice_requires_current_provider_model_key() {
        let choices = Ok(vec![choice("provider-1", "gpt-5")]);
        let selected = ProviderModelKey {
            provider_id: "provider-1".to_string(),
            model_id: "gpt-5".to_string(),
        };
        let stale = ProviderModelKey {
            provider_id: "provider-1".to_string(),
            model_id: "disabled-model".to_string(),
        };

        assert_eq!(
            selected_model_choice_in(&choices, Some(&selected))
                .map(|choice| choice.model_id.as_str()),
            Some("gpt-5")
        );
        assert!(selected_model_choice_in(&choices, Some(&stale)).is_none());
        assert!(selected_model_choice_in(&choices, None).is_none());
        assert!(selected_model_choice_in(&Err("load failed".into()), Some(&selected)).is_none());
    }

    #[test]
    fn model_empty_label_distinguishes_empty_and_error_states() {
        let i18n = I18n::english_for_test();

        assert_eq!(
            model_empty_label(&Ok(vec![]), &i18n).as_ref(),
            "No enabled models. Configure a provider and enable models first."
        );
        assert_eq!(
            model_empty_label(&Err(SharedString::from("database is unavailable")), &i18n).as_ref(),
            "Failed to load models: database is unavailable"
        );
    }

    #[gpui::test]
    fn provider_catalog_event_refreshes_mounted_chat_form(cx: &mut TestAppContext) {
        let _dir = init_chat_form_test(cx);
        let window = open_chat_form_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let form = window.root(&mut cx).unwrap();

        assert_eq!(selected_model_id(&form, &cx).as_deref(), Some("gpt-5"));

        cx.update(|_, cx| {
            let provider_id = provider_id_for_kind(cx, "openai");
            state::providers::set_provider_model_enabled(
                cx,
                &provider_id,
                &"gpt-5".to_string(),
                false,
            )
            .unwrap();
        });

        assert_eq!(selected_model_id(&form, &cx).as_deref(), Some("gpt-5-mini"));
    }

    #[gpui::test]
    fn submit_revalidates_stale_selected_model_before_emitting(cx: &mut TestAppContext) {
        let _dir = init_chat_form_test(cx);
        let window = open_chat_form_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let form = window.root(&mut cx).unwrap();

        assert_eq!(selected_model_id(&form, &cx).as_deref(), Some("gpt-5"));
        cx.update(|_, cx| {
            let provider_id = provider_id_for_kind(cx, "openai");
            database::repository(cx)
                .set_provider_model_enabled(&provider_id, "gpt-5", false)
                .unwrap();
        });

        let first_submit = submit_snapshot(&form, test_snapshot("hello"), &mut cx);
        assert!(first_submit.is_none());
        assert_eq!(selected_model_id(&form, &cx).as_deref(), Some("gpt-5-mini"));

        let second_submit = submit_snapshot(&form, test_snapshot("hello"), &mut cx)
            .expect("refreshed selected model can be submitted");
        assert_eq!(second_submit.provider_model.model_id, "gpt-5-mini");
    }

    #[gpui::test]
    fn submit_revalidation_preserves_custom_token_budget(cx: &mut TestAppContext) {
        let _dir = init_chat_form_reasoning_test(cx);
        let window = open_chat_form_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let form = window.root(&mut cx).unwrap();

        cx.update(|window, cx| {
            form.update(cx, |form, cx| {
                let input = form.token_budget_input.clone();
                form.apply_custom_token_budget(2048, &input, window, cx);
            });
        });

        let submit = submit_snapshot(&form, test_snapshot("hello"), &mut cx)
            .expect("selected model can be submitted after revalidation");

        assert_eq!(
            submit.reasoning_selection,
            Some(ReasoningSelectionSnapshot::TokenBudget {
                mode: TokenBudgetSelectionMode::Custom,
                value: Some(2048),
            })
        );
    }

    #[gpui::test]
    fn running_agent_blocks_submit_and_primary_button_stops(cx: &mut TestAppContext) {
        let _dir = init_chat_form_test(cx);
        let window = open_chat_form_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let form = window.root(&mut cx).unwrap();

        cx.update(|_, cx| {
            form.update(cx, |form, cx| {
                form.set_agent_running(true, cx);
            });
        });

        assert!(submit_snapshot(&form, test_snapshot("hello"), &mut cx).is_none());
        let action = cx.update(|window, cx| {
            form.update(cx, |form, cx| form.primary_button_action(window, cx))
        });
        assert_eq!(action, Some(ChatFormPrimaryButtonAction::Stop));

        cx.update(|_, cx| {
            form.update(cx, |form, cx| {
                form.set_agent_running(false, cx);
            });
        });

        assert!(submit_snapshot(&form, test_snapshot("hello"), &mut cx).is_some());
    }

    fn choice(provider_id: &str, model_id: &str) -> ProviderModelChoice {
        ProviderModelChoice {
            provider_id: provider_id.to_string(),
            provider_kind: "openai".to_string(),
            provider_display_name: "OpenAI".to_string(),
            model_id: model_id.to_string(),
            model_display_name: None,
            capabilities: conservative_model_capabilities("openai"),
        }
    }

    fn init_chat_form_test(cx: &mut TestAppContext) -> TempDir {
        let dir = tempdir().unwrap();
        cx.update(|cx| {
            gpui_component::init(cx);
            cx.set_global(FreshStoreGlobal::open_in_dir(dir.path()).unwrap());
            state::providers::init(cx);
            crate::foundation::i18n::init(cx);

            let repository = database::repository(cx);
            let provider = repository.insert_provider(provider_for_test()).unwrap();
            repository
                .replace_fetched_provider_models(
                    &provider.id,
                    vec![
                        provider_model_for_test(&provider.id, "gpt-5"),
                        provider_model_for_test(&provider.id, "gpt-5-mini"),
                    ],
                )
                .unwrap();
        });
        dir
    }

    fn init_chat_form_reasoning_test(cx: &mut TestAppContext) -> TempDir {
        let dir = tempdir().unwrap();
        cx.update(|cx| {
            gpui_component::init(cx);
            cx.set_global(FreshStoreGlobal::open_in_dir(dir.path()).unwrap());
            state::providers::init(cx);
            crate::foundation::i18n::init(cx);

            let repository = database::repository(cx);
            let provider = repository.insert_provider(provider_for_test()).unwrap();
            repository
                .replace_fetched_provider_models(
                    &provider.id,
                    vec![provider_model_with_capabilities(
                        &provider.id,
                        "claude-3-7-sonnet",
                        token_budget_capabilities(),
                    )],
                )
                .unwrap();
        });
        dir
    }

    fn open_chat_form_window(cx: &mut TestAppContext) -> WindowHandle<ChatForm> {
        cx.update(|cx| {
            cx.open_window(Default::default(), |window, cx| {
                cx.new(|cx| ChatForm::new(window, cx))
            })
        })
        .unwrap()
    }

    fn submit_snapshot(
        form: &gpui::Entity<ChatForm>,
        snapshot: ComposerSnapshot,
        cx: &mut VisualTestContext,
    ) -> Option<super::ChatFormSubmit> {
        cx.update(|window, cx| {
            form.update(cx, |form, cx| form.submit_snapshot(snapshot, window, cx))
        })
    }

    fn selected_model_id(form: &gpui::Entity<ChatForm>, cx: &VisualTestContext) -> Option<String> {
        form.read_with(cx, |form, _| {
            form.selected_model_choice()
                .map(|choice| choice.model_id.clone())
        })
    }

    fn provider_id_for_kind(cx: &App, kind: &str) -> String {
        database::repository(cx)
            .list_providers()
            .unwrap()
            .into_iter()
            .find(|provider| provider.kind == kind)
            .expect("provider exists")
            .id
    }

    fn provider_for_test() -> NewProvider {
        NewProvider {
            kind: "openai".to_string(),
            display_name: "OpenAI".to_string(),
            enabled: true,
            settings: ProviderSettingsPayload {
                provider_kind: "openai".to_string(),
                fields: vec![ProviderSettingFieldValue {
                    key: "base_url".to_string(),
                    value: ProviderSettingValue::String {
                        value: "https://api.openai.com/v1".to_string(),
                    },
                }],
            },
            secret_refs: ProviderSecretRefs { refs: Vec::new() },
        }
    }

    fn provider_model_for_test(provider_id: &str, model_id: &str) -> NewProviderModel {
        provider_model_with_capabilities(
            provider_id,
            model_id,
            conservative_model_capabilities("openai"),
        )
    }

    fn provider_model_with_capabilities(
        provider_id: &str,
        model_id: &str,
        capabilities: ModelCapabilitiesSnapshot,
    ) -> NewProviderModel {
        NewProviderModel {
            provider_id: provider_id.to_string(),
            model_id: model_id.to_string(),
            display_name: None,
            enabled: true,
            capabilities,
            metadata: ProviderModelMetadata {
                display_name: None,
                family: None,
                raw: None,
            },
        }
    }

    fn token_budget_capabilities() -> ModelCapabilitiesSnapshot {
        let mut capabilities = conservative_model_capabilities("anthropic");
        capabilities.reasoning = Some(ReasoningCapabilitySnapshot {
            default_effort: "4096".to_string(),
            efforts: vec!["4096".to_string()],
            summaries: true,
            control: Some(ReasoningControlSnapshot::TokenBudget {
                min: Some(1024),
                max: None,
                default_value: Some(4096),
                dynamic_supported: false,
                off_supported: false,
            }),
            source: CapabilitySourceSnapshot::Manual {
                source: "test".to_string(),
            },
        });
        capabilities
    }

    fn test_snapshot(text: &str) -> ComposerSnapshot {
        ComposerSnapshot {
            text: text.to_string(),
            content_parts: vec![ContentPart::Text {
                text: text.to_string(),
            }],
            skill_requests: Vec::new(),
            token_ranges: Vec::new(),
            attachments: Vec::new(),
            send_policy: ComposerSendPolicy::EnterToSend,
        }
    }
}

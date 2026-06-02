mod composer_editor;
mod effort_select;
mod model_select;
pub(in crate::features::home) mod picker;
mod thinking_effort;

use crate::{
    foundation::{self, assets::IconName},
    state,
    state::providers::{ProviderModelChoice, ProviderModelKey},
};
use ai_chat_core::{ReasoningSelectionSnapshot, TokenBudgetSelectionMode};
use composer_editor::{ComposerEditor, ComposerEditorEvent, ComposerSnapshot};
use effort_select::{EffortOption, effort_sections};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable, box_shadow,
    button::{Button, ButtonVariants},
    h_flex,
    input::{InputEvent, InputState, NumberInputEvent, StepAction},
    list::ListState,
    v_flex,
};
use model_select::{ModelOption, model_sections};
use picker::PickerListDelegate;
use std::{path::Path, rc::Rc};
use thinking_effort::{
    computed_default_reasoning_selection, custom_token_budget_value, reasoning_selection_is_valid,
    reasoning_selections, token_budget_bounds,
};

pub(super) const COMPOSER_BUTTON_SIZE: f32 = 28.;
pub(super) const COMPOSER_BUTTON_ICON_SIZE: f32 = 18.;
pub(super) const COMPOSER_BUTTON_RADIUS: f32 = 999.;

#[allow(dead_code)]
#[derive(Clone)]
pub(crate) enum ChatFormEvent {
    AddRequested,
    SendRequested(ChatFormSubmit),
}

impl EventEmitter<ChatFormEvent> for ChatForm {}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ChatFormSubmit {
    pub(crate) composer: ComposerSnapshot,
    pub(crate) provider_model: ProviderModelChoice,
    pub(crate) reasoning_selection: Option<ReasoningSelectionSnapshot>,
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
            |form, _composer, event: &ComposerEditorEvent, _window, cx| match event {
                ComposerEditorEvent::Changed => {
                    cx.notify();
                }
                ComposerEditorEvent::SubmitRequested(snapshot) => {
                    if let Some(submit) = form.submit_snapshot(snapshot.clone()) {
                        cx.emit(ChatFormEvent::SendRequested(submit));
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
            _subscriptions: vec![
                composer_subscription,
                token_budget_change_subscription,
                token_budget_step_subscription,
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
        self.model_choices = load_model_choices(cx);
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
        self.composer.read(cx).can_submit() && self.selected_model_choice().is_some()
    }

    fn submit_snapshot(&self, snapshot: ComposerSnapshot) -> Option<ChatFormSubmit> {
        if snapshot.is_empty() {
            return None;
        }
        Some(ChatFormSubmit {
            composer: snapshot,
            provider_model: self.selected_model_choice()?.clone(),
            reasoning_selection: self.selected_reasoning_selection.clone(),
        })
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
        let add_tooltip = cx.global::<foundation::I18n>().t("chat-form-add-tooltip");
        let send_tooltip = cx.global::<foundation::I18n>().t("chat-form-send-tooltip");
        let can_submit = self.can_send(cx);

        v_flex()
            .id("ai-chat2-chat-form-preview")
            .w_full()
            .relative()
            .gap(px(2.))
            .p(px(8.))
            .rounded(px(25.))
            .border_1()
            .border_color(cx.theme().input)
            .bg(cx.theme().input_background())
            .text_color(cx.theme().foreground)
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
                    .pt(px(6.))
                    .child(self.composer.clone()),
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
                                    .disabled(!can_submit)
                                    .child(
                                        Icon::new(IconName::Send)
                                            .with_size(px(COMPOSER_BUTTON_ICON_SIZE)),
                                    )
                                    .tooltip(send_tooltip)
                                    .on_click(cx.listener(|form, _, _, cx| {
                                        let snapshot = form.composer.read(cx).snapshot();
                                        if let Some(submit) = form.submit_snapshot(snapshot) {
                                            cx.emit(ChatFormEvent::SendRequested(submit));
                                        }
                                    })),
                            ),
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
    use super::{model_empty_label, selected_model_choice_in};
    use crate::{
        foundation::I18n,
        state::providers::{ProviderModelChoice, ProviderModelKey},
    };
    use ai_chat_core::conservative_model_capabilities;
    use gpui::SharedString;

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
}

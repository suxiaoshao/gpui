//! Shared model, reasoning and tool-access state.
//!
//! The form data in this module is deliberately independent from `ChatForm`.
//! A caller owns the form entity (and therefore persistence/validation), while
//! `RunSettingsController` owns the picker state and keeps it in sync with the
//! provider catalog. The form draft is the source of truth for business values;
//! control state only projects those values for rendering and focus.

mod policy;

use std::rc::Rc;

use crate::{
    components::{
        chat_input::{approval_select, effort_select},
        model_picker::{ModelOption, model_sections},
        picker::PickerListDelegate,
    },
    features::settings,
    foundation::{self, I18n},
    state,
    state::providers::{ProviderModelChoice, ProviderModelKey},
};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    input::{InputEvent, InputState, NumberInput, NumberInputEvent, StepAction},
    label::Label,
    list::ListState,
};
use gpui_form::SubscriptionSet;
use jaco_core::{
    ModelCapabilitiesSnapshot, ReasoningSelectionSnapshot, TokenBudgetSelectionMode,
    ToolApprovalMode,
};

pub(crate) use policy::{
    TokenBudgetBounds, computed_default_reasoning_selection, custom_token_budget_value,
    reasoning_selection_is_valid, reasoning_selection_label, reasoning_selections,
    token_budget_bounds,
};

pub(crate) type ControlOpenHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SelectionOrigin {
    Picker,
    External,
}

impl SelectionOrigin {
    fn should_sync_picker(self) -> bool {
        matches!(self, Self::External)
    }
}

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = RunSettingsFormStore)]
pub(crate) struct RunSettingsInput {
    #[form(component = "value", required)]
    pub(crate) model: Option<ProviderModelKey>,
    #[form(component = "value")]
    pub(crate) reasoning_selection: Option<ReasoningSelectionSnapshot>,
    #[form(component = "value")]
    pub(crate) approval_mode: ToolApprovalMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ModelResolutionPolicy {
    FallbackToFirstEnabled,
    RequireSelected,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RunSettingsSubmitSnapshot {
    pub(crate) provider_model: ProviderModelChoice,
    pub(crate) reasoning_selection: Option<ReasoningSelectionSnapshot>,
    pub(crate) approval_mode: ToolApprovalMode,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RunSettingsSubmitError {
    CatalogUnavailable,
    ModelRequired,
    ModelUnavailable(ProviderModelKey),
}

impl RunSettingsInput {
    pub(crate) fn new(
        model: Option<ProviderModelKey>,
        reasoning_selection: Option<ReasoningSelectionSnapshot>,
        approval_mode: ToolApprovalMode,
    ) -> Self {
        Self {
            model,
            reasoning_selection,
            approval_mode,
        }
    }
}

pub(crate) fn resolve_run_settings(
    draft: &RunSettingsInput,
    choices: &Result<Vec<ProviderModelChoice>, SharedString>,
    policy: ModelResolutionPolicy,
) -> Result<RunSettingsSubmitSnapshot, RunSettingsSubmitError> {
    let choices = choices
        .as_ref()
        .map_err(|_| RunSettingsSubmitError::CatalogUnavailable)?;
    let selected = draft
        .model
        .as_ref()
        .and_then(|key| selected_model_choice_from_slice(choices, Some(key)))
        .or_else(|| {
            if matches!(policy, ModelResolutionPolicy::FallbackToFirstEnabled) {
                choices.first()
            } else {
                None
            }
        })
        .ok_or_else(|| match (policy, draft.model.clone()) {
            (ModelResolutionPolicy::RequireSelected, None) => RunSettingsSubmitError::ModelRequired,
            (_, Some(key)) => RunSettingsSubmitError::ModelUnavailable(key),
            (ModelResolutionPolicy::FallbackToFirstEnabled, None) => {
                RunSettingsSubmitError::ModelRequired
            }
        })?;

    let reasoning_selection = (draft.model.as_ref() == Some(&selected.key()))
        .then_some(draft.reasoning_selection.as_ref())
        .flatten()
        .and_then(|requested| {
            selected
                .capabilities
                .reasoning
                .as_ref()
                .filter(|reasoning| reasoning_selection_is_valid(Some(reasoning), requested))
                .map(|_| requested.clone())
        })
        .or_else(|| computed_default_reasoning_selection(selected.capabilities.reasoning.as_ref()));

    Ok(RunSettingsSubmitSnapshot {
        provider_model: selected.clone(),
        reasoning_selection,
        approval_mode: draft.approval_mode,
    })
}

pub(crate) struct ModelControlState {
    pub(crate) choices: Result<Vec<ProviderModelChoice>, SharedString>,
    pub(crate) picker: Entity<ListState<PickerListDelegate<ModelOption>>>,
    pub(crate) open: bool,
    pub(crate) on_open_change: ControlOpenHandler,
}

pub(crate) struct ReasoningControlState {
    pub(crate) capability: Option<ModelCapabilitiesSnapshot>,
    pub(crate) picker: Entity<ListState<PickerListDelegate<effort_select::EffortOption>>>,
    pub(crate) token_budget_input: Entity<InputState>,
    pub(crate) open: bool,
    pub(crate) on_open_change: ControlOpenHandler,
}

pub(crate) struct ApprovalControlState {
    pub(crate) picker: Entity<ListState<PickerListDelegate<approval_select::ApprovalModeOption>>>,
    pub(crate) open: bool,
    pub(crate) on_open_change: ControlOpenHandler,
}

#[derive(Clone)]
pub(crate) struct RunSettingsControlStates {
    pub(crate) form: Entity<RunSettingsFormStore>,
    pub(crate) model: Entity<ModelControlState>,
    pub(crate) reasoning: Entity<ReasoningControlState>,
    pub(crate) approval: Entity<ApprovalControlState>,
}

pub(crate) struct RunSettingsController {
    form: Entity<RunSettingsFormStore>,
    states: RunSettingsControlStates,
    _subscriptions: SubscriptionSet,
}

impl RunSettingsController {
    pub(crate) fn new(
        form: Entity<RunSettingsFormStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new_with_model_fallback(form, true, window, cx)
    }

    pub(crate) fn new_without_model_fallback(
        form: Entity<RunSettingsFormStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new_with_model_fallback(form, false, window, cx)
    }

    fn new_with_model_fallback(
        form: Entity<RunSettingsFormStore>,
        fallback_to_first_model: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let draft = form.read(cx).draft();
        let choices = load_model_choices(cx);
        let selected_model =
            resolve_model_key(&choices, draft.model.as_ref(), fallback_to_first_model);
        let capability = selected_model_choice(&choices, selected_model.as_ref())
            .map(|choice| choice.capabilities.clone());
        let selected_reasoning =
            resolve_reasoning_selection(capability.as_ref(), draft.reasoning_selection.as_ref());
        let approval = draft.approval_mode;
        let state = cx.entity().downgrade();

        let model_sections = model_sections(choices.as_ref().map(Vec::as_slice).unwrap_or(&[]));
        let model_selected_ix =
            PickerListDelegate::selected_index_for(&model_sections, selected_model.as_ref());
        let model_confirm = Rc::new({
            let state = state.clone();
            move |option: ModelOption, window: &mut Window, cx: &mut App| {
                let _ = state.update(cx, |controller, cx| {
                    controller.select_model_from_picker(option.key(), window, cx);
                });
            }
        });
        let model_cancel = Rc::new({
            let state = state.clone();
            move |window: &mut Window, cx: &mut App| {
                let _ = state.update(cx, |controller, cx| {
                    controller.set_model_open(false, window, cx);
                });
            }
        });
        let model_open_change: ControlOpenHandler = Rc::new({
            let state = state.clone();
            move |open, window, cx| {
                let _ = state.update(cx, |controller, cx| {
                    controller.set_model_open(open, window, cx);
                });
            }
        });
        let model_empty = model_empty_label(&choices, cx.global::<I18n>());
        let model_picker = cx.new(|cx| {
            let mut picker = ListState::new(
                PickerListDelegate::new(
                    model_sections,
                    selected_model.clone(),
                    model_empty,
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

        let reasoning_sections =
            effort_select::effort_sections(capability.as_ref(), cx.global::<foundation::I18n>());
        let reasoning_selected_ix = PickerListDelegate::selected_index_for(
            &reasoning_sections,
            selected_reasoning.as_ref(),
        );
        let reasoning_confirm = Rc::new({
            let state = state.clone();
            move |option: effort_select::EffortOption, window: &mut Window, cx: &mut App| {
                let _ = state.update(cx, |controller, cx| {
                    controller.select_reasoning_from_picker(option.selection().clone(), window, cx);
                });
            }
        });
        let reasoning_cancel = Rc::new({
            let state = state.clone();
            move |window: &mut Window, cx: &mut App| {
                let _ = state.update(cx, |controller, cx| {
                    controller.set_reasoning_open(false, window, cx);
                });
            }
        });
        let reasoning_open_change: ControlOpenHandler = Rc::new({
            let state = state.clone();
            move |open, window, cx| {
                let _ = state.update(cx, |controller, cx| {
                    controller.set_reasoning_open(open, window, cx);
                });
            }
        });
        let reasoning_empty = cx.global::<I18n>().t("chat-form-effort-empty").into();
        let reasoning_picker = cx.new(|cx| {
            let mut picker = ListState::new(
                PickerListDelegate::new(
                    reasoning_sections,
                    selected_reasoning.clone(),
                    reasoning_empty,
                    reasoning_confirm,
                    reasoning_cancel,
                ),
                window,
                cx,
            );
            picker.set_selected_index(reasoning_selected_ix, window, cx);
            picker
        });
        let token_budget =
            initial_token_budget_value(capability.as_ref(), selected_reasoning.as_ref());
        let token_budget_input =
            cx.new(|cx| InputState::new(window, cx).default_value(token_budget.to_string()));

        let approval_sections = approval_select::approval_mode_sections(cx.global::<I18n>());
        let approval_selected_ix =
            PickerListDelegate::selected_index_for(&approval_sections, Some(&approval));
        let approval_confirm = Rc::new({
            let state = state.clone();
            move |option: approval_select::ApprovalModeOption, window: &mut Window, cx: &mut App| {
                let _ = state.update(cx, |controller, cx| {
                    controller.select_approval_from_picker(option.mode(), window, cx);
                });
            }
        });
        let approval_cancel = Rc::new({
            let state = state.clone();
            move |window: &mut Window, cx: &mut App| {
                let _ = state.update(cx, |controller, cx| {
                    controller.set_approval_open(false, window, cx);
                });
            }
        });
        let approval_open_change: ControlOpenHandler = Rc::new({
            let state = state.clone();
            move |open, window, cx| {
                let _ = state.update(cx, |controller, cx| {
                    controller.set_approval_open(open, window, cx);
                });
            }
        });
        let approval_picker = cx.new(|cx| {
            let mut picker = ListState::new(
                PickerListDelegate::new(
                    approval_sections,
                    Some(approval),
                    SharedString::from(""),
                    approval_confirm,
                    approval_cancel,
                ),
                window,
                cx,
            );
            picker.set_selected_index(approval_selected_ix, window, cx);
            picker
        });

        let initial_values =
            RunSettingsInput::new(selected_model.clone(), selected_reasoning.clone(), approval);

        let model_state = cx.new(|_| ModelControlState {
            choices,
            picker: model_picker,
            open: false,
            on_open_change: model_open_change,
        });
        let reasoning_state = cx.new(|_| ReasoningControlState {
            capability,
            picker: reasoning_picker,
            token_budget_input,
            open: false,
            on_open_change: reasoning_open_change,
        });
        let approval_state = cx.new(|_| ApprovalControlState {
            picker: approval_picker,
            open: false,
            on_open_change: approval_open_change,
        });
        let states = RunSettingsControlStates {
            form: form.clone(),
            model: model_state,
            reasoning: reasoning_state,
            approval: approval_state,
        };

        let mut subscriptions = SubscriptionSet::new();
        if cx.has_global::<state::providers::ProviderCatalogGlobal>() {
            let catalog = state::providers::catalog(cx);
            subscriptions.push(cx.observe_in(
                &catalog.entity(),
                window,
                |controller, _catalog, window, cx| {
                    controller.reload_models(window, cx);
                },
            ));
        }
        let token_budget_input = states.reasoning.read(cx).token_budget_input.clone();
        let token_budget_subscription = cx.subscribe_in(
            &token_budget_input,
            window,
            |controller, input, event: &InputEvent, window, cx| {
                if !matches!(event, InputEvent::Change) {
                    return;
                }
                if let Ok(value) = input.read(cx).value().as_ref().parse::<u32>() {
                    controller.apply_custom_token_budget(value, window, cx);
                }
            },
        );
        let token_budget_step_subscription = cx.subscribe_in(
            &token_budget_input,
            window,
            |controller, input, event: &NumberInputEvent, window, cx| {
                let NumberInputEvent::Step(action) = event;
                let bounds = controller.current_token_budget_bounds(cx);
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
                controller.apply_custom_token_budget(next, window, cx);
            },
        );
        subscriptions.push(token_budget_subscription);
        subscriptions.push(token_budget_step_subscription);

        let form_model = RunSettingsFormStore::model_handle(&form);
        let form_reasoning = RunSettingsFormStore::reasoning_selection_handle(&form);
        let form_approval = RunSettingsFormStore::approval_mode_handle(&form);
        subscriptions.push(
            form_model
                .subscribe_in(window, cx, move |controller, event, window, cx| {
                    let selected = event.draft.clone();
                    if matches!(event.cause, gpui_form::FieldChangeCause::UserInput) {
                        cx.defer_in(window, move |controller, window, cx| {
                            controller.sync_model_picker(selected, window, cx);
                            controller.sync_token_budget_input(window, cx);
                        });
                    } else {
                        controller.sync_model_picker(selected, window, cx);
                        controller.sync_token_budget_input(window, cx);
                    }
                })
                .expect("run-settings form entity is alive"),
        );
        subscriptions.push(
            form_reasoning
                .subscribe_in(window, cx, move |controller, event, window, cx| {
                    let selected = event.draft.clone();
                    if matches!(event.cause, gpui_form::FieldChangeCause::UserInput) {
                        cx.defer_in(window, move |controller, window, cx| {
                            let capability =
                                controller.states.reasoning.read(cx).capability.clone();
                            controller.sync_reasoning_picker(capability, selected, window, cx);
                            controller.sync_token_budget_input(window, cx);
                        });
                    } else {
                        let capability = controller.states.reasoning.read(cx).capability.clone();
                        controller.sync_reasoning_picker(capability, selected, window, cx);
                        controller.sync_token_budget_input(window, cx);
                    }
                })
                .expect("run-settings form entity is alive"),
        );
        subscriptions.push(
            form_approval
                .subscribe_in(window, cx, move |controller, event, window, cx| {
                    let selected = event.draft;
                    if matches!(event.cause, gpui_form::FieldChangeCause::UserInput) {
                        cx.defer_in(window, move |controller, window, cx| {
                            controller.sync_approval_picker(selected, window, cx);
                        });
                    } else {
                        controller.sync_approval_picker(selected, window, cx);
                    }
                })
                .expect("run-settings form entity is alive"),
        );

        let controller = Self {
            form,
            states,
            _subscriptions: subscriptions,
        };
        controller.write_form_values(&initial_values, cx);
        controller
    }

    pub(crate) fn control_states(&self) -> RunSettingsControlStates {
        self.states.clone()
    }

    pub(crate) fn form(&self) -> Entity<RunSettingsFormStore> {
        self.form.clone()
    }

    #[cfg(test)]
    pub(crate) fn selected_model(&self, cx: &App) -> Option<ProviderModelChoice> {
        let selected = self.form.read(cx).draft().model;
        let state = self.states.model.read(cx);
        selected_model_choice(&state.choices, selected.as_ref()).cloned()
    }

    pub(crate) fn reload_models(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let choices = load_model_choices(cx);
        let draft = self.form.read(cx).draft();
        let previous_key = draft.model.clone();
        let previous_reasoning = draft.reasoning_selection.clone();
        // A catalog/options refresh must not rebase the form draft.  Keep an
        // unavailable selected key in the form so the submit policy can make
        // the explicit fallback/require decision later.
        let selected = previous_key
            .as_ref()
            .filter(|key| selected_model_choice(&choices, Some(key)).is_some())
            .cloned();
        let preserved_reasoning = selected.as_ref().and(previous_reasoning);
        let capability = selected_model_choice(&choices, selected.as_ref())
            .map(|choice| choice.capabilities.clone());
        let reasoning =
            resolve_reasoning_selection(capability.as_ref(), preserved_reasoning.as_ref());

        self.states.model.update(cx, |state, _| {
            state.choices = choices.clone();
        });
        self.states.reasoning.update(cx, |state, _| {
            state.capability = capability.clone();
        });
        self.sync_model_picker(selected.clone(), window, cx);
        self.sync_reasoning_picker(capability.clone(), reasoning.clone(), window, cx);
        self.sync_token_budget_input(window, cx);
        cx.notify();
    }

    #[cfg(test)]
    pub(crate) fn select_model_value(
        &mut self,
        key: ProviderModelKey,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_model(key, SelectionOrigin::External, window, cx);
    }

    #[cfg(test)]
    pub(crate) fn select_approval_value(
        &mut self,
        mode: ToolApprovalMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_approval(mode, SelectionOrigin::External, window, cx);
    }

    #[cfg(test)]
    pub(crate) fn set_custom_token_budget(
        &mut self,
        value: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.apply_custom_token_budget(value, window, cx);
    }

    pub(crate) fn set_model_open(
        &mut self,
        open: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let picker = open.then(|| self.states.model.read(cx).picker.clone());
        self.states.model.update(cx, |state, _| {
            state.open = open;
        });
        if let Some(picker) = picker {
            picker.update(cx, |picker, cx| picker.focus(window, cx));
        }
        if open {
            self.set_reasoning_open(false, window, cx);
            self.set_approval_open(false, window, cx);
        }
        cx.notify();
    }

    pub(crate) fn set_reasoning_open(
        &mut self,
        open: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let has_options = !self
            .states
            .reasoning
            .read(cx)
            .capability
            .as_ref()
            .and_then(|capability| capability.reasoning.as_ref())
            .map(|reasoning| reasoning_selections(Some(reasoning)).is_empty())
            .unwrap_or(true);
        let should_focus = open && has_options;
        let picker = should_focus.then(|| self.states.reasoning.read(cx).picker.clone());
        self.states.reasoning.update(cx, |state, _| {
            state.open = should_focus;
        });
        if let Some(picker) = picker {
            picker.update(cx, |picker, cx| picker.focus(window, cx));
        }
        if open {
            self.set_model_open(false, window, cx);
            self.set_approval_open(false, window, cx);
        }
        cx.notify();
    }

    pub(crate) fn set_approval_open(
        &mut self,
        open: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let picker = open.then(|| self.states.approval.read(cx).picker.clone());
        self.states.approval.update(cx, |state, _| {
            state.open = open;
        });
        if let Some(picker) = picker {
            picker.update(cx, |picker, cx| picker.focus(window, cx));
        }
        if open {
            self.set_model_open(false, window, cx);
            self.set_reasoning_open(false, window, cx);
        }
        cx.notify();
    }

    fn select_model_from_picker(
        &mut self,
        key: ProviderModelKey,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_model(key, SelectionOrigin::Picker, window, cx);
    }

    fn select_model(
        &mut self,
        key: ProviderModelKey,
        origin: SelectionOrigin,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let draft = self.form.read(cx).draft();
        let choices = self.states.model.read(cx).choices.clone();
        let capability =
            selected_model_choice(&choices, Some(&key)).map(|choice| choice.capabilities.clone());
        let reasoning = resolve_reasoning_selection(capability.as_ref(), None);
        self.write_form_values_with_cause(
            &RunSettingsInput::new(Some(key.clone()), reasoning.clone(), draft.approval_mode),
            gpui_form::FieldChangeCause::UserInput,
            cx,
        );
        self.states.reasoning.update(cx, |state, _| {
            state.capability = capability.clone();
        });
        if origin.should_sync_picker() {
            self.sync_model_picker(Some(key), window, cx);
        }
        self.sync_reasoning_picker(capability, reasoning, window, cx);
        self.sync_token_budget_input(window, cx);
        self.set_model_open(false, window, cx);
        cx.notify();
    }

    fn select_reasoning_from_picker(
        &mut self,
        selection: ReasoningSelectionSnapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_reasoning(selection, SelectionOrigin::Picker, window, cx);
    }

    fn select_reasoning(
        &mut self,
        selection: ReasoningSelectionSnapshot,
        origin: SelectionOrigin,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut draft = self.form.read(cx).draft();
        draft.reasoning_selection = Some(selection.clone());
        self.write_form_values_with_cause(&draft, gpui_form::FieldChangeCause::UserInput, cx);
        let capability = self.states.reasoning.read(cx).capability.clone();
        self.states.reasoning.update(cx, |state, _| {
            state.open = false;
        });
        if origin.should_sync_picker() {
            self.sync_reasoning_picker(capability, Some(selection), window, cx);
        }
        self.sync_token_budget_input(window, cx);
        cx.notify();
    }

    fn select_approval_from_picker(
        &mut self,
        mode: ToolApprovalMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_approval(mode, SelectionOrigin::Picker, window, cx);
    }

    fn select_approval(
        &mut self,
        mode: ToolApprovalMode,
        origin: SelectionOrigin,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut draft = self.form.read(cx).draft();
        draft.approval_mode = mode;
        self.write_form_values_with_cause(&draft, gpui_form::FieldChangeCause::UserInput, cx);
        self.states.approval.update(cx, |state, _| {
            state.open = false;
        });
        if origin.should_sync_picker() {
            self.sync_approval_picker(mode, window, cx);
        }
        cx.notify();
    }

    fn current_token_budget_bounds(&self, cx: &App) -> Option<TokenBudgetBounds> {
        self.states
            .reasoning
            .read(cx)
            .capability
            .as_ref()
            .and_then(|capability| token_budget_bounds(capability.reasoning.as_ref()))
    }

    fn apply_custom_token_budget(
        &mut self,
        value: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(bounds) = self.current_token_budget_bounds(cx) else {
            return;
        };
        let value = bounds.clamp(value);
        let selection = ReasoningSelectionSnapshot::TokenBudget {
            mode: TokenBudgetSelectionMode::Custom,
            value: Some(value),
        };
        let mut draft = self.form.read(cx).draft();
        draft.reasoning_selection = Some(selection.clone());
        self.write_form_values_with_cause(&draft, gpui_form::FieldChangeCause::UserInput, cx);
        let capability = self.states.reasoning.read(cx).capability.clone();
        cx.defer_in(window, move |controller, window, cx| {
            controller.sync_reasoning_picker(capability, Some(selection), window, cx);
            controller.sync_token_budget_input(window, cx);
        });
        cx.notify();
    }

    fn sync_token_budget_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bounds) = self.current_token_budget_bounds(cx) else {
            return;
        };
        let selected = self.form.read(cx).draft().reasoning_selection;
        let value = custom_token_budget_value(selected.as_ref())
            .map(|value| bounds.clamp(value))
            .unwrap_or(bounds.default_value);
        let token_budget_input = self.states.reasoning.read(cx).token_budget_input.clone();
        token_budget_input.update(cx, |input, cx| {
            if input.value().as_ref() != value.to_string() {
                input.set_value(value.to_string(), window, cx);
            }
        });
    }

    fn sync_model_picker(
        &self,
        selected: Option<ProviderModelKey>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let (choices, picker) = {
            let state = self.states.model.read(cx);
            (state.choices.clone(), state.picker.clone())
        };
        let sections = model_sections(choices.as_ref().map(Vec::as_slice).unwrap_or(&[]));
        picker.update(cx, |picker, cx| {
            picker.delegate_mut().set_sections(sections);
            picker.delegate_mut().set_selected_value(selected);
            let ix = picker.delegate().selected_index();
            picker.set_selected_index(ix, window, cx);
        });
    }

    fn sync_reasoning_picker(
        &self,
        capability: Option<ModelCapabilitiesSnapshot>,
        selected: Option<ReasoningSelectionSnapshot>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let picker = self.states.reasoning.read(cx).picker.clone();
        let sections =
            effort_select::effort_sections(capability.as_ref(), cx.global::<foundation::I18n>());
        picker.update(cx, |picker, cx| {
            picker.delegate_mut().set_sections(sections);
            picker.delegate_mut().set_selected_value(selected);
            let ix = picker.delegate().selected_index();
            picker.set_selected_index(ix, window, cx);
        });
    }

    fn sync_approval_picker(&self, selected: ToolApprovalMode, window: &mut Window, cx: &mut App) {
        let picker = self.states.approval.read(cx).picker.clone();
        picker.update(cx, |picker, cx| {
            picker.delegate_mut().set_selected_value(Some(selected));
            let ix = picker.delegate().selected_index();
            picker.set_selected_index(ix, window, cx);
        });
    }

    fn write_form_values(&self, values: &RunSettingsInput, cx: &mut App) {
        self.write_form_values_with_cause(values, gpui_form::FieldChangeCause::External, cx);
    }

    fn write_form_values_with_cause(
        &self,
        values: &RunSettingsInput,
        cause: gpui_form::FieldChangeCause,
        cx: &mut App,
    ) {
        let values = values.clone();
        let model = RunSettingsFormStore::model_handle(&self.form);
        let reasoning = RunSettingsFormStore::reasoning_selection_handle(&self.form);
        let approval = RunSettingsFormStore::approval_mode_handle(&self.form);
        if model.draft(cx).ok().as_ref() != Some(&values.model) {
            let _ = model.set_draft(values.model, cause, cx);
        }
        if reasoning.draft(cx).ok().as_ref() != Some(&values.reasoning_selection) {
            let _ = reasoning.set_draft(values.reasoning_selection, cause, cx);
        }
        if approval.draft(cx).ok().as_ref() != Some(&values.approval_mode) {
            let _ = approval.set_draft(values.approval_mode, cause, cx);
        }
    }
}

fn load_model_choices(cx: &App) -> Result<Vec<ProviderModelChoice>, SharedString> {
    state::providers::enabled_provider_models(cx).map_err(|err| err.to_string().into())
}

fn selected_model_choice<'a>(
    choices: &'a Result<Vec<ProviderModelChoice>, SharedString>,
    key: Option<&ProviderModelKey>,
) -> Option<&'a ProviderModelChoice> {
    selected_model_choice_from_slice(choices.as_ref().ok()?, key)
}

fn selected_model_choice_from_slice<'a>(
    choices: &'a [ProviderModelChoice],
    key: Option<&ProviderModelKey>,
) -> Option<&'a ProviderModelChoice> {
    let key = key?;
    choices.iter().find(|choice| &choice.key() == key)
}

fn resolve_model_key(
    choices: &Result<Vec<ProviderModelChoice>, SharedString>,
    requested: Option<&ProviderModelKey>,
    fallback_to_first_model: bool,
) -> Option<ProviderModelKey> {
    let selected = requested
        .filter(|key| selected_model_choice(choices, Some(key)).is_some())
        .cloned();
    if selected.is_some() || !fallback_to_first_model {
        selected
    } else {
        choices
            .as_ref()
            .ok()
            .and_then(|choices| choices.first().map(ProviderModelChoice::key))
    }
}

fn resolve_reasoning_selection(
    capability: Option<&ModelCapabilitiesSnapshot>,
    requested: Option<&ReasoningSelectionSnapshot>,
) -> Option<ReasoningSelectionSnapshot> {
    requested
        .filter(|selection| {
            capability
                .and_then(|capability| capability.reasoning.as_ref())
                .is_some_and(|reasoning| reasoning_selection_is_valid(Some(reasoning), selection))
        })
        .cloned()
        .or_else(|| {
            capability.and_then(|capability| {
                computed_default_reasoning_selection(capability.reasoning.as_ref())
            })
        })
}

fn model_empty_label(
    choices: &Result<Vec<ProviderModelChoice>, SharedString>,
    i18n: &I18n,
) -> SharedString {
    match choices {
        Ok(_) => i18n.t("chat-form-model-none-configured").into(),
        Err(err) => format!("{}: {}", i18n.t("chat-form-model-load-failed"), err).into(),
    }
}

fn initial_token_budget_value(
    capability: Option<&ModelCapabilitiesSnapshot>,
    selected: Option<&ReasoningSelectionSnapshot>,
) -> u32 {
    capability
        .and_then(|capability| token_budget_bounds(capability.reasoning.as_ref()))
        .map(|bounds| custom_token_budget_value(selected).unwrap_or(bounds.default_value))
        .unwrap_or(1024)
}

pub(crate) fn render_model_selector(
    form: Entity<RunSettingsFormStore>,
    state: Entity<ModelControlState>,
    enabled: bool,
    cx: &mut App,
) -> AnyElement {
    let state_snapshot = state.read(cx);
    let selected = form.read(cx).draft().model;
    let i18n = cx.global::<I18n>();
    let label: SharedString = match &state_snapshot.choices {
        Err(_) => i18n.t("chat-form-model-load-failed").into(),
        Ok(_) => selected
            .as_ref()
            .and_then(|key| selected_model_choice(&state_snapshot.choices, Some(key)))
            .map(|choice| choice.display_label().into())
            .unwrap_or_else(|| i18n.t("chat-form-model-empty").into()),
    };
    let open = enabled && state_snapshot.open;
    let list = state_snapshot.picker.clone();
    let on_open_change = state_snapshot.on_open_change.clone();
    let show_provider_footer = !state_snapshot
        .choices
        .as_ref()
        .is_ok_and(|choices| !choices.is_empty());
    let trigger = crate::components::picker::picker_trigger(
        "chat-form-model-trigger",
        crate::foundation::assets::IconName::Sparkles,
        label,
        open,
    )
    .disabled(!enabled);
    crate::components::picker::picker_popover(
        cx,
        crate::components::picker::PickerPopoverConfig {
            id: "chat-form-model-popover",
            open,
            trigger,
            list,
            width: px(340.),
            max_height: rems(18.).into(),
            search_placeholder: Some(i18n.t("chat-form-model-search-placeholder").into()),
            footer: show_provider_footer.then(|| render_model_picker_footer(cx)),
            on_open_change: move |open, window, cx| {
                on_open_change(*open, window, cx);
            },
        },
    )
    .into_any_element()
}

fn render_model_picker_footer(cx: &App) -> AnyElement {
    div()
        .border_t_1()
        .border_color(cx.theme().border)
        .p_1()
        .child(
            Button::new("chat-form-configure-providers")
                .ghost()
                .icon(crate::foundation::assets::IconName::Settings)
                .label(cx.global::<I18n>().t("chat-form-configure-providers"))
                .small()
                .w_full()
                .on_click(|_, _window, cx| {
                    settings::open_settings_window_to_provider(cx);
                }),
        )
        .into_any_element()
}

pub(crate) fn render_reasoning_selector(
    form: Entity<RunSettingsFormStore>,
    state: Entity<ReasoningControlState>,
    enabled: bool,
    cx: &mut App,
) -> AnyElement {
    let (label, has_options, open, picker, capability, token_budget_input, on_open_change) = {
        let snapshot = state.read(cx);
        let selected = form.read(cx).draft().reasoning_selection;
        let label = selected
            .as_ref()
            .map(|selection| reasoning_selection_label(selection, cx.global::<I18n>()))
            .unwrap_or_else(|| cx.global::<I18n>().t("chat-form-effort-select"));
        let has_options = snapshot
            .capability
            .as_ref()
            .and_then(|capability| capability.reasoning.as_ref())
            .is_some_and(|reasoning| !reasoning_selections(Some(reasoning)).is_empty());
        (
            label,
            has_options,
            enabled && snapshot.open,
            snapshot.picker.clone(),
            snapshot.capability.clone(),
            snapshot.token_budget_input.clone(),
            snapshot.on_open_change.clone(),
        )
    };
    let footer = token_budget_footer(capability.as_ref(), token_budget_input, enabled, cx);
    crate::components::picker::picker_popover(
        cx,
        crate::components::picker::PickerPopoverConfig {
            id: "chat-form-effort-popover",
            open,
            trigger: crate::components::picker::picker_trigger(
                "chat-form-effort-trigger",
                crate::foundation::assets::IconName::Lightbulb,
                label,
                open,
            )
            .disabled(!enabled || !has_options),
            list: picker,
            width: px(180.),
            max_height: rems(16.).into(),
            search_placeholder: None,
            footer,
            on_open_change: move |open, window, cx| {
                on_open_change(*open, window, cx);
            },
        },
    )
    .into_any_element()
}

fn token_budget_footer(
    capability: Option<&ModelCapabilitiesSnapshot>,
    token_budget_input: Entity<InputState>,
    enabled: bool,
    cx: &mut App,
) -> Option<AnyElement> {
    let bounds = capability
        .as_ref()
        .and_then(|capability| token_budget_bounds(capability.reasoning.as_ref()))?;
    Some(
        h_flex()
            .items_center()
            .gap_2()
            .px_1()
            .py_1()
            .child(
                Label::new(cx.global::<I18n>().t("chat-form-effort-token-budget"))
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .truncate(),
            )
            .child(
                NumberInput::new(&token_budget_input)
                    .small()
                    .w(px(112.))
                    .disabled(!enabled),
            )
            .when(bounds.min == bounds.max, |this| this.opacity(0.7))
            .into_any_element(),
    )
}

pub(crate) fn render_approval_selector(
    form: Entity<RunSettingsFormStore>,
    state: Entity<ApprovalControlState>,
    enabled: bool,
    cx: &mut App,
) -> AnyElement {
    let snapshot = state.read(cx);
    let selected = form.read(cx).draft().approval_mode;
    let on_open_change = snapshot.on_open_change.clone();
    crate::components::picker::picker_popover(
        cx,
        crate::components::picker::PickerPopoverConfig {
            id: "chat-form-approval-popover",
            open: enabled && snapshot.open,
            trigger: crate::components::picker::picker_trigger(
                "chat-form-approval-trigger",
                crate::foundation::assets::IconName::Shield,
                approval_select::approval_mode_label(selected, cx.global::<I18n>()),
                enabled && snapshot.open,
            )
            .disabled(!enabled),
            list: snapshot.picker.clone(),
            width: px(180.),
            max_height: rems(12.).into(),
            search_placeholder: None,
            footer: None,
            on_open_change: move |open, window, cx| {
                on_open_change(*open, window, cx);
            },
        },
    )
    .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{
        ModelResolutionPolicy, RunSettingsInput, RunSettingsSubmitError, SelectionOrigin,
        resolve_model_key, resolve_run_settings,
    };
    use crate::state::providers::{ProviderModelChoice, ProviderModelKey};
    use jaco_core::conservative_model_capabilities;

    #[test]
    fn model_resolution_can_preserve_unavailable_shortcut_selection() {
        let choices = Ok(vec![choice("openai", "gpt-5")]);
        let unavailable = ProviderModelKey {
            provider_id: "openai".to_string(),
            model_id: "retired-model".to_string(),
        };
        let available = ProviderModelKey {
            provider_id: "openai".to_string(),
            model_id: "gpt-5".to_string(),
        };

        assert_eq!(
            resolve_model_key(&choices, Some(&unavailable), true),
            Some(available.clone())
        );
        assert_eq!(resolve_model_key(&choices, Some(&unavailable), false), None);
        assert_eq!(
            resolve_model_key(&choices, Some(&available), false),
            Some(available)
        );
    }

    #[test]
    fn picker_selection_does_not_reconcile_its_source_list() {
        assert!(!SelectionOrigin::Picker.should_sync_picker());
        assert!(SelectionOrigin::External.should_sync_picker());
    }

    #[test]
    fn submit_resolver_applies_policy_without_mutating_the_draft() {
        let unavailable = ProviderModelKey {
            provider_id: "openai".to_string(),
            model_id: "retired-model".to_string(),
        };
        let draft = RunSettingsInput::new(
            Some(unavailable.clone()),
            None,
            jaco_core::ToolApprovalMode::RequestApproval,
        );
        let choices = Ok(vec![choice("openai", "gpt-5")]);

        let fallback = resolve_run_settings(
            &draft,
            &choices,
            ModelResolutionPolicy::FallbackToFirstEnabled,
        )
        .expect("fallback policy resolves an unavailable model");
        assert_eq!(fallback.provider_model.model_id, "gpt-5");
        assert_eq!(draft.model, Some(unavailable.clone()));

        assert_eq!(
            resolve_run_settings(&draft, &choices, ModelResolutionPolicy::RequireSelected),
            Err(RunSettingsSubmitError::ModelUnavailable(unavailable))
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

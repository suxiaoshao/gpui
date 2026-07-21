//! Shared model, reasoning and tool-access state.
//!
//! The form data in this module is deliberately independent from `ChatForm`.
//! A caller owns the form entity (and therefore persistence/validation), while
//! `RunSettingsController` owns the picker state and keeps it in sync with the
//! provider catalog. The form draft is the source of truth for business values;
//! control state only projects those values for rendering and focus.

mod policy;

use std::ops::Deref;
use std::{marker::PhantomData, rc::Rc};

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
    input::{InputState, NumberInput},
    label::Label,
    list::ListState,
};
use gpui_form::typed::{FormField, FormStore};
use gpui_form_gpui_component::integer_input::{
    FormIntegerInput, IntegerInputPolicy, IntegerInputState,
};
use jaco_core::{ModelCapabilitiesSnapshot, ReasoningSelectionSnapshot, ToolApprovalMode};

pub(crate) use policy::{
    custom_token_budget_value, reasoning_selection_is_valid, reasoning_selection_label,
    reasoning_selections, set_existing_custom_token_budget, token_budget_bounds,
};

pub(crate) type ControlOpenHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = RunSettingsFormStore)]
pub(crate) struct RunSettingsInput {
    #[form(required)]
    pub(crate) model: Option<ProviderModelKey>,
    pub(crate) reasoning_selection: Option<ReasoningSelectionSnapshot>,
    pub(crate) approval_mode: ToolApprovalMode,
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
    ReasoningUnsupported(ReasoningSelectionSnapshot),
    TokenBudgetInvalid(u32),
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
) -> Result<RunSettingsSubmitSnapshot, RunSettingsSubmitError> {
    let choices = choices
        .as_ref()
        .map_err(|_| RunSettingsSubmitError::CatalogUnavailable)?;
    let selected = draft
        .model
        .as_ref()
        .and_then(|key| selected_model_choice_from_slice(choices, Some(key)))
        .ok_or_else(|| match draft.model.clone() {
            None => RunSettingsSubmitError::ModelRequired,
            Some(key) => RunSettingsSubmitError::ModelUnavailable(key),
        })?;

    let reasoning = selected.capabilities.reasoning.as_ref();
    if let Some(value) = custom_token_budget_value(draft.reasoning_selection.as_ref())
        && token_budget_bounds(reasoning).is_some_and(|bounds| {
            bounds.min.is_some_and(|min| value < min) || bounds.max.is_some_and(|max| value > max)
        })
    {
        return Err(RunSettingsSubmitError::TokenBudgetInvalid(value));
    }

    let reasoning_selection = match draft.reasoning_selection.as_ref() {
        Some(requested)
            if reasoning.is_some_and(|reasoning| {
                reasoning_selection_is_valid(Some(reasoning), requested)
            }) =>
        {
            Some(requested.clone())
        }
        Some(requested) => {
            return Err(RunSettingsSubmitError::ReasoningUnsupported(
                requested.clone(),
            ));
        }
        None => None,
    };

    Ok(RunSettingsSubmitSnapshot {
        provider_model: selected.clone(),
        reasoning_selection,
        approval_mode: draft.approval_mode,
    })
}

pub(crate) struct ModelControlState {
    pub(crate) choices: Result<Vec<ProviderModelChoice>, SharedString>,
    pub(crate) selected: Option<ProviderModelKey>,
    pub(crate) picker: Entity<ListState<PickerListDelegate<ModelOption>>>,
    pub(crate) open: bool,
    pub(crate) on_open_change: ControlOpenHandler,
}

pub(crate) struct ReasoningControlState {
    pub(crate) capability: Option<ModelCapabilitiesSnapshot>,
    pub(crate) selected: Option<ReasoningSelectionSnapshot>,
    pub(crate) picker: Entity<ListState<PickerListDelegate<effort_select::EffortOption>>>,
    pub(crate) token_budget_input: Option<Entity<InputState>>,
    pub(crate) open: bool,
    pub(crate) on_open_change: ControlOpenHandler,
}

pub(crate) struct ApprovalControlState {
    pub(crate) selected: ToolApprovalMode,
    pub(crate) picker: Entity<ListState<PickerListDelegate<approval_select::ApprovalModeOption>>>,
    pub(crate) open: bool,
    pub(crate) on_open_change: ControlOpenHandler,
}

#[derive(Clone)]
pub(crate) struct RunSettingsControlStates {
    pub(crate) model: Entity<ModelControlState>,
    pub(crate) reasoning: Entity<ReasoningControlState>,
    pub(crate) approval: Entity<ApprovalControlState>,
}

pub(crate) struct FormModelPicker {
    subscriptions: Vec<Subscription>,
    state: Entity<ModelControlState>,
}

impl Deref for FormModelPicker {
    type Target = Entity<ModelControlState>;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl Drop for FormModelPicker {
    fn drop(&mut self) {
        self.subscriptions.clear();
    }
}

pub(crate) struct FormReasoningPicker {
    subscriptions: Vec<Subscription>,
    state: Entity<ReasoningControlState>,
}

impl Deref for FormReasoningPicker {
    type Target = Entity<ReasoningControlState>;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl Drop for FormReasoningPicker {
    fn drop(&mut self) {
        self.subscriptions.clear();
    }
}

pub(crate) struct FormApprovalPicker {
    subscriptions: Vec<Subscription>,
    state: Entity<ApprovalControlState>,
}

impl Deref for FormApprovalPicker {
    type Target = Entity<ApprovalControlState>;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl Drop for FormApprovalPicker {
    fn drop(&mut self) {
        self.subscriptions.clear();
    }
}

pub(crate) struct RunSettingsBoundControls {
    model: FormModelPicker,
    reasoning: FormReasoningPicker,
    approval: FormApprovalPicker,
    token_budget: Option<FormIntegerInput<u32>>,
}

pub(crate) struct RunSettingsController<Form>
where
    Form: FormStore,
{
    model_field: FormField<Form, Option<ProviderModelKey>>,
    reasoning_field: FormField<Form, Option<ReasoningSelectionSnapshot>>,
    approval_field: FormField<Form, ToolApprovalMode>,
    orchestration_subscriptions: Vec<Subscription>,
    controls: RunSettingsBoundControls,
    marker: PhantomData<Form>,
}

impl<Form> Drop for RunSettingsController<Form>
where
    Form: FormStore,
{
    fn drop(&mut self) {
        self.orchestration_subscriptions.clear();
    }
}

impl<Form> RunSettingsController<Form>
where
    Form: FormStore + EventEmitter<gpui_form::typed::FormEvent<Form::Field>>,
{
    pub(crate) fn new(
        field: gpui_form::typed::FormField<Form, RunSettingsInput>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let model_field = RunSettingsFormStore::model_in(field.clone());
        let reasoning_field = RunSettingsFormStore::reasoning_selection_in(field.clone());
        let approval_field = RunSettingsFormStore::approval_mode_in(field.clone());
        let model_attachment = model_field
            .attach_control(cx)
            .expect("run-settings model field is available");
        let reasoning_attachment = reasoning_field
            .attach_control(cx)
            .expect("run-settings reasoning field is available");
        let approval_attachment = approval_field
            .attach_control(cx)
            .expect("run-settings approval field is available");
        let draft = field
            .value(cx)
            .expect("run-settings field is available while its controller is alive");
        let choices = load_model_choices(cx);
        let selected_model = resolve_model_key(&choices, draft.model.as_ref());
        let capability = selected_model_choice(&choices, selected_model.as_ref())
            .map(|choice| choice.capabilities.clone());
        let selected_reasoning = draft.reasoning_selection.clone();
        let approval = draft.approval_mode;
        let state = cx.entity().downgrade();

        let model_sections = model_sections(choices.as_ref().map(Vec::as_slice).unwrap_or(&[]));
        let model_selected_ix =
            PickerListDelegate::selected_index_for(&model_sections, selected_model.as_ref());
        let model_confirm = Rc::new({
            let state = state.clone();
            let attachment = model_attachment.clone();
            move |option: ModelOption, window: &mut Window, cx: &mut App| {
                let attachment = attachment.clone();
                let _ = state.update(cx, |controller, cx| {
                    attachment.defer_set_user_value(Some(option.key()), window, cx);
                    controller.set_model_open(false, window, cx);
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
            let attachment = reasoning_attachment.clone();
            move |option: effort_select::EffortOption, window: &mut Window, cx: &mut App| {
                let attachment = attachment.clone();
                let _ = state.update(cx, |controller, cx| {
                    attachment.defer_set_user_value(Some(option.selection().clone()), window, cx);
                    controller.set_reasoning_open(false, window, cx);
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
        let token_budget_field = reasoning_field.project_value(
            "token_budget",
            |value| custom_token_budget_value(value.as_ref()),
            set_existing_custom_token_budget,
        );
        let token_budget_control = token_budget_bounds(
            capability
                .as_ref()
                .and_then(|capability| capability.reasoning.as_ref()),
        )
        .and_then(|_| {
            let policy = token_budget_policy(capability.as_ref());
            FormIntegerInput::new(
                token_budget_field,
                move |window, cx| integer_input_state(policy, window, cx),
                window,
                cx,
            )
            .ok()
        });
        let token_budget_input = token_budget_control
            .as_ref()
            .map(|control| control.read(cx).editor().clone());

        let approval_sections = approval_select::approval_mode_sections(cx.global::<I18n>());
        let approval_selected_ix =
            PickerListDelegate::selected_index_for(&approval_sections, Some(&approval));
        let approval_confirm = Rc::new({
            let state = state.clone();
            let attachment = approval_attachment.clone();
            move |option: approval_select::ApprovalModeOption, window: &mut Window, cx: &mut App| {
                let attachment = attachment.clone();
                let _ = state.update(cx, |controller, cx| {
                    attachment.defer_set_user_value(option.mode(), window, cx);
                    controller.set_approval_open(false, window, cx);
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

        let model_state = cx.new(|_| ModelControlState {
            choices,
            selected: draft.model.clone(),
            picker: model_picker,
            open: false,
            on_open_change: model_open_change,
        });
        let reasoning_state = cx.new(|_| ReasoningControlState {
            capability,
            selected: draft.reasoning_selection.clone(),
            picker: reasoning_picker,
            token_budget_input,
            open: false,
            on_open_change: reasoning_open_change,
        });
        let approval_state = cx.new(|_| ApprovalControlState {
            selected: approval,
            picker: approval_picker,
            open: false,
            on_open_change: approval_open_change,
        });
        let model_subscription = model_field
            .subscribe_in(window, cx, {
                let field = model_field.clone();
                move |_controller, window, cx| {
                    let field = field.clone();
                    cx.defer_in(window, move |controller, window, cx| {
                        let Ok(value) = field.value(cx) else { return };
                        controller.sync_model_from_form(value, window, cx);
                    });
                }
            })
            .expect("run-settings model field is alive");
        let reasoning_subscription = reasoning_field
            .subscribe_in(window, cx, {
                let field = reasoning_field.clone();
                move |_controller, window, cx| {
                    let field = field.clone();
                    cx.defer_in(window, move |controller, window, cx| {
                        let Ok(value) = field.value(cx) else { return };
                        controller.sync_reasoning_from_form(value, window, cx);
                    });
                }
            })
            .expect("run-settings reasoning field is alive");
        let approval_subscription = approval_field
            .subscribe_in(window, cx, {
                let field = approval_field.clone();
                move |_controller, window, cx| {
                    let field = field.clone();
                    cx.defer_in(window, move |controller, window, cx| {
                        let Ok(value) = field.value(cx) else { return };
                        controller.sync_approval_picker(value, window, cx);
                    });
                }
            })
            .expect("run-settings approval field is alive");

        let mut orchestration_subscriptions = Vec::new();
        if cx.has_global::<state::providers::ProviderCatalogGlobal>() {
            let catalog = state::providers::catalog(cx);
            orchestration_subscriptions.push(cx.observe_in(
                &catalog.entity(),
                window,
                |controller, _catalog, window, cx| {
                    controller.reload_models(window, cx);
                },
            ));
        }

        Self {
            model_field,
            reasoning_field,
            approval_field,
            orchestration_subscriptions,
            controls: RunSettingsBoundControls {
                model: FormModelPicker {
                    subscriptions: vec![model_subscription],
                    state: model_state,
                },
                reasoning: FormReasoningPicker {
                    subscriptions: vec![reasoning_subscription],
                    state: reasoning_state,
                },
                approval: FormApprovalPicker {
                    subscriptions: vec![approval_subscription],
                    state: approval_state,
                },
                token_budget: token_budget_control,
            },
            marker: PhantomData,
        }
    }

    pub(crate) fn control_states(&self) -> RunSettingsControlStates {
        RunSettingsControlStates {
            model: self.controls.model.state.clone(),
            reasoning: self.controls.reasoning.state.clone(),
            approval: self.controls.approval.state.clone(),
        }
    }

    pub(crate) fn value(&self, cx: &App) -> Option<RunSettingsInput> {
        Some(RunSettingsInput::new(
            self.model_field.value(cx).ok()?,
            self.reasoning_field.value(cx).ok()?,
            self.approval_field.value(cx).ok()?,
        ))
    }

    #[cfg(test)]
    pub(crate) fn selected_model(&self, cx: &App) -> Option<ProviderModelChoice> {
        let selected = self.model_field.value(cx).ok()?;
        let state = self.controls.model.read(cx);
        selected_model_choice(&state.choices, selected.as_ref()).cloned()
    }

    pub(crate) fn reload_models(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let choices = load_model_choices(cx);
        let Ok(previous_key) = self.model_field.value(cx) else {
            return;
        };
        let Ok(previous_reasoning) = self.reasoning_field.value(cx) else {
            return;
        };
        // A catalog/options refresh must not rebase the form draft.  Keep an
        // unavailable selected key in the form so the submit policy can make
        // the explicit fallback/require decision later.
        let selected = previous_key
            .as_ref()
            .filter(|key| selected_model_choice(&choices, Some(key)).is_some())
            .cloned();
        let preserved_reasoning = previous_reasoning;
        let capability = selected_model_choice(&choices, selected.as_ref())
            .map(|choice| choice.capabilities.clone());
        let reasoning = preserved_reasoning;

        self.controls.model.update(cx, |state, _| {
            state.choices = choices.clone();
        });
        self.controls.reasoning.update(cx, |state, _| {
            state.capability = capability.clone();
        });
        self.sync_model_picker(selected.clone(), window, cx);
        self.sync_reasoning_picker(capability.clone(), reasoning.clone(), window, cx);
        self.sync_token_budget_control(window, cx);
        cx.notify();
    }

    #[cfg(test)]
    pub(crate) fn select_model_value(
        &mut self,
        key: ProviderModelKey,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _ = self.model_field.set_user_value(Some(key.clone()), cx);
        self.sync_model_picker(Some(key), window, cx);
    }

    #[cfg(test)]
    pub(crate) fn select_approval_value(
        &mut self,
        mode: ToolApprovalMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _ = self.approval_field.set_user_value(mode, cx);
        self.sync_approval_picker(mode, window, cx);
    }

    #[cfg(test)]
    pub(crate) fn select_reasoning_value(
        &mut self,
        selection: ReasoningSelectionSnapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _ = self
            .reasoning_field
            .set_user_value(Some(selection.clone()), cx);
        let capability = self.controls.reasoning.read(cx).capability.clone();
        self.sync_reasoning_picker(capability, Some(selection), window, cx);
        self.sync_token_budget_control(window, cx);
    }

    #[cfg(test)]
    pub(crate) fn set_custom_token_budget(
        &mut self,
        value: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _ = window;
        let Ok(mut reasoning) = self.reasoning_field.value(cx) else {
            return;
        };
        if set_existing_custom_token_budget(&mut reasoning, value) {
            let _ = self.reasoning_field.set_user_value(reasoning, cx);
        }
    }

    pub(crate) fn set_model_open(
        &mut self,
        open: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let picker = open.then(|| self.controls.model.read(cx).picker.clone());
        self.controls.model.update(cx, |state, _| {
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
            .controls
            .reasoning
            .read(cx)
            .capability
            .as_ref()
            .and_then(|capability| capability.reasoning.as_ref())
            .map(|reasoning| reasoning_selections(Some(reasoning)).is_empty())
            .unwrap_or(true);
        let should_focus = open && has_options;
        let picker = should_focus.then(|| self.controls.reasoning.read(cx).picker.clone());
        self.controls.reasoning.update(cx, |state, _| {
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
        let picker = open.then(|| self.controls.approval.read(cx).picker.clone());
        self.controls.approval.update(cx, |state, _| {
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

    fn sync_token_budget_control(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let capability = self.controls.reasoning.read(cx).capability.clone();
        let supports_token_budget = token_budget_bounds(
            capability
                .as_ref()
                .and_then(|capability| capability.reasoning.as_ref()),
        )
        .is_some();
        let has_custom_value = self
            .reasoning_field
            .value(cx)
            .ok()
            .flatten()
            .as_ref()
            .and_then(|selection| custom_token_budget_value(Some(selection)))
            .is_some();

        if !supports_token_budget || !has_custom_value {
            self.controls.token_budget = None;
            self.controls.reasoning.update(cx, |state, cx| {
                state.token_budget_input = None;
                cx.notify();
            });
            return;
        }

        let policy = token_budget_policy(capability.as_ref());
        if let Some(control) = self.controls.token_budget.as_ref() {
            control.update(cx, |control, _| {
                let _ = control.set_policy(policy);
            });
            return;
        }

        let token_budget_field = self.reasoning_field.project_value(
            "token_budget",
            |value| custom_token_budget_value(value.as_ref()),
            set_existing_custom_token_budget,
        );
        let Ok(control) = FormIntegerInput::new(
            token_budget_field,
            move |window, cx| integer_input_state(policy, window, cx),
            window,
            cx,
        ) else {
            return;
        };
        let input = control.read(cx).editor().clone();
        self.controls.token_budget = Some(control);
        self.controls.reasoning.update(cx, |state, cx| {
            state.token_budget_input = Some(input);
            cx.notify();
        });
    }

    fn sync_model_picker(
        &self,
        selected: Option<ProviderModelKey>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let (choices, picker) = self.controls.model.update(cx, |state, cx| {
            state.selected = selected.clone();
            cx.notify();
            (state.choices.clone(), state.picker.clone())
        });
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
        let picker = self.controls.reasoning.update(cx, |state, cx| {
            state.selected = selected.clone();
            cx.notify();
            state.picker.clone()
        });
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
        let picker = self.controls.approval.update(cx, |state, cx| {
            state.selected = selected;
            cx.notify();
            state.picker.clone()
        });
        picker.update(cx, |picker, cx| {
            picker.delegate_mut().set_selected_value(Some(selected));
            let ix = picker.delegate().selected_index();
            picker.set_selected_index(ix, window, cx);
        });
    }

    fn sync_model_from_form(
        &mut self,
        model: Option<ProviderModelKey>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let capability =
            selected_model_choice(&self.controls.model.read(cx).choices, model.as_ref())
                .map(|choice| choice.capabilities.clone());
        self.controls.reasoning.update(cx, |state, _| {
            state.capability = capability.clone();
        });
        self.sync_model_picker(model, window, cx);
        let reasoning = self.reasoning_field.value(cx).ok().flatten();
        self.sync_reasoning_picker(capability, reasoning, window, cx);
        self.sync_token_budget_control(window, cx);
    }

    fn sync_reasoning_from_form(
        &mut self,
        reasoning: Option<ReasoningSelectionSnapshot>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let capability = self.controls.reasoning.read(cx).capability.clone();
        self.sync_reasoning_picker(capability, reasoning, window, cx);
        self.sync_token_budget_control(window, cx);
    }
}

fn load_model_choices(cx: &App) -> Result<Vec<ProviderModelChoice>, SharedString> {
    if !cx.has_global::<state::providers::ProviderCatalogGlobal>() {
        return Err("provider catalog is unavailable".into());
    }
    Ok(state::providers::catalog(cx).read_cloned(cx, |snapshot| &snapshot.enabled_models))
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
) -> Option<ProviderModelKey> {
    requested
        .filter(|key| selected_model_choice(choices, Some(key)).is_some())
        .cloned()
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

fn integer_input_state(
    policy: IntegerInputPolicy<u32>,
    window: &mut Window,
    cx: &mut Context<IntegerInputState<u32>>,
) -> IntegerInputState<u32> {
    let mut input = IntegerInputState::new(window, cx);
    if let Some(min) = policy.minimum() {
        input = input.min(min);
    }
    if let Some(max) = policy.maximum() {
        input = input.max(max);
    }
    input.step(policy.step_value())
}

fn token_budget_policy(capability: Option<&ModelCapabilitiesSnapshot>) -> IntegerInputPolicy<u32> {
    let Some(bounds) =
        capability.and_then(|capability| token_budget_bounds(capability.reasoning.as_ref()))
    else {
        return IntegerInputPolicy::new().step(1024);
    };
    let mut policy = IntegerInputPolicy::new().step(bounds.step());
    if let Some(min) = bounds.min {
        policy = policy.min(min);
    }
    if let Some(max) = bounds.max {
        policy = policy.max(max);
    }
    policy
}

pub(crate) fn render_model_selector(
    state: Entity<ModelControlState>,
    enabled: bool,
    cx: &mut App,
) -> AnyElement {
    let state_snapshot = state.read(cx);
    let selected = state_snapshot.selected.clone();
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
    state: Entity<ReasoningControlState>,
    enabled: bool,
    cx: &mut App,
) -> AnyElement {
    let (label, has_options, open, picker, capability, token_budget_input, on_open_change) = {
        let snapshot = state.read(cx);
        let selected = snapshot.selected.clone();
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
    token_budget_input: Option<Entity<InputState>>,
    enabled: bool,
    cx: &mut App,
) -> Option<AnyElement> {
    let token_budget_input = token_budget_input?;
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
    state: Entity<ApprovalControlState>,
    enabled: bool,
    cx: &mut App,
) -> AnyElement {
    let snapshot = state.read(cx);
    let selected = snapshot.selected;
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
        RunSettingsInput, RunSettingsSubmitError, resolve_model_key, resolve_run_settings,
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

        assert_eq!(resolve_model_key(&choices, Some(&unavailable)), None);
        assert_eq!(
            resolve_model_key(&choices, Some(&available)),
            Some(available)
        );
    }

    #[test]
    fn submit_resolver_rejects_an_unavailable_model_without_mutating_the_form() {
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

        assert_eq!(
            resolve_run_settings(&draft, &choices),
            Err(RunSettingsSubmitError::ModelUnavailable(unavailable))
        );
    }

    #[test]
    fn submit_resolver_requires_an_explicit_model_selection() {
        let draft = RunSettingsInput::new(None, None, jaco_core::ToolApprovalMode::RequestApproval);
        let choices = Ok(vec![choice("openai", "gpt-5")]);

        assert_eq!(
            resolve_run_settings(&draft, &choices),
            Err(RunSettingsSubmitError::ModelRequired)
        );
        assert_eq!(draft.model, None);
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

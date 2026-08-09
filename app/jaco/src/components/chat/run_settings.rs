//! Shared model, reasoning and tool-access state.
//!
//! The form data in this module is deliberately independent from `ChatForm`.
//! A caller owns the form entity (and therefore persistence/validation), while
//! `RunSettingsController` owns the picker state and keeps it in sync with the
//! provider catalog. The form draft is the source of truth for business values;
//! control state only projects those values for rendering and focus.

mod policy;

use std::{cell::RefCell, ops::Deref, rc::Rc};

use crate::{
    components::{
        chat::input::{approval_select, effort_select},
        chat::model_picker::{ModelOption, model_sections},
        picker::{
            PickerContentPopoverConfig, PickerListDelegate, PickerPopover, PickerPopoverConfig,
        },
        resource_status::refresh_status,
    },
    features::settings,
    foundation::{self, I18n},
    state,
    state::providers::{ProviderModelChoice, ProviderModelKey},
};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Disableable, Sizable, Size, StyledExt,
    button::Button,
    h_flex,
    input::{InputState, NumberInput},
    label::Label,
    list::{List, ListState},
    v_flex,
};
use gpui_form::{
    ControlBinding, ControlProjection, ControlWriter, Form, FormSchema, TotalPath,
    ValidationMessage,
};
use gpui_form_gpui_component::integer_input::{
    IntegerInputError, IntegerInputEvent, IntegerInputPolicy, IntegerInputState,
};
use jaco_core::{ModelCapabilitiesSnapshot, ReasoningSelectionSnapshot, ToolApprovalMode};

pub(crate) use policy::{
    custom_token_budget_value, reasoning_selection_is_valid, reasoning_selection_label,
    reasoning_selections, set_existing_custom_token_budget, token_budget_bounds,
};
use policy::{projected_reasoning_selection, reasoning_selection_after_model_change};

pub(crate) type ControlOpenHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;
type SettingsWriter<M> = ControlWriter<M, RunSettingsInput>;
type ReasoningWriter<M> = ControlWriter<M, Option<ReasoningSelectionSnapshot>>;
type ApprovalWriter<M> = ControlWriter<M, ToolApprovalMode>;

#[derive(Clone, Debug, PartialEq, gpui_form::FormSchema)]
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
    _binding: ControlBinding,
    state: Entity<ModelControlState>,
}

impl Deref for FormModelPicker {
    type Target = Entity<ModelControlState>;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

pub(crate) struct FormReasoningPicker {
    _binding: ControlBinding,
    state: Entity<ReasoningControlState>,
}

impl Deref for FormReasoningPicker {
    type Target = Entity<ReasoningControlState>;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

pub(crate) struct FormApprovalPicker {
    _binding: ControlBinding,
    state: Entity<ApprovalControlState>,
}

struct FormTokenBudgetInput {
    _binding: ControlBinding,
    _subscription: Subscription,
    state: Entity<IntegerInputState<u32>>,
}

impl Deref for FormTokenBudgetInput {
    type Target = Entity<IntegerInputState<u32>>;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl FormTokenBudgetInput {
    fn new<M: FormSchema, Owner: 'static>(
        form: &Entity<Form<M>>,
        path: TotalPath<M, Option<ReasoningSelectionSnapshot>>,
        policy: IntegerInputPolicy<u32>,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Option<Self> {
        let value = custom_token_budget_value(path.get(form, cx).as_ref())?;
        let state = cx.new(|cx| integer_input_state(policy, window, cx));
        state.update(cx, |state, cx| state.set_value(value, window, cx));
        let (binding, writer) = path.bind_control_in(
            form,
            &state,
            |state, projection, window, cx| match projection {
                ControlProjection::Value(selection) => {
                    if let Some(value) = custom_token_budget_value(selection.as_ref()) {
                        state.set_value(value, window, cx);
                    }
                }
                ControlProjection::Retired => {}
            },
            window,
            cx,
        );
        let weak_form = form.downgrade();
        let event_path = path;
        let subscription = cx.subscribe_in(
            &state,
            window,
            move |_, _, event: &IntegerInputEvent<u32>, window, cx| match event {
                IntegerInputEvent::Change(Ok(value)) => {
                    let Some(form) = weak_form.upgrade() else {
                        return;
                    };
                    let mut selection = event_path.get(&form, cx);
                    if set_existing_custom_token_budget(&mut selection, *value) {
                        writer.defer_clear_issue(window, cx);
                        writer.defer_set(selection, window, cx);
                    }
                }
                IntegerInputEvent::Change(Err(error)) => {
                    writer.defer_set_issue(
                        token_budget_error_code(*error),
                        ValidationMessage::key("gpui-form-error-integer-invalid"),
                        window,
                        cx,
                    );
                }
                IntegerInputEvent::Blur => writer.defer_blur(window, cx),
            },
        );

        Some(Self {
            _binding: binding,
            _subscription: subscription,
            state,
        })
    }
}

fn token_budget_error_code(error: IntegerInputError<u32>) -> &'static str {
    match error {
        IntegerInputError::Incomplete => "integer_input_incomplete",
        IntegerInputError::InvalidSyntax => "integer_input_invalid",
        IntegerInputError::Overflow => "integer_input_overflow",
        IntegerInputError::OutOfRange { .. } => "integer_input_out_of_range",
    }
}

impl Deref for FormApprovalPicker {
    type Target = Entity<ApprovalControlState>;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

pub(crate) struct RunSettingsBoundControls {
    model: FormModelPicker,
    reasoning: FormReasoningPicker,
    approval: FormApprovalPicker,
    token_budget: Option<FormTokenBudgetInput>,
}

pub(crate) struct RunSettingsController<M>
where
    M: FormSchema,
{
    form: Entity<Form<M>>,
    orchestration_subscriptions: Vec<Subscription>,
    controls: RunSettingsBoundControls,
}

impl<M> Drop for RunSettingsController<M>
where
    M: FormSchema,
{
    fn drop(&mut self) {
        self.orchestration_subscriptions.clear();
    }
}

impl<M> RunSettingsController<M>
where
    M: FormSchema,
{
    pub(crate) fn new(
        form: Entity<Form<M>>,
        field: TotalPath<M, RunSettingsInput>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let reasoning_field = field.clone().then(RunSettingsInput::REASONING_SELECTION);
        let approval_field = field.clone().then(RunSettingsInput::APPROVAL_MODE);
        let draft = field.get(&form, cx);
        let choices = load_model_choices(cx);
        let selected_model = draft.model.clone();
        let capability = selected_model_choice(&choices, selected_model.as_ref())
            .map(|choice| choice.capabilities.clone());
        let selected_reasoning = draft.reasoning_selection.clone();
        let projected_reasoning = projected_reasoning_selection(
            capability
                .as_ref()
                .and_then(|capability| capability.reasoning.as_ref()),
            selected_reasoning.clone(),
        );
        let approval = draft.approval_mode;
        let state = cx.entity().downgrade();

        let model_sections = model_sections(choices.as_ref().map(Vec::as_slice).unwrap_or(&[]));
        let model_selected_ix =
            PickerListDelegate::selected_index_for(&model_sections, selected_model.as_ref());
        let model_writer: Rc<RefCell<Option<SettingsWriter<M>>>> = Rc::new(RefCell::new(None));
        let model_confirm = Rc::new({
            let state = state.clone();
            let form = form.clone();
            let field = field.clone();
            let writer = model_writer.clone();
            move |option: ModelOption, window: &mut Window, cx: &mut App| {
                if !model_catalog_is_ready(cx) {
                    return;
                }
                let settings = field.get(&form, cx);
                let Some(settings) = explicit_model_selection(settings, option.key(), cx) else {
                    return;
                };
                let _ = state.update(cx, |controller, cx| {
                    if let Some(writer) = writer.borrow().as_ref() {
                        writer.defer_set(settings, window, cx);
                    }
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
        let model_picker = cx.new(|cx| {
            let mut picker = ListState::new(
                PickerListDelegate::new(
                    model_sections,
                    selected_model.clone(),
                    |cx| {
                        cx.global::<I18n>()
                            .t("chat-form-model-none-configured")
                            .into()
                    },
                    model_confirm,
                    model_cancel,
                ),
                window,
                cx,
            )
            .searchable(true);
            picker.delegate_mut().set_selectable(
                model_catalog_is_ready(cx),
                Some(cx.global::<I18n>().t("resource-picker-read-only").into()),
            );
            picker.set_selected_index(model_selected_ix, window, cx);
            picker
        });

        let reasoning_sections =
            effort_select::effort_sections(capability.as_ref(), cx.global::<foundation::I18n>());
        let reasoning_selected_ix = PickerListDelegate::selected_index_for(
            &reasoning_sections,
            projected_reasoning.as_ref(),
        );
        let reasoning_writer: Rc<RefCell<Option<ReasoningWriter<M>>>> = Rc::new(RefCell::new(None));
        let reasoning_confirm = Rc::new({
            let state = state.clone();
            let writer = reasoning_writer.clone();
            move |option: effort_select::EffortOption, window: &mut Window, cx: &mut App| {
                if !config_is_ready(cx) {
                    return;
                }
                let _ = state.update(cx, |controller, cx| {
                    if let Some(writer) = writer.borrow().as_ref() {
                        writer.defer_set(Some(option.selection().clone()), window, cx);
                    }
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
        let reasoning_picker = cx.new(|cx| {
            let mut picker = ListState::new(
                PickerListDelegate::new(
                    reasoning_sections,
                    projected_reasoning.clone(),
                    |cx| cx.global::<I18n>().t("chat-form-effort-empty").into(),
                    reasoning_confirm,
                    reasoning_cancel,
                ),
                window,
                cx,
            );
            picker.delegate_mut().set_selectable(
                config_is_ready(cx),
                Some(cx.global::<I18n>().t("resource-picker-read-only").into()),
            );
            picker.set_selected_index(reasoning_selected_ix, window, cx);
            picker
        });
        let token_budget_control = token_budget_bounds(
            capability
                .as_ref()
                .and_then(|capability| capability.reasoning.as_ref()),
        )
        .and_then(|_| {
            let policy = token_budget_policy(capability.as_ref());
            FormTokenBudgetInput::new(&form, reasoning_field.clone(), policy, window, cx)
        });
        let token_budget_input = token_budget_control
            .as_ref()
            .map(|control| control.read(cx).editor().clone());

        let approval_sections = approval_select::approval_mode_sections(cx.global::<I18n>());
        let approval_selected_ix =
            PickerListDelegate::selected_index_for(&approval_sections, Some(&approval));
        let approval_writer: Rc<RefCell<Option<ApprovalWriter<M>>>> = Rc::new(RefCell::new(None));
        let approval_confirm = Rc::new({
            let state = state.clone();
            let writer = approval_writer.clone();
            move |option: approval_select::ApprovalModeOption, window: &mut Window, cx: &mut App| {
                if !config_is_ready(cx) {
                    return;
                }
                let _ = state.update(cx, |controller, cx| {
                    if let Some(writer) = writer.borrow().as_ref() {
                        writer.defer_set(option.mode(), window, cx);
                    }
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
                    |_| SharedString::from(""),
                    approval_confirm,
                    approval_cancel,
                ),
                window,
                cx,
            );
            picker.delegate_mut().set_selectable(
                config_is_ready(cx),
                Some(cx.global::<I18n>().t("resource-picker-read-only").into()),
            );
            picker.set_selected_index(approval_selected_ix, window, cx);
            picker
        });

        let model_state = cx.new(|_| ModelControlState {
            selected: draft.model.clone(),
            picker: model_picker,
            open: false,
            on_open_change: model_open_change,
        });
        let reasoning_state = cx.new(|_| ReasoningControlState {
            capability,
            selected: projected_reasoning,
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
        let owner = cx.entity();
        let (model_binding, writer) = field.clone().bind_control_in(
            &form,
            &owner,
            {
                let field = field.clone();
                move |controller, projection, window, cx| match projection {
                    ControlProjection::Value(settings) => {
                        controller.sync_model_from_form(field.clone(), settings.model, window, cx);
                    }
                    ControlProjection::Retired => {}
                }
            },
            window,
            cx,
        );
        *model_writer.borrow_mut() = Some(writer);
        let (reasoning_binding, writer) = reasoning_field.bind_control_in(
            &form,
            &owner,
            {
                let field = field.clone();
                move |controller, projection, window, cx| match projection {
                    ControlProjection::Value(value) => {
                        controller.sync_reasoning_from_form(field.clone(), value, window, cx);
                    }
                    ControlProjection::Retired => {}
                }
            },
            window,
            cx,
        );
        *reasoning_writer.borrow_mut() = Some(writer);
        let (approval_binding, writer) = approval_field.bind_control_in(
            &form,
            &owner,
            move |controller, projection, window, cx| match projection {
                ControlProjection::Value(value) => {
                    controller.sync_approval_picker(value, window, cx);
                }
                ControlProjection::Retired => {}
            },
            window,
            cx,
        );
        *approval_writer.borrow_mut() = Some(writer);

        let mut orchestration_subscriptions = Vec::new();
        if cx.has_global::<state::providers::ProviderStore>() {
            let catalog = state::providers::catalog(cx);
            orchestration_subscriptions.push(catalog.observe_select_in(
                cx,
                window,
                state::providers::SelectProviderModelCatalog,
                {
                    let field = field.clone();
                    move |controller, _catalog, window, cx| {
                        controller.reload_models(field.clone(), window, cx);
                    }
                },
            ));
        }
        orchestration_subscriptions.push(
            cx.observe_global_in::<I18n>(window, |controller, window, cx| {
                controller.refresh_locale(window, cx)
            }),
        );

        Self {
            form,
            orchestration_subscriptions,
            controls: RunSettingsBoundControls {
                model: FormModelPicker {
                    _binding: model_binding,
                    state: model_state,
                },
                reasoning: FormReasoningPicker {
                    _binding: reasoning_binding,
                    state: reasoning_state,
                },
                approval: FormApprovalPicker {
                    _binding: approval_binding,
                    state: approval_state,
                },
                token_budget: token_budget_control,
            },
        }
    }

    pub(crate) fn control_states(&self) -> RunSettingsControlStates {
        RunSettingsControlStates {
            model: self.controls.model.state.clone(),
            reasoning: self.controls.reasoning.state.clone(),
            approval: self.controls.approval.state.clone(),
        }
    }

    #[cfg(test)]
    pub(crate) fn selected_model(
        &self,
        field: TotalPath<M, RunSettingsInput>,
        cx: &App,
    ) -> Option<ProviderModelChoice> {
        let selected = field.then(RunSettingsInput::MODEL).get(&self.form, cx);
        let choices = load_model_choices(cx);
        selected_model_choice(&choices, selected.as_ref()).cloned()
    }

    pub(crate) fn reload_models(
        &mut self,
        field: TotalPath<M, RunSettingsInput>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let choices = load_model_choices(cx);
        let previous_key = field
            .clone()
            .then(RunSettingsInput::MODEL)
            .get(&self.form, cx);
        let previous_reasoning = field
            .clone()
            .then(RunSettingsInput::REASONING_SELECTION)
            .get(&self.form, cx);
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

        self.controls.reasoning.update(cx, |state, _| {
            state.capability = capability.clone();
        });
        self.sync_model_picker(selected.clone(), window, cx);
        self.sync_reasoning_picker(capability.clone(), reasoning.clone(), window, cx);
        self.sync_token_budget_control(field, window, cx);
        cx.notify();
    }

    fn refresh_locale(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let model_choices = load_model_choices(cx);
        let (selected_model, model_picker) = {
            let state = self.controls.model.read(cx);
            (state.selected.clone(), state.picker.clone())
        };
        let model_sections =
            model_sections(model_choices.as_ref().map(Vec::as_slice).unwrap_or(&[]));
        model_picker.update(cx, |picker, cx| {
            picker
                .delegate_mut()
                .replace_projection(model_sections, selected_model);
            picker.delegate_mut().set_selectable(
                model_catalog_is_ready(cx),
                Some(cx.global::<I18n>().t("resource-picker-read-only").into()),
            );
            let ix = picker.delegate().selected_index();
            picker.set_selected_index(ix, window, cx);
        });

        let (capability, selected_reasoning, reasoning_picker) = {
            let state = self.controls.reasoning.read(cx);
            (
                state.capability.clone(),
                state.selected.clone(),
                state.picker.clone(),
            )
        };
        let reasoning_sections =
            effort_select::effort_sections(capability.as_ref(), cx.global::<I18n>());
        reasoning_picker.update(cx, |picker, cx| {
            picker
                .delegate_mut()
                .replace_projection(reasoning_sections, selected_reasoning);
            picker.delegate_mut().set_selectable(
                config_is_ready(cx),
                Some(cx.global::<I18n>().t("resource-picker-read-only").into()),
            );
            let ix = picker.delegate().selected_index();
            picker.set_selected_index(ix, window, cx);
        });

        let (selected_approval, approval_picker) = {
            let state = self.controls.approval.read(cx);
            (state.selected, state.picker.clone())
        };
        let approval_sections = approval_select::approval_mode_sections(cx.global::<I18n>());
        approval_picker.update(cx, |picker, cx| {
            picker
                .delegate_mut()
                .replace_projection(approval_sections, Some(selected_approval));
            picker.delegate_mut().set_selectable(
                config_is_ready(cx),
                Some(cx.global::<I18n>().t("resource-picker-read-only").into()),
            );
            let ix = picker.delegate().selected_index();
            picker.set_selected_index(ix, window, cx);
        });

        cx.notify();
    }

    #[cfg(test)]
    pub(crate) fn select_model_value(
        &mut self,
        field: TotalPath<M, RunSettingsInput>,
        key: ProviderModelKey,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if apply_explicit_model_selection(&self.form, &field, key.clone(), cx) {
            self.sync_model_from_form(field, Some(key), window, cx);
        }
    }

    #[cfg(test)]
    pub(crate) fn select_approval_value(
        &mut self,
        field: TotalPath<M, RunSettingsInput>,
        mode: ToolApprovalMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        field
            .then(RunSettingsInput::APPROVAL_MODE)
            .set(&self.form, mode, cx);
        self.sync_approval_picker(mode, window, cx);
    }

    #[cfg(test)]
    pub(crate) fn select_reasoning_value(
        &mut self,
        field: TotalPath<M, RunSettingsInput>,
        selection: ReasoningSelectionSnapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        field
            .clone()
            .then(RunSettingsInput::REASONING_SELECTION)
            .set(&self.form, Some(selection.clone()), cx);
        let capability = self.controls.reasoning.read(cx).capability.clone();
        self.sync_reasoning_picker(capability, Some(selection), window, cx);
        self.sync_token_budget_control(field, window, cx);
    }

    #[cfg(test)]
    pub(crate) fn set_custom_token_budget(
        &mut self,
        field: TotalPath<M, RunSettingsInput>,
        value: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _ = window;
        let reasoning_field = field.then(RunSettingsInput::REASONING_SELECTION);
        let mut reasoning = reasoning_field.get(&self.form, cx);
        if set_existing_custom_token_budget(&mut reasoning, value) {
            reasoning_field.set(&self.form, reasoning, cx);
        }
    }

    pub(crate) fn set_model_open(
        &mut self,
        open: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let picker = (open && model_catalog_is_ready(cx))
            .then(|| self.controls.model.read(cx).picker.clone());
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
        let open = open && config_is_ready(cx);
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
        let open = open && config_is_ready(cx);
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

    fn sync_token_budget_control(
        &mut self,
        field: TotalPath<M, RunSettingsInput>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let capability = self.controls.reasoning.read(cx).capability.clone();
        let supports_token_budget = token_budget_bounds(
            capability
                .as_ref()
                .and_then(|capability| capability.reasoning.as_ref()),
        )
        .is_some();
        let reasoning_field = field.then(RunSettingsInput::REASONING_SELECTION);
        let has_custom_value = reasoning_field
            .get(&self.form, cx)
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

        let Some(control) =
            FormTokenBudgetInput::new(&self.form, reasoning_field, policy, window, cx)
        else {
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
        let choices = load_model_choices(cx);
        let picker = self.controls.model.update(cx, |state, cx| {
            state.selected = selected.clone();
            cx.notify();
            state.picker.clone()
        });
        let sections = model_sections(choices.as_ref().map(Vec::as_slice).unwrap_or(&[]));
        picker.update(cx, |picker, cx| {
            picker.delegate_mut().replace_projection(sections, selected);
            picker.delegate_mut().set_selectable(
                model_catalog_is_ready(cx),
                Some(cx.global::<I18n>().t("resource-picker-read-only").into()),
            );
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
        let projected = projected_reasoning_selection(
            capability
                .as_ref()
                .and_then(|capability| capability.reasoning.as_ref()),
            selected,
        );
        let picker = self.controls.reasoning.update(cx, |state, cx| {
            state.selected = projected.clone();
            cx.notify();
            state.picker.clone()
        });
        let sections =
            effort_select::effort_sections(capability.as_ref(), cx.global::<foundation::I18n>());
        picker.update(cx, |picker, cx| {
            picker.delegate_mut().set_sections(sections);
            picker.delegate_mut().set_selected_value(projected);
            picker.delegate_mut().set_selectable(
                config_is_ready(cx),
                Some(cx.global::<I18n>().t("resource-picker-read-only").into()),
            );
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
            picker.delegate_mut().set_selectable(
                config_is_ready(cx),
                Some(cx.global::<I18n>().t("resource-picker-read-only").into()),
            );
            let ix = picker.delegate().selected_index();
            picker.set_selected_index(ix, window, cx);
        });
    }

    fn sync_model_from_form(
        &mut self,
        field: TotalPath<M, RunSettingsInput>,
        model: Option<ProviderModelKey>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let choices = load_model_choices(cx);
        let capability = selected_model_choice(&choices, model.as_ref())
            .map(|choice| choice.capabilities.clone());
        self.controls.reasoning.update(cx, |state, _| {
            state.capability = capability.clone();
        });
        self.sync_model_picker(model, window, cx);
        let reasoning = field
            .clone()
            .then(RunSettingsInput::REASONING_SELECTION)
            .get(&self.form, cx);
        self.sync_reasoning_picker(capability, reasoning, window, cx);
        self.sync_token_budget_control(field, window, cx);
    }

    fn sync_reasoning_from_form(
        &mut self,
        field: TotalPath<M, RunSettingsInput>,
        reasoning: Option<ReasoningSelectionSnapshot>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let capability = self.controls.reasoning.read(cx).capability.clone();
        self.sync_reasoning_picker(capability, reasoning, window, cx);
        self.sync_token_budget_control(field, window, cx);
    }
}

#[cfg(test)]
fn apply_explicit_model_selection<M: FormSchema>(
    form: &Entity<Form<M>>,
    field: &TotalPath<M, RunSettingsInput>,
    key: ProviderModelKey,
    cx: &mut App,
) -> bool {
    let Some(settings) = explicit_model_selection(field.get(form, cx), key, cx) else {
        return false;
    };
    field.set(form, settings, cx);
    true
}

fn explicit_model_selection(
    mut settings: RunSettingsInput,
    key: ProviderModelKey,
    cx: &App,
) -> Option<RunSettingsInput> {
    let choices = load_model_choices(cx).ok()?;
    let choice = selected_model_choice_from_slice(&choices, Some(&key))?;
    settings.reasoning_selection = reasoning_selection_after_model_change(
        choice.capabilities.reasoning.as_ref(),
        settings.reasoning_selection.as_ref(),
    );
    settings.model = Some(key);
    Some(settings)
}

fn load_model_choices(cx: &App) -> Result<Vec<ProviderModelChoice>, SharedString> {
    if !cx.has_global::<state::providers::ProviderStore>() {
        return Err("provider catalog is unavailable".into());
    }
    state::providers::catalog(cx).read(cx, |operation| {
        operation
            .data()
            .map(|data| data.enabled_models.clone())
            .ok_or_else(|| "provider catalog is unavailable".into())
    })
}

fn model_catalog_is_ready(cx: &App) -> bool {
    crate::app::critical_resources_ready(cx)
        && cx.has_global::<state::providers::ProviderStore>()
        && state::providers::catalog(cx).read(cx, |operation| {
            matches!(operation, state::providers::ProviderOperation::Ready(_))
        })
}

fn config_is_ready(cx: &App) -> bool {
    state::config::store(cx).read(cx, |operation| {
        matches!(operation, state::config::ConfigOperation::Ready(_))
    })
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

#[derive(IntoElement)]
pub(crate) struct ModelSelector {
    state: Entity<ModelControlState>,
    enabled: bool,
}

impl ModelSelector {
    pub(crate) fn new(state: Entity<ModelControlState>, enabled: bool) -> Self {
        Self { state, enabled }
    }
}

impl View for ModelSelector {
    fn entity_id(&self) -> Option<EntityId> {
        Some(self.state.entity_id())
    }

    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state;
        let enabled = self.enabled;
        let state_snapshot = state.read(cx);
        let selected = state_snapshot.selected.clone();
        let (phase, choices, problem) = state::providers::catalog(cx).read(cx, |operation| {
            (
                operation.phase(),
                operation.data().map(|data| data.enabled_models.clone()),
                operation.problem().map(ToString::to_string),
            )
        });
        let i18n = cx.global::<I18n>();
        let label: SharedString = match choices.as_ref() {
            Some(choices) => selected
                .as_ref()
                .and_then(|key| selected_model_choice_from_slice(choices, Some(key)))
                .map(|choice| choice.display_label().into())
                .unwrap_or_else(|| i18n.t("chat-form-model-empty").into()),
            None if problem.is_some() => i18n.t("chat-form-model-load-failed").into(),
            None => i18n.t("resource-status-loading").into(),
        };
        let resource_ready = model_catalog_is_ready(cx);
        let open = enabled && state_snapshot.open;
        let list = state_snapshot.picker.clone();
        let on_open_change = state_snapshot.on_open_change.clone();
        let trigger = crate::components::picker::picker_trigger(
            "chat-form-model-trigger",
            crate::foundation::assets::IconName::Sparkles,
            label,
            open,
        )
        .disabled(!enabled)
        .when(!resource_ready, |trigger| {
            trigger.tooltip(cx.global::<I18n>().t("resource-picker-read-only"))
        });
        let list_content = || {
            List::new(&list)
                .search_placeholder(i18n.t("chat-form-model-search-placeholder"))
                .with_size(Size::Small)
                .scrollbar_visible(false)
                .max_h(rems(18.))
                .paddings(Edges::all(px(4.)))
                .into_any_element()
        };
        let content = match (phase, choices.as_ref()) {
            (gpui_operation::refresh::Phase::Ready, Some(choices)) if choices.is_empty() => {
                render_empty_model_catalog(cx)
            }
            (gpui_operation::refresh::Phase::Ready, Some(_)) => list_content(),
            (
                gpui_operation::refresh::Phase::Refreshing
                | gpui_operation::refresh::Phase::Degraded
                | gpui_operation::refresh::Phase::RefreshingDegraded,
                Some(_),
            ) => v_flex()
                .child(list_content())
                .when_some(
                    refresh_status(
                        "chat-form-refresh-providers",
                        phase,
                        problem,
                        state::providers::request_refresh,
                        cx,
                    ),
                    |this, status| this.child(div().p_2().child(status)),
                )
                .into_any_element(),
            _ => div()
                .p_2()
                .child(
                    refresh_status(
                        "chat-form-refresh-providers",
                        phase,
                        problem,
                        state::providers::request_refresh,
                        cx,
                    )
                    .unwrap_or_else(|| render_empty_model_catalog(cx)),
                )
                .into_any_element(),
        };
        crate::components::picker::picker_content_popover(
            cx,
            PickerContentPopoverConfig {
                id: "chat-form-model-popover",
                open,
                trigger,
                content,
                width: px(340.),
                footer: None,
                on_open_change: move |open, window, cx| {
                    on_open_change(*open, window, cx);
                },
            },
        )
    }
}

fn render_empty_model_catalog(cx: &App) -> AnyElement {
    v_flex()
        .items_center()
        .gap_3()
        .p_4()
        .child(
            Label::new(cx.global::<I18n>().t("chat-form-model-none-configured"))
                .text_sm()
                .text_color(cx.theme().muted_foreground),
        )
        .child(
            Button::new("chat-form-configure-providers")
                .icon(crate::foundation::assets::IconName::Settings)
                .label(cx.global::<I18n>().t("chat-form-configure-providers"))
                .small()
                .on_click(|_, _window, cx| {
                    settings::open_settings_window_to_provider(cx);
                }),
        )
        .into_any_element()
}

#[derive(IntoElement)]
pub(crate) struct ReasoningSelector {
    state: Entity<ReasoningControlState>,
    enabled: bool,
}

impl ReasoningSelector {
    pub(crate) fn new(state: Entity<ReasoningControlState>, enabled: bool) -> Self {
        Self { state, enabled }
    }
}

impl View for ReasoningSelector {
    fn entity_id(&self) -> Option<EntityId> {
        Some(self.state.entity_id())
    }

    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state;
        let enabled = self.enabled;
        let resource_ready = config_is_ready(cx);
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
                enabled && resource_ready && snapshot.open,
                snapshot.picker.clone(),
                snapshot.capability.clone(),
                snapshot.token_budget_input.clone(),
                snapshot.on_open_change.clone(),
            )
        };
        let footer = token_budget_footer(
            capability.as_ref(),
            token_budget_input,
            enabled && resource_ready,
            cx,
        );
        PickerPopover::new(PickerPopoverConfig {
            id: "chat-form-effort-popover",
            open,
            trigger: crate::components::picker::picker_trigger(
                "chat-form-effort-trigger",
                crate::foundation::assets::IconName::Lightbulb,
                label,
                open,
            )
            .disabled(!enabled || !resource_ready || !has_options)
            .when(!resource_ready, |trigger| {
                trigger.tooltip(cx.global::<I18n>().t("resource-picker-read-only"))
            }),
            list: picker,
            width: px(180.),
            max_height: rems(16.).into(),
            search_placeholder: None,
            footer,
            on_open_change: move |open, window, cx| {
                on_open_change(*open, window, cx);
            },
        })
    }
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

#[derive(IntoElement)]
pub(crate) struct ApprovalSelector {
    state: Entity<ApprovalControlState>,
    enabled: bool,
}

impl ApprovalSelector {
    pub(crate) fn new(state: Entity<ApprovalControlState>, enabled: bool) -> Self {
        Self { state, enabled }
    }
}

impl View for ApprovalSelector {
    fn entity_id(&self) -> Option<EntityId> {
        Some(self.state.entity_id())
    }

    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state;
        let enabled = self.enabled;
        let resource_ready = config_is_ready(cx);
        let snapshot = state.read(cx);
        let selected = snapshot.selected;
        let on_open_change = snapshot.on_open_change.clone();
        PickerPopover::new(PickerPopoverConfig {
            id: "chat-form-approval-popover",
            open: enabled && resource_ready && snapshot.open,
            trigger: crate::components::picker::picker_trigger(
                "chat-form-approval-trigger",
                crate::foundation::assets::IconName::Shield,
                approval_select::approval_mode_label(selected, cx.global::<I18n>()),
                enabled && resource_ready && snapshot.open,
            )
            .disabled(!enabled || !resource_ready)
            .when(!resource_ready, |trigger| {
                trigger.tooltip(cx.global::<I18n>().t("resource-picker-read-only"))
            }),
            list: snapshot.picker.clone(),
            width: px(180.),
            max_height: rems(12.).into(),
            search_placeholder: None,
            footer: None,
            on_open_change: move |open, window, cx| {
                on_open_change(*open, window, cx);
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::{
        ApprovalControlState, ApprovalSelector, ModelControlState, ModelSelector,
        ReasoningControlState, ReasoningSelector, RunSettingsInput, RunSettingsSubmitError,
        resolve_run_settings,
    };
    use crate::components::{
        chat::{
            input::{approval_select::ApprovalModeOption, effort_select::EffortOption},
            model_picker::ModelOption,
        },
        picker::PickerListDelegate,
    };
    use crate::state::providers::{ProviderModelChoice, ProviderModelKey};
    use gpui::{
        AppContext as _, Context, Entity, IntoElement, Render, TestAppContext, View as _, Window,
        div,
    };
    use gpui_component::list::ListState;
    use jaco_core::conservative_model_capabilities;

    struct TestRoot;

    impl Render for TestRoot {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    #[gpui::test]
    fn selector_views_use_backing_identity_across_rebuilds(cx: &mut TestAppContext) {
        let (_, _) = cx.add_window_view(|window, cx| {
            let model_picker: Entity<ListState<PickerListDelegate<ModelOption>>> = cx.new(|cx| {
                ListState::new(
                    PickerListDelegate::new(
                        Vec::new(),
                        None,
                        |_| "Empty".into(),
                        Rc::new(|_, _, _| {}),
                        Rc::new(|_, _| {}),
                    ),
                    window,
                    cx,
                )
            });
            let reasoning_picker: Entity<ListState<PickerListDelegate<EffortOption>>> =
                cx.new(|cx| {
                    ListState::new(
                        PickerListDelegate::new(
                            Vec::new(),
                            None,
                            |_| "Empty".into(),
                            Rc::new(|_, _, _| {}),
                            Rc::new(|_, _| {}),
                        ),
                        window,
                        cx,
                    )
                });
            let approval_picker: Entity<ListState<PickerListDelegate<ApprovalModeOption>>> = cx
                .new(|cx| {
                    ListState::new(
                        PickerListDelegate::new(
                            Vec::new(),
                            None,
                            |_| "Empty".into(),
                            Rc::new(|_, _, _| {}),
                            Rc::new(|_, _| {}),
                        ),
                        window,
                        cx,
                    )
                });

            let model_state = cx.new(|_| ModelControlState {
                selected: None,
                picker: model_picker,
                open: false,
                on_open_change: Rc::new(|_, _, _| {}),
            });
            let reasoning_state = cx.new(|_| ReasoningControlState {
                capability: None,
                selected: None,
                picker: reasoning_picker,
                token_budget_input: None,
                open: false,
                on_open_change: Rc::new(|_, _, _| {}),
            });
            let approval_state = cx.new(|_| ApprovalControlState {
                selected: jaco_core::ToolApprovalMode::RequestApproval,
                picker: approval_picker,
                open: false,
                on_open_change: Rc::new(|_, _, _| {}),
            });

            let model = ModelSelector::new(model_state.clone(), true);
            let refreshed_model = ModelSelector::new(model_state.clone(), false);
            let reasoning = ReasoningSelector::new(reasoning_state.clone(), true);
            let refreshed_reasoning = ReasoningSelector::new(reasoning_state.clone(), false);
            let approval = ApprovalSelector::new(approval_state.clone(), true);
            let refreshed_approval = ApprovalSelector::new(approval_state.clone(), false);

            assert_eq!(model.entity_id(), Some(model_state.entity_id()));
            assert_eq!(refreshed_model.entity_id(), model.entity_id());

            assert_eq!(reasoning.entity_id(), Some(reasoning_state.entity_id()));
            assert_eq!(refreshed_reasoning.entity_id(), reasoning.entity_id());

            assert_eq!(approval.entity_id(), Some(approval_state.entity_id()));
            assert_eq!(refreshed_approval.entity_id(), approval.entity_id());

            TestRoot
        });
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

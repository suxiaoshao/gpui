mod approval_select;
mod attachment_flow;
mod attachment_views;
mod attachments;
mod composer_editor;
mod effort_select;
mod model_select;
mod thinking_effort;

use crate::{
    components::{
        model_picker::{ModelOption, model_sections},
        picker::PickerListDelegate,
    },
    errors::JacoError,
    foundation::{self, assets::IconName},
    state,
    state::providers::{ProviderModelChoice, ProviderModelKey},
    state::{
        attachments::ComposerAttachment,
        config::{ChatFormConfig, ChatFormModelConfig},
    },
};
use approval_select::{ApprovalModeOption, approval_mode_sections};
use composer_editor::{ComposerEditor, ComposerEditorEvent, ComposerSnapshot};
use effort_select::{EffortOption, effort_sections};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Disableable, ElementExt, Icon, Sizable, StyledExt, box_shadow,
    button::{Button, ButtonVariants},
    h_flex,
    input::{InputEvent, InputState, NumberInputEvent, StepAction},
    label::Label,
    list::ListState,
    v_flex,
};
use gpui_store::StoreBinding;
use jaco_core::{ReasoningSelectionSnapshot, TokenBudgetSelectionMode, ToolApprovalMode};
use std::{
    path::{Path, PathBuf},
    rc::Rc,
};
use thinking_effort::{
    computed_default_reasoning_selection, custom_token_budget_value, reasoning_selection_is_valid,
    reasoning_selections, token_budget_bounds,
};
use tracing::{Level, event};

pub(super) const COMPOSER_BUTTON_SIZE: f32 = 28.;
pub(super) const COMPOSER_BUTTON_ICON_SIZE: f32 = 18.;
pub(super) const COMPOSER_BUTTON_RADIUS: f32 = 999.;
pub(crate) const COMPOSER_EDITOR_KEY_CONTEXT: &str = composer_editor::KEY_CONTEXT;
const COMPOSER_INPUT_HORIZONTAL_PADDING: f32 = 12.;
const COMPOSER_INPUT_TOP_PADDING: f32 = 12.;
const COMPOSER_INPUT_BOTTOM_MARGIN: f32 = 4.;
const COMPOSER_FOOTER_HORIZONTAL_PADDING: f32 = 8.;
const COMPOSER_FOOTER_BOTTOM_MARGIN: f32 = 8.;
const SKILL_COMPLETION_GAP: f32 = 6.;
const SKILL_COMPLETION_WINDOW_MARGIN: f32 = 8.;
const SKILL_COMPLETION_MAX_HEIGHT: f32 = 360.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChatFormSkillCompletionPlacement {
    AboveForm,
    BelowForm,
}

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
    pub(crate) approval_mode: ToolApprovalMode,
}

#[derive(Debug, PartialEq)]
enum ChatFormPrimaryButtonAction {
    Send(Box<ChatFormSubmit>),
    Stop,
}

pub(crate) struct ChatForm {
    composer: Entity<ComposerEditor>,
    bounds: Option<Bounds<Pixels>>,
    skill_completion_placement: ChatFormSkillCompletionPlacement,
    model_choices: Result<Vec<ProviderModelChoice>, SharedString>,
    selected_model_key: Option<ProviderModelKey>,
    selected_reasoning_selection: Option<ReasoningSelectionSnapshot>,
    selected_approval_mode: ToolApprovalMode,
    chat_form_config: StoreBinding<ChatFormConfig, JacoError>,
    token_budget_input: Entity<InputState>,
    effort_picker_open: bool,
    effort_picker: Entity<ListState<PickerListDelegate<EffortOption>>>,
    approval_picker_open: bool,
    approval_picker: Entity<ListState<PickerListDelegate<ApprovalModeOption>>>,
    model_picker_open: bool,
    model_picker: Entity<ListState<PickerListDelegate<ModelOption>>>,
    attachments: Vec<ComposerAttachment>,
    next_attachment_id: u64,
    preview_attachment: Option<ComposerAttachment>,
    agent_running: bool,
    skill_catalog_scope: state::skills::SkillCatalogScope,
    skill_catalog_task: Task<()>,
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
        if cx.has_global::<state::skills::GlobalSkillCatalogStore>() {
            let entries = state::skills::catalog(cx).read_cloned(cx, |state| state.entry_records());
            composer.update(cx, |composer, cx| composer.set_skill_entries(&entries, cx));
        }
        let model_choices = load_model_choices(cx);
        let config_store = state::config::store(cx);
        let chat_form_config = config_store.bind_committed(
            cx,
            |config| config.chat_form.clone(),
            |config, chat_form| {
                config.chat_form = chat_form;
            },
        );
        let configured_chat_form = chat_form_config.cloned();
        let selected_model_key =
            configured_model_key_in(&model_choices, configured_chat_form.model.as_ref()).or_else(
                || {
                    model_choices
                        .as_ref()
                        .ok()
                        .and_then(|choices| choices.first().map(ProviderModelChoice::key))
                },
            );
        let selected_reasoning_selection = initial_reasoning_selection(
            &model_choices,
            selected_model_key.as_ref(),
            configured_chat_form.reasoning_selection.as_ref(),
        );
        let initial_token_budget =
            initial_token_budget_value(&model_choices, selected_model_key.as_ref());
        let token_budget_input = cx
            .new(|cx| InputState::new(window, cx).default_value(initial_token_budget.to_string()));
        let state = cx.entity().downgrade();
        let selected_approval_mode = configured_chat_form.approval_mode;
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

        let approval_sections = approval_mode_sections(cx.global::<foundation::I18n>());
        let approval_selected_ix = PickerListDelegate::selected_index_for(
            &approval_sections,
            Some(&selected_approval_mode),
        );
        let approval_confirm = Rc::new({
            let state = state.clone();
            move |option: ApprovalModeOption, window: &mut Window, cx: &mut App| {
                let _ = state.update(cx, |form, cx| {
                    form.select_approval_mode(option.mode(), window, cx);
                });
            }
        });
        let approval_cancel = Rc::new({
            let state = state.clone();
            move |window: &mut Window, cx: &mut App| {
                let _ = state.update(cx, |form, cx| {
                    form.set_approval_picker_open(false, window, cx);
                });
            }
        });
        let approval_picker = cx.new(|cx| {
            let mut picker = ListState::new(
                PickerListDelegate::new(
                    approval_sections,
                    Some(selected_approval_mode),
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
        let mut subscriptions = vec![
            composer_subscription,
            token_budget_change_subscription,
            token_budget_step_subscription,
            provider_catalog_subscription,
        ];

        if cx.has_global::<state::skills::GlobalSkillCatalogStore>() {
            let skill_catalog = state::skills::catalog(cx);
            subscriptions.push(skill_catalog.observe_select_in(
                cx,
                window,
                |catalog_state| catalog_state.entry_records().clone(),
                |form, entries, _window, cx| {
                    if matches!(
                        form.skill_catalog_scope,
                        state::skills::SkillCatalogScope::Global
                    ) {
                        form.apply_skill_catalog_entries(entries.clone(), cx);
                    }
                },
            ));
        }

        let form = Self {
            composer,
            bounds: None,
            skill_completion_placement: ChatFormSkillCompletionPlacement::BelowForm,
            model_choices,
            selected_model_key,
            selected_reasoning_selection,
            selected_approval_mode,
            chat_form_config,
            token_budget_input,
            effort_picker_open: false,
            effort_picker,
            approval_picker_open: false,
            approval_picker,
            model_picker_open: false,
            model_picker,
            attachments: Vec::new(),
            next_attachment_id: 1,
            preview_attachment: None,
            agent_running: false,
            skill_catalog_scope: state::skills::SkillCatalogScope::Global,
            skill_catalog_task: Task::ready(()),
            _subscriptions: subscriptions,
        };

        if form.model_choices.is_ok() {
            form.save_chat_form_config(cx);
        }

        form
    }

    pub(crate) fn set_skill_completion_placement(
        &mut self,
        placement: ChatFormSkillCompletionPlacement,
    ) {
        self.skill_completion_placement = placement;
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

        if self.approval_picker_open {
            self.approval_picker
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
        let scope = project_root
            .map(|root| state::skills::SkillCatalogScope::Project {
                root: root.to_path_buf(),
            })
            .unwrap_or(state::skills::SkillCatalogScope::Global);
        self.skill_catalog_scope = scope.clone();
        match scope {
            state::skills::SkillCatalogScope::Global => {
                self.skill_catalog_task = Task::ready(());
                self.sync_global_skill_catalog(cx);
            }
            state::skills::SkillCatalogScope::Project { root } => {
                self.load_project_skill_catalog(root, cx);
            }
        }
    }

    fn sync_global_skill_catalog(&mut self, cx: &mut Context<Self>) {
        if !cx.has_global::<state::skills::GlobalSkillCatalogStore>() {
            self.apply_skill_catalog_entries(Vec::new(), cx);
            return;
        }

        let entries = state::skills::catalog(cx).read_cloned(cx, |state| state.entry_records());
        self.apply_skill_catalog_entries(entries, cx);
    }

    fn load_project_skill_catalog(&mut self, root: PathBuf, cx: &mut Context<Self>) {
        let scope = state::skills::SkillCatalogScope::Project { root };
        let task_scope = scope.clone();
        let load =
            cx.background_spawn(async move { state::skills::load_catalog_entries(task_scope) });

        self.skill_catalog_task = cx.spawn(async move |form, cx| {
            let result = load.await;
            let Some(form) = form.upgrade() else {
                return;
            };
            form.update(cx, |form, cx| {
                if form.skill_catalog_scope != scope {
                    return;
                }

                match result {
                    Ok(entries) => form.apply_skill_catalog_entries(entries, cx),
                    Err(err) => {
                        event!(
                            Level::ERROR,
                            error = ?err,
                            "load project skill catalog failed"
                        );
                        form.apply_skill_catalog_entries(Vec::new(), cx);
                    }
                }
            });
        });
    }

    fn apply_skill_catalog_entries(
        &mut self,
        entries: Vec<state::skills::GlobalSkillEntry>,
        cx: &mut Context<Self>,
    ) {
        self.composer
            .update(cx, |composer, cx| composer.set_skill_entries(&entries, cx));
        cx.notify();
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
            self.approval_picker_open = false;
            self.effort_picker
                .update(cx, |picker, cx| picker.focus(window, cx));
        }
        cx.notify();
    }

    fn set_approval_picker_open(
        &mut self,
        open: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.approval_picker_open = open;
        if open {
            self.effort_picker_open = false;
            self.model_picker_open = false;
            self.approval_picker
                .update(cx, |picker, cx| picker.focus(window, cx));
        }
        cx.notify();
    }

    fn set_model_picker_open(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.model_picker_open = open;
        if open {
            self.reload_model_choices(window, cx);
            self.effort_picker_open = false;
            self.approval_picker_open = false;
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
        self.save_chat_form_config(cx);
        self.sync_token_budget_input(window, cx);
        self.set_effort_picker_open(false, window, cx);
    }

    fn select_approval_mode(
        &mut self,
        mode: ToolApprovalMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_approval_mode = mode;
        self.save_chat_form_config(cx);
        self.approval_picker.update(cx, |picker, cx| {
            picker.delegate_mut().set_selected_value(Some(mode));
            let selected_ix = picker.delegate().selected_index();
            picker.set_selected_index(selected_ix, window, cx);
        });
        self.set_approval_picker_open(false, window, cx);
    }

    fn select_model(&mut self, key: ProviderModelKey, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_model_key = Some(key);
        self.selected_reasoning_selection = self.selected_model_choice().and_then(|choice| {
            computed_default_reasoning_selection(choice.capabilities.reasoning.as_ref())
        });
        self.save_chat_form_config(cx);
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
        if self.model_choices.is_ok() {
            self.save_chat_form_config(cx);
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
        self.save_chat_form_config(cx);
        self.sync_effort_picker(window, cx);
        cx.notify();
    }

    fn save_chat_form_config(&self, cx: &mut Context<Self>) {
        let model = self
            .selected_model_key
            .as_ref()
            .map(|key| ChatFormModelConfig {
                provider_id: key.provider_id.clone(),
                model_id: key.model_id.clone(),
            });
        let reasoning_selection = self.selected_reasoning_selection.clone();
        let approval_mode = self.selected_approval_mode;

        if let Err(err) = self.chat_form_config.try_update(cx, move |config| {
            config.model = model;
            config.reasoning_selection = reasoning_selection;
            config.approval_mode = approval_mode;
        }) {
            event!(Level::ERROR, error = ?err, "save chat form config failed");
        }
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
            approval_mode: self.selected_approval_mode,
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
    ) -> Option<&jaco_core::ModelCapabilitiesSnapshot> {
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

    fn render_skill_completion(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(bounds) = self.bounds else {
            return div().into_any_element();
        };
        let Some(layout) = skill_completion_popup_layout(
            bounds,
            window.viewport_size(),
            self.skill_completion_placement,
        ) else {
            return div().into_any_element();
        };
        let panel = self.composer.update(cx, |composer, cx| {
            composer.render_skill_completion_panel(layout.max_height, window, cx)
        });

        deferred(
            anchored()
                .anchor(layout.anchor)
                .position(layout.position)
                .offset(layout.offset)
                .snap_to_window_with_margin(px(SKILL_COMPLETION_WINDOW_MARGIN))
                .child(
                    div()
                        .debug_selector(|| "jaco-skill-completion-popup".into())
                        .w(bounds.size.width)
                        .child(panel),
                ),
        )
        .with_priority(1)
        .into_any_element()
    }
}

impl Render for ChatForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let send_tooltip = cx.global::<foundation::I18n>().t("chat-form-send-tooltip");
        let stop_tooltip = cx.global::<foundation::I18n>().t("chat-form-stop-tooltip");
        let drop_label = cx
            .global::<foundation::I18n>()
            .t("chat-form-attachment-drop");
        let agent_running = self.agent_running;
        let can_submit = self.can_send(cx);
        let skill_completion_open = self.composer.read(cx).skill_completion_open();
        let form = cx.entity();

        v_flex()
            .id("jaco-chat-form-preview")
            .debug_selector(|| "jaco-chat-form".into())
            .w_full()
            .relative()
            .on_prepaint(move |bounds, _, cx| {
                form.update(cx, |form, _| {
                    form.bounds = Some(bounds);
                });
            })
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
                            .child(self.render_effort_selector(cx))
                            .child(self.render_approval_selector(cx)),
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
            .when(skill_completion_open, |this| {
                this.child(self.render_skill_completion(window, cx))
            })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct SkillCompletionPopupLayout {
    anchor: Anchor,
    position: Point<Pixels>,
    offset: Point<Pixels>,
    max_height: Pixels,
}

fn skill_completion_popup_layout(
    form_bounds: Bounds<Pixels>,
    viewport_size: Size<Pixels>,
    placement: ChatFormSkillCompletionPlacement,
) -> Option<SkillCompletionPopupLayout> {
    let gap = px(SKILL_COMPLETION_GAP);
    let margin = px(SKILL_COMPLETION_WINDOW_MARGIN);
    let max_height = px(SKILL_COMPLETION_MAX_HEIGHT);

    let (anchor, position, offset, available_height) = match placement {
        ChatFormSkillCompletionPlacement::AboveForm => (
            Anchor::BottomLeft,
            point(form_bounds.left(), form_bounds.top()),
            point(px(0.), -gap),
            form_bounds.top() - margin - gap,
        ),
        ChatFormSkillCompletionPlacement::BelowForm => (
            Anchor::TopLeft,
            point(form_bounds.left(), form_bounds.bottom()),
            point(px(0.), gap),
            viewport_size.height - form_bounds.bottom() - margin - gap,
        ),
    };

    let max_height = available_height.max(px(0.)).min(max_height);
    (max_height > px(0.)).then_some(SkillCompletionPopupLayout {
        anchor,
        position,
        offset,
        max_height,
    })
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

fn configured_model_key_in(
    choices: &Result<Vec<ProviderModelChoice>, SharedString>,
    model: Option<&ChatFormModelConfig>,
) -> Option<ProviderModelKey> {
    let model = model?;
    let key = ProviderModelKey {
        provider_id: model.provider_id.clone(),
        model_id: model.model_id.clone(),
    };
    selected_model_choice_in(choices, Some(&key)).map(|_| key)
}

fn initial_reasoning_selection(
    choices: &Result<Vec<ProviderModelChoice>, SharedString>,
    key: Option<&ProviderModelKey>,
    configured: Option<&ReasoningSelectionSnapshot>,
) -> Option<ReasoningSelectionSnapshot> {
    let choice = selected_model_choice_in(choices, key)?;
    configured
        .filter(|selection| {
            reasoning_selection_is_valid(choice.capabilities.reasoning.as_ref(), selection)
        })
        .cloned()
        .or_else(|| computed_default_reasoning_selection(choice.capabilities.reasoning.as_ref()))
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
        ChatForm, ChatFormPrimaryButtonAction, ChatFormSkillCompletionPlacement,
        SKILL_COMPLETION_GAP, SKILL_COMPLETION_MAX_HEIGHT,
        composer_editor::{ComposerSendPolicy, ComposerSnapshot},
        model_empty_label, selected_model_choice_in, skill_completion_popup_layout,
    };
    use crate::{
        database::{self, FreshStoreGlobal},
        foundation::I18n,
        state,
        state::config::ChatFormModelConfig,
        state::providers::{ProviderModelChoice, ProviderModelKey},
    };
    use gpui::{
        Anchor, App, AppContext as _, Bounds, SharedString, TestAppContext, VisualTestContext,
        WindowHandle, point, px, size,
    };
    use jaco_core::{
        CapabilitySourceSnapshot, ContentPart, ModelCapabilitiesSnapshot, ProviderModelMetadata,
        ProviderSecretRefs, ProviderSettingFieldValue, ProviderSettingValue,
        ProviderSettingsPayload, ReasoningCapabilitySnapshot, ReasoningControlSnapshot,
        ReasoningSelectionSnapshot, SkillSourceKind, TokenBudgetSelectionMode, ToolApprovalMode,
        conservative_model_capabilities,
    };
    use jaco_db::{NewProvider, NewProviderModel};
    use std::path::PathBuf;
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

    #[test]
    fn skill_completion_popup_layout_respects_requested_side_and_window_space() {
        let form_bounds = Bounds::new(point(px(100.), px(400.)), size(px(600.), px(120.)));
        let viewport = size(px(1000.), px(800.));

        let above = skill_completion_popup_layout(
            form_bounds,
            viewport,
            ChatFormSkillCompletionPlacement::AboveForm,
        )
        .unwrap();
        assert_eq!(above.anchor, Anchor::BottomLeft);
        assert_eq!(above.position, point(px(100.), px(400.)));
        assert_eq!(above.offset, point(px(0.), px(-SKILL_COMPLETION_GAP)));
        assert_eq!(above.max_height, px(SKILL_COMPLETION_MAX_HEIGHT));

        let below = skill_completion_popup_layout(
            form_bounds,
            viewport,
            ChatFormSkillCompletionPlacement::BelowForm,
        )
        .unwrap();
        assert_eq!(below.anchor, Anchor::TopLeft);
        assert_eq!(below.position, point(px(100.), px(520.)));
        assert_eq!(below.offset, point(px(0.), px(SKILL_COMPLETION_GAP)));
        assert_eq!(below.max_height, px(266.));
    }

    #[test]
    fn skill_completion_popup_layout_skips_when_no_window_space_remains() {
        let form_bounds = Bounds::new(point(px(100.), px(786.)), size(px(600.), px(12.)));
        let viewport = size(px(1000.), px(800.));

        assert_eq!(
            skill_completion_popup_layout(
                form_bounds,
                viewport,
                ChatFormSkillCompletionPlacement::BelowForm,
            ),
            None
        );
    }

    #[gpui::test]
    fn skill_completion_popup_matches_chat_form_bounds(cx: &mut TestAppContext) {
        let _dir = init_chat_form_test(cx);
        let window = open_chat_form_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let form = window.root(&mut cx).unwrap();

        cx.simulate_resize(size(px(800.), px(600.)));
        cx.update(|_, cx| {
            form.update(cx, |form, cx| {
                form.apply_skill_catalog_entries(vec![test_skill_entry("gpui")], cx);
            });
        });
        cx.simulate_input("$");

        let form_bounds = cx.debug_bounds("jaco-chat-form").expect("chat form bounds");
        let popup_bounds = cx
            .debug_bounds("jaco-skill-completion-popup")
            .expect("skill completion popup bounds");
        let viewport = cx.update(|window, _| window.viewport_size());

        let width_delta =
            (popup_bounds.size.width.as_f32() - form_bounds.size.width.as_f32()).abs();
        assert!(
            width_delta <= 2.,
            "popup={popup_bounds:?}, form={form_bounds:?}",
        );
        assert!(
            popup_bounds.top() >= form_bounds.bottom(),
            "popup={popup_bounds:?}, form={form_bounds:?}",
        );
        assert!(
            popup_bounds.bottom() <= viewport.height,
            "popup={popup_bounds:?}, viewport={viewport:?}",
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
    fn submit_includes_selected_approval_mode(cx: &mut TestAppContext) {
        let _dir = init_chat_form_test(cx);
        let window = open_chat_form_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let form = window.root(&mut cx).unwrap();

        let default_submit = submit_snapshot(&form, test_snapshot("hello"), &mut cx)
            .expect("selected model can be submitted");
        assert_eq!(
            default_submit.approval_mode,
            ToolApprovalMode::RequestApproval
        );

        cx.update(|window, cx| {
            form.update(cx, |form, cx| {
                form.select_approval_mode(ToolApprovalMode::FullAccess, window, cx);
            });
        });
        let changed_submit = submit_snapshot(&form, test_snapshot("hello"), &mut cx)
            .expect("selected model can be submitted");
        assert_eq!(changed_submit.approval_mode, ToolApprovalMode::FullAccess);
    }

    #[gpui::test]
    fn chat_form_initializes_from_config_preferences(cx: &mut TestAppContext) {
        let _dir = init_chat_form_test(cx);
        let provider_id = cx.update(|cx| provider_id_for_kind(cx, "openai"));
        cx.update(|cx| {
            state::config::update_chat_form_config(cx, |config| {
                config.model = Some(ChatFormModelConfig {
                    provider_id,
                    model_id: "gpt-5-mini".to_string(),
                });
                config.approval_mode = ToolApprovalMode::FullAccess;
            })
            .unwrap();
        });

        let window = open_chat_form_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let form = window.root(&mut cx).unwrap();

        assert_eq!(selected_model_id(&form, &cx).as_deref(), Some("gpt-5-mini"));
        let submit = submit_snapshot(&form, test_snapshot("hello"), &mut cx)
            .expect("configured model can be submitted");
        assert_eq!(submit.approval_mode, ToolApprovalMode::FullAccess);
    }

    #[gpui::test]
    fn selecting_model_and_approval_mode_persists_config(cx: &mut TestAppContext) {
        let dir = init_chat_form_test(cx);
        let config_path = test_config_path(&dir);
        let window = open_chat_form_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let form = window.root(&mut cx).unwrap();
        let provider_id = cx.update(|_, cx| provider_id_for_kind(cx, "openai"));

        cx.update(|window, cx| {
            form.update(cx, |form, cx| {
                form.select_model(
                    ProviderModelKey {
                        provider_id: provider_id.clone(),
                        model_id: "gpt-5-mini".to_string(),
                    },
                    window,
                    cx,
                );
                form.select_approval_mode(ToolApprovalMode::FullAccess, window, cx);
            });
        });

        let config =
            state::JacoConfig::load_from_path_for_test(&config_path).expect("reload config");
        assert_eq!(
            config
                .chat_form
                .model
                .as_ref()
                .map(|model| (model.provider_id.as_str(), model.model_id.as_str())),
            Some((provider_id.as_str(), "gpt-5-mini"))
        );
        assert_eq!(config.chat_form.approval_mode, ToolApprovalMode::FullAccess);
    }

    #[gpui::test]
    fn custom_token_budget_persists_config(cx: &mut TestAppContext) {
        let dir = init_chat_form_reasoning_test(cx);
        let config_path = test_config_path(&dir);
        let window = open_chat_form_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let form = window.root(&mut cx).unwrap();

        cx.update(|window, cx| {
            form.update(cx, |form, cx| {
                let input = form.token_budget_input.clone();
                form.apply_custom_token_budget(2048, &input, window, cx);
            });
        });

        let config =
            state::JacoConfig::load_from_path_for_test(&config_path).expect("reload config");
        assert_eq!(
            config.chat_form.reasoning_selection,
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
            let config =
                state::JacoConfig::load_from_path_for_test(&test_config_path(&dir)).unwrap();
            state::config::install_for_test(cx, config).expect("install config store");
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
            state::providers::init(cx);
        });
        dir
    }

    fn init_chat_form_reasoning_test(cx: &mut TestAppContext) -> TempDir {
        let dir = tempdir().unwrap();
        cx.update(|cx| {
            gpui_component::init(cx);
            cx.set_global(FreshStoreGlobal::open_in_dir(dir.path()).unwrap());
            let config =
                state::JacoConfig::load_from_path_for_test(&test_config_path(&dir)).unwrap();
            state::config::install_for_test(cx, config).expect("install config store");
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
            state::providers::init(cx);
        });
        dir
    }

    fn test_skill_entry(name: &str) -> state::skills::GlobalSkillEntry {
        state::skills::GlobalSkillEntry {
            name: name.to_string(),
            description: Some("GPUI framework knowledge".to_string()),
            source_kind: SkillSourceKind::User,
            skill_file_path: PathBuf::from(format!("/skills/{name}/SKILL.md")),
            directory_path: PathBuf::from(format!("/skills/{name}")),
            search_text: format!("{name} GPUI framework knowledge /skills/{name}/SKILL.md"),
        }
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

    fn test_config_path(dir: &TempDir) -> PathBuf {
        dir.path().join("config.toml")
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

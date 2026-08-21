#![allow(dead_code)]

pub(crate) mod approval_select;
mod attachment_flow;
pub(crate) mod attachments;
pub(crate) mod composer_editor;
pub(crate) mod effort_select;
mod form_state;

pub(crate) use composer_editor::{ComposerEditor, ComposerEditorEvent, ComposerSnapshot};
pub(crate) use form_state::ChatInputInput;

use crate::components::chat::run_settings::reasoning_selection_is_valid;
use crate::{
    app::file_watch::{self, FileWatchBinding},
    components::{
        chat::context_occupancy::composer_context_projection,
        chat::form::{
            AgentRunControlStatus, AgentRunStatusSource, AttachmentControlState, ChatForm,
            ChatFormControls, ChatFormState, ChatFormUiEvent, ControlSlot,
            PrimaryActionControlState, ProjectControlState, RunSettingsControls,
        },
        chat::run_settings::resolve_run_settings,
        chat::run_settings::{RunSettingsController, RunSettingsInput},
    },
    features::{conversation, skills},
    foundation, state,
    state::config::ChatFormModelConfig,
    state::providers::{ProviderModelChoice, ProviderModelKey},
};
use conversation::attachments::{ComposerAttachment, ModelAttachmentSupportIssue};
use gpui::*;
use gpui_component::{
    ActiveTheme, Disableable, Sizable, WindowExt as _,
    button::Button,
    h_flex,
    label::Label,
    notification::{Notification, NotificationType},
    v_flex,
};
use gpui_form::{Form, FormEvent};
use gpui_operation::{Complete, Load, Refresh, Retry, Transition};
use jaco_core::{ConversationContextRequestUsage, ReasoningSelectionSnapshot, ToolApprovalMode};
use std::{path::Path, rc::Rc};
use tracing::{Level, event};

pub(super) const COMPOSER_BUTTON_SIZE: f32 = 28.;
pub(super) const COMPOSER_BUTTON_ICON_SIZE: f32 = 18.;
pub(super) const COMPOSER_BUTTON_RADIUS: f32 = 999.;
pub(crate) const COMPOSER_EDITOR_KEY_CONTEXT: &str = composer_editor::KEY_CONTEXT;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChatFormSkillCompletionPlacement {
    AboveForm,
    BelowForm,
}

#[allow(dead_code, clippy::enum_variant_names)]
#[derive(Clone)]
pub(crate) enum ChatInputEvent {
    AddRequested,
    AddProjectRequested,
    SendRequested(Box<ChatInputSubmit>),
    StopRequested,
}

impl EventEmitter<ChatInputEvent> for ChatInputController {}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ChatInputSubmit {
    pub(crate) composer: ComposerSnapshot,
    pub(crate) attachments: Vec<ComposerAttachment>,
    pub(crate) provider_model: ProviderModelChoice,
    pub(crate) reasoning_selection: Option<ReasoningSelectionSnapshot>,
    pub(crate) approval_mode: ToolApprovalMode,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ChatInputSubmitError {
    Empty,
    AgentRunning,
    RunSettings(crate::components::chat::run_settings::RunSettingsSubmitError),
    Attachment(ModelAttachmentSupportIssue),
}

pub(crate) fn build_chat_input_submit(
    prepared: ChatInputInput,
    catalog: &Result<Vec<ProviderModelChoice>, SharedString>,
) -> Result<ChatInputSubmit, ChatInputSubmitError> {
    if prepared.composer.is_empty() && prepared.attachments.is_empty() {
        return Err(ChatInputSubmitError::Empty);
    }
    let resolved = resolve_run_settings(&prepared.run_settings, catalog)
        .map_err(ChatInputSubmitError::RunSettings)?;
    if let Some(issue) = conversation::attachments::model_support_issue(
        &prepared.attachments,
        Some(&resolved.provider_model.capabilities),
    ) {
        return Err(ChatInputSubmitError::Attachment(issue));
    }
    Ok(ChatInputSubmit {
        composer: prepared.composer,
        attachments: prepared.attachments,
        provider_model: resolved.provider_model,
        reasoning_selection: resolved.reasoning_selection,
        approval_mode: resolved.approval_mode,
    })
}

#[derive(Debug, PartialEq)]
enum ChatInputPrimaryButtonAction {
    Send(Box<ChatInputSubmit>),
    Stop,
}

#[derive(Clone, Debug)]
enum ChatFormFieldPatch {
    Model(Option<ChatFormModelConfig>),
    ReasoningSelection(Option<ReasoningSelectionSnapshot>),
    ApprovalMode(ToolApprovalMode),
}

fn skill_load_is_current(
    current_scope: &skills::SkillCatalogScope,
    current_generation: u64,
    completed_scope: &skills::SkillCatalogScope,
    completed_generation: u64,
) -> bool {
    current_scope == completed_scope && current_generation == completed_generation
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SkillWatchRefreshRequest {
    Immediate,
    Pending,
}

fn skill_watch_refresh_request(is_running: bool) -> SkillWatchRefreshRequest {
    if is_running {
        SkillWatchRefreshRequest::Pending
    } else {
        SkillWatchRefreshRequest::Immediate
    }
}

pub(crate) struct ChatInputController {
    composer: Entity<ComposerEditor>,
    chat_form: Entity<ChatFormState>,
    chat_form_controls: ChatFormControls,
    form: Entity<Form<ChatInputInput>>,
    run_settings: Entity<RunSettingsController<ChatInputInput>>,
    primary_action_state: Entity<PrimaryActionControlState>,
    latest_context_request_usage: Option<ConversationContextRequestUsage>,
    next_attachment_id: u64,
    submission_problem: Option<SharedString>,
    skill_catalog_scope: skills::SkillCatalogScope,
    skill_catalog: skills::SkillCatalogOperation,
    skill_watch_binding: FileWatchBinding,
    pending_skill_dirty: bool,
    skill_load_generation: u64,
    _subscriptions: Vec<Subscription>,
}

#[derive(IntoElement)]
pub(crate) struct ChatInput {
    controller: Entity<ChatInputController>,
    skill_completion_placement: ChatFormSkillCompletionPlacement,
}

impl ChatInput {
    pub(crate) fn new(
        controller: &Entity<ChatInputController>,
        skill_completion_placement: ChatFormSkillCompletionPlacement,
    ) -> Self {
        Self {
            controller: controller.clone(),
            skill_completion_placement,
        }
    }
}

pub(crate) fn init(cx: &mut App) {
    composer_editor::init(cx);
}

impl ChatInputController {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::new_with_project_slot(ControlSlot::Hidden, true, window, cx)
    }

    pub(crate) fn new_without_focus(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::new_with_project_slot(ControlSlot::Hidden, false, window, cx)
    }

    pub(crate) fn new_with_project(
        project: Entity<ProjectControlState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new_with_project_slot(ControlSlot::Enabled(project), true, window, cx)
    }

    fn new_with_project_slot(
        project: ControlSlot<Entity<ProjectControlState>>,
        focus_composer: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let placeholder = cx.global::<foundation::I18n>().t("chat-form-placeholder");
        let composer = cx.new(|cx| ComposerEditor::new(placeholder.clone(), window, cx));
        if focus_composer {
            composer.update(cx, |composer, cx| composer.focus(window, cx));
        }
        let skill_catalog_scope = skills::SkillCatalogScope::Global;
        let skill_watch_binding = Self::create_skill_watch_binding(&skill_catalog_scope, cx);
        let mut skill_catalog = skills::SkillCatalogOperation::new();
        skill_catalog.transition(Load(Task::ready(())));
        skill_catalog.transition(Complete(skills::load_catalog(
            skills::SkillCatalogScope::Global,
        )));
        if let Some(data) = skill_catalog.data() {
            composer.update(cx, |composer, cx| {
                composer.set_skill_entries(data.entries(), cx)
            });
        }
        let model_choices = load_model_choices(cx);
        let configured_chat_form = state::config::read(cx, |config| config.chat_form.clone());
        let selected_model_key = configured_model_key_in(configured_chat_form.model.as_ref());
        let selected_reasoning_selection = initial_reasoning_selection(
            &model_choices,
            selected_model_key.as_ref(),
            configured_chat_form.reasoning_selection.as_ref(),
        );
        let selected_approval_mode = configured_chat_form.approval_mode;
        let composer_subscription = cx.subscribe_in(
            &composer,
            window,
            |form, composer, event: &ComposerEditorEvent, window, cx| match event {
                ComposerEditorEvent::Changed => {
                    let snapshot = composer.read(cx).snapshot();
                    ChatInputInput::COMPOSER.set(&form.form, snapshot, cx);
                    cx.notify();
                }
                ComposerEditorEvent::PasteAttachmentRequested(item) => {
                    form.add_attachments_from_clipboard(item.clone(), window, cx);
                }
                ComposerEditorEvent::SubmitRequested(snapshot) => {
                    if let Some(submit) = form.submit_snapshot(snapshot.clone(), window, cx) {
                        cx.emit(ChatInputEvent::SendRequested(Box::new(submit)));
                    }
                }
            },
        );
        let mut subscriptions = vec![composer_subscription];

        let form = cx.new(|cx| {
            Form::new(ChatInputInput::new(
                composer.read(cx).snapshot(),
                Vec::new(),
                RunSettingsInput::new(
                    selected_model_key.clone(),
                    selected_reasoning_selection.clone(),
                    selected_approval_mode,
                ),
            ))
        });
        let run_settings_field = ChatInputInput::ROOT.then(ChatInputInput::RUN_SETTINGS);
        let run_settings = cx.new(|cx| {
            RunSettingsController::new(form.clone(), run_settings_field.clone(), window, cx)
        });
        let run_settings_states = run_settings.read(cx).control_states();
        let attachments_state = cx.new(|_| AttachmentControlState {
            attachments: Vec::new(),
        });
        let primary_action_state = cx.new(|_| PrimaryActionControlState::default());
        let chat_form_controls = ChatFormControls {
            project,
            composer: ControlSlot::Enabled(composer.clone()),
            attachments: ControlSlot::Enabled(attachments_state.clone()),
            add_attachment: ControlSlot::Enabled(
                crate::components::chat::form::AddAttachmentControl,
            ),
            run_settings: RunSettingsControls {
                model: ControlSlot::Enabled(run_settings_states.model),
                reasoning: ControlSlot::Enabled(run_settings_states.reasoning),
                approval: ControlSlot::Enabled(run_settings_states.approval),
            },
            primary_action: ControlSlot::Enabled(primary_action_state.clone()),
        };
        let chat_form = cx.new(|cx| ChatFormState::new(&chat_form_controls, cx));
        let chat_form_subscription = cx.subscribe_in(
            &chat_form,
            window,
            |form, _chat_form, event: &ChatFormUiEvent, window, cx| match event {
                ChatFormUiEvent::AddAttachmentFilesRequested => {
                    form.open_add_attachment_prompt(window, cx);
                }
                ChatFormUiEvent::ExternalPathsDropped(paths) => {
                    form.add_attachment_paths(paths.clone(), window, cx);
                }
                ChatFormUiEvent::AddAttachmentFromClipboardRequested => {
                    form.add_attachments_from_current_clipboard(window, cx);
                }
                ChatFormUiEvent::OpenAttachmentRequested(attachment) => {
                    form.open_attachment(attachment.clone(), window, cx);
                }
                ChatFormUiEvent::RemoveAttachmentRequested(local_id) => {
                    form.remove_attachment(*local_id, window, cx);
                }
                ChatFormUiEvent::AddProjectRequested => {
                    cx.emit(ChatInputEvent::AddProjectRequested);
                }
                ChatFormUiEvent::PrimaryActionRequested => {
                    if matches!(event, ChatFormUiEvent::PrimaryActionRequested) {
                        form.emit_primary_button_action(window, cx);
                    }
                }
            },
        );
        subscriptions.push(chat_form_subscription);
        let settings_keys = {
            let current = form.read(cx);
            [
                run_settings_field
                    .clone()
                    .then(RunSettingsInput::MODEL)
                    .key(current),
                run_settings_field
                    .clone()
                    .then(RunSettingsInput::REASONING_SELECTION)
                    .key(current),
                run_settings_field
                    .clone()
                    .then(RunSettingsInput::APPROVAL_MODE)
                    .key(current),
            ]
        };
        subscriptions.push(cx.subscribe_in(
            &form,
            window,
            move |form, _, event: &FormEvent<ChatInputInput>, window, cx| {
                let FormEvent::ModelChanged(change) = event else {
                    return;
                };
                let changed = std::array::from_fn::<_, 3, _>(|index| {
                    change.impact(&settings_keys[index]).value_changed()
                });
                if !changed.into_iter().any(|changed| changed) {
                    return;
                }
                let settings = ChatInputInput::ROOT
                    .then(ChatInputInput::RUN_SETTINGS)
                    .get(&form.form, cx);
                let mut patches = Vec::with_capacity(3);
                if changed[0] {
                    patches.push(ChatFormFieldPatch::Model(settings.model.as_ref().map(
                        |key| ChatFormModelConfig {
                            provider_id: key.provider_id.clone(),
                            model_id: key.model_id.clone(),
                        },
                    )));
                }
                if changed[1] {
                    patches.push(ChatFormFieldPatch::ReasoningSelection(
                        settings.reasoning_selection,
                    ));
                }
                if changed[2] {
                    patches.push(ChatFormFieldPatch::ApprovalMode(settings.approval_mode));
                }
                form.save_chat_form_config(patches, window, cx);
            },
        ));
        let attachments_key = ChatInputInput::ROOT
            .then(ChatInputInput::ATTACHMENTS)
            .key(form.read(cx));
        subscriptions.push(cx.subscribe_in(&form, window, {
            let form = form.clone();
            let attachments_state = attachments_state.clone();
            move |_, _, event: &FormEvent<ChatInputInput>, _window, cx| {
                let FormEvent::ModelChanged(change) = event else {
                    return;
                };
                if !change.impact(&attachments_key).value_changed() {
                    return;
                }
                let attachments = ChatInputInput::ATTACHMENTS.get(&form, cx);
                attachments_state.update(cx, |state, cx| {
                    state.attachments = attachments;
                    cx.notify();
                });
            }
        }));
        subscriptions.push(cx.observe(&form, |_, _, cx| cx.notify()));
        subscriptions.push(state::config::store(cx).observe_select_in(
            cx,
            window,
            state::config::SelectConfigGateStatus,
            |_form, _status, _window, cx| cx.notify(),
        ));
        subscriptions.push(crate::database::store(cx).observe_select_in(
            cx,
            window,
            crate::database::SelectDatabaseReady,
            |_form, _ready, _window, cx| cx.notify(),
        ));
        if cx.has_global::<state::providers::ProviderStore>() {
            subscriptions.push(state::providers::catalog(cx).observe_select_in(
                cx,
                window,
                state::providers::SelectProviderModelCatalog,
                |_form, _catalog, _window, cx| cx.notify(),
            ));
        }

        Self {
            composer,
            chat_form,
            chat_form_controls,
            form,
            run_settings,
            primary_action_state,
            latest_context_request_usage: None,
            next_attachment_id: 1,
            submission_problem: None,
            skill_catalog_scope,
            skill_catalog,
            skill_watch_binding,
            pending_skill_dirty: false,
            skill_load_generation: 0,
            _subscriptions: subscriptions,
        }
    }

    pub(crate) fn focus_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.composer
            .update(cx, |composer, cx| composer.focus(window, cx));
    }

    pub(crate) fn set_latest_context_request_usage(
        &mut self,
        latest_context_request_usage: Option<ConversationContextRequestUsage>,
        cx: &mut Context<Self>,
    ) {
        if self.latest_context_request_usage == latest_context_request_usage {
            return;
        }
        self.latest_context_request_usage = latest_context_request_usage;
        cx.notify();
    }

    pub(crate) fn refresh_skill_catalog(
        &mut self,
        project_root: Option<&Path>,
        cx: &mut Context<Self>,
    ) {
        let scope = project_root
            .map(|root| skills::SkillCatalogScope::Project {
                root: root.to_path_buf(),
            })
            .unwrap_or(skills::SkillCatalogScope::Global);
        if self.skill_catalog_scope == scope {
            self.request_skill_catalog_refresh(cx);
            return;
        }

        self.skill_load_generation = self.skill_load_generation.wrapping_add(1);
        self.skill_catalog_scope = scope.clone();
        self.skill_watch_binding = Self::create_skill_watch_binding(&scope, cx);
        self.pending_skill_dirty = false;
        self.skill_catalog = skills::SkillCatalogOperation::new();
        self.composer
            .update(cx, |composer, cx| composer.set_skill_entries(&[], cx));
        self.start_skill_catalog_load(scope, self.skill_load_generation, cx);
    }

    fn create_skill_watch_binding(
        scope: &skills::SkillCatalogScope,
        cx: &mut Context<Self>,
    ) -> FileWatchBinding {
        let targets = skills::watch_roots(scope)
            .into_iter()
            .filter_map(|path| match file_watch::directory_tree(path) {
                Ok(target) => Some(target),
                Err(problem) => {
                    file_watch::report_problem(problem, cx);
                    None
                }
            })
            .collect();
        file_watch::bind(targets, cx, |controller, cx| {
            controller.on_skill_watch_dirty(cx)
        })
    }

    fn on_skill_watch_dirty(&mut self, cx: &mut Context<Self>) {
        match skill_watch_refresh_request(self.skill_catalog.is_running()) {
            SkillWatchRefreshRequest::Pending => self.pending_skill_dirty = true,
            SkillWatchRefreshRequest::Immediate => self.request_skill_catalog_refresh(cx),
        }
    }

    fn request_skill_catalog_refresh(&mut self, cx: &mut Context<Self>) {
        match skill_watch_refresh_request(self.skill_catalog.is_running()) {
            SkillWatchRefreshRequest::Pending => self.pending_skill_dirty = true,
            SkillWatchRefreshRequest::Immediate => self.start_skill_catalog_load(
                self.skill_catalog_scope.clone(),
                self.skill_load_generation,
                cx,
            ),
        }
    }

    fn start_skill_catalog_load(
        &mut self,
        scope: skills::SkillCatalogScope,
        generation: u64,
        cx: &mut Context<Self>,
    ) {
        let task_scope = scope.clone();
        let load = cx.background_spawn(async move { skills::load_catalog(task_scope) });

        let task = cx.spawn(async move |form, cx| {
            let result = load.await;
            let Some(form) = form.upgrade() else {
                return;
            };
            form.update(cx, |form, cx| {
                if !skill_load_is_current(
                    &form.skill_catalog_scope,
                    form.skill_load_generation,
                    &scope,
                    generation,
                ) {
                    return;
                }
                form.skill_catalog.transition(Complete(result));
                if let Some(data) = form.skill_catalog.data() {
                    let entries = data.entries().to_vec();
                    form.apply_skill_catalog_entries(entries, cx);
                }
                if std::mem::take(&mut form.pending_skill_dirty) {
                    form.request_skill_catalog_refresh(cx);
                }
                cx.notify();
            });
        });
        match &self.skill_catalog {
            skills::SkillCatalogOperation::Idle(_) => {
                self.skill_catalog.transition(Load(task));
            }
            skills::SkillCatalogOperation::Ready(_)
            | skills::SkillCatalogOperation::Degraded(_) => {
                self.skill_catalog.transition(Refresh(task));
            }
            skills::SkillCatalogOperation::Unavailable(_) => {
                self.skill_catalog.transition(Retry(task));
            }
            skills::SkillCatalogOperation::Loading(_)
            | skills::SkillCatalogOperation::Refreshing(_)
            | skills::SkillCatalogOperation::Retrying(_)
            | skills::SkillCatalogOperation::RefreshingDegraded(_) => {
                self.pending_skill_dirty = true;
            }
        }
        cx.notify();
    }

    fn apply_skill_catalog_entries(
        &mut self,
        entries: Vec<skills::GlobalSkillEntry>,
        cx: &mut Context<Self>,
    ) {
        self.composer
            .update(cx, |composer, cx| composer.set_skill_entries(&entries, cx));
        cx.notify();
    }

    pub(crate) fn set_agent_run_status(
        &mut self,
        source: Rc<dyn AgentRunStatusSource>,
        cx: &mut Context<Self>,
    ) {
        self.primary_action_state.update(cx, |state, cx| {
            state.set_agent_run_status(source);
            cx.notify();
        });
    }

    pub(crate) fn refresh_primary_action(&self, cx: &mut Context<Self>) {
        cx.notify();
    }

    pub(crate) fn set_submission_problem(
        &mut self,
        problem: Option<SharedString>,
        cx: &mut Context<Self>,
    ) {
        if self.submission_problem == problem {
            return;
        }
        self.submission_problem = problem;
        cx.notify();
    }

    pub(crate) fn clear_after_submit(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.composer.update(cx, |composer, cx| composer.clear(cx));
        let empty_composer = self.composer.read(cx).snapshot();
        ChatInputInput::COMPOSER.set(&self.form, empty_composer, cx);
        ChatInputInput::ATTACHMENTS.set(&self.form, Vec::new(), cx);
        cx.notify();
    }

    fn primary_action_busy(&self, cx: &App) -> bool {
        self.primary_action_state.read(cx).agent_status(cx) != AgentRunControlStatus::Idle
    }

    fn agent_status(&self, cx: &App) -> AgentRunControlStatus {
        self.primary_action_state.read(cx).agent_status(cx)
    }

    fn save_chat_form_config(
        &self,
        patches: Vec<ChatFormFieldPatch>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Err(err) = state::config::update_chat_form_config(cx, move |config| {
            for patch in patches {
                match patch {
                    ChatFormFieldPatch::Model(model) => config.model = model,
                    ChatFormFieldPatch::ReasoningSelection(selection) => {
                        config.reasoning_selection = selection;
                    }
                    ChatFormFieldPatch::ApprovalMode(mode) => config.approval_mode = mode,
                }
            }
        }) {
            event!(Level::ERROR, error = ?err, "save chat form config failed");
            window.push_notification(
                Notification::new()
                    .title(
                        cx.global::<foundation::I18n>()
                            .t("notify-save-settings-failed"),
                    )
                    .message(err.to_string())
                    .with_type(NotificationType::Error),
                cx,
            );
        }
    }

    fn can_send(&self, cx: &App) -> bool {
        if self.submission_problem.is_some() || send_resource_problem(cx).is_some() {
            return false;
        }
        let draft = ChatInputInput::ROOT.get(&self.form, cx);
        let choices = load_model_choices(cx);
        build_chat_input_submit(draft, &choices).is_ok()
    }

    fn submit_snapshot(
        &mut self,
        snapshot: ComposerSnapshot,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<ChatInputSubmit> {
        if self.primary_action_busy(cx) || send_resource_problem(cx).is_some() {
            return None;
        }
        ChatInputInput::COMPOSER.set(&self.form, snapshot, cx);
        let choices = load_model_choices(cx);
        self.form
            .update(cx, |form, cx| form.prepare(cx))
            .ok()?
            .map(|draft| build_chat_input_submit(draft, &choices))
            .into_parts()
            .1
            .ok()
    }

    fn primary_button_action(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<ChatInputPrimaryButtonAction> {
        match self.agent_status(cx) {
            AgentRunControlStatus::Running => {
                return Some(ChatInputPrimaryButtonAction::Stop);
            }
            AgentRunControlStatus::Submitting | AgentRunControlStatus::Stopping => return None,
            AgentRunControlStatus::Idle => {}
        }

        let snapshot = self.composer.read(cx).snapshot();
        self.submit_snapshot(snapshot, window, cx)
            .map(|submit| ChatInputPrimaryButtonAction::Send(Box::new(submit)))
    }

    fn emit_primary_button_action(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.primary_button_action(window, cx) {
            Some(ChatInputPrimaryButtonAction::Send(submit)) => {
                cx.emit(ChatInputEvent::SendRequested(submit));
            }
            Some(ChatInputPrimaryButtonAction::Stop) => {
                cx.emit(ChatInputEvent::StopRequested);
            }
            None => {}
        }
    }
}

fn send_resource_problem(cx: &App) -> Option<&'static str> {
    if !state::config::store(cx).read(cx, |operation| {
        matches!(operation, state::config::ConfigOperation::Ready(_))
    }) {
        return Some("chat-form-send-config-unavailable");
    }
    if !crate::database::is_ready(cx) {
        return Some("chat-form-send-database-unavailable");
    }
    if !cx.has_global::<state::providers::ProviderStore>()
        || !state::providers::catalog(cx).read(cx, |operation| {
            matches!(operation, state::providers::ProviderOperation::Ready(_))
        })
    {
        return Some("chat-form-send-provider-unavailable");
    }
    None
}

impl View for ChatInput {
    fn entity_id(&self) -> Option<EntityId> {
        Some(self.controller.entity_id())
    }

    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let controller = self.controller.read(cx);
        let can_submit = controller.can_send(cx);
        let disabled_reason = controller.submission_problem.clone().or_else(|| {
            send_resource_problem(cx).map(|key| cx.global::<foundation::I18n>().t(key).into())
        });
        let selected_model = ChatInputInput::ROOT
            .then(ChatInputInput::RUN_SETTINGS)
            .then(RunSettingsInput::MODEL)
            .get(&controller.form, cx);
        let model_choices = load_model_choices(cx);
        let context_occupancy_projection = composer_context_projection(
            selected_model.as_ref(),
            &model_choices,
            controller.latest_context_request_usage.as_ref(),
        );
        let chat_form = ChatForm::new(&controller.chat_form, controller.chat_form_controls.clone())
            .skill_completion_placement(self.skill_completion_placement)
            .primary_action_projection(can_submit, disabled_reason)
            .context_occupancy_projection(context_occupancy_projection);
        let refresh_target = self.controller.downgrade();
        let status = (!matches!(
            controller.skill_catalog,
            skills::SkillCatalogOperation::Ready(_)
        ))
        .then(|| {
            let running = controller.skill_catalog.is_running();
            let message = controller
                .skill_catalog
                .problem()
                .map(ToString::to_string)
                .unwrap_or_else(|| cx.global::<foundation::I18n>().t("resource-status-loading"));
            let warning = controller.skill_catalog.problem().is_some();
            h_flex()
                .w_full()
                .items_center()
                .gap_2()
                .px_3()
                .py_2()
                .rounded(cx.theme().radius)
                .bg(cx.theme().warning.opacity(0.08))
                .child(
                    Label::new(message)
                        .text_xs()
                        .text_color(if warning {
                            cx.theme().warning
                        } else {
                            cx.theme().muted_foreground
                        })
                        .flex_1(),
                )
                .child(
                    Button::new("chat-form-refresh-skills")
                        .label(cx.global::<foundation::I18n>().t("resource-status-refresh"))
                        .xsmall()
                        .disabled(running)
                        .on_click(move |_, _window, cx| {
                            let _ = refresh_target.update(cx, |controller, cx| {
                                controller.request_skill_catalog_refresh(cx);
                            });
                        }),
                )
        });
        v_flex().w_full().gap_2().children(status).child(chat_form)
    }
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

fn configured_model_key_in(model: Option<&ChatFormModelConfig>) -> Option<ProviderModelKey> {
    let model = model?;
    Some(ProviderModelKey {
        provider_id: model.provider_id.clone(),
        model_id: model.model_id.clone(),
    })
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
}

#[cfg(test)]
mod tests {
    use super::{
        ChatFormSkillCompletionPlacement, ChatInput, ChatInputController, ChatInputInput,
        ChatInputPrimaryButtonAction,
        composer_editor::{ComposerSendPolicy, ComposerSnapshot},
        selected_model_choice_in, skill_load_is_current,
    };
    use super::{SkillWatchRefreshRequest, skill_watch_refresh_request};
    use crate::{
        components::chat::form::{
            AgentRunControlStatus, AgentRunStatusSource, SKILL_COMPLETION_GAP,
            SKILL_COMPLETION_MAX_HEIGHT, skill_completion_popup_layout,
        },
        database,
        features::skills,
        state,
        state::config::ChatFormModelConfig,
        state::providers::{ProviderModelChoice, ProviderModelKey},
    };
    use gpui::{
        Anchor, App, AppContext as _, Bounds, Entity, IntoElement, ParentElement as _, Render,
        Styled as _, Subscription, TestAppContext, View, VisualTestContext, WindowHandle, div,
        point, px, size,
    };
    use gpui_operation::Transition as _;
    use jaco_core::{
        CapabilitySourceSnapshot, ContentPart, ConversationContextRequestUsage,
        ModelCapabilitiesSnapshot, ProviderModelMetadata, ProviderSecretRefs,
        ProviderSettingFieldValue, ProviderSettingValue, ProviderSettingsPayload,
        ProviderUsageSnapshot, ReasoningCapabilitySnapshot, ReasoningControlSnapshot,
        ReasoningSelectionSnapshot, SkillSourceKind, TokenBudgetSelectionMode, ToolApprovalMode,
        conservative_model_capabilities,
    };
    use jaco_db::{NewProvider, NewProviderModel};
    use std::{cell::Cell, path::PathBuf, rc::Rc};
    use tempfile::{TempDir, tempdir};
    use time::OffsetDateTime;

    struct ConfigObservationProbe {
        _subscription: Subscription,
    }

    struct ContextUsageObservationProbe {
        _subscription: Subscription,
    }

    struct TestAgentRunStatus {
        status: Rc<Cell<AgentRunControlStatus>>,
    }

    impl AgentRunStatusSource for TestAgentRunStatus {
        fn status(&self, _cx: &App) -> AgentRunControlStatus {
            self.status.get()
        }
    }

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
    fn skill_load_generation_rejects_a_stale_same_scope_completion() {
        let scope = skills::SkillCatalogScope::Project {
            root: PathBuf::from("/work/project-a"),
        };

        assert!(skill_load_is_current(&scope, 3, &scope, 3));
        assert!(!skill_load_is_current(&scope, 3, &scope, 1));
        assert!(!skill_load_is_current(
            &scope,
            3,
            &skills::SkillCatalogScope::Global,
            3,
        ));
    }

    #[test]
    fn skill_load_generation_rejects_stale_completion_after_scope_returns_to_a() {
        let scope_a = skills::SkillCatalogScope::Project {
            root: PathBuf::from("/work/project-a"),
        };
        let scope_b = skills::SkillCatalogScope::Project {
            root: PathBuf::from("/work/project-b"),
        };

        // A@0 -> B@1 -> A@2. Both older completions must be ignored.
        assert!(!skill_load_is_current(&scope_a, 2, &scope_a, 0));
        assert!(!skill_load_is_current(&scope_a, 2, &scope_b, 1));
        assert!(skill_load_is_current(&scope_a, 2, &scope_a, 2));
    }

    #[test]
    fn same_scope_running_skill_dirty_is_held_for_refresh() {
        assert_eq!(
            skill_watch_refresh_request(true),
            SkillWatchRefreshRequest::Pending
        );
        assert_eq!(
            skill_watch_refresh_request(false),
            SkillWatchRefreshRequest::Immediate
        );

        let mut pending = false;
        if skill_watch_refresh_request(true) == SkillWatchRefreshRequest::Pending {
            pending = true;
        }
        assert!(pending);
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
        let window = open_chat_form_layout_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let host = window.root(&mut cx).unwrap();
        let form = host.read_with(&cx, |host, _| host.form.clone());

        cx.simulate_resize(size(px(800.), px(600.)));
        cx.update(|_, cx| {
            form.update(cx, |form, cx| {
                form.apply_skill_catalog_entries(vec![test_skill_entry("gpui")], cx);
            });
        });
        cx.update(|window, cx| {
            form.update(cx, |form, cx| form.focus_composer(window, cx));
        });
        cx.simulate_input("$");
        cx.run_until_parked();
        cx.update(|window, cx| {
            assert!(form.read(cx).composer.read(cx).skill_completion_open());
            window.refresh();
        });
        cx.run_until_parked();

        let form_bounds = cx.debug_bounds("jaco-chat-form").expect("chat form bounds");
        let viewport = cx.update(|window, _| window.viewport_size());
        let popup_bounds = cx
            .debug_bounds("jaco-skill-completion-popup")
            .unwrap_or_else(|| {
                panic!("skill completion popup bounds; form={form_bounds:?}, viewport={viewport:?}")
            });

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
    fn provider_catalog_refresh_updates_options_without_rebasing_form(cx: &mut TestAppContext) {
        let _dir = init_chat_form_test(cx);
        configure_chat_form_model(cx, "gpt-5");
        let window = open_chat_form_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let form = chat_input_controller(window, &mut cx);

        assert_eq!(selected_model_id(&form, &cx).as_deref(), Some("gpt-5"));

        cx.update(|_, cx| {
            let provider_id = provider_id_for_kind(cx, "openai");
            let record = test_repository(cx)
                .set_provider_model_enabled(&provider_id, "gpt-5", false)
                .expect("disable provider model");
            state::providers::catalog(cx).update(cx, |operation| {
                let state::providers::ProviderOperation::Ready(ready) = operation else {
                    panic!("provider catalog is ready");
                };
                ready.transition(state::providers::ProviderMessage::UpsertModel(Box::new(
                    record,
                )));
            });
        });

        // Catalog refreshes update the available options, but must not silently
        // rewrite the form value or choose another model.
        assert_eq!(selected_model_id(&form, &cx), None);

        assert!(submit_snapshot(&form, test_snapshot("hello"), &mut cx).is_none());
    }

    #[gpui::test]
    fn submit_revalidation_preserves_custom_token_budget(cx: &mut TestAppContext) {
        let _dir = init_chat_form_reasoning_test(cx);
        configure_chat_form_model(cx, "claude-3-7-sonnet");
        let window = open_chat_form_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let form = chat_input_controller(window, &mut cx);

        cx.update(|window, cx| {
            form.update(cx, |form, cx| {
                form.run_settings.update(cx, |settings, cx| {
                    settings.select_reasoning_value(
                        ChatInputInput::ROOT.then(ChatInputInput::RUN_SETTINGS),
                        ReasoningSelectionSnapshot::TokenBudget {
                            mode: TokenBudgetSelectionMode::Custom,
                            value: Some(4096),
                        },
                        window,
                        cx,
                    );
                    settings.set_custom_token_budget(
                        ChatInputInput::ROOT.then(ChatInputInput::RUN_SETTINGS),
                        2048,
                        window,
                        cx,
                    );
                });
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
        configure_chat_form_model(cx, "gpt-5");
        let window = open_chat_form_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let form = chat_input_controller(window, &mut cx);

        let default_submit = submit_snapshot(&form, test_snapshot("hello"), &mut cx)
            .expect("selected model can be submitted");
        assert_eq!(
            default_submit.approval_mode,
            ToolApprovalMode::RequestApproval
        );

        cx.update(|window, cx| {
            form.update(cx, |form, cx| {
                form.run_settings.update(cx, |settings, cx| {
                    settings.select_approval_value(
                        ChatInputInput::ROOT.then(ChatInputInput::RUN_SETTINGS),
                        ToolApprovalMode::FullAccess,
                        window,
                        cx,
                    );
                });
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
            .expect("update chat form config");
        });

        let window = open_chat_form_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let form = chat_input_controller(window, &mut cx);

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
        let form = chat_input_controller(window, &mut cx);
        let provider_id = cx.update(|_, cx| provider_id_for_kind(cx, "openai"));

        cx.update(|window, cx| {
            form.update(cx, |form, cx| {
                form.run_settings.update(cx, |settings, cx| {
                    settings.select_model_value(
                        ChatInputInput::ROOT.then(ChatInputInput::RUN_SETTINGS),
                        ProviderModelKey {
                            provider_id: provider_id.clone(),
                            model_id: "gpt-5-mini".to_string(),
                        },
                        window,
                        cx,
                    );
                    settings.select_approval_value(
                        ChatInputInput::ROOT.then(ChatInputInput::RUN_SETTINGS),
                        ToolApprovalMode::FullAccess,
                        window,
                        cx,
                    );
                });
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
    fn chat_field_patch_preserves_external_sibling_fields(cx: &mut TestAppContext) {
        let dir = init_chat_form_test(cx);
        let config_path = test_config_path(&dir);
        let window = open_chat_form_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let form = chat_input_controller(window, &mut cx);
        let external_reasoning = ReasoningSelectionSnapshot::Level {
            value: "external".to_string(),
        };

        cx.update(|_window, cx| {
            state::config::update_chat_form_config(cx, |config| {
                config.reasoning_selection = Some(external_reasoning.clone());
            })
            .expect("apply external sibling config");
        });
        cx.update(|window, cx| {
            form.update(cx, |form, cx| {
                form.run_settings.update(cx, |settings, cx| {
                    settings.select_approval_value(
                        ChatInputInput::ROOT.then(ChatInputInput::RUN_SETTINGS),
                        ToolApprovalMode::FullAccess,
                        window,
                        cx,
                    );
                });
            });
        });

        let config =
            state::JacoConfig::load_from_path_for_test(&config_path).expect("reload config");
        assert_eq!(
            config.chat_form.reasoning_selection,
            Some(external_reasoning)
        );
        assert_eq!(config.chat_form.approval_mode, ToolApprovalMode::FullAccess);
    }

    #[gpui::test]
    fn switching_models_resets_an_unsupported_reasoning_level_to_the_new_default(
        cx: &mut TestAppContext,
    ) {
        let dir = init_chat_form_test(cx);
        let config_path = test_config_path(&dir);
        let provider_id = cx.update(|cx| provider_id_for_kind(cx, "openai"));
        cx.update(|cx| {
            state::config::update_chat_form_config(cx, |config| {
                config.model = Some(ChatFormModelConfig {
                    provider_id: provider_id.clone(),
                    model_id: "gpt-5.6".to_string(),
                });
                config.reasoning_selection = Some(ReasoningSelectionSnapshot::Level {
                    value: "max".to_string(),
                });
            })
            .expect("configure GPT-5.6 reasoning");
        });

        let window = open_chat_form_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let form = chat_input_controller(window, &mut cx);
        assert_eq!(
            submit_snapshot(&form, test_snapshot("hello"), &mut cx)
                .expect("GPT-5.6 max can be submitted")
                .reasoning_selection,
            Some(ReasoningSelectionSnapshot::Level {
                value: "max".to_string(),
            })
        );

        cx.update(|window, cx| {
            form.update(cx, |form, cx| {
                form.run_settings.update(cx, |settings, cx| {
                    settings.select_model_value(
                        ChatInputInput::ROOT.then(ChatInputInput::RUN_SETTINGS),
                        ProviderModelKey {
                            provider_id: provider_id.clone(),
                            model_id: "gpt-5.5".to_string(),
                        },
                        window,
                        cx,
                    );
                });
            });
        });
        cx.run_until_parked();

        assert_eq!(
            submit_snapshot(&form, test_snapshot("hello"), &mut cx)
                .expect("GPT-5.5 default reasoning can be submitted")
                .reasoning_selection,
            Some(ReasoningSelectionSnapshot::Level {
                value: "medium".to_string(),
            })
        );
        let config =
            state::JacoConfig::load_from_path_for_test(&config_path).expect("reload config");
        assert_eq!(
            config.chat_form.reasoning_selection,
            Some(ReasoningSelectionSnapshot::Level {
                value: "medium".to_string(),
            })
        );
    }

    #[gpui::test]
    fn composer_changes_do_not_publish_config(cx: &mut TestAppContext) {
        let _dir = init_chat_form_test(cx);
        let deliveries = Rc::new(Cell::new(0));
        let _probe = cx.update(|cx| {
            cx.new(|cx| {
                let observed_deliveries = deliveries.clone();
                let subscription =
                    state::config::store(cx).observe(cx, move |_probe, _config, _cx| {
                        observed_deliveries.set(observed_deliveries.get() + 1);
                    });
                ConfigObservationProbe {
                    _subscription: subscription,
                }
            })
        });
        cx.run_until_parked();
        assert_eq!(deliveries.get(), 1);

        let window = open_chat_form_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let form = chat_input_controller(window, &mut cx);
        cx.update(|_window, cx| {
            form.update(cx, |form, cx| {
                ChatInputInput::COMPOSER.set(&form.form, test_snapshot("draft"), cx);
            });
        });
        cx.run_until_parked();

        assert_eq!(deliveries.get(), 1);
    }

    #[gpui::test]
    fn custom_token_budget_persists_config(cx: &mut TestAppContext) {
        let dir = init_chat_form_reasoning_test(cx);
        configure_chat_form_model(cx, "claude-3-7-sonnet");
        let config_path = test_config_path(&dir);
        let window = open_chat_form_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let form = chat_input_controller(window, &mut cx);

        cx.update(|window, cx| {
            form.update(cx, |form, cx| {
                form.run_settings.update(cx, |settings, cx| {
                    settings.select_reasoning_value(
                        ChatInputInput::ROOT.then(ChatInputInput::RUN_SETTINGS),
                        ReasoningSelectionSnapshot::TokenBudget {
                            mode: TokenBudgetSelectionMode::Custom,
                            value: Some(4096),
                        },
                        window,
                        cx,
                    );
                    settings.set_custom_token_budget(
                        ChatInputInput::ROOT.then(ChatInputInput::RUN_SETTINGS),
                        2048,
                        window,
                        cx,
                    );
                });
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
        configure_chat_form_model(cx, "gpt-5");
        let window = open_chat_form_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let form = chat_input_controller(window, &mut cx);
        let status = Rc::new(Cell::new(AgentRunControlStatus::Running));

        cx.update(|_, cx| {
            form.update(cx, |form, cx| {
                form.set_agent_run_status(
                    Rc::new(TestAgentRunStatus {
                        status: status.clone(),
                    }),
                    cx,
                );
            });
        });

        assert!(submit_snapshot(&form, test_snapshot("hello"), &mut cx).is_none());
        let action = cx.update(|window, cx| {
            form.update(cx, |form, cx| form.primary_button_action(window, cx))
        });
        assert_eq!(action, Some(ChatInputPrimaryButtonAction::Stop));

        status.set(AgentRunControlStatus::Idle);

        assert!(submit_snapshot(&form, test_snapshot("hello"), &mut cx).is_some());
    }

    #[gpui::test]
    fn stopping_agent_blocks_submit_and_primary_button_action(cx: &mut TestAppContext) {
        let _dir = init_chat_form_test(cx);
        configure_chat_form_model(cx, "gpt-5");
        let window = open_chat_form_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let form = chat_input_controller(window, &mut cx);

        cx.update(|_, cx| {
            form.update(cx, |form, cx| {
                form.set_agent_run_status(
                    Rc::new(TestAgentRunStatus {
                        status: Rc::new(Cell::new(AgentRunControlStatus::Stopping)),
                    }),
                    cx,
                );
            });
        });

        assert!(submit_snapshot(&form, test_snapshot("hello"), &mut cx).is_none());
        let action = cx.update(|window, cx| {
            form.update(cx, |form, cx| form.primary_button_action(window, cx))
        });
        assert_eq!(action, None);
    }

    #[gpui::test]
    fn submitting_agent_blocks_repeated_submit(cx: &mut TestAppContext) {
        let _dir = init_chat_form_test(cx);
        configure_chat_form_model(cx, "gpt-5");
        let window = open_chat_form_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let form = chat_input_controller(window, &mut cx);

        let status = Rc::new(Cell::new(AgentRunControlStatus::Submitting));
        cx.update(|_, cx| {
            form.update(cx, |form, cx| {
                form.set_agent_run_status(
                    Rc::new(TestAgentRunStatus {
                        status: status.clone(),
                    }),
                    cx,
                );
            });
        });

        assert!(submit_snapshot(&form, test_snapshot("hello"), &mut cx).is_none());
        let action = cx.update(|window, cx| {
            form.update(cx, |form, cx| form.primary_button_action(window, cx))
        });
        assert_eq!(action, None);

        status.set(AgentRunControlStatus::Idle);

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
            database::install_for_test(cx, dir.path());
            let config =
                state::JacoConfig::load_from_path_for_test(&test_config_path(&dir)).unwrap();
            state::config::install_for_test(cx, test_config_path(&dir), config)
                .expect("install config store");
            crate::foundation::i18n::init(cx);

            let repository = test_repository(cx);
            let provider = repository.insert_provider(provider_for_test()).unwrap();
            repository
                .replace_fetched_provider_models(
                    &provider.id,
                    vec![
                        provider_model_for_test(&provider.id, "gpt-5"),
                        provider_model_for_test(&provider.id, "gpt-5-mini"),
                        provider_model_with_capabilities(
                            &provider.id,
                            "gpt-5.6",
                            level_capabilities(&["low", "medium", "high", "max"], "medium"),
                        ),
                        provider_model_with_capabilities(
                            &provider.id,
                            "gpt-5.5",
                            level_capabilities(&["low", "medium", "high"], "medium"),
                        ),
                    ],
                )
                .unwrap();
            state::providers::init(cx);
        });
        cx.run_until_parked();
        dir
    }

    fn init_chat_form_reasoning_test(cx: &mut TestAppContext) -> TempDir {
        let dir = tempdir().unwrap();
        cx.update(|cx| {
            gpui_component::init(cx);
            database::install_for_test(cx, dir.path());
            let config =
                state::JacoConfig::load_from_path_for_test(&test_config_path(&dir)).unwrap();
            state::config::install_for_test(cx, test_config_path(&dir), config)
                .expect("install config store");
            crate::foundation::i18n::init(cx);

            let repository = test_repository(cx);
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
        cx.run_until_parked();
        dir
    }

    fn test_skill_entry(name: &str) -> skills::GlobalSkillEntry {
        skills::GlobalSkillEntry {
            name: name.to_string(),
            description: Some("GPUI framework knowledge".to_string()),
            source_kind: SkillSourceKind::User,
            skill_file_path: PathBuf::from(format!("/skills/{name}/SKILL.md")),
            directory_path: PathBuf::from(format!("/skills/{name}")),
            search_text: format!("{name} GPUI framework knowledge /skills/{name}/SKILL.md"),
        }
    }

    struct ChatInputTestHost {
        form: Entity<ChatInputController>,
    }

    impl Render for ChatInputTestHost {
        fn render(
            &mut self,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<Self>,
        ) -> impl IntoElement {
            ChatInput::new(&self.form, ChatFormSkillCompletionPlacement::BelowForm)
        }
    }

    fn open_chat_form_window(cx: &mut TestAppContext) -> WindowHandle<ChatInputTestHost> {
        cx.update(|cx| {
            cx.open_window(Default::default(), |window, cx| {
                let form = cx.new(|cx| ChatInputController::new(window, cx));
                cx.new(|_| ChatInputTestHost { form })
            })
        })
        .unwrap()
    }

    fn chat_input_controller(
        window: WindowHandle<ChatInputTestHost>,
        cx: &mut VisualTestContext,
    ) -> Entity<ChatInputController> {
        window
            .root(cx)
            .expect("chat input test host")
            .read_with(cx, |host, _| host.form.clone())
    }

    struct ChatInputLayoutTestHost {
        form: Entity<ChatInputController>,
    }

    impl Render for ChatInputLayoutTestHost {
        fn render(
            &mut self,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<Self>,
        ) -> impl IntoElement {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(div().w(px(600.)).h(px(180.)).child(ChatInput::new(
                    &self.form,
                    ChatFormSkillCompletionPlacement::BelowForm,
                )))
        }
    }

    fn open_chat_form_layout_window(
        cx: &mut TestAppContext,
    ) -> WindowHandle<ChatInputLayoutTestHost> {
        cx.update(|cx| {
            cx.open_window(Default::default(), |window, cx| {
                let form = cx.new(|cx| ChatInputController::new(window, cx));
                cx.new(|_| ChatInputLayoutTestHost { form })
            })
        })
        .unwrap()
    }

    #[gpui::test]
    fn view_uses_controller_identity_across_rebuilds(cx: &mut TestAppContext) {
        let _dir = init_chat_form_test(cx);
        let window = open_chat_form_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let controller = chat_input_controller(window, &mut cx);

        let below = ChatInput::new(&controller, ChatFormSkillCompletionPlacement::BelowForm);
        let above = ChatInput::new(&controller, ChatFormSkillCompletionPlacement::AboveForm);

        assert_eq!(below.entity_id(), Some(controller.entity_id()));
        assert_eq!(above.entity_id(), Some(controller.entity_id()));
    }

    #[gpui::test]
    fn composer_context_occupancy_precedes_the_model_selector(cx: &mut TestAppContext) {
        let _dir = init_chat_form_test(cx);
        configure_chat_form_model(cx, "gpt-5");
        let window = open_chat_form_layout_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        cx.run_until_parked();

        let occupancy = cx
            .debug_bounds("conversation-context-occupancy-trigger")
            .expect("composer context occupancy trigger");
        let model = cx
            .debug_bounds("picker-trigger-label:chat-form-model-trigger")
            .expect("model selector label");
        assert!(
            occupancy.right() <= model.left(),
            "occupancy={occupancy:?}, model={model:?}"
        );
    }

    #[gpui::test]
    fn context_request_usage_setter_notifies_only_for_value_changes(cx: &mut TestAppContext) {
        let _dir = init_chat_form_test(cx);
        let window = open_chat_form_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let controller = chat_input_controller(window, &mut cx);
        let deliveries = Rc::new(Cell::new(0));
        let _probe = cx.update(|_window, cx| {
            cx.new(|cx| {
                let observed_deliveries = deliveries.clone();
                let subscription = cx.observe(&controller, move |_probe, _controller, _cx| {
                    observed_deliveries.set(observed_deliveries.get() + 1);
                });
                ContextUsageObservationProbe {
                    _subscription: subscription,
                }
            })
        });
        cx.run_until_parked();
        let initial_deliveries = deliveries.get();
        let request_usage = ConversationContextRequestUsage {
            agent_run_id: "run-1".to_string(),
            provider_step_id: "step-1".to_string(),
            provider_step_seq: 1,
            provider_id: "provider-1".to_string(),
            model_id: "gpt-5".to_string(),
            provider_step_completed_at: OffsetDateTime::UNIX_EPOCH,
            agent_run_completed_at: OffsetDateTime::UNIX_EPOCH,
            usage: Some(ProviderUsageSnapshot {
                input_tokens: 1_200,
                output_tokens: 80,
                cached_input_tokens: 0,
                cache_write_input_tokens: 0,
                reasoning_tokens: 0,
                total_tokens: 1_280,
                metadata: None,
            }),
        };

        cx.update(|_window, cx| {
            controller.update(cx, |controller, cx| {
                controller.set_latest_context_request_usage(Some(request_usage.clone()), cx);
            });
        });
        cx.run_until_parked();
        assert_eq!(deliveries.get(), initial_deliveries + 1);

        cx.update(|_window, cx| {
            controller.update(cx, |controller, cx| {
                controller.set_latest_context_request_usage(Some(request_usage), cx);
            });
        });
        cx.run_until_parked();
        assert_eq!(deliveries.get(), initial_deliveries + 1);

        cx.update(|_window, cx| {
            controller.update(cx, |controller, cx| {
                controller.set_latest_context_request_usage(None, cx);
            });
        });
        cx.run_until_parked();
        assert_eq!(deliveries.get(), initial_deliveries + 2);
    }

    #[gpui::test]
    fn constructor_can_leave_composer_unfocused_for_embedded_inputs(cx: &mut TestAppContext) {
        let _dir = init_chat_form_test(cx);
        let window = cx
            .update(|cx| {
                cx.open_window(Default::default(), |window, cx| {
                    let form = cx.new(|cx| ChatInputController::new_without_focus(window, cx));
                    cx.new(|_| ChatInputTestHost { form })
                })
            })
            .unwrap();
        let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
        let form = chat_input_controller(window, &mut visual_cx);

        let composer_focused = visual_cx.update(|window, cx| {
            form.read(cx)
                .composer
                .read(cx)
                .focus_handle()
                .is_focused(window)
        });

        assert!(!composer_focused);
    }

    fn submit_snapshot(
        form: &gpui::Entity<ChatInputController>,
        snapshot: ComposerSnapshot,
        cx: &mut VisualTestContext,
    ) -> Option<super::ChatInputSubmit> {
        cx.update(|window, cx| {
            form.update(cx, |form, cx| form.submit_snapshot(snapshot, window, cx))
        })
    }

    fn selected_model_id(
        form: &gpui::Entity<ChatInputController>,
        cx: &VisualTestContext,
    ) -> Option<String> {
        form.read_with(cx, |form, cx| {
            form.run_settings
                .read(cx)
                .selected_model(ChatInputInput::ROOT.then(ChatInputInput::RUN_SETTINGS), cx)
                .map(|choice| choice.model_id)
        })
    }

    fn provider_id_for_kind(cx: &App, kind: &str) -> String {
        test_repository(cx)
            .list_providers()
            .unwrap()
            .into_iter()
            .find(|provider| provider.kind == kind)
            .expect("provider exists")
            .id
    }

    fn configure_chat_form_model(cx: &mut TestAppContext, model_id: &str) {
        let model_id = model_id.to_string();
        cx.update(|cx| {
            let provider_id = provider_id_for_kind(cx, "openai");
            state::config::update_chat_form_config(cx, move |config| {
                config.model = Some(ChatFormModelConfig {
                    provider_id,
                    model_id,
                });
            })
            .expect("configure chat form model");
        });
    }

    fn test_repository(cx: &App) -> jaco_db::FreshRepository {
        database::with_ready_repository(cx, |repository| Ok(repository.clone())).unwrap()
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
            pricing: None,
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

    fn level_capabilities(values: &[&str], default_value: &str) -> ModelCapabilitiesSnapshot {
        let mut capabilities = conservative_model_capabilities("openai");
        capabilities.reasoning = Some(ReasoningCapabilitySnapshot {
            default_effort: default_value.to_string(),
            efforts: values.iter().map(|value| (*value).to_string()).collect(),
            summaries: true,
            control: Some(ReasoningControlSnapshot::Levels {
                values: values.iter().map(|value| (*value).to_string()).collect(),
                default_value: Some(default_value.to_string()),
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
            send_policy: ComposerSendPolicy::EnterToSend,
        }
    }
}

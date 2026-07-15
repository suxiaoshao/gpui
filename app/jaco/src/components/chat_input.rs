#![allow(dead_code)]

pub(crate) mod approval_select;
mod attachment_flow;
pub(crate) mod attachments;
pub(crate) mod composer_editor;
pub(crate) mod effort_select;
mod form_state;

pub(crate) use composer_editor::{ComposerEditor, ComposerEditorEvent, ComposerSnapshot};
pub(crate) use form_state::{ChatInputFormStore, ChatInputInput};

use crate::components::run_settings::{
    computed_default_reasoning_selection, reasoning_selection_is_valid,
};
use crate::{
    components::{
        chat_form::{
            AttachmentControlState, ChatForm, ChatFormControls, ChatFormUiEvent, ControlSlot,
            PrimaryActionControlState, ProjectControlState, RunSettingsControls,
        },
        run_settings::{RunSettingsController, RunSettingsInput},
    },
    errors::JacoError,
    foundation, state,
    state::providers::{ProviderModelChoice, ProviderModelKey},
    state::{
        attachments::ComposerAttachment,
        config::{ChatFormConfig, ChatFormModelConfig},
    },
};
use gpui::*;
use gpui_store::StoreBinding;
use jaco_core::{ReasoningSelectionSnapshot, ToolApprovalMode};
use std::path::{Path, PathBuf};
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
    pub(crate) provider_model: ProviderModelChoice,
    pub(crate) reasoning_selection: Option<ReasoningSelectionSnapshot>,
    pub(crate) approval_mode: ToolApprovalMode,
}

#[derive(Debug, PartialEq)]
enum ChatInputPrimaryButtonAction {
    Send(Box<ChatInputSubmit>),
    Stop,
}

pub(crate) struct ChatInputController {
    composer: Entity<ComposerEditor>,
    chat_form: Entity<ChatForm>,
    form: Entity<ChatInputFormStore>,
    run_settings: Entity<RunSettingsController>,
    attachments_state: Entity<AttachmentControlState>,
    primary_action_state: Entity<PrimaryActionControlState>,
    chat_form_config: StoreBinding<ChatFormConfig, JacoError>,
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

impl ChatInputController {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::new_with_project_slot(ControlSlot::Hidden, window, cx)
    }

    pub(crate) fn new_with_project(
        project: Entity<ProjectControlState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new_with_project_slot(ControlSlot::Enabled(project), window, cx)
    }

    fn new_with_project_slot(
        project: ControlSlot<Entity<ProjectControlState>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
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
        let selected_approval_mode = configured_chat_form.approval_mode;
        let composer_subscription = cx.subscribe_in(
            &composer,
            window,
            |form, composer, event: &ComposerEditorEvent, window, cx| match event {
                ComposerEditorEvent::Changed => {
                    let snapshot = composer.read(cx).snapshot();
                    form.form.update(cx, |form, cx| {
                        form.set_composer_value(
                            snapshot,
                            gpui_form::FieldChangeCause::UserInput,
                            window,
                            cx,
                        );
                    });
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

        let form = cx.new(|cx| {
            ChatInputFormStore::from_value(
                ChatInputInput::new(
                    composer.read(cx).snapshot(),
                    Vec::new(),
                    RunSettingsInput::new(
                        selected_model_key.clone(),
                        selected_reasoning_selection.clone(),
                        selected_approval_mode,
                    ),
                ),
                window,
                cx,
            )
        });
        let run_settings_form = form.read(cx).run_settings_store();
        let run_settings_form_observer = run_settings_form.clone();
        let run_settings = cx.new(|cx| RunSettingsController::new(run_settings_form, window, cx));
        let run_settings_states = run_settings.read(cx).control_states();
        let attachments_state = cx.new(|_| AttachmentControlState::default());
        let primary_action_state = cx.new(|_| PrimaryActionControlState::default());
        let chat_form = cx.new(|cx| {
            ChatForm::new(
                ChatFormControls {
                    project,
                    composer: ControlSlot::Enabled(composer.clone()),
                    attachments: ControlSlot::Enabled(attachments_state.clone()),
                    add_attachment: ControlSlot::Enabled(
                        crate::components::chat_form::AddAttachmentControl,
                    ),
                    run_settings: RunSettingsControls {
                        model: ControlSlot::Enabled(run_settings_states.model),
                        reasoning: ControlSlot::Enabled(run_settings_states.reasoning),
                        approval: ControlSlot::Enabled(run_settings_states.approval),
                    },
                    primary_action: ControlSlot::Enabled(primary_action_state.clone()),
                },
                window,
                cx,
            )
        });
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
        subscriptions.push(cx.observe(&run_settings_form_observer, |form, _, cx| {
            form.save_chat_form_config(cx);
            form.sync_chat_form_projection(cx);
            cx.notify();
        }));

        let mut form = Self {
            composer,
            chat_form,
            form,
            run_settings,
            attachments_state,
            primary_action_state,
            chat_form_config,
            attachments: Vec::new(),
            next_attachment_id: 1,
            preview_attachment: None,
            agent_running: false,
            skill_catalog_scope: state::skills::SkillCatalogScope::Global,
            skill_catalog_task: Task::ready(()),
            _subscriptions: subscriptions,
        };

        form.sync_chat_form_projection(cx);
        if model_choices.is_ok() {
            form.save_chat_form_config(cx);
        }

        form
    }

    pub(crate) fn set_skill_completion_placement(
        &mut self,
        placement: ChatFormSkillCompletionPlacement,
        cx: &mut Context<Self>,
    ) {
        self.chat_form.update(cx, |form, _| {
            form.set_skill_completion_placement(placement);
        });
    }

    pub(crate) fn focus_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        self.sync_chat_form_projection(cx);
        cx.notify();
    }

    pub(crate) fn clear_after_submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.composer.update(cx, |composer, cx| composer.clear(cx));
        self.attachments.clear();
        self.preview_attachment = None;
        let empty_composer = self.composer.read(cx).snapshot();
        self.form.update(cx, |form, cx| {
            form.set_composer_value(
                empty_composer,
                gpui_form::FieldChangeCause::External,
                window,
                cx,
            );
            form.set_attachments_value(
                Vec::new(),
                gpui_form::FieldChangeCause::External,
                window,
                cx,
            );
        });
        self.sync_chat_form_projection(cx);
        cx.notify();
    }

    pub(crate) fn sync_chat_form_projection(&mut self, cx: &mut Context<Self>) {
        let attachments = self.attachments.clone();
        let agent_running = self.agent_running;
        let can_submit = self.can_send(cx);
        self.attachments_state.update(cx, |state, cx| {
            state.attachments = attachments;
            state.preview = self.preview_attachment.clone();
            cx.notify();
        });
        self.primary_action_state.update(cx, |state, cx| {
            state.agent_running = agent_running;
            state.can_submit = can_submit;
            cx.notify();
        });
    }

    pub(super) fn sync_form_attachments(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let attachments = self.attachments.clone();
        self.form.update(cx, |form, cx| {
            form.set_attachments_value(
                attachments,
                gpui_form::FieldChangeCause::External,
                window,
                cx,
            );
        });
    }

    fn save_chat_form_config(&self, cx: &mut Context<Self>) {
        let settings = self.run_settings.read(cx).form().read(cx).draft();
        let model = settings.model.as_ref().map(|key| ChatFormModelConfig {
            provider_id: key.provider_id.clone(),
            model_id: key.model_id.clone(),
        });
        let reasoning_selection = settings.reasoning_selection;
        let approval_mode = settings.approval_mode;

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
            && self.run_settings.read(cx).selected_model(cx).is_some()
            && self.attachment_support_issue_for(cx).is_none()
    }

    fn attachment_support_issue_for(
        &self,
        cx: &App,
    ) -> Option<state::attachments::ModelAttachmentSupportIssue> {
        let selected_model = self.run_settings.read(cx).selected_model(cx);
        state::attachments::model_support_issue(
            &self.attachments,
            selected_model.as_ref().map(|choice| &choice.capabilities),
        )
    }

    fn submit_snapshot(
        &mut self,
        mut snapshot: ComposerSnapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<ChatInputSubmit> {
        snapshot.attachments = self.attachments.clone();
        if snapshot.is_empty() {
            return None;
        }
        if self.attachment_support_issue_for(cx).is_some() {
            return None;
        }
        if self.agent_running {
            return None;
        }
        self.run_settings
            .update(cx, |settings, cx| settings.prepare_submit(window, cx));
        let form_snapshot = snapshot.clone();
        let form_attachments = self.attachments.clone();
        self.form.update(cx, |form, cx| {
            form.set_composer_value(
                form_snapshot,
                gpui_form::FieldChangeCause::External,
                window,
                cx,
            );
            form.set_attachments_value(
                form_attachments,
                gpui_form::FieldChangeCause::External,
                window,
                cx,
            );
        });
        let run_settings_form = self.run_settings.read(cx).form();
        let run_settings = run_settings_form.read(cx).draft();
        let provider_model = self.run_settings.read(cx).selected_model(cx)?;
        self.save_chat_form_config(cx);
        Some(ChatInputSubmit {
            composer: snapshot,
            provider_model,
            reasoning_selection: run_settings.reasoning_selection,
            approval_mode: run_settings.approval_mode,
        })
    }

    fn primary_button_action(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<ChatInputPrimaryButtonAction> {
        if self.agent_running {
            return Some(ChatInputPrimaryButtonAction::Stop);
        }

        let mut snapshot = self.composer.read(cx).snapshot();
        snapshot.attachments = self.attachments.clone();
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

impl Render for ChatInputController {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.chat_form.clone()
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
        ChatFormSkillCompletionPlacement, ChatInputController, ChatInputPrimaryButtonAction,
        composer_editor::{ComposerSendPolicy, ComposerSnapshot},
        model_empty_label, selected_model_choice_in,
    };
    use crate::{
        components::chat_form::{
            SKILL_COMPLETION_GAP, SKILL_COMPLETION_MAX_HEIGHT, skill_completion_popup_layout,
        },
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
                form.run_settings.update(cx, |settings, cx| {
                    settings.set_custom_token_budget(2048, window, cx);
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
                form.run_settings.update(cx, |settings, cx| {
                    settings.select_approval_value(ToolApprovalMode::FullAccess, window, cx);
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
                form.run_settings.update(cx, |settings, cx| {
                    settings.select_model_value(
                        ProviderModelKey {
                            provider_id: provider_id.clone(),
                            model_id: "gpt-5-mini".to_string(),
                        },
                        window,
                        cx,
                    );
                    settings.select_approval_value(ToolApprovalMode::FullAccess, window, cx);
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
    fn custom_token_budget_persists_config(cx: &mut TestAppContext) {
        let dir = init_chat_form_reasoning_test(cx);
        let config_path = test_config_path(&dir);
        let window = open_chat_form_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let form = window.root(&mut cx).unwrap();

        cx.update(|window, cx| {
            form.update(cx, |form, cx| {
                form.run_settings.update(cx, |settings, cx| {
                    settings.set_custom_token_budget(2048, window, cx);
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
        assert_eq!(action, Some(ChatInputPrimaryButtonAction::Stop));

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

    fn open_chat_form_window(cx: &mut TestAppContext) -> WindowHandle<ChatInputController> {
        cx.update(|cx| {
            cx.open_window(Default::default(), |window, cx| {
                cx.new(|cx| ChatInputController::new(window, cx))
            })
        })
        .unwrap()
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
                .selected_model(cx)
                .map(|choice| choice.model_id)
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

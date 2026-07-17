use std::{collections::BTreeMap, str::FromStr, time::SystemTime};

use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};
use gpui::{
    AnyWindowHandle, App, BorrowAppContext, Global, Image, ImageFormat, SharedString, Task,
};
use gpui_component::{
    Root, WindowExt as NotificationWindowExt,
    notification::{Notification, NotificationType},
};
use jaco_core::{
    AgentRunTriggerKind, ContentPart, PromptContent, PromptId, ShortcutAction, ShortcutId,
    ShortcutInputSource,
};
use jaco_db::ShortcutRecord;
use platform_ext::{OcrError, ocr::ImageFrame};
use tracing::{Level, event};

use crate::{
    app::{menus::ToggleTemporaryConversation, temporary_window},
    components::run_settings::reasoning_selection_is_valid,
    database,
    errors::{JacoError, JacoResult},
    features::screenshot::overlay as screenshot_overlay,
    foundation::I18n,
    platform::capture::CaptureError,
    state::{
        self,
        attachments::{ComposerAttachment, generated_image_attachment},
        config,
        providers::ProviderModelChoice,
    },
};

#[derive(Clone, Debug, PartialEq, Eq)]
enum RegisteredHotkeyAction {
    TemporaryConversation,
    Shortcut { shortcut_id: ShortcutId },
}

impl RegisteredHotkeyAction {
    fn label(&self) -> String {
        match self {
            Self::TemporaryConversation => "temporary".to_string(),
            Self::Shortcut { shortcut_id } => format!("shortcut:{shortcut_id}"),
        }
    }
}

trait HotkeyBackend {
    fn register(&mut self, hotkey: HotKey) -> JacoResult<()>;
    fn unregister(&mut self, hotkey: HotKey) -> JacoResult<()>;
}

struct SystemHotkeyBackend {
    manager: GlobalHotKeyManager,
}

impl HotkeyBackend for SystemHotkeyBackend {
    fn register(&mut self, hotkey: HotKey) -> JacoResult<()> {
        Ok(self.manager.register(hotkey)?)
    }

    fn unregister(&mut self, hotkey: HotKey) -> JacoResult<()> {
        Ok(self.manager.unregister(hotkey)?)
    }
}

struct DisabledHotkeyBackend {
    error: String,
}

impl HotkeyBackend for DisabledHotkeyBackend {
    fn register(&mut self, _hotkey: HotKey) -> JacoResult<()> {
        Err(JacoError::HotkeyUnavailable(self.error.clone()))
    }

    fn unregister(&mut self, _hotkey: HotKey) -> JacoResult<()> {
        Ok(())
    }
}

#[cfg(test)]
#[derive(Default)]
struct FakeHotkeyBackend;

#[cfg(test)]
impl HotkeyBackend for FakeHotkeyBackend {
    fn register(&mut self, _hotkey: HotKey) -> JacoResult<()> {
        Ok(())
    }

    fn unregister(&mut self, _hotkey: HotKey) -> JacoResult<()> {
        Ok(())
    }
}

pub(crate) struct GlobalHotkeyState {
    backend: Box<dyn HotkeyBackend>,
    temporary_hotkey: Option<String>,
    registered_shortcuts: BTreeMap<ShortcutId, String>,
    hotkey_actions: BTreeMap<u32, RegisteredHotkeyAction>,
    registration_errors: BTreeMap<String, String>,
    last_pressed: Option<HotkeyPressDiagnostics>,
    _task: Task<()>,
}

impl Global for GlobalHotkeyState {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HotkeyPressDiagnostics {
    pub(crate) hotkey_id: u32,
    pub(crate) action: String,
    pub(crate) pressed_at: SystemTime,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ShortcutRuntimeDiagnostics {
    pub(crate) temporary_hotkey: Option<String>,
    pub(crate) registered_shortcuts: BTreeMap<ShortcutId, String>,
    pub(crate) registration_errors: BTreeMap<String, String>,
    pub(crate) last_pressed: Option<HotkeyPressDiagnostics>,
}

#[derive(Clone)]
struct ShortcutTriggerContext {
    shortcut: ShortcutRecord,
    provider_model: ProviderModelChoice,
    prompt_id: Option<PromptId>,
    prompt_snapshot: Option<PromptContent>,
}

pub(crate) fn init(cx: &mut App) -> JacoResult<()> {
    let (tx, rx) = smol::channel::unbounded();
    GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
        if event.state() == HotKeyState::Pressed
            && let Err(err) = tx.send_blocking(event)
        {
            event!(Level::ERROR, error = ?err, "send hotkey event failed");
        }
    }));

    let task = cx.spawn(async move |cx| {
        while let Ok(event) = rx.recv().await {
            let hotkey_id = event.id();
            cx.update_global::<GlobalHotkeyState, _>(|hotkeys, cx| {
                let Some(action) = hotkeys.handle_pressed_hotkey(hotkey_id) else {
                    return;
                };

                match action {
                    RegisteredHotkeyAction::TemporaryConversation => {
                        cx.dispatch_action(&ToggleTemporaryConversation);
                    }
                    RegisteredHotkeyAction::Shortcut { shortcut_id } => {
                        hotkeys.dispatch_shortcut_trigger(shortcut_id, cx);
                    }
                }
            });
        }
    });

    let mut manager_error = None;
    let backend: Box<dyn HotkeyBackend> = match GlobalHotKeyManager::new() {
        Ok(manager) => Box::new(SystemHotkeyBackend { manager }),
        Err(err) => {
            event!(Level::ERROR, error = ?err, "global hotkey manager unavailable");
            manager_error = Some(err.to_string());
            Box::new(DisabledHotkeyBackend {
                error: err.to_string(),
            })
        }
    };
    let mut hotkeys = GlobalHotkeyState::new(backend, task);
    if let Some(err) = manager_error {
        hotkeys
            .registration_errors
            .insert("manager".to_string(), err);
    }
    hotkeys.load_initial_shortcuts(cx)?;
    event!(
        Level::INFO,
        temporary_hotkey = ?hotkeys.temporary_hotkey,
        registered_shortcuts = hotkeys.registered_shortcuts.len(),
        registration_errors = hotkeys.registration_errors.len(),
        "initialized jaco global hotkeys"
    );
    cx.set_global(hotkeys);
    Ok(())
}

#[cfg(test)]
pub(crate) fn set_test_hotkey_state(cx: &mut App) {
    cx.set_global(GlobalHotkeyState::new(
        Box::<FakeHotkeyBackend>::default(),
        Task::ready(()),
    ));
}

impl GlobalHotkeyState {
    fn new(backend: Box<dyn HotkeyBackend>, task: Task<()>) -> Self {
        Self {
            backend,
            temporary_hotkey: None,
            registered_shortcuts: BTreeMap::new(),
            hotkey_actions: BTreeMap::new(),
            registration_errors: BTreeMap::new(),
            last_pressed: None,
            _task: task,
        }
    }

    fn parse_hotkey(hotkey: &str) -> JacoResult<HotKey> {
        Ok(HotKey::from_str(hotkey)?)
    }

    fn register_action(&mut self, hotkey: &str, action: RegisteredHotkeyAction) -> JacoResult<()> {
        let hotkey = Self::parse_hotkey(hotkey)?;
        if self.hotkey_actions.get(&hotkey.id()) == Some(&action) {
            return Ok(());
        }

        self.backend.register(hotkey)?;
        event!(
            Level::INFO,
            hotkey = %hotkey,
            hotkey_id = hotkey.id(),
            action = ?action,
            "registered jaco hotkey"
        );
        self.hotkey_actions.insert(hotkey.id(), action);
        Ok(())
    }

    fn unregister_action(
        &mut self,
        hotkey: &str,
        expected_action: RegisteredHotkeyAction,
    ) -> JacoResult<()> {
        let hotkey = Self::parse_hotkey(hotkey)?;
        if self.hotkey_actions.get(&hotkey.id()) != Some(&expected_action) {
            event!(
                Level::INFO,
                hotkey = %hotkey,
                hotkey_id = hotkey.id(),
                expected_action = ?expected_action,
                "skip jaco hotkey unregister because registered action does not match"
            );
            return Ok(());
        }

        self.backend.unregister(hotkey)?;
        self.hotkey_actions.remove(&hotkey.id());
        event!(
            Level::INFO,
            hotkey = %hotkey,
            hotkey_id = hotkey.id(),
            action = ?expected_action,
            "unregistered jaco hotkey"
        );
        Ok(())
    }

    fn load_initial_shortcuts(&mut self, cx: &mut App) -> JacoResult<()> {
        if let Some(hotkey) = config::app_settings(cx)
            .temporary_hotkey()
            .map(str::to_string)
            && let Err(err) = self.register_temporary_hotkey(hotkey)
        {
            event!(Level::ERROR, error = ?err, "failed to load temporary hotkey");
        }

        let shortcuts = database::repository(cx).list_shortcuts()?;
        for shortcut in shortcuts {
            self.register_shortcut(shortcut);
        }

        Ok(())
    }

    fn register_temporary_hotkey(&mut self, hotkey: String) -> JacoResult<()> {
        let result = self.register_action(&hotkey, RegisteredHotkeyAction::TemporaryConversation);

        match result {
            Ok(()) => {
                self.temporary_hotkey = Some(hotkey);
                self.registration_errors.remove("temporary");
                Ok(())
            }
            Err(err) => {
                self.registration_errors
                    .insert("temporary".to_string(), err.to_string());
                event!(Level::ERROR, error = ?err, "failed to register temporary hotkey");
                Err(err)
            }
        }
    }

    pub(crate) fn update_temporary_hotkey(
        old_hotkey: Option<&str>,
        new_hotkey: Option<&str>,
        cx: &mut App,
    ) -> JacoResult<()> {
        if !cx.has_global::<GlobalHotkeyState>() {
            return Ok(());
        }

        let mut result = Ok(());
        cx.update_global::<GlobalHotkeyState, _>(|hotkeys, _cx| {
            result = hotkeys.update_temporary_hotkey_runtime(old_hotkey, new_hotkey);
        });
        result
    }

    fn update_temporary_hotkey_runtime(
        &mut self,
        old_hotkey: Option<&str>,
        new_hotkey: Option<&str>,
    ) -> JacoResult<()> {
        event!(
            Level::INFO,
            old_hotkey = ?old_hotkey,
            new_hotkey = ?new_hotkey,
            "updating jaco temporary hotkey"
        );

        if old_hotkey == new_hotkey {
            match new_hotkey {
                Some(new_hotkey) if self.temporary_hotkey.as_deref() != Some(new_hotkey) => {
                    self.register_temporary_hotkey(new_hotkey.to_string())?;
                }
                Some(_) => {
                    self.registration_errors.remove("temporary");
                }
                None => {
                    self.temporary_hotkey = None;
                    self.registration_errors.remove("temporary");
                }
            }
            return Ok(());
        }

        let registered_old_hotkey = self.temporary_hotkey.clone();

        if let Some(new_hotkey) = new_hotkey {
            self.register_temporary_hotkey(new_hotkey.to_string())?;

            if let Some(old_hotkey) = registered_old_hotkey
                .as_deref()
                .filter(|old_hotkey| *old_hotkey != new_hotkey)
                && let Err(err) = self
                    .unregister_action(old_hotkey, RegisteredHotkeyAction::TemporaryConversation)
            {
                self.registration_errors
                    .insert("temporary".to_string(), err.to_string());
                let _ = self
                    .unregister_action(new_hotkey, RegisteredHotkeyAction::TemporaryConversation);
                self.temporary_hotkey = registered_old_hotkey;
                return Err(err);
            }

            return Ok(());
        }

        if let Some(old_hotkey) = registered_old_hotkey.as_deref()
            && let Err(err) =
                self.unregister_action(old_hotkey, RegisteredHotkeyAction::TemporaryConversation)
        {
            self.registration_errors
                .insert("temporary".to_string(), err.to_string());
            return Err(err);
        }
        self.temporary_hotkey = None;
        self.registration_errors.remove("temporary");

        Ok(())
    }

    fn register_shortcut(&mut self, shortcut: ShortcutRecord) {
        if let Err(err) = self.upsert_shortcut_runtime(None, &shortcut) {
            crate::state::shortcuts::log_shortcut_runtime_sync_error(&shortcut.id, err);
        }
    }

    pub(crate) fn sync_shortcut_registration(
        previous: Option<&ShortcutRecord>,
        next: Option<&ShortcutRecord>,
        cx: &mut App,
    ) {
        if !cx.has_global::<GlobalHotkeyState>() {
            return;
        }

        cx.update_global::<GlobalHotkeyState, _>(|hotkeys, _cx| match (previous, next) {
            (None, Some(next)) => {
                if let Err(err) = hotkeys.upsert_shortcut_runtime(None, next) {
                    crate::state::shortcuts::log_shortcut_runtime_sync_error(&next.id, err);
                }
            }
            (Some(previous), Some(next)) => {
                if let Err(err) = hotkeys.upsert_shortcut_runtime(Some(previous), next) {
                    crate::state::shortcuts::log_shortcut_runtime_sync_error(&next.id, err);
                }
            }
            (Some(previous), None) => {
                if let Err(err) = hotkeys.remove_shortcut_runtime(previous) {
                    crate::state::shortcuts::log_shortcut_runtime_sync_error(&previous.id, err);
                }
            }
            (None, None) => {}
        });
    }

    fn upsert_shortcut_runtime(
        &mut self,
        previous: Option<&ShortcutRecord>,
        shortcut: &ShortcutRecord,
    ) -> JacoResult<()> {
        let action = RegisteredHotkeyAction::Shortcut {
            shortcut_id: shortcut.id.clone(),
        };
        let registered_previous = previous
            .and_then(|previous| {
                self.registered_shortcuts
                    .get(&previous.id)
                    .map(|hotkey| (previous.id.clone(), hotkey.clone()))
            })
            .or_else(|| {
                self.registered_shortcuts
                    .get(&shortcut.id)
                    .map(|hotkey| (shortcut.id.clone(), hotkey.clone()))
            });
        let previous_hotkey = registered_previous
            .as_ref()
            .map(|(_, hotkey)| hotkey.clone());
        let should_unregister_previous = previous_hotkey
            .as_deref()
            .is_some_and(|old_hotkey| old_hotkey != shortcut.hotkey || !shortcut.enabled);

        if let Some(old_hotkey) = previous_hotkey
            .as_deref()
            .filter(|_| should_unregister_previous)
        {
            self.unregister_action(old_hotkey, action.clone())?;
            self.registered_shortcuts.remove(&shortcut.id);
        }

        if shortcut.enabled
            && let Err(err) = self.register_action(&shortcut.hotkey, action.clone())
        {
            self.registration_errors
                .insert(shortcut.id.clone(), err.to_string());
            if let Some((previous_id, previous_hotkey)) = registered_previous {
                let _ = self.register_action(&previous_hotkey, action);
                self.registered_shortcuts
                    .insert(previous_id, previous_hotkey);
            }
            return Err(err);
        }

        if shortcut.enabled {
            self.registered_shortcuts
                .insert(shortcut.id.clone(), shortcut.hotkey.clone());
        } else {
            self.registered_shortcuts.remove(&shortcut.id);
        }
        self.registration_errors.remove(&shortcut.id);
        Ok(())
    }

    fn remove_shortcut_runtime(&mut self, shortcut: &ShortcutRecord) -> JacoResult<()> {
        let action = RegisteredHotkeyAction::Shortcut {
            shortcut_id: shortcut.id.clone(),
        };
        let hotkey = self
            .registered_shortcuts
            .remove(&shortcut.id)
            .unwrap_or_else(|| shortcut.hotkey.clone());
        self.registration_errors.remove(&shortcut.id);
        if shortcut.enabled {
            self.unregister_action(&hotkey, action)?;
        }
        Ok(())
    }

    fn handle_pressed_hotkey(&mut self, hotkey_id: u32) -> Option<RegisteredHotkeyAction> {
        let Some(action) = self.hotkey_actions.get(&hotkey_id).cloned() else {
            event!(
                Level::INFO,
                hotkey_id,
                "ignoring jaco hotkey press with no registered action"
            );
            return None;
        };

        let action_label = action.label();
        self.last_pressed = Some(HotkeyPressDiagnostics {
            hotkey_id,
            action: action_label.clone(),
            pressed_at: SystemTime::now(),
        });
        event!(
            Level::INFO,
            hotkey_id,
            action = %action_label,
            "recorded jaco hotkey press"
        );
        Some(action)
    }

    fn dispatch_shortcut_trigger(&mut self, shortcut_id: ShortcutId, cx: &mut App) {
        if screenshot_overlay::is_active(cx) {
            event!(
                Level::INFO,
                shortcut_id = %shortcut_id,
                "ignoring shortcut while screenshot overlay is active"
            );
            return;
        }

        let trigger = match self.resolve_shortcut_trigger_context(shortcut_id.clone(), cx) {
            Ok(Some(trigger)) => trigger,
            Ok(None) => return,
            Err(err) => {
                self.push_notification(
                    "notify-shortcut-trigger-model-unavailable-title",
                    err.to_string(),
                    NotificationType::Error,
                    cx,
                );
                return;
            }
        };

        match trigger.shortcut.input_source {
            ShortcutInputSource::SelectionOrClipboard => {
                self.trigger_selection_or_clipboard_shortcut(trigger, cx);
            }
            ShortcutInputSource::Screenshot => {
                if let Err(err) = screenshot_overlay::open(trigger.shortcut.clone(), cx) {
                    self.handle_screenshot_capture_failure(err, cx);
                }
            }
        }
    }

    fn resolve_shortcut_trigger_context(
        &self,
        shortcut_id: ShortcutId,
        cx: &App,
    ) -> JacoResult<Option<ShortcutTriggerContext>> {
        let Some(shortcut) = database::repository(cx).get_shortcut(&shortcut_id)? else {
            event!(
                Level::ERROR,
                shortcut_id = %shortcut_id,
                "shortcut hotkey was pressed but shortcut record is missing"
            );
            return Ok(None);
        };
        if !shortcut.enabled {
            event!(
                Level::INFO,
                shortcut_id = %shortcut.id,
                "ignoring disabled shortcut hotkey"
            );
            return Ok(None);
        }
        if !matches!(shortcut.action, ShortcutAction::OpenTemporaryConversation) {
            event!(
                Level::ERROR,
                shortcut_id = %shortcut.id,
                action = ?shortcut.action,
                "shortcut action is not supported by jaco runtime"
            );
            return Ok(None);
        }

        let prompt_snapshot = match shortcut.prompt_id.as_ref() {
            Some(prompt_id) => {
                let Some(prompt) = database::repository(cx).get_prompt(prompt_id)? else {
                    return Err(jaco_db::DbError::Invariant(format!(
                        "prompt {prompt_id} is missing"
                    ))
                    .into());
                };
                if !prompt.enabled {
                    return Err(jaco_db::DbError::Invariant(format!(
                        "prompt {prompt_id} is disabled"
                    ))
                    .into());
                }
                Some(prompt.content)
            }
            None => None,
        };

        let provider_id = shortcut.provider_id.as_ref().ok_or_else(|| {
            jaco_db::DbError::Invariant(format!("shortcut {} has no provider", shortcut.id))
        })?;
        let model_id = shortcut.model_id.as_ref().ok_or_else(|| {
            jaco_db::DbError::Invariant(format!("shortcut {} has no model", shortcut.id))
        })?;
        let provider_model = state::providers::enabled_provider_models(cx)?
            .into_iter()
            .find(|choice| &choice.provider_id == provider_id && &choice.model_id == model_id)
            .ok_or_else(|| {
                jaco_db::DbError::Invariant(format!(
                    "model {provider_id}/{model_id} is unavailable"
                ))
            })?;

        Ok(Some(ShortcutTriggerContext {
            prompt_id: shortcut.prompt_id.clone(),
            shortcut,
            provider_model,
            prompt_snapshot,
        }))
    }

    fn trigger_selection_or_clipboard_shortcut(
        &self,
        trigger: ShortcutTriggerContext,
        cx: &mut App,
    ) {
        cx.spawn(async move |cx| {
            event!(
                Level::INFO,
                shortcut_id = %trigger.shortcut.id,
                hotkey = %trigger.shortcut.hotkey,
                "triggering selection or clipboard shortcut"
            );
            let selected_text =
                smol::unblock(move || get_selected_text::get_selected_text().ok()).await;
            cx.update_global::<GlobalHotkeyState, _>(|hotkeys, cx| {
                match hotkeys.resolve_clipboard_fallback(selected_text, cx) {
                    Some(text) => {
                        let parts = vec![ContentPart::Text { text: text.clone() }];
                        hotkeys.trigger_shortcut_with_parts(trigger, text, parts, cx);
                    }
                    None => hotkeys.handle_empty_shortcut_input(cx),
                }
            });
        })
        .detach();
    }

    fn resolve_clipboard_fallback(
        &self,
        selected_text: Option<String>,
        cx: &App,
    ) -> Option<String> {
        let selected_text = normalized_text(selected_text);
        if selected_text.is_some() {
            event!(
                Level::INFO,
                source = "selected_text",
                "resolved shortcut input"
            );
            return selected_text;
        }

        let clipboard_text = cx
            .read_from_clipboard()
            .and_then(|item| item.text())
            .and_then(|text| normalized_text(Some(text.to_string())));
        if clipboard_text.is_some() {
            event!(Level::INFO, source = "clipboard", "resolved shortcut input");
        } else {
            event!(
                Level::INFO,
                "no selected text or clipboard text available for shortcut"
            );
        }
        clipboard_text
    }

    pub(crate) fn process_captured_screenshot(
        &mut self,
        shortcut: ShortcutRecord,
        image: ImageFrame,
        cx: &mut App,
    ) {
        let trigger = match self.resolve_shortcut_trigger_context(shortcut.id.clone(), cx) {
            Ok(Some(trigger)) => trigger,
            Ok(None) => return,
            Err(err) => {
                self.push_notification(
                    "notify-shortcut-trigger-model-unavailable-title",
                    err.to_string(),
                    NotificationType::Error,
                    cx,
                );
                return;
            }
        };

        if trigger
            .provider_model
            .capabilities
            .image_input
            .as_ref()
            .is_some()
        {
            if let Err(err) = self.trigger_shortcut_with_image(trigger, image, cx) {
                self.push_notification(
                    "notify-shortcut-trigger-screenshot-title",
                    err.to_string(),
                    NotificationType::Error,
                    cx,
                );
            }
            return;
        }

        self.trigger_screenshot_ocr_shortcut(trigger, image, cx);
    }

    fn trigger_screenshot_ocr_shortcut(
        &self,
        trigger: ShortcutTriggerContext,
        image: ImageFrame,
        cx: &mut App,
    ) {
        cx.spawn(async move |cx| {
            let recognized = smol::unblock(move || platform_ext::ocr::recognize_text(&image)).await;
            cx.update_global::<GlobalHotkeyState, _>(|hotkeys, cx| match recognized {
                Ok(text) => match normalized_text(Some(text)) {
                    Some(text) => {
                        let parts = vec![ContentPart::Text { text: text.clone() }];
                        hotkeys.trigger_shortcut_with_parts(trigger, text, parts, cx);
                    }
                    None => hotkeys.handle_empty_shortcut_input(cx),
                },
                Err(err) => hotkeys.handle_screenshot_ocr_failure(err, cx),
            });
        })
        .detach();
    }

    fn trigger_shortcut_with_image(
        &mut self,
        trigger: ShortcutTriggerContext,
        image: ImageFrame,
        cx: &mut App,
    ) -> JacoResult<()> {
        let png = screenshot_png_bytes(&image)
            .map_err(|err| JacoError::Window(format!("encode screenshot failed: {err}")))?;
        let attachment = screenshot_composer_attachment(&image, &png)?;
        let created = self.create_shortcut_conversation(
            &trigger,
            Vec::new(),
            vec![attachment],
            String::new(),
            cx,
        )?;
        self.finish_shortcut_trigger(created, cx);
        Ok(())
    }

    fn trigger_shortcut_with_parts(
        &mut self,
        trigger: ShortcutTriggerContext,
        title_seed: String,
        content_parts: Vec<ContentPart>,
        cx: &mut App,
    ) {
        let result =
            self.create_shortcut_conversation(&trigger, content_parts, Vec::new(), title_seed, cx);
        match result {
            Ok(created) => self.finish_shortcut_trigger(created, cx),
            Err(err) => self.push_notification(
                "notify-shortcut-trigger-model-unavailable-title",
                err.to_string(),
                NotificationType::Error,
                cx,
            ),
        }
    }

    fn create_shortcut_conversation(
        &self,
        trigger: &ShortcutTriggerContext,
        content_parts: Vec<ContentPart>,
        attachments: Vec<ComposerAttachment>,
        title_seed: String,
        cx: &mut App,
    ) -> JacoResult<state::conversations::CreatedConversation> {
        if let Some(selection) = trigger
            .shortcut
            .settings_snapshot
            .reasoning_selection
            .as_ref()
            && !reasoning_selection_is_valid(
                trigger.provider_model.capabilities.reasoning.as_ref(),
                selection,
            )
        {
            return Err(JacoError::Window(
                "shortcut reasoning setting is not supported by the selected model".to_string(),
            ));
        }
        state::conversations::create_conversation(
            state::conversations::CreateConversationRequest {
                project_id: None,
                content_parts,
                attachments,
                title_seed,
                skill_requests: Vec::new(),
                provider_model: trigger.provider_model.clone(),
                reasoning_selection: trigger
                    .shortcut
                    .settings_snapshot
                    .reasoning_selection
                    .clone(),
                approval_mode: trigger.shortcut.settings_snapshot.tool_policy.approval_mode,
                prompt_id: trigger.prompt_id.clone(),
                prompt_snapshot: trigger.prompt_snapshot.clone(),
                trigger_kind: AgentRunTriggerKind::Shortcut,
            },
            cx,
        )
    }

    fn finish_shortcut_trigger(
        &self,
        created: state::conversations::CreatedConversation,
        cx: &mut App,
    ) {
        // Selection/OCR/screenshot completion normally arrives from inside a
        // `GlobalHotkeyState` update. The temporary-window lifecycle performs
        // nested global/window/entity updates, so dispatch it after the
        // current update scope has released its borrows.
        cx.defer(move |cx| {
            if temporary_window::show_created_conversation(created, cx).is_none() {
                event!(
                    Level::ERROR,
                    "failed to show shortcut conversation in temporary window"
                );
            }
        });
    }

    pub(crate) fn handle_screenshot_capture_failure(&self, error: CaptureError, cx: &mut App) {
        let Some(message) = screenshot_capture_error_message(&error) else {
            return;
        };
        self.push_notification(
            "notify-shortcut-trigger-screenshot-title",
            message,
            NotificationType::Error,
            cx,
        );
    }

    fn handle_screenshot_ocr_failure(&self, error: OcrError, cx: &mut App) {
        self.push_notification(
            "notify-shortcut-trigger-ocr-title",
            screenshot_ocr_error_message(&error),
            NotificationType::Error,
            cx,
        );
    }

    fn handle_empty_shortcut_input(&self, cx: &mut App) {
        self.push_notification(
            "notify-shortcut-trigger-empty-input-title",
            cx.global::<I18n>()
                .t("notify-shortcut-trigger-empty-input-message")
                .to_string(),
            NotificationType::Warning,
            cx,
        );
    }

    fn push_notification(
        &self,
        title_key: &'static str,
        message: impl Into<SharedString>,
        kind: NotificationType,
        cx: &mut App,
    ) {
        let title = cx.global::<I18n>().t(title_key);
        let notification = Notification::new()
            .title(title)
            .message(message.into())
            .with_type(kind);

        let window = cx
            .active_window()
            .and_then(|window| window.downcast::<Root>())
            .or_else(|| {
                cx.windows()
                    .iter()
                    .find_map(|window| window.downcast::<Root>())
            });
        let Some(window) = window else {
            event!(
                Level::ERROR,
                title_key,
                "no Root window available for shortcut notification"
            );
            return;
        };

        let window: AnyWindowHandle = window.into();
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| {
                window.push_notification(notification, cx);
            });
        });
    }

    pub(crate) fn diagnostics(&self) -> ShortcutRuntimeDiagnostics {
        ShortcutRuntimeDiagnostics {
            temporary_hotkey: self.temporary_hotkey.clone(),
            registered_shortcuts: self.registered_shortcuts.clone(),
            registration_errors: self.registration_errors.clone(),
            last_pressed: self.last_pressed.clone(),
        }
    }

    pub(crate) fn diagnostics_snapshot(cx: &App) -> ShortcutRuntimeDiagnostics {
        cx.try_global::<GlobalHotkeyState>()
            .map(GlobalHotkeyState::diagnostics)
            .unwrap_or_default()
    }
}

fn normalized_text(text: Option<String>) -> Option<String> {
    text.map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn screenshot_png_bytes(image: &ImageFrame) -> Result<Vec<u8>, String> {
    use image::ImageEncoder as _;

    let raw = image::RgbaImage::from_raw(image.width, image.height, image.bytes_rgba8.clone())
        .ok_or_else(|| "invalid screenshot image buffer".to_string())?;
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(
            raw.as_raw(),
            image.width,
            image.height,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|err| err.to_string())?;
    Ok(png)
}

fn screenshot_composer_attachment(
    image: &ImageFrame,
    png: &[u8],
) -> JacoResult<ComposerAttachment> {
    Ok(generated_image_attachment(
        "screenshot.png".to_string(),
        Image::from_bytes(ImageFormat::Png, png.to_vec()),
        "image/png".to_string(),
        (image.width, image.height),
        0,
    ))
}

fn screenshot_capture_error_message(error: &CaptureError) -> Option<String> {
    match error {
        CaptureError::Cancelled => None,
        _ => Some(error.to_string()),
    }
}

fn screenshot_ocr_error_message(error: &OcrError) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        FakeHotkeyBackend, GlobalHotkeyState, normalized_text, screenshot_capture_error_message,
        screenshot_composer_attachment, screenshot_ocr_error_message, screenshot_png_bytes,
    };
    use crate::{
        platform::capture::CaptureError,
        state::attachments::{ComposerAttachmentKind, ComposerAttachmentSource},
    };
    use global_hotkey::hotkey::HotKey;
    use gpui::Task;
    use platform_ext::{OcrError, ocr::ImageFrame};
    use std::str::FromStr;

    #[test]
    fn temporary_hotkey_registration_records_diagnostics() {
        let mut hotkeys =
            GlobalHotkeyState::new(Box::<FakeHotkeyBackend>::default(), Task::ready(()));
        hotkeys
            .register_temporary_hotkey("cmd+shift+j".to_string())
            .expect("register temporary hotkey");

        let hotkey = HotKey::from_str("cmd+shift+j").unwrap();
        hotkeys.handle_pressed_hotkey(hotkey.id());
        let diagnostics = hotkeys.diagnostics();

        assert_eq!(diagnostics.temporary_hotkey.as_deref(), Some("cmd+shift+j"));
        assert!(diagnostics.registration_errors.is_empty());
        assert_eq!(
            diagnostics
                .last_pressed
                .as_ref()
                .map(|press| press.action.as_str()),
            Some("temporary")
        );
    }

    #[test]
    fn invalid_temporary_hotkey_is_reported_without_panicking() {
        let mut hotkeys =
            GlobalHotkeyState::new(Box::<FakeHotkeyBackend>::default(), Task::ready(()));
        let result = hotkeys.register_temporary_hotkey("cmd+shift+".to_string());

        assert!(result.is_err());
        assert_eq!(hotkeys.diagnostics().temporary_hotkey, None);
        assert!(
            hotkeys
                .diagnostics()
                .registration_errors
                .contains_key("temporary")
        );
    }

    #[test]
    fn invalid_temporary_hotkey_update_preserves_previous_runtime_hotkey() {
        let mut hotkeys =
            GlobalHotkeyState::new(Box::<FakeHotkeyBackend>::default(), Task::ready(()));
        hotkeys
            .register_temporary_hotkey("cmd+shift+j".to_string())
            .expect("register temporary hotkey");

        let result =
            hotkeys.update_temporary_hotkey_runtime(Some("cmd+shift+j"), Some("cmd+shift+"));

        assert!(result.is_err());
        let diagnostics = hotkeys.diagnostics();
        assert_eq!(diagnostics.temporary_hotkey.as_deref(), Some("cmd+shift+j"));
        assert!(diagnostics.registration_errors.contains_key("temporary"));
    }

    #[test]
    fn temporary_hotkey_runtime_update_can_clear_registration() {
        let mut hotkeys =
            GlobalHotkeyState::new(Box::<FakeHotkeyBackend>::default(), Task::ready(()));
        hotkeys
            .register_temporary_hotkey("cmd+shift+j".to_string())
            .expect("register temporary hotkey");

        hotkeys
            .update_temporary_hotkey_runtime(Some("cmd+shift+j"), None)
            .expect("clear temporary hotkey");

        assert_eq!(hotkeys.diagnostics().temporary_hotkey, None);
        assert!(hotkeys.diagnostics().registration_errors.is_empty());
    }

    #[test]
    fn normalized_text_rejects_empty_and_whitespace_only_values() {
        assert_eq!(normalized_text(None), None);
        assert_eq!(normalized_text(Some(String::new())), None);
        assert_eq!(normalized_text(Some("   \n\t  ".to_string())), None);
        assert_eq!(
            normalized_text(Some("  selected text  ".to_string())),
            Some("selected text".to_string())
        );
    }

    #[test]
    fn screenshot_capture_cancellation_is_silent() {
        assert_eq!(
            screenshot_capture_error_message(&CaptureError::Cancelled),
            None
        );
    }

    #[test]
    fn screenshot_capture_failures_map_to_error_messages() {
        assert_eq!(
            screenshot_capture_error_message(&CaptureError::PermissionDenied),
            Some("capture permission was denied".to_string())
        );
        assert_eq!(
            screenshot_capture_error_message(&CaptureError::BackendUnavailable("missing backend")),
            Some("capture backend is unavailable: missing backend".to_string())
        );
    }

    #[test]
    fn screenshot_ocr_failures_map_to_error_messages() {
        assert_eq!(
            screenshot_ocr_error_message(&OcrError::BackendUnavailable("missing ocr")),
            "ocr backend is unavailable: missing ocr".to_string()
        );
        assert_eq!(
            screenshot_ocr_error_message(&OcrError::SystemFailure("vision failed".to_string())),
            "ocr failed: vision failed".to_string()
        );
    }

    #[test]
    fn screenshot_png_bytes_encodes_rgba_frame() {
        let image = ImageFrame {
            width: 1,
            height: 1,
            scale_factor: 1.0,
            bytes_rgba8: vec![255, 0, 0, 255],
        };

        let png = screenshot_png_bytes(&image).unwrap();

        assert!(png.starts_with(&[137, 80, 78, 71]));
    }

    #[gpui::test]
    fn screenshot_composer_attachment_uses_memory_image() {
        let image = ImageFrame {
            width: 1,
            height: 1,
            scale_factor: 1.0,
            bytes_rgba8: vec![255, 0, 0, 255],
        };
        let png = screenshot_png_bytes(&image).unwrap();

        let attachment = screenshot_composer_attachment(&image, &png).unwrap();

        assert_eq!(attachment.local_id, 0);
        assert_eq!(attachment.kind, ComposerAttachmentKind::Image);
        assert_eq!(attachment.name, "screenshot.png");
        assert_eq!(attachment.mime_type.as_deref(), Some("image/png"));
        assert_eq!(attachment.size_bytes, Some(png.len() as u64));
        assert_eq!(attachment.width, Some(1));
        assert_eq!(attachment.height, Some(1));
        let ComposerAttachmentSource::GeneratedImage { image } = attachment.source else {
            panic!("screenshot attachment should keep image bytes in memory");
        };
        assert_eq!(image.bytes(), png);
    }
}

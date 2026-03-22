use crate::{
    capture::CaptureError,
    database::{Db, GlobalShortcutBinding, NewGlobalShortcutBinding, UpdateGlobalShortcutBinding},
    errors::AiChatResult,
    i18n::I18n,
    state::{AiChatConfig, ConversationDraft, ModelStore},
    views::{screenshot_overlay, temporary::TemporaryView},
};
use get_selected_text::get_selected_text;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};
use gpui::*;
use gpui_component::{
    Root,
    notification::{Notification, NotificationType},
    WindowExt as NotificationWindowExt,
};
#[cfg(target_os = "macos")]
pub use platform_ext::app::{NSRunningApplication, Retained};
#[cfg(target_os = "macos")]
use platform_ext::app::{record_frontmost_app, restore_frontmost_app};
use platform_ext::{OcrError, ocr::ImageFrame};
use std::{any::TypeId, collections::BTreeMap, str::FromStr, time::Duration};
use tracing::{Level, event};
use window_ext::WindowExt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RegisteredHotkeyAction {
    TemporaryWindow,
    ShortcutBinding { binding_id: i32 },
}

trait HotkeyBackend {
    fn register(&mut self, hotkey: HotKey) -> AiChatResult<()>;
    fn unregister(&mut self, hotkey: HotKey) -> AiChatResult<()>;
}

struct SystemHotkeyBackend {
    manager: GlobalHotKeyManager,
}

impl HotkeyBackend for SystemHotkeyBackend {
    fn register(&mut self, hotkey: HotKey) -> AiChatResult<()> {
        self.manager.register(hotkey)?;
        Ok(())
    }

    fn unregister(&mut self, hotkey: HotKey) -> AiChatResult<()> {
        self.manager.unregister(hotkey)?;
        Ok(())
    }
}

pub struct GlobalHotkeyState {
    backend: Box<dyn HotkeyBackend>,
    temporary_hotkey: Option<String>,
    shortcut_bindings: BTreeMap<i32, GlobalShortcutBinding>,
    hotkey_actions: BTreeMap<u32, RegisteredHotkeyAction>,
    _task: Task<()>,
    #[cfg(target_os = "macos")]
    front_app: Option<Retained<NSRunningApplication>>,
    pub delay_close: Option<Task<()>>,
}

impl GlobalHotkeyState {
    pub fn delay_close(window: &mut Window, cx: &mut App) -> Task<()> {
        window.spawn(cx, async |cx| {
            Timer::after(Duration::from_secs(600)).await;
            if let Err(err) = cx.window_handle().update(cx, |_, window, _cx| {
                window.remove_window();
            }) {
                event!(Level::ERROR, "Failed to remove temporary window: {:?}", err);
            };
        })
    }

    fn find_temporary_window(cx: &App) -> Option<WindowHandle<Root>> {
        cx.windows().iter().find_map(|window| {
            window.downcast::<Root>().filter(|root| {
                root.read(cx)
                    .ok()
                    .map(|root| root.view().entity_type() == TypeId::of::<TemporaryView>())
                    .unwrap_or(false)
            })
        })
    }

    fn delay_or_hide_temporary_window(&mut self, window: &mut Window, cx: &mut App) {
        let task = Self::delay_close(window, cx);
        self.delay_close = Some(task);
        self.hide_temporary_window(window);
    }

    pub fn request_hide_with_delay(window: &mut Window, cx: &mut App) {
        if !cx.has_global::<GlobalHotkeyState>() {
            event!(
                Level::ERROR,
                "Failed to hide temporary window with delay: GlobalHotkeyState is not initialized"
            );
            return;
        }

        cx.update_global::<GlobalHotkeyState, _>(|hotkeys, cx| {
            hotkeys.delay_or_hide_temporary_window(window, cx);
        });
    }

    fn hide_temporary_window(&mut self, window: &mut Window) {
        if let Err(err) = window.hide() {
            event!(Level::ERROR, "Failed to hide temporary window: {:?}", err);
        };
        #[cfg(target_os = "macos")]
        {
            restore_frontmost_app(&self.front_app);
            self.front_app = None;
        }
    }

    fn show_temporary_window(&mut self, window: &mut Window) {
        self.delay_close = None;
        #[cfg(target_os = "macos")]
        {
            let prev_app = record_frontmost_app();
            self.front_app = prev_app;
        }
        if let Err(err) = window.show() {
            window.activate_window();
            event!(Level::ERROR, "Failed to show temporary window: {:?}", err);
        };
        window.activate_window();
    }

    fn create_temporary_window(&mut self, cx: &mut App) -> Option<WindowHandle<Root>> {
        #[cfg(target_os = "macos")]
        {
            let front_app = record_frontmost_app();
            self.front_app = front_app;
        }
        match cx.open_window(
            WindowOptions {
                kind: WindowKind::Floating,
                titlebar: Some(TitlebarOptions {
                    title: None,
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(-100.), px(-100.))),
                }),
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    cx.primary_display().map(|display| display.id()),
                    size(px(800.), px(600.)),
                    cx,
                ))),
                is_resizable: false,
                ..Default::default()
            },
            |window, cx| {
                window.activate_window();
                if let Err(err) = window.set_floating() {
                    event!(Level::ERROR, error = ?err, "Failed to set floating");
                }
                let view = cx.new(|cx| TemporaryView::new(window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            },
        ) {
            Ok(handle) => Some(handle),
            Err(err) => {
                event!(Level::ERROR, error = ?err, "Failed to open temporary window");
                None
            }
        }
    }

    fn ensure_temporary_window_visible(&mut self, cx: &mut App) -> Option<WindowHandle<Root>> {
        let window = Self::find_temporary_window(cx).or_else(|| self.create_temporary_window(cx))?;
        let _ = window.update(cx, |_, window, _cx| {
            self.show_temporary_window(window);
        });
        Some(window)
    }

    fn parse_hotkey(hotkey: &str) -> AiChatResult<HotKey> {
        Ok(HotKey::from_str(hotkey)?)
    }

    fn register_action(
        &mut self,
        hotkey: &str,
        action: RegisteredHotkeyAction,
    ) -> AiChatResult<()> {
        let hotkey = Self::parse_hotkey(hotkey)?;
        if self.hotkey_actions.get(&hotkey.id()).copied() == Some(action) {
            event!(
                Level::INFO,
                hotkey = %hotkey,
                hotkey_id = hotkey.id(),
                action = ?action,
                "Hotkey action already registered"
            );
            return Ok(());
        }
        event!(
            Level::INFO,
            hotkey = %hotkey,
            hotkey_id = hotkey.id(),
            action = ?action,
            "Registering hotkey action"
        );
        self.backend.register(hotkey)?;
        self.hotkey_actions.insert(hotkey.id(), action);
        event!(
            Level::INFO,
            hotkey = %hotkey,
            hotkey_id = hotkey.id(),
            action = ?action,
            "Registered hotkey action"
        );
        Ok(())
    }

    fn unregister_action(
        &mut self,
        hotkey: &str,
        expected_action: RegisteredHotkeyAction,
    ) -> AiChatResult<()> {
        let hotkey = Self::parse_hotkey(hotkey)?;
        if self.hotkey_actions.get(&hotkey.id()).copied() != Some(expected_action) {
            event!(
                Level::INFO,
                hotkey = %hotkey,
                hotkey_id = hotkey.id(),
                expected_action = ?expected_action,
                "Skipping hotkey unregistration because registered action does not match"
            );
            return Ok(());
        }
        event!(
            Level::INFO,
            hotkey = %hotkey,
            hotkey_id = hotkey.id(),
            action = ?expected_action,
            "Unregistering hotkey action"
        );
        self.backend.unregister(hotkey)?;
        self.hotkey_actions.remove(&hotkey.id());
        event!(
            Level::INFO,
            hotkey = %hotkey,
            hotkey_id = hotkey.id(),
            action = ?expected_action,
            "Unregistered hotkey action"
        );
        Ok(())
    }

    fn upsert_binding_runtime(&mut self, binding: &GlobalShortcutBinding) -> AiChatResult<()> {
        event!(
            Level::INFO,
            binding_id = binding.id,
            hotkey = %binding.hotkey,
            enabled = binding.enabled,
            input_source = %binding.input_source,
            "Upserting global shortcut runtime binding"
        );
        let action = RegisteredHotkeyAction::ShortcutBinding {
            binding_id: binding.id,
        };
        let previous = self.shortcut_bindings.get(&binding.id).cloned();
        let should_unregister_previous = previous.as_ref().is_some_and(|old_binding| {
            old_binding.enabled && (old_binding.hotkey != binding.hotkey || !binding.enabled)
        });

        if let Some(old_binding) = previous.as_ref()
            && should_unregister_previous
        {
            self.unregister_action(&old_binding.hotkey, action)?;
        }

        if binding.enabled
            && let Err(err) = self.register_action(&binding.hotkey, action)
        {
            if let Some(old_binding) = previous.as_ref()
                && should_unregister_previous
                && old_binding.enabled
            {
                let _ = self.register_action(&old_binding.hotkey, action);
            }
            return Err(err);
        }

        self.shortcut_bindings.insert(binding.id, binding.clone());
        event!(
            Level::INFO,
            binding_id = binding.id,
            hotkey = %binding.hotkey,
            enabled = binding.enabled,
            "Upserted global shortcut runtime binding"
        );
        Ok(())
    }

    fn remove_binding_runtime(&mut self, binding: &GlobalShortcutBinding) -> AiChatResult<()> {
        event!(
            Level::INFO,
            binding_id = binding.id,
            hotkey = %binding.hotkey,
            enabled = binding.enabled,
            "Removing global shortcut runtime binding"
        );
        self.shortcut_bindings.remove(&binding.id);
        if binding.enabled {
            self.unregister_action(
                &binding.hotkey,
                RegisteredHotkeyAction::ShortcutBinding {
                    binding_id: binding.id,
                },
            )?;
        }
        event!(
            Level::INFO,
            binding_id = binding.id,
            hotkey = %binding.hotkey,
            "Removed global shortcut runtime binding"
        );
        Ok(())
    }

    fn load_initial_shortcuts(&mut self, cx: &mut App) -> AiChatResult<()> {
        self.temporary_hotkey = cx.global::<AiChatConfig>().temporary_hotkey.clone();
        if let Some(hotkey) = self.temporary_hotkey.clone() {
            event!(Level::INFO, hotkey = %hotkey, "Loading temporary hotkey");
            self.register_action(&hotkey, RegisteredHotkeyAction::TemporaryWindow)?;
        }

        let mut conn = cx.global::<Db>().get()?;
        let bindings = GlobalShortcutBinding::all(&mut conn)?;
        event!(Level::INFO, count = bindings.len(), "Loading initial global shortcut bindings");
        for binding in bindings {
            self.upsert_binding_runtime(&binding)?;
        }
        event!(
            Level::INFO,
            temporary_hotkey = ?self.temporary_hotkey,
            registered_actions = self.hotkey_actions.len(),
            shortcut_bindings = self.shortcut_bindings.len(),
            "Loaded initial hotkeys"
        );
        Ok(())
    }

    fn toggle_temporary_window(&mut self, cx: &mut App) {
        match Self::find_temporary_window(cx) {
            Some(temporary_window) => {
                if let Err(err) = temporary_window.update(cx, |_this, window, cx| {
                    if window.is_visible().unwrap_or(false) {
                        self.delay_or_hide_temporary_window(window, cx);
                    } else {
                        self.show_temporary_window(window);
                    }
                }) {
                    event!(Level::ERROR, "Failed to update temporary window: {:?}", err);
                };
            }
            None => {
                let _ = self.ensure_temporary_window_visible(cx);
            }
        }
    }

    fn action_for_id(&self, hotkey_id: u32) -> Option<RegisteredHotkeyAction> {
        self.hotkey_actions.get(&hotkey_id).copied()
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
            .or_else(|| cx.windows().iter().find_map(|window| window.downcast::<Root>()));
        let Some(window) = window else {
            event!(
                Level::ERROR,
                title_key,
                "No Root window available for notification"
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

    fn handle_busy_shortcut(&self, cx: &mut App) {
        self.push_notification(
            "notify-shortcut-trigger-busy-title",
            cx.global::<I18n>()
                .t("notify-shortcut-trigger-busy-message"),
            NotificationType::Error,
            cx,
        );
    }

    pub(crate) fn handle_screenshot_capture_failure(&self, err: CaptureError, cx: &mut App) {
        let Some(message) = screenshot_capture_error_message(&err) else {
            event!(Level::INFO, error = ?err, "Screenshot capture cancelled");
            return;
        };
        event!(Level::ERROR, error = ?err, "Screenshot capture failed");
        self.push_notification(
            "notify-shortcut-trigger-screenshot-title",
            message,
            NotificationType::Error,
            cx,
        );
    }

    pub(crate) fn handle_screenshot_ocr_failure(&self, err: OcrError, cx: &mut App) {
        let message = screenshot_ocr_error_message(&err);
        event!(Level::ERROR, error = ?err, "Screenshot OCR failed");
        self.push_notification(
            "notify-shortcut-trigger-ocr-title",
            message,
            NotificationType::Error,
            cx,
        );
    }

    pub(crate) fn handle_empty_shortcut_input(&self, cx: &mut App) {
        self.push_notification(
            "notify-shortcut-trigger-empty-input-title",
            cx.global::<I18n>()
                .t("notify-shortcut-trigger-empty-input-message"),
            NotificationType::Error,
            cx,
        );
    }

    fn handle_unavailable_model(&self, binding: &GlobalShortcutBinding, cx: &mut App) {
        let message = format!(
            "{} / {}",
            binding.provider_name, binding.model_id
        );
        event!(
            Level::ERROR,
            binding_id = binding.id,
            provider_name = %binding.provider_name,
            model_id = %binding.model_id,
            "Shortcut binding model is unavailable"
        );
        self.push_notification(
            "notify-shortcut-trigger-model-unavailable-title",
            message,
            NotificationType::Error,
            cx,
        );
    }

    fn resolve_clipboard_fallback(&self, selected_text: Option<String>, cx: &App) -> Option<String> {
        let selected_text = normalized_text(selected_text);
        if selected_text.is_some() {
            event!(Level::INFO, source = "selected_text", "Resolved shortcut input");
            return selected_text;
        }

        let clipboard_text = cx
            .read_from_clipboard()
            .and_then(|item| item.text())
            .and_then(|text| normalized_text(Some(text.to_string())));
        if clipboard_text.is_some() {
            event!(Level::INFO, source = "clipboard", "Resolved shortcut input");
        } else {
            event!(Level::INFO, "No selected text or clipboard text available for shortcut");
        }
        clipboard_text
    }

    pub(crate) fn trigger_shortcut_with_input(
        &mut self,
        binding: GlobalShortcutBinding,
        text: String,
        cx: &mut App,
    ) {
        let models = cx.global::<ModelStore>().read(cx).snapshot().models;
        let model_available = models.iter().any(|model| {
            model.provider_name == binding.provider_name && model.id == binding.model_id
        });
        if !model_available {
            self.handle_unavailable_model(&binding, cx);
            return;
        }

        let Some(window) = self.ensure_temporary_window_visible(cx) else {
            return;
        };
        let draft = ConversationDraft {
            text,
            provider_name: binding.provider_name.clone(),
            model_id: binding.model_id.clone(),
            mode: binding.mode,
            selected_template_id: binding.template_id,
            request_template: binding.request_template.clone(),
        };

        let binding_for_notification = binding.clone();
        let _ = window.update(cx, move |root, window, cx| {
            let Ok(view) = root.view().clone().downcast::<TemporaryView>() else {
                return;
            };
            view.update(cx, |view, cx| {
                view.detail.update(cx, |detail, cx| {
                    detail.restore_chat_form_draft(draft.clone(), window, cx);
                    let ready = detail
                        .chat_form
                        .read(cx)
                        .snapshot(cx)
                        .ok()
                        .flatten()
                        .is_some();
                    if ready {
                        detail.send_chat_form(window, cx);
                    } else {
                        self.handle_unavailable_model(&binding_for_notification, cx);
                    }
                });
            });
        });
    }

    fn trigger_selection_or_clipboard_shortcut(&self, binding: GlobalShortcutBinding, cx: &mut App) {
        cx.spawn(async move |cx| {
            event!(
                Level::INFO,
                binding_id = binding.id,
                hotkey = %binding.hotkey,
                "Triggering selection or clipboard shortcut"
            );
            let selected_text = smol::unblock(move || get_selected_text().ok()).await;
            let _ = cx.update_global::<GlobalHotkeyState, _>(|hotkeys, cx| {
                match hotkeys.resolve_clipboard_fallback(selected_text, cx) {
                    Some(text) => hotkeys.trigger_shortcut_with_input(binding, text, cx),
                    None => hotkeys.handle_empty_shortcut_input(cx),
                }
            });
        })
        .detach();
    }

    fn trigger_screenshot_shortcut(&self, binding: GlobalShortcutBinding, cx: &mut App) {
        event!(
            Level::INFO,
            binding_id = binding.id,
            hotkey = %binding.hotkey,
            "Triggering screenshot shortcut"
        );
        if let Err(err) = screenshot_overlay::open(binding, cx) {
            self.handle_screenshot_capture_failure(err, cx);
        }
    }

    fn dispatch_shortcut_trigger(&mut self, binding_id: i32, cx: &mut App) {
        if let Some(window) = Self::find_temporary_window(cx) {
            let mut was_visible = false;
            let _ = window.update(cx, |_root, window, cx| {
                was_visible = window.is_visible().unwrap_or(false);
                if was_visible {
                    self.delay_or_hide_temporary_window(window, cx);
                }
            });
            if was_visible {
                return;
            }
        }

        let Some(binding) = self.shortcut_bindings.get(&binding_id).cloned() else {
            event!(Level::ERROR, binding_id, "Shortcut binding not found");
            return;
        };
        if !binding.enabled {
            event!(Level::INFO, binding_id = binding.id, "Shortcut binding is disabled");
            return;
        }

        let temporary_is_running = Self::find_temporary_window(cx)
            .and_then(|root| root.read(cx).ok())
            .and_then(|root| root.view().clone().downcast::<TemporaryView>().ok())
            .is_some_and(|view| view.read(cx).detail.read(cx).has_running_task());
        if screenshot_overlay::is_active(cx) {
            event!(
                Level::INFO,
                binding_id = binding.id,
                "Shortcut ignored because screenshot overlay is already active"
            );
            return;
        }
        if temporary_is_running {
            event!(
                Level::INFO,
                binding_id = binding.id,
                "Shortcut ignored because temporary window task is running"
            );
            self.handle_busy_shortcut(cx);
            return;
        }

        event!(
            Level::INFO,
            binding_id = binding.id,
            hotkey = %binding.hotkey,
            input_source = %binding.input_source,
            provider_name = %binding.provider_name,
            model_id = %binding.model_id,
            "Dispatching shortcut trigger"
        );
        match binding.input_source {
            crate::database::ShortcutInputSource::Screenshot => {
                self.trigger_screenshot_shortcut(binding, cx);
            }
            crate::database::ShortcutInputSource::SelectionOrClipboard => {
                self.trigger_selection_or_clipboard_shortcut(binding, cx);
            }
        }
    }

    fn handle_pressed_hotkey(&mut self, hotkey_id: u32, cx: &mut App) {
        let Some(action) = self.action_for_id(hotkey_id) else {
            event!(Level::INFO, hotkey_id, "Ignoring hotkey press with no registered action");
            return;
        };
        event!(Level::INFO, hotkey_id, action = ?action, "Handling pressed hotkey");
        match action {
            RegisteredHotkeyAction::TemporaryWindow => self.toggle_temporary_window(cx),
            RegisteredHotkeyAction::ShortcutBinding { binding_id } => {
                self.dispatch_shortcut_trigger(binding_id, cx)
            }
        }
    }

    pub fn update_temporary_hotkey(
        old_hotkey: Option<&str>,
        new_hotkey: Option<&str>,
        cx: &mut App,
    ) -> AiChatResult<()> {
        event!(
            Level::INFO,
            old_hotkey = ?old_hotkey,
            new_hotkey = ?new_hotkey,
            "Updating temporary hotkey"
        );
        let hotkeys = cx.global_mut::<GlobalHotkeyState>();
        if let Some(old_hotkey) = old_hotkey {
            hotkeys.unregister_action(old_hotkey, RegisteredHotkeyAction::TemporaryWindow)?;
        }
        if let Some(new_hotkey) = new_hotkey
            && let Err(err) =
                hotkeys.register_action(new_hotkey, RegisteredHotkeyAction::TemporaryWindow)
        {
            if let Some(old_hotkey) = old_hotkey {
                let _ =
                    hotkeys.register_action(old_hotkey, RegisteredHotkeyAction::TemporaryWindow);
            }
            return Err(err);
        }
        hotkeys.temporary_hotkey = new_hotkey.map(str::to_string);
        event!(
            Level::INFO,
            temporary_hotkey = ?hotkeys.temporary_hotkey,
            "Updated temporary hotkey"
        );
        Ok(())
    }

    pub fn save_global_shortcut_binding(
        id: Option<i32>,
        binding: NewGlobalShortcutBinding,
        cx: &mut App,
    ) -> AiChatResult<GlobalShortcutBinding> {
        event!(
            Level::INFO,
            binding_id = ?id,
            hotkey = %binding.hotkey,
            enabled = binding.enabled,
            provider_name = %binding.provider_name,
            model_id = %binding.model_id,
            input_source = %binding.input_source,
            "Saving global shortcut binding"
        );
        let mut conn = cx.global::<Db>().get()?;
        match id {
            Some(id) => {
                let previous = GlobalShortcutBinding::find(id, &mut conn)?;
                GlobalShortcutBinding::update(
                    id,
                    UpdateGlobalShortcutBinding {
                        hotkey: binding.hotkey.clone(),
                        enabled: binding.enabled,
                        template_id: binding.template_id,
                        provider_name: binding.provider_name.clone(),
                        model_id: binding.model_id.clone(),
                        mode: binding.mode,
                        request_template: binding.request_template.clone(),
                        input_source: binding.input_source,
                    },
                    &mut conn,
                )?;
                let updated = GlobalShortcutBinding::find(id, &mut conn)?;
                let mut runtime_result = Ok(());
                cx.update_global::<GlobalHotkeyState, _>(|hotkeys, _cx| {
                    runtime_result = hotkeys.upsert_binding_runtime(&updated);
                });
                if let Err(err) = runtime_result {
                    let _ = GlobalShortcutBinding::update(
                        id,
                        UpdateGlobalShortcutBinding {
                            hotkey: previous.hotkey.clone(),
                            enabled: previous.enabled,
                            template_id: previous.template_id,
                            provider_name: previous.provider_name.clone(),
                            model_id: previous.model_id.clone(),
                            mode: previous.mode,
                            request_template: previous.request_template.clone(),
                            input_source: previous.input_source,
                        },
                        &mut conn,
                    );
                    let _ = cx.update_global::<GlobalHotkeyState, _>(|hotkeys, _cx| {
                        hotkeys.upsert_binding_runtime(&previous)
                    });
                    return Err(err);
                }
                event!(
                    Level::INFO,
                    binding_id = updated.id,
                    hotkey = %updated.hotkey,
                    enabled = updated.enabled,
                    "Updated global shortcut binding"
                );
                Ok(updated)
            }
            None => {
                let created = GlobalShortcutBinding::insert(binding, &mut conn)?;
                let mut runtime_result = Ok(());
                cx.update_global::<GlobalHotkeyState, _>(|hotkeys, _cx| {
                    runtime_result = hotkeys.upsert_binding_runtime(&created);
                });
                if let Err(err) = runtime_result {
                    let _ = GlobalShortcutBinding::delete(created.id, &mut conn);
                    return Err(err);
                }
                event!(
                    Level::INFO,
                    binding_id = created.id,
                    hotkey = %created.hotkey,
                    enabled = created.enabled,
                    "Created global shortcut binding"
                );
                Ok(created)
            }
        }
    }

    pub fn delete_global_shortcut_binding(id: i32, cx: &mut App) -> AiChatResult<()> {
        event!(Level::INFO, binding_id = id, "Deleting global shortcut binding");
        let mut conn = cx.global::<Db>().get()?;
        let previous = GlobalShortcutBinding::find(id, &mut conn)?;
        let mut runtime_result = Ok(());
        cx.update_global::<GlobalHotkeyState, _>(|hotkeys, _cx| {
            runtime_result = hotkeys.remove_binding_runtime(&previous);
        });
        runtime_result?;
        if let Err(err) = GlobalShortcutBinding::delete(id, &mut conn) {
            let _ = cx.update_global::<GlobalHotkeyState, _>(|hotkeys, _cx| {
                hotkeys.upsert_binding_runtime(&previous)
            });
            return Err(err);
        }
        event!(
            Level::INFO,
            binding_id = id,
            hotkey = %previous.hotkey,
            "Deleted global shortcut binding"
        );
        Ok(())
    }
}

impl Global for GlobalHotkeyState {}

impl GlobalHotkeyState {
    pub(crate) fn process_captured_screenshot(
        binding: GlobalShortcutBinding,
        image: ImageFrame,
        cx: &mut App,
    ) {
        cx.spawn(async move |cx| {
            let recognized = smol::unblock(move || platform_ext::ocr::recognize_text(&image)).await;
            let _ = cx.update_global::<GlobalHotkeyState, _>(|hotkeys, cx| {
                match recognized {
                    Ok(text) => {
                        event!(
                            Level::INFO,
                            binding_id = binding.id,
                            recognized_chars = text.chars().count(),
                            recognized_is_empty = text.trim().is_empty(),
                            "Screenshot OCR completed"
                        );
                        match normalized_text(Some(text)) {
                            Some(text) => {
                                event!(
                                    Level::INFO,
                                    binding_id = binding.id,
                                    input_chars = text.chars().count(),
                                    "Submitting OCR shortcut input"
                                );
                                hotkeys.trigger_shortcut_with_input(binding, text, cx)
                            }
                            None => hotkeys.handle_empty_shortcut_input(cx),
                        }
                    }
                    Err(err) => {
                        hotkeys.handle_screenshot_ocr_failure(err, cx);
                    }
                }
            });
        })
        .detach();
    }
}

pub fn init(cx: &mut App) {
    let span = tracing::info_span!("hotkey::init");
    let _enter = span.enter();
    event!(Level::INFO, "hotkey init");
    match inner_init(cx) {
        Ok(_) => {}
        Err(err) => {
            event!(Level::ERROR, error = ?err, "Failed to initialize hotkeys");
        }
    };
}

fn inner_init(cx: &mut App) -> AiChatResult<()> {
    let (tx, rx) = smol::channel::unbounded();
    GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
        if let GlobalHotKeyEvent {
            state: HotKeyState::Pressed,
            ..
        } = e
        {
            match tx.send_blocking(e) {
                Ok(_) => {}
                Err(err) => {
                    event!(Level::ERROR, "send hotkey event failed: {}", err);
                }
            };
        }
    }));
    let task = cx.spawn(async move |cx| {
        while let Ok(event) = rx.recv().await {
            event!(Level::INFO, "hotkey event received: {:?}", event);
            if let Err(err) = cx.update_global::<GlobalHotkeyState, _>(|hotkeys, cx| {
                hotkeys.handle_pressed_hotkey(event.id(), cx);
            }) {
                event!(Level::ERROR, "handle hotkey event failed: {}", err);
            }
        }
    });

    let mut hotkeys = GlobalHotkeyState {
        backend: Box::new(SystemHotkeyBackend {
            manager: GlobalHotKeyManager::new()?,
        }),
        temporary_hotkey: None,
        shortcut_bindings: BTreeMap::new(),
        hotkey_actions: BTreeMap::new(),
        _task: task,
        #[cfg(target_os = "macos")]
        front_app: None,
        delay_close: None,
    };
    hotkeys.load_initial_shortcuts(cx)?;
    cx.set_global(hotkeys);
    Ok(())
}

fn normalized_text(text: Option<String>) -> Option<String> {
    text.map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
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
        normalized_text, screenshot_capture_error_message, screenshot_ocr_error_message,
    };
    use crate::capture::CaptureError;
    use platform_ext::OcrError;

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
        assert_eq!(screenshot_capture_error_message(&CaptureError::Cancelled), None);
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
}

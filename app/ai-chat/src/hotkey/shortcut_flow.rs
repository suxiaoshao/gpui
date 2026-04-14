use super::*;

impl GlobalHotkeyState {
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

    pub(crate) fn handle_screenshot_capture_failure(&mut self, err: CaptureError, cx: &mut App) {
        #[cfg(target_os = "macos")]
        self.clear_front_app_for_screenshot();
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

    pub(crate) fn handle_screenshot_ocr_failure(&mut self, err: OcrError, cx: &mut App) {
        #[cfg(target_os = "macos")]
        self.clear_front_app_for_screenshot();
        let message = screenshot_ocr_error_message(&err);
        event!(Level::ERROR, error = ?err, "Screenshot OCR failed");
        self.push_notification(
            "notify-shortcut-trigger-ocr-title",
            message,
            NotificationType::Error,
            cx,
        );
    }

    pub(crate) fn handle_empty_shortcut_input(&mut self, cx: &mut App) {
        #[cfg(target_os = "macos")]
        self.clear_front_app_for_screenshot();
        self.push_notification(
            "notify-shortcut-trigger-empty-input-title",
            cx.global::<I18n>()
                .t("notify-shortcut-trigger-empty-input-message"),
            NotificationType::Error,
            cx,
        );
    }

    fn handle_unavailable_model(&self, binding: &GlobalShortcutBinding, cx: &mut App) {
        let message = format!("{} / {}", binding.provider_name, binding.model_id);
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
            event!(
                Level::INFO,
                source = "selected_text",
                "Resolved shortcut input"
            );
            return selected_text;
        }

        let clipboard_text = cx
            .read_from_clipboard()
            .and_then(|item| item.text())
            .and_then(|text| normalized_text(Some(text.to_string())));
        if clipboard_text.is_some() {
            event!(Level::INFO, source = "clipboard", "Resolved shortcut input");
        } else {
            event!(
                Level::INFO,
                "No selected text or clipboard text available for shortcut"
            );
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

    fn trigger_screenshot_shortcut(&mut self, binding: GlobalShortcutBinding, cx: &mut App) {
        #[cfg(target_os = "macos")]
        self.record_front_app_for_screenshot();
        if let Err(err) = screenshot_overlay::open(binding, cx) {
            self.handle_screenshot_capture_failure(err, cx);
        }
    }

    pub(super) fn dispatch_shortcut_trigger(&mut self, binding_id: i32, cx: &mut App) {
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
            event!(
                Level::INFO,
                binding_id = binding.id,
                "Shortcut binding is disabled"
            );
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

    pub(crate) fn process_captured_screenshot(
        binding: GlobalShortcutBinding,
        image: ImageFrame,
        cx: &mut App,
    ) {
        cx.spawn(async move |cx| {
            let recognized = smol::unblock(move || platform_ext::ocr::recognize_text(&image)).await;
            let _ = cx.update_global::<GlobalHotkeyState, _>(|hotkeys, cx| match recognized {
                Ok(text) => {
                    event!(
                        Level::INFO,
                        binding_id = binding.id,
                        recognized_chars = text.chars().count(),
                        recognized_is_empty = text.trim().is_empty(),
                        "Screenshot OCR completed"
                    );
                    match normalized_text(Some(text)) {
                        Some(text) => hotkeys.trigger_shortcut_with_input(binding, text, cx),
                        None => hotkeys.handle_empty_shortcut_input(cx),
                    }
                }
                Err(err) => {
                    hotkeys.handle_screenshot_ocr_failure(err, cx);
                }
            });
        })
        .detach();
    }
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
    use super::{normalized_text, screenshot_capture_error_message, screenshot_ocr_error_message};
    use crate::platform::capture::CaptureError;
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
}

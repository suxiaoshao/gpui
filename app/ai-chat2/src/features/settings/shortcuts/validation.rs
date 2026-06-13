use std::str::FromStr;

use crate::components::hotkey_input::string_to_keystroke;
use ai_chat_core::ShortcutId;
use ai_chat_db::ShortcutRecord;
use global_hotkey::hotkey::HotKey;
use gpui::Keystroke;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ShortcutValidationError {
    HotkeyRequired,
    HotkeyInvalid,
    HotkeyPlainKey,
    TemporaryConflict,
    BindingConflict,
    ModelRequired,
}

impl ShortcutValidationError {
    pub(super) fn i18n_key(&self) -> &'static str {
        match self {
            Self::HotkeyRequired => "shortcut-validation-hotkey-required",
            Self::HotkeyInvalid => "shortcut-validation-hotkey-invalid",
            Self::HotkeyPlainKey => "shortcut-validation-hotkey-invalid",
            Self::TemporaryConflict => "shortcut-validation-temporary-conflict",
            Self::BindingConflict => "shortcut-validation-binding-conflict",
            Self::ModelRequired => "shortcut-validation-model-required",
        }
    }
}

pub(super) fn validate_shortcut_hotkey(
    hotkey: Option<String>,
    current_id: Option<&ShortcutId>,
    shortcuts: &[ShortcutRecord],
    temporary_hotkey: Option<&str>,
) -> Result<String, ShortcutValidationError> {
    let Some(hotkey) = hotkey else {
        return Err(ShortcutValidationError::HotkeyRequired);
    };
    let hotkey = hotkey.trim();
    if hotkey.is_empty() {
        return Err(ShortcutValidationError::HotkeyRequired);
    }
    let canonical = canonical_hotkey(hotkey)?;

    if temporary_hotkey
        .and_then(|hotkey| canonical_hotkey(hotkey).ok())
        .as_deref()
        == Some(canonical.as_str())
    {
        return Err(ShortcutValidationError::TemporaryConflict);
    }

    if shortcuts.iter().any(|shortcut| {
        Some(&shortcut.id) != current_id
            && canonical_hotkey(&shortcut.hotkey).ok().as_deref() == Some(canonical.as_str())
    }) {
        return Err(ShortcutValidationError::BindingConflict);
    }

    Ok(canonical)
}

pub(super) fn canonical_hotkey(hotkey: &str) -> Result<String, ShortcutValidationError> {
    let keystroke = string_to_keystroke(hotkey).ok_or(ShortcutValidationError::HotkeyInvalid)?;
    if !keystroke.modifiers.modified() {
        return Err(ShortcutValidationError::HotkeyPlainKey);
    }
    let canonical = canonical_keystroke(&keystroke);
    HotKey::from_str(&canonical).map_err(|_| ShortcutValidationError::HotkeyInvalid)?;
    Ok(canonical)
}

fn canonical_keystroke(keystroke: &Keystroke) -> String {
    let mut parts = Vec::new();
    if keystroke.modifiers.control {
        parts.push("ctrl".to_string());
    }
    if keystroke.modifiers.alt {
        parts.push("alt".to_string());
    }
    if keystroke.modifiers.shift {
        parts.push("shift".to_string());
    }
    if keystroke.modifiers.platform {
        parts.push("super".to_string());
    }
    parts.push(keystroke.key.to_string());
    parts.join("+")
}

#[cfg(test)]
mod tests {
    use super::{ShortcutValidationError, canonical_hotkey, validate_shortcut_hotkey};
    use ai_chat_core::{
        RunSettingsSnapshot, ShortcutAction, ShortcutInputSource, ToolApprovalPolicy,
        ToolPolicySnapshot, conservative_model_capabilities,
    };
    use ai_chat_db::ShortcutRecord;
    use time::OffsetDateTime;

    #[test]
    fn canonical_hotkey_normalizes_command_alias() {
        assert_eq!(canonical_hotkey("cmd+shift+k").unwrap(), "shift+super+k");
    }

    #[test]
    fn validate_shortcut_hotkey_rejects_temporary_conflict() {
        let result = validate_shortcut_hotkey(
            Some("cmd+shift+j".to_string()),
            None,
            &[],
            Some("super+shift+j"),
        );

        assert_eq!(result, Err(ShortcutValidationError::TemporaryConflict));
    }

    #[test]
    fn validate_shortcut_hotkey_rejects_other_shortcut_conflict() {
        let shortcuts = vec![shortcut_record("shortcut-1", "super+shift+j")];
        let result =
            validate_shortcut_hotkey(Some("cmd+shift+j".to_string()), None, &shortcuts, None);

        assert_eq!(result, Err(ShortcutValidationError::BindingConflict));
    }

    #[test]
    fn validate_shortcut_hotkey_allows_current_shortcut() {
        let shortcuts = vec![shortcut_record("shortcut-1", "super+shift+j")];
        let result = validate_shortcut_hotkey(
            Some("cmd+shift+j".to_string()),
            Some(&"shortcut-1".to_string()),
            &shortcuts,
            None,
        );

        assert_eq!(result.unwrap(), "shift+super+j");
    }

    fn shortcut_record(id: &str, hotkey: &str) -> ShortcutRecord {
        ShortcutRecord {
            id: id.to_string(),
            hotkey: hotkey.to_string(),
            enabled: true,
            prompt_id: None,
            provider_id: Some("provider".to_string()),
            model_id: Some("model".to_string()),
            input_source: ShortcutInputSource::SelectionOrClipboard,
            action: ShortcutAction::OpenTemporaryConversation,
            settings_snapshot: RunSettingsSnapshot {
                prompt: None,
                provider_id: "provider".to_string(),
                model_id: "model".to_string(),
                model_capabilities: conservative_model_capabilities("openai"),
                provider_settings: ai_chat_core::ProviderSettingsPayload {
                    provider_kind: "openai".to_string(),
                    fields: Vec::new(),
                },
                reasoning_selection: None,
                tool_policy: ToolPolicySnapshot {
                    approval_policy: ToolApprovalPolicy::OnRequest,
                    enabled_sources: Vec::new(),
                    max_steps: 32,
                },
            },
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }
    }
}

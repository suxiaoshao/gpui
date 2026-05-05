use crate::{
    components::hotkey_input::string_to_keystroke, database::GlobalShortcutBinding,
    foundation::i18n::I18n,
};
use gpui::{App, SharedString};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ShortcutValidationError {
    EmptyHotkey,
    InvalidHotkey,
    TemporaryHotkeyConflict,
    BindingConflict { binding_id: i32 },
}

impl ShortcutValidationError {
    pub(super) fn message(&self, cx: &App) -> SharedString {
        let i18n = cx.global::<I18n>();
        match self {
            Self::EmptyHotkey | Self::InvalidHotkey => {
                i18n.t("notify-invalid-shortcut-hotkey").into()
            }
            Self::TemporaryHotkeyConflict => {
                i18n.t("shortcut-validation-temporary-conflict").into()
            }
            Self::BindingConflict { binding_id } => format!(
                "{} {}",
                i18n.t("shortcut-validation-binding-conflict"),
                binding_id
            )
            .into(),
        }
    }

    pub(super) fn is_conflict(&self) -> bool {
        matches!(
            self,
            Self::TemporaryHotkeyConflict | Self::BindingConflict { .. }
        )
    }
}

pub(super) fn validate_hotkey(
    binding_id: Option<i32>,
    hotkey: Option<&str>,
    existing_bindings: &[GlobalShortcutBinding],
    temporary_hotkey: Option<&str>,
) -> Result<String, ShortcutValidationError> {
    let Some(hotkey) = hotkey.map(str::trim).filter(|hotkey| !hotkey.is_empty()) else {
        return Err(ShortcutValidationError::EmptyHotkey);
    };
    let Some(canonical) = canonical_hotkey(hotkey) else {
        return Err(ShortcutValidationError::InvalidHotkey);
    };

    if temporary_hotkey.and_then(canonical_hotkey).as_deref() == Some(canonical.as_str()) {
        return Err(ShortcutValidationError::TemporaryHotkeyConflict);
    }

    if let Some(conflict) = existing_bindings.iter().find(|binding| {
        Some(binding.id) != binding_id
            && canonical_hotkey(&binding.hotkey).as_deref() == Some(canonical.as_str())
    }) {
        return Err(ShortcutValidationError::BindingConflict {
            binding_id: conflict.id,
        });
    }

    Ok(canonical)
}

pub(super) fn canonical_hotkey(hotkey: &str) -> Option<String> {
    let keystroke = string_to_keystroke(hotkey)?;
    let mut parts = Vec::new();
    if keystroke.modifiers.control {
        parts.push("ctrl");
    }
    if keystroke.modifiers.alt {
        parts.push("alt");
    }
    if keystroke.modifiers.shift {
        parts.push("shift");
    }
    if keystroke.modifiers.platform {
        parts.push("super");
    }
    let key = keystroke.key.trim().to_lowercase();
    if key.is_empty() {
        return None;
    }
    parts.push(key.as_str());
    Some(parts.join("+"))
}

#[cfg(test)]
mod tests {
    use super::{ShortcutValidationError, canonical_hotkey, validate_hotkey};
    use crate::database::{GlobalShortcutBinding, Mode, ShortcutInputSource};
    use time::OffsetDateTime;

    fn binding(id: i32, hotkey: &str) -> GlobalShortcutBinding {
        let now = OffsetDateTime::now_utc();
        GlobalShortcutBinding {
            id,
            hotkey: hotkey.to_string(),
            enabled: true,
            template_id: None,
            provider_name: "OpenAI".to_string(),
            model_id: "gpt-5.4-mini".to_string(),
            mode: Mode::Single,
            request_template: serde_json::json!({}),
            input_source: ShortcutInputSource::SelectionOrClipboard,
            created_time: now,
            updated_time: now,
        }
    }

    #[test]
    fn canonical_hotkey_normalizes_supported_aliases() {
        assert_eq!(
            canonical_hotkey("cmd+shift+K"),
            Some("shift+super+k".to_string())
        );
        assert_eq!(
            canonical_hotkey("super+shift+k"),
            Some("shift+super+k".to_string())
        );
        assert_eq!(canonical_hotkey("cmd-shift-k"), None);
    }

    #[test]
    fn validate_hotkey_detects_temporary_conflict() {
        assert_eq!(
            validate_hotkey(None, Some("super+shift+k"), &[], Some("cmd+shift+k")),
            Err(ShortcutValidationError::TemporaryHotkeyConflict)
        );
    }

    #[test]
    fn validate_hotkey_ignores_current_binding_and_rejects_other_conflict() {
        let bindings = vec![binding(1, "super+shift+k"), binding(2, "super+shift+j")];
        assert_eq!(
            validate_hotkey(Some(1), Some("cmd+shift+k"), &bindings, None),
            Ok("shift+super+k".to_string())
        );
        assert_eq!(
            validate_hotkey(Some(1), Some("cmd+shift+j"), &bindings, None),
            Err(ShortcutValidationError::BindingConflict { binding_id: 2 })
        );
    }
}

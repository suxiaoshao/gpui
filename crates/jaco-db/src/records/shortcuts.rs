use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ShortcutRecord {
    pub id: ShortcutId,
    pub hotkey: String,
    pub enabled: bool,
    pub prompt_id: Option<PromptId>,
    pub provider_id: Option<ProviderId>,
    pub model_id: Option<ProviderModelId>,
    pub input_source: ShortcutInputSource,
    pub action: ShortcutAction,
    pub settings_snapshot: RunSettingsSnapshot,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewShortcut {
    pub hotkey: String,
    pub enabled: bool,
    pub prompt_id: Option<PromptId>,
    pub provider_id: Option<ProviderId>,
    pub model_id: Option<ProviderModelId>,
    pub input_source: ShortcutInputSource,
    pub action: ShortcutAction,
    pub settings_snapshot: RunSettingsSnapshot,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpdateShortcut {
    pub hotkey: String,
    pub enabled: bool,
    pub prompt_id: Option<PromptId>,
    pub provider_id: Option<ProviderId>,
    pub model_id: Option<ProviderModelId>,
    pub input_source: ShortcutInputSource,
    pub action: ShortcutAction,
    pub settings_snapshot: RunSettingsSnapshot,
}

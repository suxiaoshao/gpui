#![allow(dead_code)]

use std::collections::BTreeMap;

use gpui::{Entity, SharedString, Subscription};
use gpui_component::input::InputState;
use jaco_core::{
    ModelCapabilitiesSnapshot, ProviderId, ProviderModelId, ProviderModelMetadata,
    ProviderSecretRefs, ProviderSettingValue,
};

use super::{capabilities::CapabilityDraft, catalog::ProviderKindKey};

#[derive(Debug, Clone, PartialEq)]
pub(super) enum ProviderSelection {
    Builtin {
        kind: ProviderKindKey,
        provider_id: Option<ProviderId>,
    },
    Custom {
        provider_id: ProviderId,
    },
    NewCustom,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ProviderFormSeed {
    pub(super) provider_id: Option<ProviderId>,
    pub(super) kind: ProviderKindKey,
    pub(super) display_name: String,
    pub(super) enabled: bool,
    pub(super) fields: BTreeMap<String, ProviderSettingValue>,
    pub(super) existing_secret_refs: ProviderSecretRefs,
}

impl ProviderFormSeed {
    pub(super) fn field_string(&self, key: &str) -> String {
        match self.fields.get(key) {
            Some(ProviderSettingValue::String { value }) => value.clone(),
            Some(ProviderSettingValue::Bool { value }) => value.to_string(),
            Some(ProviderSettingValue::Number { value }) => value.to_string(),
            Some(ProviderSettingValue::Object { .. }) => String::new(),
            None => String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ProviderEditorMetadata {
    pub(super) provider_id: Option<ProviderId>,
    pub(super) kind: ProviderKindKey,
    pub(super) existing_secret_refs: ProviderSecretRefs,
}

impl ProviderEditorMetadata {
    pub(super) fn from_seed(seed: &ProviderFormSeed) -> Self {
        Self {
            provider_id: seed.provider_id.clone(),
            kind: seed.kind.clone(),
            existing_secret_refs: seed.existing_secret_refs.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ProviderValidationState {
    Idle,
    Valid,
    Invalid(SharedString),
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ProviderModelDraft {
    pub(super) row_id: Option<ProviderModelId>,
    pub(super) provider_id: ProviderId,
    pub(super) model_id: String,
    pub(super) display_name: Option<String>,
    pub(super) enabled: bool,
    pub(super) capabilities: ModelCapabilitiesSnapshot,
    pub(super) metadata: ProviderModelMetadata,
    pub(super) fetched_at: Option<String>,
    pub(super) dirty: bool,
}

pub(super) struct ManualModelEditor {
    pub(super) mode: ManualModelEditorMode,
    pub(super) model_id_input: Entity<InputState>,
    pub(super) display_name_input: Entity<InputState>,
    pub(super) context_window_input: Entity<InputState>,
    pub(super) capabilities: CapabilityDraft,
    pub(super) error: Option<SharedString>,
    pub(super) _subscriptions: Vec<Subscription>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ManualModelEditorMode {
    Add,
    Edit,
}
